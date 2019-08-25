use clap::{App, SubCommand};
use tools::{pull_test_suite, pull_locales, pull_styles};
// use std::{env, path::PathBuf};

fn main() -> Result<(), ()> {
    let matches = App::new("tasks")
        .setting(clap::AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("pull-test-suite"))
        .subcommand(SubCommand::with_name("pull-locales"))
        .subcommand(SubCommand::with_name("pull-styles"))
        .get_matches();
    match matches.subcommand() {
        ("pull-test-suite", Some(_matches)) => pull_test_suite(),
        ("pull-locales", Some(_matches)) => pull_locales(),
        ("pull-styles", Some(_matches)) => pull_styles(),
        _ => unreachable!(),
    }
    Ok(())
}
