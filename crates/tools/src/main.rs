use tools::*;
use anyhow::{Error, anyhow};
use structopt::StructOpt;
// use std::{env, path::PathBuf};

#[derive(StructOpt)]
struct TestSuiteDiff {
    base: String,
    to: String,
}

impl Default for TestSuiteDiff {
    fn default() -> Self {
        TestSuiteDiff { base: "blessed".into(), to: "current".into() }
    }
}

impl std::str::FromStr for TestSuiteDiff {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bits: Vec<_> = s.split("..").map(|x| x.to_owned()).collect();
        let mut first = None;
        let mut second = None;
        for bit in bits {
            if first.is_none() {
                first = Some(bit);
            } else if second.is_none() {
                second = Some(bit);
            } else {
                return Err(anyhow!("could not parse diff range"));
            }
        }
        match (first, second) {
            (Some(base), Some(to)) => Ok(TestSuiteDiff { base, to }),
            (Some(to), None) => Ok(TestSuiteDiff { base: "blessed".into(), to }),
            (None, None) => Ok(TestSuiteDiff::default()),
            _ => unreachable!(),
        }
    }
}

#[derive(StructOpt)]
enum TestSuiteSub {
    /// Runs the test suite and saves the result in .snapshots
    /// Runs by default if no subcommand provided.
    /// Also saves the result as "$current_git_commit_hash", if the Git working directory is clean
    /// (ignoring untracked files).
    Store {
        /// The name to store the result in.
        #[structopt(default_value = "current")]
        to: String,
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
    ///         compare, where the base defaults to 'blessed' (see bless command)
    /// Default: bless..current
    Diff {
        #[structopt(parse(try_from_str))]
        opts: Option<TestSuiteDiff>,
    }
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
    TestSuite(TestSuite)
}

fn main() -> Result<(), Error> {
    let opt = Tools::from_args();
    match opt {
        Tools::PullTestSuite => pull_test_suite(),
        Tools::PullLocales => pull_locales(),
        Tools::TestSuite(test_suite) => match test_suite.sub {
            None => log_tests("current"),
            Some(TestSuiteSub::Store { to }) => log_tests(&to),
            Some(TestSuiteSub::Bless { name }) => bless(&name),
            Some(TestSuiteSub::Diff { opts: Some(TestSuiteDiff { base, to, }) }) => diff_tests(&base, &to),
            Some(TestSuiteSub::Diff { opts: None }) => diff_tests("blessed", "current"),
        }
    }
}
