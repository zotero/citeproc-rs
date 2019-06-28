// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

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

pub struct JsLocaleFetcher {
    root: PathBuf,
}

impl Default for JsLocaleFetcher {
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

impl JsLocaleFetcher {
    pub fn new(repo_dir: impl Into<PathBuf>) -> Self {
        Filesystem {
            root: repo_dir.into(),
        }
    }
}

impl LocaleFetcher for JsLocaleFetcher {
    fn fetch_string(&self, lang: &Lang) -> Result<String, std::io::Error> {
        let mut path = self.root.clone();
        path.push(&format!("locales-{}.xml", lang));
        fs::read_to_string(path)
    }
}
