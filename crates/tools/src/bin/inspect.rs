use anyhow::Error;
use clap::arg_enum;
use structopt::StructOpt;
use test_utils::humans::parse_human_test;
use test_utils::inspect::*;
use test_utils::yaml::parse_yaml_test;
use tools::workspace_root;
use test_utils::citeproc as citeproc;
use citeproc::prelude::*;

arg_enum! {
    #[derive(Debug)]
    enum InspectKind {
        All,
        Output,
        Gen4,
        Cluster,
    }
}

impl Default for InspectKind {
    fn default() -> Self {
        InspectKind::All
    }
}

#[derive(StructOpt)]
struct Inspect {
    test_name: String,
    kind: InspectKind,
}

fn main() -> Result<(), Error> {
    use env_logger::Env;
    env_logger::from_env(Env::default().default_filter_or("citeproc_proc=debug")).init();
    let opt = Inspect::from_args();
    let mut path = workspace_root();
    path.push("crates");
    path.push("citeproc");
    path.push("tests");
    path.push("data");
    let mut case = if opt.test_name.ends_with(".txt") {
        path.push("test-suite");
        path.push("processor-tests");
        path.push("humans");
        path.push(&opt.test_name);
        let input = std::fs::read_to_string(path)?;
        parse_human_test(&input)
    } else {
        path.push("humans");
        path.push(&opt.test_name);
        let input = std::fs::read_to_string(path)?;
        parse_yaml_test(&input)?
    };
    let output = case.execute();
    let print_output = || {
        if let Some(o) = output {
            println!("{}", o);
        } else {
            println!("No output");
        }
    };
    let gen4 = || {
        for &id in case.processor.all_cite_ids().iter() {
            let g = debug_gen4_flat(&case.processor, id).unwrap();
            println!("{}", g);
        }
    };
    match opt.kind {
        InspectKind::Output => print_output(),
        InspectKind::Gen4 => gen4(),
        InspectKind::All => {
            print_output();
            gen4();
        }
        _ => {}
    }
    Ok(())
}
