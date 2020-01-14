use anyhow::{anyhow, Error};
use citeproc::prelude::*;
use csl::{Lang, TextTermSelector};
use structopt::StructOpt;
use test_utils::citeproc;
use test_utils::humans::parse_human_test;
use test_utils::yaml::parse_yaml_test;
use tools::workspace_root;

#[derive(StructOpt, Debug)]
enum InspectKind {
    Default,
    Output,
    Gen4,
    /// A debug view of built_cluster_before_output
    Cluster,
    CitePositions,
    Locale {
        #[structopt(long)]
        lang: Option<Lang>,
        #[structopt(long)]
        term: Option<String>,
        #[structopt(long)]
        form: Option<String>,
        #[structopt(long)]
        plural: Option<bool>,
    },
}

impl Default for InspectKind {
    fn default() -> Self {
        InspectKind::Default
    }
}

#[derive(StructOpt)]
struct Inspect {
    test_name: String,
    #[structopt(subcommand)]
    kind: Option<InspectKind>,
}

fn main() -> Result<(), Error> {
    use env_logger::Env;
    env_logger::from_env(Env::default().default_filter_or("citeproc_proc=debug,citeproc_io=debug,citeproc_db=debug,citeproc_io::output::markup::move_punctuation=warn")).init();
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
            if let Ok(flat) = debug_gen4_flat(&case.processor, id) {
                println!("{}", flat);
            } else {
                println!("{:?} empty", id);
            }
        }
    };
    let style = case.processor.style();
    let features = &style.features;
    let cluster = || {
        for cluster in case.processor.clusters_cites_sorted().iter() {
            use test_utils::citeproc_proc::built_cluster_before_output;
            let built = built_cluster_before_output(&case.processor, cluster.id);
            println!("ClusterId({:?}): {:#?}", cluster.id, built);
        }
    };
    let positions = || {
        let positions = case.processor.cite_positions();
        for cluster in case.processor.clusters_cites_sorted().iter() {
            println!("ClusterId({:?})", cluster.id);
            for id in cluster.cites.iter() {
                println!("- {:?}", positions.get(id).unwrap());
            }
        }
    };
    match opt.kind.unwrap_or_else(Default::default) {
        InspectKind::Output => print_output(),
        InspectKind::Gen4 => gen4(),
        InspectKind::Cluster => cluster(),
        InspectKind::CitePositions => positions(),
        InspectKind::Default => {
            print_output();
            gen4();
        }
        InspectKind::Locale {
            lang,
            term,
            form,
            plural,
        } => {
            let locale = lang
                .map(|l| case.processor.merged_locale(l))
                .unwrap_or_else(|| case.processor.default_locale());
            if let Some(term) = term {
                let selector = TextTermSelector::from_term_form_unwrap(
                    &term,
                    form.as_ref().map(|x| x.as_ref()),
                    features,
                );
                let result = locale.get_text_term(selector, plural.unwrap_or(false));
                println!("{:?}", result);
            } else {
                println!("{:#?}", locale);
            }
        }
    }
    Ok(())
}

fn debug_gen4_flat(eng: &Processor, cite_id: CiteId) -> Result<String, Error> {
    let ir = eng.ir_gen4_conditionals(cite_id);
    let fmt = eng.get_formatter();
    let flat = ir
        .ir
        .flatten(&fmt)
        .ok_or_else(|| anyhow!("flatten was none"))?;
    Ok(format!("{:#?}", &flat))
    // Ok(serde_sexpr::to_string(&flat)?)
}
