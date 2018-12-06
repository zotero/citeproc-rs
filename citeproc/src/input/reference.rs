// it's a tiny crate + type alias to use a faster algorithm (FNV, obviously) on short keys than
// std::collections::HashMap does
extern crate fnv;

use super::date::DateOrRange;
use super::numeric::NumericValue;
use super::names::Name;
use crate::style::element::CslType;
use crate::style::variables::{AnyVariable, DateVariable, NameVariable, NumberVariable, Variable};
use fnv::FnvHashMap;

// We're saving copies and allocations by not using String here.
pub struct Reference<'r> {
    pub id: &'r str,
    pub csl_type: CslType,

    // each field type gets its own hashmap, as its data type is different
    // and writing a Fn(Variable::Xxx) -> CslJson.xxx; would be O(n)
    // whereas these hashes are essentially O(1) for our purposes
    pub ordinary: FnvHashMap<Variable, &'r str>,
    // we do the conversion on the input side, so is-numeric is just Result::ok
    pub number: FnvHashMap<NumberVariable, NumericValue<'r>>,
    pub name: FnvHashMap<NameVariable, Vec<Name<'r>>>,
    pub date: FnvHashMap<DateVariable, DateOrRange>,
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

    pub fn has_variable(&self, var: &AnyVariable) -> bool {
        match *var {
            AnyVariable::Ordinary(ref v) => self.ordinary.contains_key(v),
            AnyVariable::Number(ref v) => self.number.contains_key(v),
            AnyVariable::Name(ref v) => self.name.contains_key(v),
            AnyVariable::Date(ref v) => self.date.contains_key(v),
        }
    }
}
