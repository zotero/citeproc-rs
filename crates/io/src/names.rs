// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

// kebab-case here is the same as Strum's "kebab_case",
// but with a more accurate name
#[derive(Default, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
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
}

#[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Clone)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum Name {
    // Put literal first, because PersonName's properties are all Options and derived
    // Deserialize impls run in order.
    Literal {
        // the untagged macro uses the field names on Literal { literal } instead of the discriminant, so don't change that
        literal: String,
    },
    Person(PersonName),
    // TODO: represent an institution in CSL-M?
}

// Parsing particles
// Ported from https://github.com/Juris-M/citeproc-js/blob/1aa49dd2ab9a1c85d3060073780d65c86754a438/src/util_name_particles.js

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

use std::borrow::Cow;

fn split_particles(mut orig_name_str: &str, is_given: bool) -> Option<(String, String)> {
    let givenn_particles_re = regex!("^(?:\u{02bb}\\s|\u{2019}\\s|\\s|'\\s)?\\S+\\s*");
    let family_particles_re = regex!("^\\S+(?:\\-|\u{02bb}|\u{2019}|\\s|\')\\s*");
    debug!("split_particles: {:?}", orig_name_str);
    let (splitter, name_str) = if is_given {
        (
            givenn_particles_re,
            Cow::Owned(orig_name_str.chars().rev().collect()),
        )
    } else {
        (family_particles_re, Cow::Borrowed(orig_name_str))
    };
    let mut particles = Vec::new();

    let mut slice = &name_str[..];
    let mut eaten = 0;
    while let Some(mat) = splitter.find(slice) {
        let matched_particle = mat.as_str();
        let particle = if is_given {
            Cow::Owned(matched_particle.chars().rev().collect())
        } else {
            Cow::Borrowed(matched_particle)
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
                    particles[i - 1].to_mut().push(' ');
                }
            }
        }
        for i in 0..particles.len() {
            if particles[i].chars().nth(0) == Some(' ') {
                particles[i].to_mut().remove(0);
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
            particles.iter().map(|cow| cow.as_ref()).join(""),
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
            suff = Some((possible_suffix.to_owned(), force_comma))
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

impl PersonName {
    pub fn parse_particles(&mut self) {
        let PersonName {
            family,
            given,
            non_dropping_particle,
            dropping_particle,
            suffix,
            static_particles,
            comma_suffix,
        } = self;
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
            return;
        }
        if let Some(family) = family {
            if let Some((mut nondrops, remain)) = split_particles(family.as_ref(), false) {
                trim_last(&mut nondrops);
                *non_dropping_particle = Some(replace_apostrophes(nondrops));
                *family = remain;
            } else {
                *family = replace_apostrophes(&family);
            }
        }
        if let Some(given) = given {
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
    }
}

#[test]
fn parse_particles() {
    env_logger::init();

    let mut hi = " hi ".to_owned();
    hi.trim_in_place();
    assert_eq!(hi, "hi");

    let mut init = PersonName {
        given: Some("Schnitzel".to_owned()),
        family: Some("von Crumb".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(
        init,
        PersonName {
            given: Some("Schnitzel".to_owned()),
            non_dropping_particle: Some("von".to_owned()),
            family: Some("Crumb".to_owned()),
            ..Default::default()
        }
    );

    let mut init = PersonName {
        given: Some("Eric".to_owned()),
        family: Some("van der Vlist".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(
        init,
        PersonName {
            given: Some("Eric".to_owned()),
            non_dropping_particle: Some("van der".to_owned()),
            family: Some("Vlist".to_owned()),
            ..Default::default()
        }
    );

    let mut init = PersonName {
        given: Some("Eric".to_owned()),
        family: Some("del Familyname".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(
        init,
        PersonName {
            given: Some("Eric".to_owned()),
            non_dropping_particle: Some("del".to_owned()),
            family: Some("Familyname".to_owned()),
            ..Default::default()
        }
    );

    let mut init = PersonName {
        given: Some("Givenname d'".to_owned()),
        family: Some("Familyname".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(
        init,
        PersonName {
            given: Some("Givenname".to_owned()),
            dropping_particle: Some("d\u{2019}".to_owned()),
            family: Some("Familyname".to_owned()),
            ..Default::default()
        }
    );

    let mut init = PersonName {
        family: Some("Aubignac".to_owned()),
        given: Some("François Hédelin d’".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(
        init,
        PersonName {
            given: Some("François Hédelin".to_owned()),
            dropping_particle: Some("d\u{2019}".to_owned()),
            family: Some("Aubignac".to_owned()),
            ..Default::default()
        }
    );

    let mut init = PersonName {
        family: Some("d’Aubignac".to_owned()),
        given: Some("François Hédelin".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(
        init,
        PersonName {
            given: Some("François Hédelin".to_owned()),
            non_dropping_particle: Some("d\u{2019}".to_owned()),
            family: Some("Aubignac".to_owned()),
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
        unsafe {
            core::ptr::copy(
                start,
                self.as_bytes_mut().as_mut_ptr(), // no str::as_mut_ptr() in std ...
                len,
            );
        }
        self.truncate(len); // no String::set_len() in std ...
    }
    fn trim_start_in_place(self: &'_ mut Self) {
        let (start, len): (*const u8, usize) = {
            let self_trimmed: &str = self.trim_start();
            (self_trimmed.as_ptr(), self_trimmed.len())
        };
        unsafe {
            core::ptr::copy(
                start,
                self.as_bytes_mut().as_mut_ptr(), // no str::as_mut_ptr() in std ...
                len,
            );
        }
        self.truncate(len); // no String::set_len() in std ...
    }
    fn trim_end_in_place(self: &'_ mut Self) {
        self.truncate(self.trim_end().len());
    }
}

fn replace_apostrophes(s: impl AsRef<str>) -> String {
    s.as_ref().replace("\'", "\u{2019}")
}
