use clap::{App, Arg};

extern crate citeproc;
use citeproc::style::drive_style;
use std::fs::File;
use std::io::prelude::*;

fn parse(path: &str) {
    let mut f = File::open(path).expect("no file at path");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    println!("{}", drive_style(path, &contents));
}

fn main() {
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
        parse(path);
    }
}
