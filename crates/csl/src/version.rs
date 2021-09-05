// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::attr::EnumGetAttribute;
use crate::Atom;
use semver::{Version, VersionReq};
use strum::EnumProperty;

// Version::new could be a const fn, but isn't.
pub const COMPILED_VERSION: Version = Version {
    major: 1,
    minor: 0,
    patch: 1,
    pre: Vec::new(),
    build: Vec::new(),
};
pub const COMPILED_VERSION_M: Version = Version {
    major: 1,
    minor: 1,
    patch: 0,
    pre: Vec::new(),
    build: Vec::new(),
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CslVersionReq(pub VersionReq);

#[cfg(feature = "serde")]
impl serde::Serialize for CslVersionReq {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CslCslMVersionReq(pub CslVariant, pub VersionReq);

impl CslVersionReq {
    pub(crate) fn current_csl() -> Self {
        CslVersionReq(VersionReq::exact(&COMPILED_VERSION))
    }
}

#[derive(AsRefStr, EnumString, EnumProperty, Debug, PartialEq, Eq, Copy, Clone)]
pub enum CslVariant {
    // these strums are for reading from the <style> element
    #[strum(serialize = "csl", serialize = "CSL")]
    Csl,
    #[strum(serialize = "csl-m", serialize = "CSL-M")]
    CslM,
}
impl EnumGetAttribute for CslVariant {}

impl Default for CslVariant {
    fn default() -> Self {
        CslVariant::Csl
    }
}

impl CslVariant {
    pub fn filter_arg<T: EnumProperty>(self, val: T) -> Option<T> {
        let version = match self {
            CslVariant::Csl => "csl",
            CslVariant::CslM => "cslM",
        };
        if let Some("0") = val.get_str(version) {
            return None;
        }
        Some(val)
    }
}

// These macros `set` and `declare_features` are from Rustc's `src/syntax/feature_gate.rs`.
// Copyright 2013-2019 The Rust Project Developers.
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>.

macro_rules! set {
    ($field: ident) => {{
        fn f(features: &mut Features) {
            features.$field = true;
        }
        f as fn(&mut Features)
    }};
}

macro_rules! declare_features {
    ($(
        $(#[$feat_meta:meta])*
        (active, $feature: ident, $ver: expr, $issue: expr, $edition: expr),
    )+) => {
        /// Represents active features that are currently being implemented or
        /// currently being considered for addition/removal.
        const ACTIVE_FEATURES:
            &[(&str, &str, Option<u32>, Option<()>, fn(&mut Features))] =
            &[$((stringify!($feature), $ver, $issue, $edition, set!($feature))),+];

        #[cfg(feature = "serde")]
        fn is_false(b: &bool) -> bool {
            !*b
        }

        /// A set of features declared / enabled by a style.
        #[derive(Clone, Eq, PartialEq, Default)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        #[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
        pub struct Features {
            /// `(name, Option<since_version>)`: already accepted features that have nevertheless been declared by a style
            #[cfg_attr(feature = "serde", serde(skip_serializing))]
            pub declared_lang_features: Vec<(Atom, Option<Atom>)>,
            $(
                $(#[$feat_meta])*
                #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
                pub $feature: bool,
            )+
        }

        impl Features {
            pub fn new() -> Features {
                Features {
                    declared_lang_features: Vec::new(),
                    $($feature: false),+
                }
            }

            pub fn walk_feature_fields<F>(&self, mut f: F)
                where F: FnMut(&str, bool)
            {
                $(f(stringify!($feature), self.$feature);)+
            }

            pub(crate) fn filter_arg<T: EnumProperty>(&self, val: T) -> Option<T> {
                if let Some(csv) = val.get_str("feature") {
                    for feat in csv.split(',') {
                        if !self.str_enabled(feat) {
                            return None;
                        }
                    }
                }
                Some(val)
            }

            pub(crate) fn str_enabled(&self, fstr: &str) -> bool {
                match fstr {
                    $(stringify!($feature) => self.$feature,)+
                    _ => false,
                }
            }

        }

        use std::fmt;
        impl fmt::Debug for Features {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "Features ")?;
                let mut set = f.debug_set();
                $(
                    if self.$feature {
                        set.entry(&stringify!($feature));
                    }
                )+
                set.finish()
            }
        }

    };

    ($((removed, $feature: ident, $ver: expr, $issue: expr, None, $reason: expr),)+) => {
        /// Represents unstable features which have since been removed (it was once Active)
        const REMOVED_FEATURES: &[(&str, &str, Option<u32>, Option<&str>)] = &[
            $((stringify!($feature), $ver, $issue, $reason)),+
        ];
    };

    ($((stable_removed, $feature: ident, $ver: expr, $issue: expr, None),)+) => {
        /// Represents stable features which have since been removed (it was once Accepted)
        const STABLE_REMOVED_FEATURES: &[(&str, &str, Option<u32>, Option<&str>)] = &[
            $((stringify!($feature), $ver, $issue, None)),+
        ];
    };

    ($((accepted, $feature: ident, $ver: expr, $issue: expr, None),)+) => {
        /// Those language feature has since been Accepted (it was once Active)
        const ACCEPTED_FEATURES: &[(&str, &str, Option<u32>, Option<&str>)] = &[
            $((stringify!($feature), $ver, $issue, None)),+
        ];
    };

    ($((placeholder, $feature: ident, $ver: expr, $issue: expr, None),)+) => {
        // nothing
    };
}
// placeholders
declare_features!(
    // Processor features
    (placeholder, parallel_citations, "1.0.1", None, None),
    // includes legal_case form=short abbreviations, for now
    (placeholder, abbreviations, "1.0.1", None, None),
    (placeholder, condition_page, "1.0.1", None, None),
    (placeholder, condition_context, "1.0.1", None, None),
    (placeholder, condition_genre, "1.0.1", None, None),
    // should include Authority being an institutional author?
    (placeholder, institutions, "1.0.1", None, None),
    // layout locale matching, default-locale-sort, name-as-sort-order languages, name-never-sort
    (placeholder, multilingual, "1.0.1", None, None),
    (placeholder, hereinafter, "1.0.1", None, None),
    (placeholder, date_form_imperial, "1.0.1", None, None),
    // (currently includes the dodgy macro label-form="..." business)
    (placeholder, multiple_locators, "1.0.1", None, None),
    (placeholder, locator_extras, "1.0.1", None, None),
    (placeholder, leading_noise_words, "1.0.1", None, None),
    (placeholder, name_as_reverse_order, "1.0.1", None, None),
    (placeholder, skip_words, "1.0.1", None, None),
    (placeholder, subgroup_delimiter, "1.0.1", None, None),
    (placeholder, suppress_min_max, "1.0.1", None, None),
    (placeholder, text_case_normal, "1.0.1", None, None),
    (placeholder, year_range_format, "1.0.1", None, None),
    (placeholder, jurisdictions, "1.0.1", None, None),
    // E.g. page and page-first become numeric variables
    (placeholder, more_numerics, "1.0.1", None, None),
    (placeholder, var_license, "1.0.1", None, None),
    (placeholder, var_document_name, "1.0.1", None, None),
    (placeholder, var_part_number, "1.0.1", None, None),
    // bare if/else statements without surrounding `<choose>`
    (placeholder, bare_choose, "1.0.1", None, None),
);

// status, name, first added version, tracking issue, edition
// add an issue number as in the first None when you get tracking issues sorted
declare_features!(
    /// `<intext>` element and split cite modes
    ///
    /// - <https://github.com/zotero/zotero/issues/1580>
    /// - <https://citeproc-js.readthedocs.io/en/latest/running.html#special-citation-forms>
    (active, custom_intext, "1.1", None, None),
    /// includes cs:conditions, match="nand"
    (active, conditions, "1.0.1", None, None),
    /// includes condition matchers `has-day="issued [date vars...]"`/`has-year-only="issued"`/`has-month-or-season="issued"`
    (active, condition_date_parts, "1.0.1", None, None),
    /// `issued: "1981-09"`; `issued: "198X"` etc. Also via `"issued": { "edtf": "..." }`.
    (active, edtf_dates, "1.1", None, None),
    /// includes types: gazette, hearing, regulation
    (active, cslm_legal_types, "1.0.1", None, None),
    /// `locator-date` date variable
    (active, var_locator_date, "1.0.1", None, None),
    /// `<names variable="dummy">`
    (active, var_dummy_name, "1.0.1", None, None),
    /// variables: `publication-date`, `publication-number`, `available-date`>
    (active, var_publications, "1.0.1", None, None),
    /// variable: `supplement`
    (active, var_supplement, "1.0.1", None, None),
    /// Enables using editortranslator as a CSL-JSON and CSL variable directly, avoiding
    /// the need for "editor translator"
    ///
    /// <https://discourse.citationstyles.org/t/more-flexible-editortranslator-behavior/1498/7>
    (active, var_editortranslator, "1.0.1", None, None),
    /// article, subparagraph, rule, subsection, schedule, title as locator types
    (active, legal_locators, "1.0.1", None, None),
    /// `<text term="unpublished">`
    (active, term_unpublished, "1.0.1", None, None),
);

// status, name, first added version, tracking issue, edition, None
declare_features!(
    (accepted, supplement, "1.0.2", None, None),
    (accepted, var_volume_title, "1.0.2", None, None),
    (accepted, standard_type, "1.0.2", None, None),
    (accepted, software_type, "1.0.2", None, None),
    (accepted, periodical_type, "1.0.2", None, None),
);

// status, name, first added version, tracking issue, None, reason(str)
declare_features!((
    removed,
    legal_case_form_short,
    "1.0.1",
    None,
    None,
    Some("could be done without a breaking change")
),);

// // status, name, first added version, tracking issue, reason
// declare_features! (
//     (stable_removed, no_stack_check, "1.0.0", None, None),
// );

pub fn read_features<'a>(
    input_features: impl Iterator<Item = &'a str>,
) -> Result<Features, &'a str> {
    let mut features = Features::new();
    read_features_into(input_features, &mut features)?;
    Ok(features)
}

impl Features {
    pub fn try_set_feature<'a>(&mut self, feat_str: &'a str) -> Result<(), &'a str> {
        let replaced;
        let name = if feat_str.contains('-') {
            replaced = feat_str.replace('-', "_");
            replaced.as_str()
        } else {
            feat_str
        };
        if let Some((.., set)) = ACTIVE_FEATURES.iter().find(|f| name == f.0) {
            set(self);
            return Ok(());
        }

        let removed = REMOVED_FEATURES.iter().find(|f| name == f.0);
        // let stable_removed = STABLE_REMOVED_FEATURES.iter().find(|f| name == f.0);
        // if let Some((.., reason)) = removed.or(stable_removed) {
        if let Some((.., reason)) = removed {
            log::warn!("{:?}", reason);
            // feature_removed(span_handler, mi.span, *reason);
            // continue
            return Err(feat_str);
        }

        if let Some((name, since, ..)) = ACCEPTED_FEATURES.iter().find(|f| name == f.0) {
            let name = Atom::from(*name);
            let since = Some(Atom::from(*since));
            self.declared_lang_features.push((name, since));
            return Ok(());
        }

        return Err(feat_str);
    }
}

pub fn read_features_into<'a>(
    input_features: impl Iterator<Item = &'a str>,
    features: &mut Features,
) -> Result<(), &'a str> {
    for kebab in input_features {
        let _ = features.try_set_feature(kebab)?;
    }
    Ok(())
}

#[cfg(feature = "serde")]
use serde::de::{DeserializeSeed, Deserializer, Error, Unexpected, Visitor};

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Features {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SetFeature<'a>(&'a mut Features);
        impl<'a, 'de> Visitor<'de> for SetFeature<'a> {
            type Value = ();

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a valid CSL feature name (kebab-case)")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.0
                    .try_set_feature(v)
                    .map_err(|str_err| Error::invalid_value(Unexpected::Str(str_err), &self))
            }
        }

        struct SingleFeature<'a>(&'a mut Features);

        impl<'a, 'de> DeserializeSeed<'de> for SingleFeature<'a> {
            type Value = ();

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                let _ = deserializer.deserialize_str(SetFeature(self.0));
                Ok(())
            }
        }

        struct FeatureVisitor;
        impl<'de> Visitor<'de> for FeatureVisitor {
            type Value = Features;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a list of valid CSL feature names as strings")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut features = Features::new();
                while let Some(_) = seq.next_element_seed(SingleFeature(&mut features))? {}
                Ok(features)
            }
        }

        deserializer.deserialize_seq(FeatureVisitor)
    }
}
