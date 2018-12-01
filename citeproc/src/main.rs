use clap::{App, Arg};
use std::str::FromStr;

extern crate citeproc;
use citeproc::output::*;
use citeproc::proc::Proc;
use citeproc::input::*;
use citeproc::style::element::{ Style, CslType };
use citeproc::style::error::StyleError;
use citeproc::style::variables::*;
use citeproc::style::{build_style, drive_style};
use std::fs::File;
use std::io::prelude::*;

fn parse(path: &str) -> Result<Style, StyleError> {
    let mut f = File::open(path).expect("no file at path");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    println!("{}", drive_style(path, &contents));
    let style = build_style(&contents)?;
    flame_it::flame_it(&style);
    Ok(style)
}

fn main() -> Result<(), StyleError> {
    let matches = App::new("citeproc")
        .version("0.0.0")
        .author("Cormac Relf")
        .about("Processes citations")
        .arg(
            Arg::with_name("csl")
            .short("c")
            .long("csl")
            .value_name("FILE")
            .takes_value(true),
            )
        .get_matches();
    if let Some(path) = matches.value_of("csl") {
        let style = parse(path)?;
        let pandoc = PlainText::new();
        let mut refr = Reference::empty("id", CslType::LegalCase);
        refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
        refr.number.insert(NumberVariable::Number, 55);
        refr.date.insert(DateVariable::Issued, DateOrRange::from_str("1998-01-04").unwrap());
        let p = style.proc_intermediate(&pandoc, &refr);
        println!("{:?}", p);
    }
    Ok(())
}

#[cfg(not(feature = "flame_it"))]
mod flame_it {
    use citeproc::style::element::Style;
    pub fn flame_it(_style: &Style) {}
}

#[cfg(feature = "flame_it")]
mod flame_it {
    use citeproc::output::PlainText;
    use citeproc::proc::proc_intermediate;
    use citeproc::style::element::Style;
    use std::fs::File;
    pub fn flame_it(style: &Style) {
        let pandoc = PlainTextFormat::new();
        flame::span_of("bench_run", || {
            proc_intermediate(&style, &pandoc);
        });
        // Dump the report to disk
        flame::dump_html(&mut File::create("flame-graph.html").unwrap()).unwrap();
    }
}
