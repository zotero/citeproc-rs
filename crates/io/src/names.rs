// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::csl_json::RelaxedBool;
use crate::{lazy, String};

#[derive(Default, Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
struct PersonNameInput {
    pub family: Option<String>,
    pub given: Option<String>,
    pub non_dropping_particle: Option<String>,
    pub dropping_particle: Option<String>,
    pub suffix: Option<String>,
    #[serde(default)]
    pub static_particles: bool,
    #[serde(default, deserialize_with = "RelaxedBool::deserialize_bool")]
    pub comma_suffix: bool,
}

// kebab-case here is the same as Strum's "kebab_case",
// but with a more accurate name
#[derive(Default, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(from = "PersonNameInput")]
pub struct PersonName {
    pub family: Option<String>,
    pub given: Option<String>,
    pub non_dropping_particle: Option<String>,
    pub dropping_particle: Option<String>,
    pub suffix: Option<String>,
    #[serde(default)]
    pub static_particles: bool,
    #[serde(default)]
    pub comma_suffix: bool,
    #[serde(default, skip_serializing)]
    pub is_latin_cyrillic: bool,
}

#[derive(Deserialize)]
#[serde(untagged, rename_all = "kebab-case")]
enum NameInput {
    // Put literal first, because PersonName's properties are all Options and derived
    // Deserialize impls run in order.
    Literal {
        // the untagged macro uses the field names on Literal { literal } instead of the discriminant, so don't change that
        literal: String,
    },
    Person(PersonNameInput),
    // TODO: represent an institution in CSL-M?
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize, Serialize, Clone)]
#[serde(from = "NameInput")]
pub enum Name {
    // Put literal first, because PersonName's properties are all Options and derived
    // Deserialize impls run in order.
    Literal {
        // the untagged macro uses the field names on Literal { literal } instead of the discriminant, so don't change that
        literal: String,
        #[serde(skip_serializing)]
        is_latin_cyrillic: bool,
    },
    Person(PersonName),
    // TODO: represent an institution in CSL-M?
}

impl From<NameInput> for Name {
    fn from(input: NameInput) -> Self {
        match input {
            // Normalise literal names into lone family names.
            //
            // There is no special case for literal names in
            // CSL, so this just helps do the formatting
            // uniformly. They can still be created by using
            // the Rust API directly, so this has to be
            // removed at some point.
            NameInput::Literal { literal } => Name::Person(PersonName {
                is_latin_cyrillic: is_latin_cyrillic(&literal),
                family: Some(literal),
                ..Default::default()
            }),
            NameInput::Person(pn) => Name::Person(pn.into()),
        }
    }
}

// Now we implement From<PersonNameInput> for PersonName

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

fn split_nondrop_family(family: &mut String) -> Option<String> {
    // our last name might start with 1 or more of:
    // - lowercase anycase* apostrophe, i.e. "d'", "d’", "dʻ" (backwards, but sure), some other things like "d-"
    // - lowercase anycase* space, i.e. "d ", "von " etc.
    // - lowercase anycase* apostrophe space, i.e. "d’ "
    // - apostrophe lowercase anycase*...
    //
    // overall regex should be therefore have either an apostrophe and space*, or apostrophe* and
    // space+. Then we will repeat the whole thing to catch any number of them, starting at ^ with
    // some possible leading whitespace.
    //
    let re = regex!(r#"^\s*(?:['’ʻ\.-]?\p{Lowercase}\p{Alphabetic}*(?:['’ʻ\s\.-]|\b))+\s*"#);

    let mut particles = String::new();
    let mut remain = &family[..];
    if let Some(found) = re.find(remain) {
        let particle = found.as_str();
        if found.end() != remain.len() {
            particles.push_str(particle);
            remain = &remain[found.end()..];
            *family = replace_apostrophes(remain);
        }
    }
    if particles.is_empty() {
        replace_apostrophes_mut(family);
        return None;
    }
    if particles.trim_end_in_place() != 0 {
        // these particular particle-terminals can be forced to have a space after them.
        // simply input { family: "d' Lastname" }. normally, having an apostrophe at the
        // end of the ndparticles will cause the space between the particles and the last
        // name to be suppressed, so adding one here means it ends up in the output. (see
        // dp_should_append_space in proc/src/names.rs)
        match particles.chars().rev().next() {
            Some('\'') | Some('\u{2018}') | Some('\u{2019}') => particles.push_str(" "),
            _ => {}
        }
    }
    replace_apostrophes_mut(&mut particles);
    Some(particles)
}

fn split_drop_given(given: &mut String) -> Option<String> {
    let re = regex!(r#"\s+(?:['’ʻ\.-]?\p{Lowercase}\p{Alphabetic}*(?:['’ʻ\s\.-]|\b)\s*)+$"#);
    let mut particles = String::new();
    // find gives us the leftmost-starting match, and that's what we want.
    if let Some(found) = re.find(given) {
        if found.start() != 0 {
            let all = found.as_str().trim_start();
            particles = replace_apostrophes(all);
            let start = found.start();
            drop(found);
            given.truncate(start);
        }
    }
    replace_apostrophes_mut(given);
    if particles.is_empty() {
        return None;
    }
    Some(particles)
}

fn split_suffix(given: &mut String) -> Option<(String, bool)> {
    let re = regex!(r#",!?\s+\S.*$"#);
    if let Some(found) = re.find(given) {
        let s = found.as_str().trim_start_matches(",");
        let after_excl = s.strip_prefix("!").map(|x| (x, true));
        let (spaced_suffix, force_comma) = after_excl.unwrap_or((s, false));
        let suffix: String = spaced_suffix.trim().into();
        let start = found.start();
        drop(found);
        given.truncate(start);
        given.trim_end_in_place();
        return Some((suffix, force_comma));
    }
    None
}

impl From<PersonNameInput> for PersonName {
    fn from(input: PersonNameInput) -> Self {
        let is_latin_cyrillic = pn_is_latin_cyrillic(&input);

        let PersonNameInput {
            family,
            given,
            non_dropping_particle,
            dropping_particle,
            suffix,
            static_particles,
            comma_suffix,
        } = input;

        let mut pn = PersonName {
            family,
            given,
            non_dropping_particle,
            dropping_particle,
            suffix,
            static_particles,
            comma_suffix,
            is_latin_cyrillic,
        };

        let PersonName {
            family,
            given,
            non_dropping_particle,
            dropping_particle,
            suffix,
            static_particles,
            comma_suffix,
            is_latin_cyrillic: _,
        } = &mut pn;

        // Don't parse if these are supplied
        if *static_particles
            || non_dropping_particle.is_some()
            || dropping_particle.is_some()
            || suffix.is_some()
        {
            family.as_mut().map(replace_apostrophes_mut);
            given.as_mut().map(replace_apostrophes_mut);
            non_dropping_particle.as_mut().map(replace_apostrophes_mut);
            dropping_particle.as_mut().map(replace_apostrophes_mut);
            return pn;
        }

        if let Some(family) = family {
            if family.starts_with('"') && family.ends_with('"') {
                replace_apostrophes_mut(family);
            } else {
                *non_dropping_particle = split_nondrop_family(family);
            }
        }
        if let Some(given) = given {
            if given.starts_with('"') && given.ends_with('"') {
                replace_apostrophes_mut(given);
                return pn;
            }
            if let Some((suff, force_comma)) = split_suffix(given) {
                *suffix = Some(suff);
                *comma_suffix = force_comma;
            }
            *dropping_particle = split_drop_given(given);
        }

        pn
    }
}

#[test]
fn parse_particles() {
    impl PersonNameInput {
        fn parse_particles(self) -> PersonName {
            self.into()
        }
    }

    env_logger::init();

    let mut hi: String = " hi ".into();
    hi.trim_in_place();
    assert_eq!(&hi, "hi");

    let init = PersonNameInput {
        given: Some("Schnitzel".into()),
        family: Some("von Crumb".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("Schnitzel".into()),
            non_dropping_particle: Some("von".into()),
            family: Some("Crumb".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        given: Some("Eric".into()),
        family: Some("van der Vlist".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("Eric".into()),
            non_dropping_particle: Some("van der".into()),
            family: Some("Vlist".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        given: Some("Eric".into()),
        family: Some("del Familyname".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("Eric".into()),
            non_dropping_particle: Some("del".into()),
            family: Some("Familyname".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        given: Some("Givenname d'".into()),
        family: Some("Familyname".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("Givenname".into()),
            dropping_particle: Some("d\u{2019}".into()),
            family: Some("Familyname".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        given: Some("Givenname de".into()),
        family: Some("Familyname".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("Givenname".into()),
            dropping_particle: Some("de".into()),
            family: Some("Familyname".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        family: Some("Aubignac".into()),
        given: Some("François Hédelin d’".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("François Hédelin".into()),
            dropping_particle: Some("d\u{2019}".into()),
            family: Some("Aubignac".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        family: Some("d’Aubignac".into()),
        given: Some("François Hédelin".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("François Hédelin".into()),
            non_dropping_particle: Some("d\u{2019}".into()),
            family: Some("Aubignac".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );

    let init = PersonNameInput {
        given: Some("Dick".into()),
        family: Some("\"Van Dyke\"".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            given: Some("Dick".into()),
            family: Some("Van Dyke".into()),
            is_latin_cyrillic: true,
            ..Default::default()
        }
    );
    let init = PersonNameInput {
        family: Some("강".into()),
        given: Some("소라".into()),
        ..Default::default()
    };
    assert_eq!(
        init.parse_particles(),
        PersonName {
            family: Some("강".into()),
            given: Some("소라".into()),
            is_latin_cyrillic: false,
            ..Default::default()
        }
    );
}

/// https://users.rust-lang.org/t/trim-string-in-place/15809/8
pub trait TrimInPlace {
    fn trim_in_place(self: &'_ mut Self);
    // Returns number of bytes trimmed, if any
    fn trim_start_in_place(self: &'_ mut Self) -> usize;
    // Returns number of bytes trimmed, if any
    fn trim_end_in_place(self: &'_ mut Self) -> usize;
}
impl TrimInPlace for String {
    fn trim_in_place(self: &'_ mut Self) {
        let (start, len): (*const u8, usize) = {
            let self_trimmed: &str = self.trim();
            (self_trimmed.as_ptr(), self_trimmed.len())
        };
        if len == self.len() {
            return;
        }
        // Safety: src and dst here are both valid for len * size_of::<u8>() bytes.
        // Logic-wise, this copy allows copying between overlapping regions.
        // It's essentially libc's memmove.
        unsafe {
            core::ptr::copy(start, self.as_bytes_mut().as_mut_ptr(), len);
        }
        self.truncate(len);
    }
    fn trim_start_in_place(self: &'_ mut Self) -> usize {
        let old_len = self.len();
        let (start, new_len): (*const u8, usize) = {
            let self_trimmed: &str = self.trim_start();
            (self_trimmed.as_ptr(), self_trimmed.len())
        };
        if new_len == self.len() {
            return 0;
        }
        // See trim_in_place's unsafe block
        unsafe {
            core::ptr::copy(start, self.as_bytes_mut().as_mut_ptr(), new_len);
        }
        self.truncate(new_len);
        return old_len - new_len;
    }
    fn trim_end_in_place(self: &'_ mut Self) -> usize {
        // Nothing special here.
        let old_len = self.len();
        let new_len = self.trim_end().len();
        self.truncate(new_len);
        return old_len - new_len;
    }
}

fn replace_apostrophes_mut(s: &mut String) {
    let trim_quoted = s.trim_matches('\"');
    let replaced = lazy::lazy_replace_char(trim_quoted, '\'', "\u{2019}");
    if trim_quoted.len() == s.len() && replaced.is_borrowed() {
        return;
    }
    *s = replaced.into_owned();
}

fn replace_apostrophes(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    let trim_quoted = s.trim_matches('\"');
    lazy::lazy_replace_char(trim_quoted, '\'', "\u{2019}").into_owned()
}

use crate::unicode::is_latin_cyrillic;

fn pn_is_latin_cyrillic(pn: &PersonNameInput) -> bool {
    pn.family.as_ref().map_or(true, |s| is_latin_cyrillic(s))
        && pn.given.as_ref().map_or(true, |s| is_latin_cyrillic(s))
        && pn.suffix.as_ref().map_or(true, |s| is_latin_cyrillic(s))
        && pn
            .non_dropping_particle
            .as_ref()
            .map_or(true, |s| is_latin_cyrillic(s))
        && pn
            .dropping_particle
            .as_ref()
            .map_or(true, |s| is_latin_cyrillic(s))
}

#[test]
fn test_is_latin() {
    let pn = PersonNameInput {
        family: Some("강".into()),
        given: Some("소라".into()),
        ..Default::default()
    };
    assert!(!pn_is_latin_cyrillic(&pn));
    let pn = PersonNameInput {
        family: Some("Kang".into()),
        given: Some("So-ra".into()),
        ..Default::default()
    };
    assert!(pn_is_latin_cyrillic(&pn));
}
