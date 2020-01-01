use crate::prelude::*;
use csl::Atom;
use std::mem;

impl<O: OutputFormat> IR<O> {
    pub fn split_first_field(&mut self) {
        if let Some(((first, gv), mut me)) = match self {
            IR::Seq(seq) => if seq.contents.len() > 1 {
                Some(seq.contents.remove(0))
            } else {
                None
            }
            .and_then(|f| Some((f, mem::take(seq)))),
            _ => None,
        } {
            me.display = Some(DisplayMode::RightInline);
            let (afpre, afsuf) = me.affixes.map(|mine| (
                Some(Affixes {
                    prefix: mine.prefix,
                    suffix: Atom::from(""),
                }),
                Some(Affixes {
                    prefix: Atom::from(""),
                    suffix: mine.suffix,
                }),
            )).unwrap_or((None, None));
            mem::replace(
                self,
                IR::Seq(IrSeq {
                    contents: vec![
                        (
                            IR::Seq(IrSeq {
                                contents: vec![(first, gv)],
                                display: Some(DisplayMode::LeftMargin),
                                affixes: afpre,
                                ..Default::default()
                            }),
                            gv,
                        ),
                        (
                            IR::Seq(IrSeq {
                                contents: me.contents,
                                display: Some(DisplayMode::RightInline),
                                affixes: afsuf,
                                ..Default::default()
                            }),
                            GroupVars::Important,
                        ),
                    ],
                    display: None,
                    formatting: me.formatting,
                    affixes: None,
                    delimiter: me.delimiter.clone(),
                    dropped_gv: None,
                    quotes: me.quotes.clone(),
                    text_case: me.text_case,
                }),
            );
        }
    }
}
