use std::io::prelude::*;
use std::fs::File;
use std::process::{Command, Stdio};
use std::path::{PathBuf, Path};
use anyhow::{Error, anyhow};
use cargo_suity::results::{parse_test_results, Test, Event, EventKind};
use std::collections::{HashMap, HashSet};
use git2::{Repository, Branch};

use std::sync::Mutex;
use once_cell::sync::Lazy;

struct RepoInfo {
    repo: Repository,
}

impl RepoInfo {
    fn get() -> Result<Self, Error> {
        let repo = Repository::open(workspace_root())?;
        Ok(RepoInfo {
            repo,
        })
    }
    fn is_clean(&self) -> Result<bool, Error> {
        // i.e. not currently rebasing, etc.
        if self.repo.state() != git2::RepositoryState::Clean {
            return Ok(false);
        }
        Ok(true)
    }
    fn current_branch_commit(&self) -> Result<(Option<String>, String), Error> {
        if !self.is_clean()? {
            return Err(anyhow!("Not checked out, clean, on a branch."));
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
        Ok((found_branch, commit))
    }
}

static WORKSPACE_ROOT: Lazy<Mutex<PathBuf>> = Lazy::new(|| {
    let metadata = cargo_metadata::MetadataCommand::new().exec().expect("Unable to read cargo metadata");
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
                Event::Suite(_suite) => {}, // { summary.suites.insert(suite.name.clone(), suite); },
                Event::Test(test) => {
                    summary.test_names.insert(test.name.clone());
                    match &test.event {
                        // TODO: support allowed_fail when datatest does
                        EventKind::Started => {},
                        EventKind::Ok => {summary.ok.insert(test.name.clone(), test);},
                        EventKind::Failed => {summary.failed.insert(test.name.clone(), test);},
                        EventKind::Ignored => {summary.ignored.insert(test.name.clone(), test);},
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
                (EventKind::Ignored, EventKind::Ok) |
                (EventKind::Failed, EventKind::Ok) => {
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
        }
    }
}

pub struct TestDiff<'a> {
    regressions: Vec<&'a Test>,
    improvements: Vec<&'a Test>,
    new_ignores: Vec<&'a Test>,
}

impl TestDiff<'_> {
    fn print(&self) -> usize {
        for test in &self.regressions {
            println!("regression: {}\noutput:\n{}", &test.name, test.stdout.as_ref().map(|x| x.as_str()).unwrap_or(""));
        }
        for test in &self.improvements {
            println!("improved: {}", &test.name);
        }
        for test in &self.new_ignores {
            println!("newly ignored: {}", &test.name);
        }
        println!("{} regressions, {} new passing tests, {}  new ignores", self.regressions.len(), self.improvements.len(), self.new_ignores.len());
        self.regressions.len()
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
    path.push("commit");
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
    Err(anyhow!("Snapshot ref {} did not point to a stored snapshot", s))
}

pub fn log_tests(name: &str) -> Result<(), Error> {
    let child = Command::new("sh")
        .arg("-c")
        // .arg("cargo test --package citeproc --test suite | grep '^test ' | sort")
        .arg("cargo +nightly test -Z unstable-options --package citeproc --test suite -- -Z unstable-options --format json")
        .stderr(Stdio::inherit())
        .output()?;
    // Check it's parseable
    let output_str = std::str::from_utf8(&child.stdout)?;
    let _events = parse_test_results(&output_str); // panics with Result::unwrap if not parseable by suity
    let repo = RepoInfo::get()?;
    let output = &child.stdout;
    write_snapshot(&snapshot_path(name)?, output)?;
    if repo.is_clean()? {
        let (branch, commit) = repo.current_branch_commit()?;
        write_snapshot(&snapshot_path_commit(&commit)?, output)?;
        if let Some(branch) = branch {
            write_snapshot(&snapshot_path_branch(&branch)?, output)?;
        }
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
    let num_regressions = diff.print();
    if num_regressions > 0 {
        std::process::exit(1);
    }
    Ok(())
}

pub fn pull_test_suite() -> Result<(), Error> {
    let mut child = Command::new("git")
        .arg("submodule")
        .arg("init")
        .spawn()?;
    child.wait()?;
    let mut child = Command::new("git")
        .arg("submodule")
        .arg("update")
        .spawn()?;
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
