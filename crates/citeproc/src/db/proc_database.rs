// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use std::sync::Arc;

use csl::locale::Locale;
use csl::style::{Position, Style, Name};
use citeproc_io::CiteId;
use citeproc_proc::ProcDatabase;

use super::Processor;
use super::{StyleDatabase, LocaleDatabase, CiteDatabase};

// We don't want too tight a coupling between the salsa DB and the proc module.
// It's just too annoying to refactor any changes here through all the Proc implementations.
impl ProcDatabase for Processor {
    #[inline]
    fn default_locale(&self) -> Arc<Locale> {
        self.merged_locale(self.style().default_locale.clone())
    }
    #[inline]
    fn locale(&self, id: CiteId) -> Arc<Locale> {
        self.locale_by_cite(id)
    }
    #[inline]
    fn style_el(&self) -> Arc<Style> {
        self.style()
    }
    #[inline]
    fn cite_pos(&self, id: CiteId) -> Position {
        self.cite_position(id).0
    }
    #[inline]
    fn cite_frnn(&self, id: CiteId) -> Option<u32> {
        self.cite_position(id).1
    }
    #[inline]
    fn name_citation(&self) -> Arc<Name> {
        StyleDatabase::name_citation(self)
    }
    fn bib_number(&self, id: CiteId) -> Option<u32> {
        let cite = self.cite(id);
        if let Some(abc) = self.sorted_refs() {
            let (_, ref lookup) = &*abc;
            lookup.get(&cite.ref_id).cloned()
        } else {
            None
        }
    }
}

