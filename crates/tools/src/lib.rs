use std::process::Command;

pub fn pull_test_suite() {
    Command::new("git")
            .arg("submodule")
            .arg("init")
            .output()
            .expect("failed to execute process");
    Command::new("git")
            .arg("submodule")
            .arg("update")
            .output()
            .expect("failed to execute process");
}
