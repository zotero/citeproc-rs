use roxmltree::Document;
use clap::{Arg, App};
#[allow(dead_code)]
mod style;
use self::style::drive_style;
#[macro_use]
extern crate strum_macros;
use std::fs::File;
use std::io::prelude::*;

fn parse(path: &str) {
    let mut f = File::open(path).expect("no file at path");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    drive_style(path, &contents);
}

fn main() {
    let matches = App::new("citeproc")
        .version("0.0.0")
        .author("Cormac Relf")
        .about("Processes citations")
        .arg(Arg::with_name("csl")
             .short("c")
             .long("csl")
             .value_name("FILE")
             .takes_value(true))
        .get_matches();
    match matches.value_of("csl") {
        Some(csl_path) => {
            parse(csl_path);
        },
        None => {}
    }
}
