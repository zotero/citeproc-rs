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

fn read<'s>(path: &str) -> String {
    let mut f = File::open(path).expect("no file at path");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    contents
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
        let text = read(&path);
        let formatter = PlainText::new();
        let driver_r = Driver::new(&text, &formatter);
        if let Ok(driver) = driver_r {
            let mut refr = Reference::empty("id", CslType::LegalCase);
            refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
            refr.number.insert(NumberVariable::Number, Ok(55));
            refr.date.insert(
                DateVariable::Issued,
                DateOrRange::from_str("1998-01-04").unwrap(),
            );
            //
            // driver.dump_style();

            let serialized = driver.single(&refr, &"".to_owned());
            println!("{}", serialized);

            // driver.dump_ir(&refr);

        // let header = r#"{"blocks":[{"t":"Para","c":"#;
        // let footer = r#"}],"pandoc-api-version":[1,17,5,4],"meta":{}}"#;
        // println!("{}{}{}", header, serialized, footer);
        } else if let Err(e) = driver_r {
            citeproc::style::error::file_diagnostics(&e, &path, &text);
        }
    }
}

// #[cfg(not(feature = "flame_it"))]
// mod flame_it {
//     use citeproc::style::element::Style;
//     pub fn flame_it(_style: &Style) {}
// }

// #[cfg(feature = "flame_it")]
// mod flame_it {
//     use citeproc::output::*;
//     use citeproc::proc::*;
//     use citeproc::style::element::Style;
//     use std::fs::File;
//     pub fn flame_it(style: &Style) {
//         let fmt = PlainText::new();
//         flame::span_of("bench_run", || {
//             style.proc_intermediate(&fmt, &refr);
//         });
//         // Dump the report to disk
//         flame::dump_html(&mut File::create("flame-graph.html").unwrap()).unwrap();
//     }
// }
