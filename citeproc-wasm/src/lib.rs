mod utils;

extern crate wasm_bindgen;
use wasm_bindgen::prelude::*;

use citeproc::output::*;
// use citeproc::input::*;
// use citeproc::style::element::CslType;
// use citeproc::style::variables::*;
use citeproc::Driver;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

/// Documentation
#[wasm_bindgen]
pub fn parse(style: &str) -> String {
    let formatter = Pandoc::new();
    if let Ok(_driver) = Driver::new(style, &formatter) {
        "done!".into()
    } else {
        "failed".into()
    }
}
