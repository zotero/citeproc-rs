use clap::{App, SubCommand};
use tools::pull_test_suite;
// use std::{env, path::PathBuf};

fn main() -> Result<(), ()> {
    let matches = App::new("tasks")
        .setting(clap::AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("pull-test-suite"))
        .get_matches();
    match matches.subcommand() {
        ("pull-test-suite", Some(_matches)) => pull_test_suite(),
        _ => unreachable!(),
    }
    Ok(())
}
