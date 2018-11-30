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

use citeproc::output::plain::PlainTextFormat;
use citeproc::proc::proc_intermediate;
use citeproc::style::build_style;

extern crate wasm_bindgen;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn parse(style: &str) -> String {
    let s = build_style(&style.to_owned());
    if let Ok(style) = s {
        let fmt = PlainTextFormat::new();
        proc_intermediate(&style, &fmt);
        "done!".into()
    } else {
        "failed".into()
    }
}
