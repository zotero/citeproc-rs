#![feature(test)]
#![cfg_attr(feature = "flame_it", feature(proc_macro_hygiene))]

#[cfg(feature = "flame_it")]
extern crate flame;
#[cfg(feature = "flame_it")]
#[macro_use]
extern crate flamer;

#[macro_use]
extern crate nom;

pub mod input;
pub mod output;
pub mod style;
mod utils;

#[cfg_attr(feature = "flame_it", flame)]
pub mod proc;

#[macro_use]
extern crate strum_macros;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate cfg_if;

#[cfg(test)]
extern crate test;

#[cfg(test)]
mod tests {
    use crate::style::drive_style;
    use crate::test::Bencher;
    use std::fs::File;
    use std::io::prelude::*;

    #[bench]
    fn bench_build_tree(b: &mut Bencher) {
        let path = "/Users/cormac/Zotero/styles/australian-guide-to-legal-citation.csl";
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        println!("hello?");
        b.iter(|| {
            drive_style(path, &contents);
        });
    }

    // #[bench]
    // fn bench_fail(b: &mut Bencher) {
    //     let path = "path";
    //     let contents = "<content></content>".to_owned();
    //     b.iter(|| {
    //         drive_style(path, &contents);
    //     });
    // }

}
