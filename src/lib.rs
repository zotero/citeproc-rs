#![feature(test)]

pub mod style;
pub mod input;
pub mod output;
mod utils;

#[macro_use]
extern crate strum_macros;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate cfg_if;
use cfg_if::cfg_if;

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {

        use self::style::drive_style;

        extern crate wasm_bindgen;
        use wasm_bindgen::prelude::*;

        #[wasm_bindgen]
        extern {
            fn alert(s: &str);
        }

        #[wasm_bindgen]
        pub fn greet() {
            alert("Hello, {{project-name}}!");
        }

        #[wasm_bindgen]
        pub fn parse(str: &str) -> String {
            drive_style("in-memory", &str.to_owned())
        }

    }
}

#[cfg(test)]
extern crate test;

#[cfg(test)]
mod tests {
    use crate::test::{Bencher};
    use std::fs::File;
    use std::io::prelude::*;
    use crate::style::drive_style;

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

    #[bench]
    fn bench_fail(b: &mut Bencher) {
        let path = "path";
        let contents = "<content></content>".to_owned();
        b.iter(|| {
            drive_style(path, &contents);
        });
    }
}
