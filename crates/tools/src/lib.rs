use std::process::{Command, Stdio};

pub fn pull_test_suite() {
    let mut child = Command::new("git")
        .arg("submodule")
        .arg("init")
        .spawn()
        .expect("failed to execute process");
    child.wait().expect("failed to wait on child");
    let mut child = Command::new("git")
        .arg("submodule")
        .arg("update")
        .spawn()
        .expect("failed to execute process");
    child.wait().expect("failed to wait on child");
}

use directories::ProjectDirs;

// TODO: should update an existing one.
pub fn pull_styles() {
    let pd =
        ProjectDirs::from("net", "cormacrelf", "citeproc-rs").expect("No home directory found.");
    let mut styles_dir = pd.cache_dir().to_owned();
    styles_dir.push("styles");

    let mut child = Command::new("git")
        .arg("clone")
        .arg("https://github.com/citation-style-language/styles")
        .arg(styles_dir)
        .stdout(Stdio::inherit())
        .spawn()
        .expect("failed to clone");
    child.wait().expect("failed to wait on child");
}

// TODO: should update an existing one.
pub fn pull_locales() {
    let pd =
        ProjectDirs::from("net", "cormacrelf", "citeproc-rs").expect("No home directory found.");
    let mut locales_dir = pd.cache_dir().to_owned();
    locales_dir.push("locales");

    let mut child = Command::new("git")
        .arg("clone")
        .arg("https://github.com/citation-style-language/locales")
        .arg(locales_dir)
        .stdout(Stdio::inherit())
        .spawn()
        .expect("failed to clone");

    child.wait().expect("failed to wait on child");
}
