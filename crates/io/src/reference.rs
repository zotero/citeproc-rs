// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// it's a tiny crate + type alias to use a faster algorithm (FNV, obviously) on short keys than
// std::collections::HashMap does
extern crate fnv;

use fnv::FnvHashMap;

use super::date::DateOrRange;
use super::names::Name;
use crate::NumberLike;
use csl::{Atom, CslType, DateVariable, Lang, NameVariable, NumberVariable, Variable};

// We're saving copies and allocations by not using String here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub id: Atom,
    pub csl_type: CslType,
    pub language: Option<Lang>,

    // each field type gets its own hashmap, as its data type is different
    // and writing a Fn(Variable::Xxx) -> CslJson.xxx; would be O(n)
    // whereas these hashes are essentially O(1) for our purposes
    pub ordinary: FnvHashMap<Variable, String>,
    // we do the conversion on the input side, so is-numeric is just Result::ok
    pub number: FnvHashMap<NumberVariable, NumberLike>,
    pub name: FnvHashMap<NameVariable, Vec<Name>>,
    pub date: FnvHashMap<DateVariable, DateOrRange>,
}

impl Reference {
    pub fn empty(id: Atom, csl_type: CslType) -> Reference {
        Reference {
            id,
            csl_type,
            language: None,
            ordinary: FnvHashMap::default(),
            number: FnvHashMap::default(),
            name: FnvHashMap::default(),
            date: FnvHashMap::default(),
        }
    }
}
