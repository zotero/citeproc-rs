mod utils;

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

extern crate wasm_bindgen;
use wasm_bindgen::prelude::*;

use citeproc::input::*;
use citeproc::output::*;
use citeproc::style::element::CslType;
use citeproc::style::variables::*;
use citeproc::Driver;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn parse(style: &str) -> String {
    let formatter = Pandoc::new();
    if let Ok(driver) = Driver::new(style, &formatter) {
        "done!".into()
    } else {
        "failed".into()
    }
}
