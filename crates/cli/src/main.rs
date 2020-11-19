// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(feature="jemalloc")] {
        use jemallocator::Jemalloc;
        #[global_allocator]
        static A: Jemalloc = Jemalloc;
    } else {
        use std::alloc::System;
        #[global_allocator]
        static A: System = System;
    }
}

use citeproc::input::Reference;
use clap::{App, Arg, SubCommand};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

mod error;
mod pandoc;
use pandoc_types::definition::{Inline, MetaValue, Pandoc as PandocDocument};

use citeproc::{LocaleFetchError, LocaleFetcher, Processor};
use csl::{Lang, Locale};

fn main() {
    // heuristically determine if we're running as an external pandoc filter
    // TODO: work out earliest pandoc that sets PANDOC_VERSION
    let not_a_tty = !atty::is(atty::Stream::Stdin) && !atty::is(atty::Stream::Stdout);
    if std::env::var("PANDOC_VERSION").is_ok() && not_a_tty {
        do_pandoc();
        return;
    }

    let matches = App::new("citeproc")
        .version("0.0.0")
        .author("Cormac Relf")
        .about("Processes citations")
        .subcommand(
            SubCommand::with_name("parse-locale")
                .about("Parses a locale file (without performing fallback)")
                .arg(
                    Arg::with_name("lang")
                        .short("l")
                        .long("lang")
                        .takes_value(true),
                ),
        )
        .subcommand(SubCommand::with_name("pandoc").about(
            "Force Pandoc JSON filter mode. Operates on stdin > stdout.\
             \nNormally, you can just use `pandoc -F citeproc-rs`.",
        ))
        .subcommand(
            SubCommand::with_name("disamb-index")
                .about("Prints the inverted disambiguation index for the reference library"),
        )
        // .arg(
        //     Arg::with_name("format")
        //         .short("f")
        //         .long("format")
        //         .value_name("FORMAT")
        //         .takes_value(true),
        // )
        .arg(
            Arg::with_name("library")
                .short("l")
                .long("library")
                .value_name("FILE.json")
                .help("A CSL-JSON file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("csl")
                .short("c")
                .long("csl")
                .value_name("FILE")
                .help("A CSL style")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("key")
                .short("k")
                .long("key")
                .value_name("CITEKEY")
                .help("Run against a specific citekey")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("locales-dir")
                .long("locales-dir")
                .value_name("DIR")
                .help("Directory with locales-xx-XX.xml files in it")
                .takes_value(true),
        )
        .get_matches();

    let lib_text = r#"
    [
        {
            "id": "quagmire2018",
            "type": "legal_case",
            "volume": "4",
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
    "#;

    let filesystem_fetcher = {
        let locales_dir = matches
            .value_of("locales-dir")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let pd = ProjectDirs::from("net", "cormacrelf", "citeproc-rs")
                    .expect("No home directory found.");
                let mut locales_dir = pd.cache_dir().to_owned();
                locales_dir.push("locales");
                locales_dir
            });
        if matches.subcommand_matches("parse-locale").is_some() {
            let locales_dir = locales_dir.clone();
            dbg!(locales_dir);
        }
        Arc::new(Filesystem::new(locales_dir))
    };

    if let Some(matches) = matches.subcommand_matches("parse-locale") {
        let lang = if let Some(lan) = matches.value_of("lang") {
            if let Ok(l) = Lang::from_str(lan) {
                l
            } else {
                eprintln!(
                    "`{}` is not a valid language",
                    matches.value_of("lang").unwrap_or("")
                );
                return;
            }
        } else {
            Lang::en_us()
        };
        fn fetch_cli(fetcher: &Filesystem, lang: &Lang) -> Option<Locale> {
            let string = match fetcher.fetch_string(lang) {
                Ok(opt) => opt?,
                Err(e) => panic!("failed to read locale file, exiting\n{:?}", e),
            };
            let with_errors = |s: &str| Ok(Locale::from_str(s)?);
            match with_errors(&string) {
                Ok(l) => Some(l),
                Err(e) => {
                    self::error::file_diagnostics(&e, "input", &string);
                    None
                }
            }
        }
        let locale = fetch_cli(&filesystem_fetcher, &lang);
        dbg!(locale);
        return;
    }

    // if let Some(_) = matches.subcommand_matches("disamb-index") {
    //     let mut db = Processor::new(filesystem_fetcher);
    //     db.insert_references(refs);
    //     for (tok, ids) in db.inverted_index().iter() {
    //         // if ids.len() > 1 {
    //         let token = tok.clone();
    //         let citekeys: Vec<_> = ids.iter().map(|atom| atom.to_string()).collect();
    //         dbg!((token, citekeys));
    //         // }
    //     }
    //     return;
    // }

    if let Some(csl_path) = matches.value_of("csl") {
        let key = matches
            .value_of("key")
            .map(citeproc::Atom::from)
            .unwrap_or("quagmire2018".into());

        let text = fs::read_to_string(&csl_path).expect("No CSL file found at that path");

        match Processor::new(&text, filesystem_fetcher) {
            Ok(mut db) => {
                let refs = if let Some(library_path) = matches.value_of("library") {
                    expect_refs(library_path)
                } else {
                    serde_json::from_str(&lib_text).expect("sample lib_text not parseable")
                };

                db.init_clusters(vec![citeproc::input::Cluster {
                    id: 0,
                    note_number: 1,
                    cites: vec![citeproc::input::Cite::basic(key)],
                }]);
                db.inssert_references(refs);

                let inlines = db.get_cluster(0).to_vec();

                use pandoc_types::definition::{Block, Meta, Pandoc};
                let doc = Pandoc(Meta::null(), vec![Block::Para(inlines)]);
                let out = serde_json::to_string(&doc).unwrap();
                println!("{}", out);
            }
            Err(e) => {
                self::error::file_diagnostics(&e, &csl_path, &text);
            }
        }
    }
}

fn pandoc_meta_str<'a>(doc: &'a PandocDocument, key: &str) -> Option<&'a str> {
    doc.0.lookup(key).and_then(|value| match value {
        // for metadata passed through the command line
        // --metadata csl=my-style.csl
        MetaValue::MetaString(s) => Some(s.as_str()),
        MetaValue::MetaInlines(inlines) => match &inlines[..] {
            // for inline paths with no spaces (otherwise they get split with
            // Inline::Space)
            // csl: "my-style.csl"
            &[Inline::Str(ref s)] => Some(s.as_str()),
            // for inline paths with spaces
            // csl: "`my style.csl`{=raw}"
            &[Inline::RawInline(_, ref s)] => Some(s.as_str()),
            _ => None,
        },
        _ => None,
    })
}

fn do_pandoc() {
    let filter_args = App::new("pandoc_filter")
        .arg(Arg::with_name("output_format").required(false).index(1))
        .get_matches();
    let _output_format = filter_args.value_of("output_format").unwrap_or("none");
    let input = std::io::stdin();
    // already LineWriter buffered, but we're only writing one line of JSON so not a problem
    let output = std::io::stdout();

    let mut doc: PandocDocument =
        serde_json::from_reader(input).expect("could not parse pandoc json");

    let csl_path = pandoc_meta_str(&doc, "csl").expect("No csl path provided through metadata");
    let text = fs::read_to_string(&csl_path).expect("No CSL file found at that path");

    match Processor::new(&text, Arc::new(Filesystem::default())) {
        Ok(mut db) => {
            if let Some(library_path) = pandoc_meta_str(&doc, "bibliography") {
                db.reset_references(expect_refs(library_path));
            }
            db.init_clusters(pandoc::get_clusters(&mut doc));
            db.compute();
            pandoc::write_clusters(&mut doc, &db);
            serde_json::to_writer(output, &doc).expect("could not write pandoc json");
        }
        Err(e) => {
            self::error::file_diagnostics(&e, &csl_path, &text);
        }
    }
}

pub struct Filesystem {
    root: PathBuf,
}

impl Default for Filesystem {
    fn default() -> Self {
        let locales_dir = None
            // TODO: read metadata
            .unwrap_or_else(|| {
                let pd = ProjectDirs::from("net", "cormacrelf", "citeproc-rs")
                    .expect("No home directory found.");
                let mut locales_dir = pd.cache_dir().to_owned();
                locales_dir.push("locales");
                locales_dir
            });
        Filesystem::new(locales_dir)
    }
}

impl Filesystem {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Filesystem {
            root: repo_dir.into(),
        }
    }
}

use std::io;

impl LocaleFetcher for Filesystem {
    fn fetch_string(&self, lang: &Lang) -> Result<Option<String>, LocaleFetchError> {
        let mut path = self.root.clone();
        path.push(&format!("locales-{}.xml", lang));
        let read = fs::read_to_string(path);
        match read {
            Ok(string) => Ok(Some(string)),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(None),
                _ => Err(LocaleFetchError::Io(e)),
            },
        }
    }
}

fn expect_refs(library_path: &str) -> Vec<Reference> {
    use std::fs::File;
    use std::io::BufReader;
    let file = File::open(&library_path).expect("No library found at that path");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("Could not parse JSON")
}
