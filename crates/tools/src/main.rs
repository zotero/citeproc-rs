use anyhow::Error;
use structopt::StructOpt;
use tools::ucd::build_superscript_trie;
use tools::*;
// use std::{env, path::PathBuf};

#[derive(StructOpt)]
enum TestSuiteSub {
    /// Just run the test suite.
    /// Runs by default if no subcommand provided.
    #[structopt(setting = structopt::clap::AppSettings::TrailingVarArg)]
    #[structopt(setting = structopt::clap::AppSettings::AllowLeadingHyphen)]
    Run {
        #[structopt(long)]
        release: bool,
        /// Any additional arguments are passed to the test harness (i.e. with -- --args)
        rest: Vec<String>,
    },
    /// Runs the test suite and saves the result in .snapshots.
    /// Also saves the result as "$current_git_commit_hash", if the Git working directory is clean
    /// (ignoring untracked files).
    #[structopt(setting = structopt::clap::AppSettings::TrailingVarArg)]
    #[structopt(setting = structopt::clap::AppSettings::AllowLeadingHyphen)]
    Store {
        /// The name to store the result in.
        #[structopt(default_value = "current")]
        to: String,
        /// Any additional arguments are passed to the test harness (i.e. with -- --args)
        rest: Vec<String>,
    },
    /// If your working directory is clean, attempts to checkout a provided git ref and store a
    /// result from there.
    CheckoutStore {
        /// A commit-ish to checkout
        rev: String,
        /// An optional name to store the result in as well
        #[structopt(long)]
        to: Option<String>,
    },
    /// Set the default result to compare to
    Bless {
        /// The stored result name to treat as "blessed". Must exist in .snapshots already.
        #[structopt(default_value = "current")]
        name: String,
    },
    /// Compare result runs for regressions. Exits with code 1 if any regressions found.
    ///
    /// Syntax: base..compare, where each of base and compare have been stored in .snapshots already.
    ///         base, where the compare defaults to 'current'
    ///         ..compare, where the base defaults to 'blessed' (see bless subcommand)
    /// Default: bless..current
    Diff {
        #[structopt(parse(try_from_str))]
        opts: Option<TestSuiteDiff>,
    },
}

#[derive(StructOpt)]
#[structopt(about = "run the test suite and compare the results for regressions")]
struct TestSuite {
    #[structopt(subcommand)]
    sub: Option<TestSuiteSub>,
}

#[derive(StructOpt)]
enum Tools {
    PullTestSuite,
    PullLocales,
    BuildUcd,
    TestSuite(TestSuite),
}

fn main() -> Result<(), Error> {
    let opt = Tools::from_args();
    match opt {
        Tools::PullTestSuite => pull_test_suite(),
        Tools::PullLocales => pull_locales(),
        Tools::BuildUcd => build_superscript_trie(),
        Tools::TestSuite(test_suite) => match test_suite.sub {
            None => run(Vec::new(), false),
            Some(TestSuiteSub::Run { release, rest }) => run(rest, release),
            Some(TestSuiteSub::Store { to, rest, .. }) => log_tests(&to, rest),
            Some(TestSuiteSub::CheckoutStore { rev, to }) => {
                store_at_rev(&rev, to.as_ref().map(|x| x.as_ref()))
            }
            Some(TestSuiteSub::Bless { name }) => bless(&name),
            Some(TestSuiteSub::Diff {
                opts: Some(TestSuiteDiff { base, to }),
            }) => diff_tests(&base, &to),
            Some(TestSuiteSub::Diff { opts: None }) => diff_tests("blessed", "current"),
        },
    }
}
