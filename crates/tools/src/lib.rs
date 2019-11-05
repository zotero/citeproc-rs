use anyhow::{anyhow, Error};
use cargo_suity::results::{Event, EventKind, Test};
use git2::{Branch, Repository, StatusOptions, Tree};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use once_cell::sync::Lazy;
use std::sync::Mutex;

struct RepoInfo {
    repo: Repository,
    clean: bool,
}

impl RepoInfo {
    fn get() -> Result<Self, Error> {
        let repo = Repository::open(workspace_root())?;
        let mut info = RepoInfo { repo, clean: false };
        info.clean = info.is_clean()?;
        Ok(info)
    }
    fn is_clean(&self) -> Result<bool, Error> {
        // i.e. not currently rebasing, etc.
        if self.repo.state() != git2::RepositoryState::Clean {
            return Ok(false);
        }
        let mut options = StatusOptions::new();
        options.include_untracked(false);
        options.include_ignored(false);
        let statuses = self.repo.statuses(Some(&mut options))?;
        // for stat in statuses.iter() {
        //     println!("{:?}, {:?}", stat.status(), stat.path());
        // }
        Ok(statuses.is_empty())
    }
    fn current_branch_commit(&self) -> Result<Option<(Option<String>, String)>, Error> {
        if !self.clean {
            return Ok(None);
        }
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        let commit = commit.id().to_string();
        let mut found_branch = None;
        if head.is_branch() {
            let branch = Branch::wrap(head);
            if let Some(name) = branch.name()? {
                found_branch = Some(String::from(name));
            }
        }
        Ok(Some((found_branch, commit)))
    }
    // f is called with the commit hash that the revision parsed to
    fn run_with_checkout<T>(
        &self,
        rev_to_parse: &str,
        mut f: impl FnMut(String) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if !self.clean {
            return Err(anyhow!(
                "unable to checkout {}; repo was not clean",
                rev_to_parse
            ));
        }
        let head = self.repo.head()?;
        let rev = self.repo.revparse_single(rev_to_parse)?;
        let rev_commit = rev.peel_to_commit()?;
        let _guard = CheckoutGuard {
            repo: &self.repo,
            head: head
                .name()
                .ok_or_else(|| anyhow!("head.name was not unicode"))?,
            head_tree: head.peel_to_tree()?,
        };
        println!(
            "------------------- CHECKING OUT {} -------------------",
            rev_to_parse
        );
        self.repo.checkout_tree(&rev, None)?;
        let commit_id = rev_commit.id().to_string();
        self.repo.set_head_detached(rev_commit.id())?;
        let o = f(commit_id)?;
        Ok(o)
    }
}

struct CheckoutGuard<'a> {
    repo: &'a Repository,
    head: &'a str,
    head_tree: Tree<'a>,
}

impl<'a> Drop for CheckoutGuard<'a> {
    fn drop(&mut self) {
        self.repo
            .checkout_tree(self.head_tree.as_object(), None)
            .expect("could not reverse checkout in CheckoutGuard::drop");
        self.repo
            .set_head(self.head)
            .expect("unable to set head in CheckoutGuard::drop");
    }
}

static WORKSPACE_ROOT: Lazy<Mutex<PathBuf>> = Lazy::new(|| {
    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .expect("Unable to read cargo metadata");
    Mutex::new(metadata.workspace_root)
});

fn workspace_root() -> PathBuf {
    WORKSPACE_ROOT.lock().unwrap().clone()
}

#[derive(Default, Debug)]
struct TestSummary {
    test_names: HashSet<String>,
    ok: HashMap<String, Test>,
    failed: HashMap<String, Test>,
    ignored: HashMap<String, Test>,
    // suites: HashMap<String, Suite>,
}

impl TestSummary {
    fn from_events(events: Vec<Event>) -> Self {
        let mut summary = TestSummary::default();
        for event in events {
            match event {
                Event::Suite(_suite) => {} // { summary.suites.insert(suite.name.clone(), suite); },
                Event::Test(test) => {
                    summary.test_names.insert(test.name.clone());
                    match &test.event {
                        // TODO: support allowed_fail when datatest does
                        EventKind::Started => {}
                        EventKind::Ok => {
                            summary.ok.insert(test.name.clone(), test);
                        }
                        EventKind::Failed => {
                            summary.failed.insert(test.name.clone(), test);
                        }
                        EventKind::Ignored => {
                            summary.ignored.insert(test.name.clone(), test);
                        }
                    }
                }
            }
        }
        summary
    }

    fn kind_for_name(&self, name: &str) -> Option<EventKind> {
        if let Some(_) = self.ok.get(name) {
            return Some(EventKind::Ok);
        } else if let Some(_) = self.failed.get(name) {
            return Some(EventKind::Failed);
        } else if let Some(_) = self.ignored.get(name) {
            return Some(EventKind::Ignored);
        }
        None
    }

    fn diff(&self, base: &TestSummary) -> TestDiff<'_> {
        let common_keys = base.test_names.intersection(&self.test_names);
        let count = common_keys.clone().count();
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut new_ignores = Vec::new();
        for key in common_keys {
            let base_kind = base.kind_for_name(key).unwrap();
            let my_kind = self.kind_for_name(key).unwrap();
            match (base_kind, my_kind) {
                (EventKind::Ok, EventKind::Failed) => {
                    regressions.push(self.failed.get(key).unwrap());
                }
                (EventKind::Ignored, EventKind::Ok) | (EventKind::Failed, EventKind::Ok) => {
                    improvements.push(self.ok.get(key).unwrap());
                }
                (x, EventKind::Ignored) if x != EventKind::Ignored => {
                    new_ignores.push(self.ignored.get(key).unwrap());
                }
                _ => {}
            }
        }
        TestDiff {
            regressions,
            improvements,
            new_ignores,
            count,
        }
    }
}

pub struct TestDiff<'a> {
    regressions: Vec<&'a Test>,
    improvements: Vec<&'a Test>,
    new_ignores: Vec<&'a Test>,
    count: usize,
}

impl TestDiff<'_> {
    // True if should fail
    fn print(&self) -> bool {
        for test in &self.regressions {
            println!(
                "regression: {}\noutput:\n{}",
                &test.name,
                test.stdout.as_ref().map(|x| x.as_str()).unwrap_or("")
            );
        }
        for test in &self.improvements {
            println!("improved: {}", &test.name);
        }
        for test in &self.new_ignores {
            println!("newly ignored: {}", &test.name);
        }
        println!(
            "{} regressions, {} new passing tests, {} new ignores, out of {} intersecting tests",
            self.regressions.len(),
            self.improvements.len(),
            self.new_ignores.len(),
            self.count
        );
        self.regressions.len() > 0 || self.count == 0
    }
}

// fn repo() -> Result<Repository, Error> {
//     let repo = Repository::open(".")?;
//     repo
// }

fn snapshot_path(name: &str) -> Result<PathBuf, Error> {
    let mut path = workspace_root();
    path.push(".snapshots");
    std::fs::create_dir_all(&path)?;
    path.push(name);
    Ok(path)
}

fn snapshot_path_branch(name: &str) -> Result<PathBuf, Error> {
    let mut path = workspace_root();
    path.push(".snapshots");
    path.push("branches");
    std::fs::create_dir_all(&path)?;
    path.push(name);
    Ok(path)
}

fn snapshot_path_commit(name: &str) -> Result<PathBuf, Error> {
    let mut path = workspace_root();
    path.push(".snapshots");
    path.push("commits");
    std::fs::create_dir_all(&path)?;
    path.push(name);
    Ok(path)
}

fn write_snapshot(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    Ok(())
}

fn read_snapshot(name: &str) -> Result<TestSummary, Error> {
    let file = std::fs::read_to_string(&follow_snapshot_ref(name)?)?;
    let base_result: Result<Vec<Event>, _> = file.lines().map(serde_json::from_str).collect();
    Ok(TestSummary::from_events(base_result?))
}

fn follow_snapshot_ref(s: &str) -> Result<PathBuf, Error> {
    // named takes precedence
    let named_path = snapshot_path(s)?;
    if named_path.exists() {
        return Ok(named_path);
    }
    let branch_path = snapshot_path_branch(s)?;
    if branch_path.exists() {
        return Ok(branch_path);
    }
    let commit_path = snapshot_path_commit(s)?;
    if commit_path.exists() {
        return Ok(commit_path);
    }
    Err(anyhow!(
        "Snapshot ref {} did not point to a stored snapshot",
        s
    ))
}

fn get_test_stdout() -> Result<Vec<u8>, Error> {
    let child = Command::new("sh")
        .arg("-c")
        .arg("cargo +nightly test -Z unstable-options --package citeproc --test suite -- -Z unstable-options --format json")
        .stderr(Stdio::inherit())
        .output()?;
    // Check it's parseable
    let output_str = std::str::from_utf8(&child.stdout)?;
    let events: Result<Vec<Event>, _> = output_str.lines().map(serde_json::from_str).collect();
    events?;
    Ok(child.stdout)
}

pub fn log_tests(name: &str) -> Result<(), Error> {
    let stdout = get_test_stdout()?;
    let repo = RepoInfo::get()?;
    write_snapshot(&snapshot_path(name)?, &stdout)?;
    if let Some((branch, commit)) = repo.current_branch_commit()? {
        write_snapshot(&snapshot_path_commit(&commit)?, &stdout)?;
        if let Some(branch) = branch {
            write_snapshot(&snapshot_path_branch(&branch)?, &stdout)?;
        }
    }
    Ok(())
}

pub fn store_at_rev(rev: &str, name: Option<&str>) -> Result<(), Error> {
    let repo = RepoInfo::get()?;
    let mut commit_id = None;
    // Wait until the checkout has returned to HEAD before writing anything in .snapshot
    let stdout = repo.run_with_checkout(rev, |cid| {
        commit_id = Some(cid);
        get_test_stdout()
    })?;
    let commit = commit_id.unwrap();
    let output = &stdout;
    write_snapshot(&snapshot_path_commit(&commit)?, output)?;
    if let Some(name) = name {
        write_snapshot(&snapshot_path(name)?, output)?;
    }
    Ok(())
}

pub fn bless(name: &str) -> Result<(), Error> {
    let current_path = follow_snapshot_ref(name)?;
    let blessed_path = snapshot_path("blessed")?;
    std::fs::copy(current_path, blessed_path)?;
    Ok(())
}

pub fn diff_tests(base_name: &str, current_name: &str) -> Result<(), Error> {
    let blessed = read_snapshot(base_name)?;
    let current = read_snapshot(current_name)?;
    let diff = current.diff(&blessed);
    let should_fail = diff.print();
    if should_fail {
        std::process::exit(1);
    }
    Ok(())
}

pub fn pull_test_suite() -> Result<(), Error> {
    let mut child = Command::new("git").arg("submodule").arg("init").spawn()?;
    child.wait()?;
    let mut child = Command::new("git").arg("submodule").arg("update").spawn()?;
    child.wait()?;
    Ok(())
}

use directories::ProjectDirs;

// TODO: should update an existing one.
pub fn pull_styles() -> Result<(), Error> {
    let pd =
        ProjectDirs::from("net", "cormacrelf", "citeproc-rs").expect("No home directory found.");
    let mut styles_dir = pd.cache_dir().to_owned();
    styles_dir.push("styles");

    let mut child = Command::new("git")
        .arg("clone")
        .arg("https://github.com/citation-style-language/styles")
        .arg(styles_dir)
        .stdout(Stdio::inherit())
        .spawn()?;
    child.wait()?;
    Ok(())
}

// TODO: should update an existing one.
pub fn pull_locales() -> Result<(), Error> {
    let pd =
        ProjectDirs::from("net", "cormacrelf", "citeproc-rs").expect("No home directory found.");
    let mut locales_dir = pd.cache_dir().to_owned();
    locales_dir.push("locales");

    let mut child = Command::new("git")
        .arg("clone")
        .arg("https://github.com/citation-style-language/locales")
        .arg(locales_dir)
        .stdout(Stdio::inherit())
        .spawn()?;

    child.wait()?;
    Ok(())
}
