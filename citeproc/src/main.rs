use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(feature="alloc_system")] {
        use std::alloc::System;
        #[global_allocator]
        static A: System = System;
    }
}

use clap::{App, Arg, SubCommand};

extern crate citeproc;
use citeproc::output::*;
use citeproc::locale::{Filesystem, Lang, LocaleFetcher};
use citeproc::Driver;
use std::fs::File;
use std::io::prelude::*;

use citeproc::db_impl::RootDatabase;
use citeproc::db::ReferenceDatabase;

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
        .subcommand(
            SubCommand::with_name("locale")
                .about("parses a locale file just because it can")
                .arg(
                    Arg::with_name("lang")
                        .short("l")
                        .long("lang")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("locales")
                        .long("locales")
                        .value_name("DIR")
                        .takes_value(true),
                ),
        )
        .arg(
            Arg::with_name("format")
                .short("f")
                .long("format")
                .value_name("FORMAT")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("library")
                .short("l")
                .long("library")
                .value_name("FILE.json")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("csl")
                .short("c")
                .long("csl")
                .value_name("FILE")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("key")
                .short("k")
                .long("key")
                .value_name("CITEKEY")
                .takes_value(true),
        )
        .get_matches();

    let mut lib_text = String::from(
        r#"
    [
        {
            "id": "quagmire2018",
            "type": "legal_case",
            "volume": "2, 4",
            "edition": "128th & 7-9, 17th",
            "page": "1-5",
            "container-title": "TASCC",
            "title": "Solomon v Garrity",
            "author": [
                {"family": "Beethoven", "dropping-particle": "van", "given": "Ludwig"}
            ],
            "editor": [
                {"family": "Paul", "given": "John", "suffix": "II"},
                {"family": "Mozart", "given": "Wolfgang Amadeus"},
                {"family": "Beethoven", "dropping-particle": "van", "given": "Ludwig"}
            ],
            "issued": {"raw": "1995-03-01"}
        }
    ]
    "#,
    );

    use directories::ProjectDirs;
    use std::path::PathBuf;

    if let Some(matches) = matches.subcommand_matches("locale") {
        use std::str::FromStr;
        let locales = matches
            .value_of("locales")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let pd = ProjectDirs::from("net", "cormacrelf", "citeproc-rs")
                    .expect("No home directory found.");
                pd.cache_dir().to_owned()
            });
        let lang = matches
            .value_of("lang")
            .and_then(|l| Lang::from_str(l).ok())
            .unwrap_or(Lang::en_us());
        let mut fsf = Filesystem::new(dbg!(locales));
        let locale = fsf.fetch_cli(&lang);
        dbg!(locale);
        return;
    }

    if let Some(library_path) = matches.value_of("library") {
        lib_text = read(&library_path);
    }
    let mut db = RootDatabase::new(Box::new(Filesystem::new("/Users/cormac/git/locales")));
    db.add_references(&lib_text);
    let key = matches.value_of("key").map(citeproc::Atom::from).unwrap_or("quagmire2018".into());
    let refr = db.reference(key).expect("Citekey not present in library");

    if let Some(path) = matches.value_of("csl") {
        let text = read(&path);
        let formatter = Pandoc::new();
        let driver_r = Driver::new(&text, &formatter);
        if let Ok(driver) = driver_r {
            // driver.dump_macro("issued-year");
            // driver.dump_ir(&refr);

            let serialized = driver.single(&refr);

            // println!("{}", serialized);

            let header = r#"{"blocks":[{"t":"Para","c":"#;
            let footer = r#"}],"pandoc-api-version":[1,17,5,4],"meta":{}}"#;
            println!("{}{}{}", header, serialized, footer);
        } else if let Err(e) = driver_r {
            citeproc::style::error::file_diagnostics(&e, &path, &text);
        }
    }
}
