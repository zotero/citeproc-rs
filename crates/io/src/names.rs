// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::{String, SmartCow, lazy};

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
    // TODO: support "string", "number", "boolean"
    #[serde(default)]
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
            NameInput::Literal { literal } => {
                Name::Person(PersonName {
                    is_latin_cyrillic: is_latin_cyrillic(&literal),
                    family: Some(literal),
                    ..Default::default()
                })
            }
            NameInput::Person(pn) => Name::Person(pn.into()),
        }
    }
}

// Now we implement From<PersonNameInput> for PersonName

// Parsing particles
// Ported from https://github.com/Juris-M/citeproc-js/blob/1aa49dd2ab9a1c85d3060073780d65c86754a438/src/util_name_particles.js

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

fn split_particles(orig_name_str: &str, is_given: bool) -> Option<(String, String)> {
    let givenn_particles_re = regex!("^(?:\u{02bb}\\s|\u{2019}\\s|\\s|'\\s)?\\S+\\s*");
    let family_particles_re = regex!("^\\S+(?:\\-|\u{02bb}|\u{2019}|\\s|\')\\s*");
    debug!("split_particles: {:?}", orig_name_str);
    let (splitter, name_str) = if is_given {
        (
            givenn_particles_re,
            SmartCow::Owned(orig_name_str.chars().rev().collect()),
        )
    } else {
        (family_particles_re, SmartCow::Borrowed(orig_name_str))
    };
    let mut particles = Vec::new();

    let mut slice = &name_str[..];
    let mut eaten = 0;
    while let Some(mat) = splitter.find(slice) {
        let matched_particle = mat.as_str();
        let particle = if is_given {
            SmartCow::Owned(matched_particle.chars().rev().collect())
        } else {
            SmartCow::Borrowed(matched_particle)
        };
        debug!("found particle? {:?}", &particle);
        // first sign of an uppercase word -- break out
        let has_particle = particle
            .chars()
            // For " d'", etc
            .filter(|c| !c.is_whitespace() && !['-', '\'', '\u{02bb}', '\u{2019}'].contains(c))
            .nth(0)
            .map_or(false, |c| c.is_lowercase());
        if !has_particle {
            break;
        }
        slice = &slice[particle.len()..];
        eaten += particle.len();
        particles.push(particle);
    }
    let remain = if is_given {
        particles.reverse();
        if particles.len() > 1 {
            for i in 1..particles.len() {
                if particles[i].chars().nth(0) == Some(' ') {
                    particles[i - 1].make_mut().push(' ');
                }
            }
        }
        for i in 0..particles.len() {
            if particles[i].chars().nth(0) == Some(' ') {
                particles[i].make_mut().remove(0);
            }
        }
        &orig_name_str[..orig_name_str.len() - eaten]
    } else {
        &orig_name_str[eaten..]
    };
    if particles.is_empty() {
        None
    } else {
        use itertools::Itertools;
        Some((
            String::from(particles.iter().map(|cow| cow.as_ref()).join("")),
            replace_apostrophes(remain),
        ))
    }
}

// Maybe {truncates given, returns a suffix}
fn parse_suffix(given: &mut String, has_dropping_particle: bool) -> Option<(String, bool)> {
    let comma = regex!(r"\s*,!?\s*");
    let mut suff = None;
    let trunc_len = if let Some(mat) = comma.find(given) {
        let possible_suffix = &given[mat.end()..];
        let possible_comma = mat.as_str().trim();
        if (possible_suffix == "et al" || possible_suffix == "et al.") && !has_dropping_particle {
            warn!("used et-al as a suffix in name, not handled with citeproc-js-style hacks");
            return None;
        } else {
            let force_comma = possible_comma.len() == 2;
            suff = Some((possible_suffix.into(), force_comma))
        }
        Some(mat.start())
    } else {
        None
    };
    if let Some(trun) = trunc_len {
        given.truncate(trun);
    }
    suff
}

fn trim_last(string: &mut String) {
    let last_char = string.chars().rev().nth(0);
    string.trim_in_place();

    if string.is_empty() {
        return;
    }
    // graphemes unnecessary as particles basically end with one of a few select characters in the
    // regex below
    if let Some(last_char) = last_char {
        if last_char == ' '
            && string.chars().rev().nth(0).map_or(false, |second_last| {
                second_last == '\'' || second_last == '\u{2019}'
            })
        {
            string.push(' ');
        }
    }
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
            *family = family.as_ref().map(|x| replace_apostrophes(x));
            *given = given.as_ref().map(|x| replace_apostrophes(x));
            *non_dropping_particle = non_dropping_particle
                .as_ref()
                .map(|x| replace_apostrophes(x));
            *dropping_particle = dropping_particle.as_ref().map(|x| replace_apostrophes(x));
            return pn;
        }
        if let Some(family) = family {
            if family.starts_with('"') && family.ends_with('"') {
                *family = replace_apostrophes(&family);
            } else if let Some((mut nondrops, remain)) = split_particles(family.as_ref(), false) {
                trim_last(&mut nondrops);
                *non_dropping_particle = Some(replace_apostrophes(nondrops));
                *family = remain;
            } else {
                *family = replace_apostrophes(&family);
            }
        }
        if let Some(given) = given {
            if given.starts_with('"') && given.ends_with('"') {
                *given = replace_apostrophes(&given);
                return pn;
            }
            if let Some((suff, force_comma)) = parse_suffix(given, dropping_particle.is_some()) {
                *suffix = Some(suff);
                *comma_suffix = force_comma;
            }
            if let Some((drops, remain)) = split_particles(given.as_ref(), true) {
                *dropping_particle = Some(replace_apostrophes(drops.trim()));
                *given = remain;
            } else {
                *given = replace_apostrophes(&given);
            }
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
    fn trim_start_in_place(self: &'_ mut Self);
    fn trim_end_in_place(self: &'_ mut Self);
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
            core::ptr::copy(
                start,
                self.as_bytes_mut().as_mut_ptr(),
                len,
            );
        }
        self.truncate(len);
    }
    fn trim_start_in_place(self: &'_ mut Self) {
        let (start, len): (*const u8, usize) = {
            let self_trimmed: &str = self.trim_start();
            (self_trimmed.as_ptr(), self_trimmed.len())
        };
        if len == self.len() {
            return;
        }
        // See trim_in_place's unsafe block
        unsafe {
            core::ptr::copy(
                start,
                self.as_bytes_mut().as_mut_ptr(),
                len,
            );
        }
        self.truncate(len);
    }
    fn trim_end_in_place(self: &'_ mut Self) {
        // Nothing special here.
        self.truncate(self.trim_end().len());
    }
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

