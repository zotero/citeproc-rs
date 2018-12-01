// it's a tiny crate + type alias to use a faster algorithm (FNV, obviously) on short keys than
// std::collections::HashMap does
extern crate fnv;

use fnv::FnvHashMap;
use crate::style::element::{ CslType };
use crate::style::variables::{ Variable, NumberVariable, DateVariable, NameVariable };

// kebab-case here is the same as Strum's "kebab_case",
// but with a more accurate name
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub struct Name<'r> {
    pub family: Option<&'r str>,
    pub given: Option<&'r str>,
    pub non_dropping_particle: Option<&'r str>,
    pub dropping_particle: Option<&'r str>,
    pub suffix: Option<&'r str>,
}

// This is a fairly primitive date type, possible CSL-extensions could get more fine-grained, and
// then we'd just use chrono::DateTime and support ISO input
#[derive(Deserialize, Clone, Eq, PartialEq)]
pub struct Date {
    /// think 10,000 BC; it's a signed int
    /// "not present" is expressed by not having a date in the first place
    pub year: i32,
    /// range 1 to 12 inclusive
    /// 0 is "not present"
    pub month: u8,
    /// range 1 to 31 inclusive
    /// 0 is "not present"
    pub day: u16,
}

// TODO: implement PartialOrd?

impl Date {
    pub fn has_month(&self) -> bool { self.month != 0 }
    pub fn has_day(&self) -> bool { self.day != 0 }
}

// TODO: implement deserialize for date-parts array, date-parts raw, { year, month, day } 
#[derive(Clone, Eq, PartialEq)]
pub enum DateOrRange {
    Single(Date),
    Range(Date, Date),
}

impl DateOrRange {
    pub fn new(year: i32, month: u8, day: u16) -> Self {
        DateOrRange::Single(Date { year, month, day })
    }
}

// We're saving copies and allocations by not using String here.
pub struct Reference<'r> {
    pub id: &'r str,
    pub csl_type: CslType,

    // each field type gets its own hashmap, as its data type is different
    // and writing a Fn(Variable::Xxx) -> CslJson.xxx; would be O(n)
    // whereas these hashes are essentially O(1) for our purposes
    pub ordinary: FnvHashMap<Variable, &'r str>,
    pub number: FnvHashMap<NumberVariable, i32>,
    pub name: FnvHashMap<NameVariable, Vec<Name<'r>>>,
    pub date: FnvHashMap<DateVariable, Vec<DateOrRange>>,
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

