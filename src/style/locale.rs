use std::fmt;
use std::str::FromStr;
use crate::style::error::*;
use crate::style::element::{ Form, Date };
use strum::{ AsStaticRef };

#[derive(Debug, PartialEq, Eq)]
pub struct CslOption(String, String);

#[derive(Debug, PartialEq, Eq)]
pub struct Term {
    pub name: String,
    pub form: Form,
    pub gender: Gender,
    pub singular: String,
    pub plural: String,
    pub ordinal_match: OrdinalMatch,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum OrdinalMatch {
    LastTwoDigits,
    WholeNumber,
}

#[derive(AsStaticStr, EnumString, Debug, PartialEq, Eq)]
#[strum(serialize_all="kebab_case")]
pub enum Gender {
    Masculine,
    Feminine,
    Neuter,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Locale {
    pub version: String,
    pub lang: String,
    pub options: Vec<CslOption>,
    pub terms: Vec<Term>,
    pub date: Vec<Date>,
}

fn merge_locales(_base: Locale, locales: Vec<Locale>) -> Vec<Locale> {
    locales
}

