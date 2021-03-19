use citeproc_io::{Name, PersonName, Reference};
use csl::{DateVariable, NameVariable};

#[derive(Debug, PartialEq, Eq)]
pub struct Trigraph(Vec<Vec<ConfigCell>>);

impl Trigraph {
    pub fn parse(s: &str) -> Result<Self, ()> {
        parser::colon_separated(s)
            .map_err(|_| ())
            .and_then(|(remain, x)| {
                if remain.is_empty() {
                    Ok(Trigraph(x))
                } else {
                    Err(())
                }
            })
    }
    pub fn make_label(&self, refr: &Reference) -> String {
        let mut string = String::with_capacity(6);
        let authors = refr.name.get(&NameVariable::Author);
        let issued = refr.date.get(&DateVariable::Issued);
        if self.0.len() == 0 {
            return string;
        }
        use std::fmt::Write;
        use unic_segment::Graphemes;
        if let Some(authors) = authors {
            let count = authors.len();
            let ix = std::cmp::min(count, self.0.len()) - 1;
            if count > 0 {
                let mut prog = 0usize;
                for author_printer in self.0[ix].iter().filter_map(|cell| match cell {
                    ConfigCell::Author { first_n_letters } => Some(*first_n_letters),
                    _ => None,
                }) {
                    let name_to_write = match &authors[prog] {
                        Name::Literal { literal, .. } => literal,
                        Name::Person(PersonName {
                            family: Some(family),
                            ..
                        }) => family,
                        Name::Person(PersonName {
                            family: None,
                            given: Some(given),
                            ..
                        }) => given,
                        _ => {
                            prog += 1;
                            continue;
                        }
                    };
                    let len = Graphemes::new(name_to_write)
                        .take(author_printer as usize)
                        .fold(0, |acc, x| acc + x.len());
                    write!(string, "{}", &name_to_write[..len]).unwrap();
                    prog += 1;
                }
            }
            if let Some(issued) = issued {
                if let Some(single) = issued.single_or_first() {
                    for year_digits in self.0[ix].iter().filter_map(|cell| match cell {
                        ConfigCell::Year { last_n_digits } => Some(*last_n_digits),
                        _ => None,
                    }) {
                        // Probably behaves weirdly for BC dates
                        write!(string, "{:02}", single.year % (10i32.pow(year_digits))).unwrap();
                    }
                }
            }
        }
        string
    }
}

impl Default for Trigraph {
    fn default() -> Self {
        Trigraph::parse("Aaaa00:AaAa00:AaAA00:AAAA00")
            .expect("Trigraph ought to parse the default!")
    }
}

#[test]
fn test_write_label() {
    use citeproc_io::{Date, DateOrRange};
    let trigraph = Trigraph::default();
    use csl::CslType;
    let mut refr = Reference::empty("ref_id".into(), CslType::Book);
    refr.name.insert(
        NameVariable::Author,
        vec![Name::Person(PersonName {
            family: Some("Jobs".into()),
            ..Default::default()
        })],
    );
    refr.date.insert(
        DateVariable::Issued,
        DateOrRange::Single(Date::new(1995, 0, 0)),
    );
    assert_eq!(trigraph.make_label(&refr), "Jobs95".to_owned());
    refr.name.insert(
        NameVariable::Author,
        vec![
            Name::Person(PersonName {
                family: Some("Boris".into()),
                ..Default::default()
            }),
            Name::Person(PersonName {
                family: Some("Johnson".into()),
                ..Default::default()
            }),
        ],
    );
    assert_eq!(trigraph.make_label(&refr), "BoJo95".to_owned());
}

#[test]
fn test_parse_trigraph() {
    assert_eq!(
        Trigraph::parse("Aaaa00:AaAa00:AaAA00:AAAA00"),
        Ok(Trigraph(vec![
            vec![
                ConfigCell::Author { first_n_letters: 4 },
                ConfigCell::Year { last_n_digits: 2 },
            ],
            vec![
                ConfigCell::Author { first_n_letters: 2 },
                ConfigCell::Author { first_n_letters: 2 },
                ConfigCell::Year { last_n_digits: 2 },
            ],
            vec![
                ConfigCell::Author { first_n_letters: 2 },
                ConfigCell::Author { first_n_letters: 1 },
                ConfigCell::Author { first_n_letters: 1 },
                ConfigCell::Year { last_n_digits: 2 },
            ],
            vec![
                ConfigCell::Author { first_n_letters: 1 },
                ConfigCell::Author { first_n_letters: 1 },
                ConfigCell::Author { first_n_letters: 1 },
                ConfigCell::Author { first_n_letters: 1 },
                ConfigCell::Year { last_n_digits: 2 },
            ],
        ]))
    )
}

#[derive(Debug, PartialEq, Eq)]
enum ConfigCell {
    Author { first_n_letters: u32 },
    Year { last_n_digits: u32 },
}

mod parser {
    use super::*;
    use nom::{
        branch::alt,
        bytes::complete::{take_while, take_while1},
        character::complete::char,
        combinator::recognize,
        multi::{many1, separated_list1},
        IResult,
    };

    fn author(inp: &str) -> IResult<&str, ConfigCell> {
        let (rest, _a) = char('A')(inp)?;
        let (rest, lowers) = recognize(take_while(|c: char| c == 'a'))(rest)?;
        Ok((
            rest,
            ConfigCell::Author {
                first_n_letters: 1 + lowers.len() as u32,
            },
        ))
    }

    fn year(inp: &str) -> IResult<&str, ConfigCell> {
        let (rest, zeroes) = recognize(take_while1(|c| c == '0'))(inp)?;
        Ok((
            rest,
            ConfigCell::Year {
                last_n_digits: zeroes.len() as u32,
            },
        ))
    }

    #[test]
    fn test_author() {
        assert_eq!(
            author("Aaaa"),
            Ok(("", ConfigCell::Author { first_n_letters: 4 }))
        )
    }

    pub(super) fn colon_separated(inp: &str) -> IResult<&str, Vec<Vec<ConfigCell>>> {
        separated_list1(char(':'), many1(alt((author, year))))(inp)
    }
}
