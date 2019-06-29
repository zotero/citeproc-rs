// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

mod utils;

extern crate wasm_bindgen;
use wasm_bindgen::prelude::*;

use csl::locale::Lang;
use citeproc::LocaleFetcher;
use std::sync::Arc;
// use citeproc::input::*;
// use citeproc::style::element::CslType;
// use citeproc::style::variables::*;
use citeproc::Processor;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

// XXX: this is pretty large for a minified binary
const EN_US: &'static str = include_str!("locales-en-US.xml");

/// Documentation
#[wasm_bindgen]
pub fn parse(style: &str) -> Result<String, JsValue> {
    let mut locales = Predefined::default();
    locales.0.insert(Lang::en_us(), EN_US.to_string());
    match Processor::new(style, Arc::new(locales)) {
        Ok(_) => Ok("done!".into()),
        Err(e) => Err(JsValue::from_serde(&e).unwrap())
    }
}

use std::collections::HashMap;

#[derive(Default)]
pub struct Predefined(pub HashMap<Lang, String>);

impl LocaleFetcher for Predefined {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        Ok(self.0.get(lang).cloned().unwrap_or_else(|| {
            String::from(
                r#"<?xml version="1.0" encoding="utf-8"?>
        <locale xmlns="http://purl.org/net/xbiblio/csl" version="1.0" xml:lang="en-US">
        </locale>"#,
            )
        }))
    }
}
