// it's a tiny crate + type alias to use a faster algorithm (FNV, obviously) on short keys than
// std::collections::HashMap does
extern crate fnv;

use fnv::FnvHashMap;
use std::borrow::Cow;

use super::date::DateOrRange;
use super::names::Name;
use super::numeric::NumericValue;
use crate::style::element::CslType;
use crate::style::variables::{DateVariable, NameVariable, NumberVariable, Variable};

// We're saving copies and allocations by not using String here.
#[derive(Debug)]
pub struct Reference<'r> {
    pub id: &'r str,
    pub csl_type: CslType,

    // each field type gets its own hashmap, as its data type is different
    // and writing a Fn(Variable::Xxx) -> CslJson.xxx; would be O(n)
    // whereas these hashes are essentially O(1) for our purposes
    pub ordinary: FnvHashMap<Variable, Cow<'r, str>>,
    // we do the conversion on the input side, so is-numeric is just Result::ok
    pub number: FnvHashMap<NumberVariable, NumericValue<'r>>,
    pub name: FnvHashMap<NameVariable, Vec<Name<'r>>>,
    pub date: FnvHashMap<DateVariable, DateOrRange<'r>>,
}

impl<'r> Reference<'r> {
    pub fn empty(id: &'r str, csl_type: CslType) -> Reference<'r> {
        Reference {
            id,
            csl_type,
            ordinary: FnvHashMap::default(),
            number: FnvHashMap::default(),
            name: FnvHashMap::default(),
            date: FnvHashMap::default(),
        }
    }
}
