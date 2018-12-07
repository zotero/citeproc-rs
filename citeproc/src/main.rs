use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(feature="alloc_system")] {
        use std::alloc::System;
        #[global_allocator]
        static A: System = System;
    }
}

// #![cfg_attr(feature="flame_it", feature(plugin, custom_attribute))]
// #![cfg_attr(feature="flame_it", plugin(flamer))]

use clap::{App, Arg};
use std::str::FromStr;

extern crate citeproc;
use citeproc::input::*;
use citeproc::output::*;
use citeproc::style::element::CslType;
use citeproc::style::variables::*;
use citeproc::Driver;
use std::fs::File;
use std::io::prelude::*;

#[cfg(feature = "flame_it")]
mod flame_span;

fn read<'s>(path: &str) -> String {
    let mut f = File::open(path).expect("no file at path");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    contents
}

// use pandoc_types::definition::Inline;

fn main() {
    let matches = App::new("citeproc")
        .version("0.0.0")
        .author("Cormac Relf")
        .about("Processes citations")
        .arg(
            Arg::with_name("format")
                .short("f")
                .long("format")
                .value_name("FORMAT")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("csl")
                .short("c")
                .long("csl")
                .value_name("FILE")
                .takes_value(true),
        )
        .get_matches();

    if let Some(path) = matches.value_of("csl") {
        let text = read(&path);
        let formatter = Pandoc::new();
        let driver_r = Driver::new(&text, &formatter);
        if let Ok(driver) = driver_r {
            let mut refr = Reference::empty("id", CslType::LegalCase);
            refr.number.insert(
                NumberVariable::Edition,
                NumericValue::from("1"),
            );
            refr.number.insert(
                NumberVariable::Volume,
                NumericValue::from("128th & 7-9, 17th"),
            );
            // TODO: recognize requests for Page and PageFirst as number vars
            refr.ordinary.insert(Variable::Page, "194");
            refr.ordinary.insert(Variable::PageFirst, "194");
            // refr.number.insert(NumberVariable::Number, NumericValue::Int(55));
            refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
            refr.ordinary.insert(Variable::Title, "Barnaby v Joyce");
            refr.date.insert(
                DateVariable::Issued,
                DateOrRange::from_str("1998-01-04").unwrap(),
            );

            // driver.dump_style();
            // driver.dump_ir(&refr);

            let serialized = driver.single(&refr);

            #[cfg(feature = "flame_it")]
            {
                self::flame_span::write_flamegraph("flame-intermediate.html");
                // flame::dump_html(&mut File::create("flame-graph.html").unwrap()).unwrap();
                // flame::dump_json(&mut File::create("flame-out.json").unwrap()).unwrap();
            }

            // println!("{}", serialized);

            let header = r#"{"blocks":[{"t":"Para","c":"#;
            let footer = r#"}],"pandoc-api-version":[1,17,5,4],"meta":{}}"#;
            println!("{}{}{}", header, serialized, footer);
        } else if let Err(e) = driver_r {
            citeproc::style::error::file_diagnostics(&e, &path, &text);
        }
    }
}
