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
    let givenn_particles_re = regex!("^[^ ]+(?:\\-|\u{02bb}|\u{2019}| |\') *");
    let family_particles_re = regex!("^[^ ]+(?:\u{02bb} |\u{2019} | |\' ) *");
    let (splitter, name_str) = if is_given {
        (givenn_particles_re, Cow::Owned(orig_name_str.chars().rev().collect()))
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
        let has_particle = particle.chars().nth(0).map_or(false, |c| c.is_lowercase());
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
                    particles[i-1].to_mut().push(' ');
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
        Some((particles.iter().map(|cow| cow.as_ref()).join(""), remain.replace('\'', "\u{2019}")))
    }
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
        if last_char == ' ' && string.chars().rev().nth(0).map_or(false, |second_last| {
            second_last == '\'' || second_last == '\u{2019}'
        }) {
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
        } = self;
        // Don't parse if these are supplied
        if *static_particles || non_dropping_particle.is_some() || dropping_particle.is_some() || suffix.is_some() {
            return;
        }
        if let Some(family) = family {
            if let Some((mut nondrops, remain)) = split_particles(family.as_ref(), false) {
                trim_last(&mut nondrops);
                *non_dropping_particle = Some(nondrops.replace('\'', "\u{2019}"));
                *family = remain;
            } else {
                *family = family.replace('\'', "\u{2019}");
            }
        }
        if let Some(given) = given {
            if let Some((mut drops, remain)) = split_particles(given.as_ref(), true) {
                drops.trim_in_place();
                *dropping_particle = Some(drops.replace('\'', "\u{2019}"));
                *given = remain;
            } else {
                *given = given.replace('\'', "\u{2019}");
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
    assert_eq!(init, PersonName {
        given: Some("Schnitzel".to_owned()),
        non_dropping_particle: Some("von".to_owned()),
        family: Some("Crumb".to_owned()),
        ..Default::default()
    });

    let mut init = PersonName {
        given: Some("Eric".to_owned()),
        family: Some("van der Vlist".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(init, PersonName {
        given: Some("Eric".to_owned()),
        non_dropping_particle: Some("van der".to_owned()),
        family: Some("Vlist".to_owned()),
        ..Default::default()
    });

    let mut init = PersonName {
        given: Some("Eric".to_owned()),
        family: Some("del Familyname".to_owned()),
        ..Default::default()
    };
    init.parse_particles();
    assert_eq!(init, PersonName {
        given: Some("Eric".to_owned()),
        non_dropping_particle: Some("del".to_owned()),
        family: Some("Familyname".to_owned()),
        ..Default::default()
    });





}

/// https://users.rust-lang.org/t/trim-string-in-place/15809/8
trait TrimInPlace { fn trim_in_place (self: &'_ mut Self); }
impl TrimInPlace for String {
    fn trim_in_place (self: &'_ mut Self)
    {
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
}

