// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

use crate::disamb::names::RefNameIR;
use crate::disamb::Nfa;
use crate::prelude::*;
use citeproc_io::output::LocalizedQuotes;
use csl::{Affixes, Formatting};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum RefIR {
    /// A piece of output that a cite can match in the final DFA.
    /// e.g.
    ///
    /// ```txt
    /// EdgeData::Output(r#"<span style="font-weight: bold;">"#)
    /// EdgeData::Output("Some title, <i>23/4/1969</i>")
    /// EdgeData::Locator
    /// ```
    Edge(Option<EdgeData>),

    /// When constructing RefIR, we know whether the names variables exist or not.
    /// So we don't have to handle 'substitute' any special way -- just drill down into the
    /// names element, apply its formatting, and end up with
    ///
    /// ```txt
    /// [
    ///     Edge("<whatever formatting>"),
    ///     // whatever the substitute element outputted
    ///     Edge("</whatever>")
    /// ]
    /// ```
    ///
    /// The Nfa represents all the edge streams that a Names block can output for one of its
    /// variables.
    Name(RefNameIR, Nfa),

    /// A non-string EdgeData can be surrounded by a Seq with other strings to apply its
    /// formatting. This will use `OutputFormat::stack_preorder() / ::stack_postorder()`.
    ///
    /// ```txt
    /// RefIR::Seq(vec![
    ///     EdgeData::Output("<i>"),
    ///     EdgeData::Locator,
    ///     EdgeData::Output("</i>"),
    /// ])
    /// ```
    Seq(RefIrSeq),
    // Could use this to apply a FreeCond set to a reference to create a path through the
    // constructed NFA.
    // See the module level documentation for `disamb`.
    // Branch(Arc<Conditions>, Box<IR<O>>),
}

impl Default for RefIR {
    fn default() -> Self {
        RefIR::Edge(None)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct RefIrSeq {
    pub contents: Vec<RefIR>,
    pub formatting: Option<Formatting>,
    pub affixes: Option<Affixes>,
    pub delimiter: Option<SmartString>,
    pub quotes: Option<LocalizedQuotes>,
    pub text_case: TextCase,
    pub should_inherit_delim: bool,
}

impl RefIR {
    pub fn debug(&self, db: &dyn IrDatabase) -> String {
        match self {
            RefIR::Edge(Some(e)) => format!("{:?}", e),
            RefIR::Edge(None) => "None".into(),
            RefIR::Seq(seq) => {
                let mut s = String::new();
                s.push_str("[");
                let mut seen = false;
                for x in &seq.contents {
                    if seen {
                        s.push_str(",");
                    }
                    seen = true;
                    s.push_str(&x.debug(db));
                }
                s.push_str("]");
                s
            }
            RefIR::Name(rnir, _nfa) => format!("NameVariable::{:?}", rnir.variable),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            RefIR::Edge(None) => true,
            RefIR::Seq(seq) => seq.contents.is_empty(),
            RefIR::Name(_rnir, nfa) => nfa.is_empty(),
            _ => false,
        }
    }

    pub(crate) fn keep_first_ysh(
        &mut self,
        ysh_explicit_edge: EdgeData,
        ysh_plain_edge: EdgeData,
        ysh_edge: EdgeData,
    ) {
        let found = &mut false;
        self.visit_ysh(ysh_explicit_edge, &mut |opt_e| {
            if !*found {
                // first time
                *found = true;
                *opt_e = Some(ysh_edge.clone());
            } else {
                // subsequent ones are extraneous, so make them disappear
                *opt_e = None;
            }
            false
        });
        self.visit_ysh(ysh_plain_edge, &mut |opt_e| {
            if !*found {
                *found = true;
                *opt_e = Some(ysh_edge.clone());
            } else {
                *opt_e = None;
            }
            false
        });
    }

    pub(crate) fn visit_ysh<F>(&mut self, ysh_edge: EdgeData, callback: &mut F) -> bool
    where
        F: (FnMut(&mut Option<EdgeData>) -> bool),
    {
        match self {
            RefIR::Edge(ref mut opt_e) if opt_e.as_ref() == Some(&ysh_edge) => callback(opt_e),
            RefIR::Seq(seq) => {
                for ir in seq.contents.iter_mut() {
                    let done = ir.visit_ysh(ysh_edge.clone(), callback);
                    if done {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
}

// #[derive(Debug, Clone)]
// pub struct RefIrNameCounter {
//     name_irs: Vec<RefNameIR>,
// }
// impl RefIrNameCounter {
//     fn count(&self) -> u32 {
//         500
//     }
//     pub fn render_ref(&self, db: &dyn IrDatabase, ctx: &RefContext<'_, Markup>, stack: Formatting, piq: Option<bool>) -> (RefIR, GroupVars) {
//         let count = self.count();
//         let fmt = ctx.format;
//         let out = fmt.output_in_context(fmt.text_node(format!("{}", count), None), stack, piq);
//         let edge = db.edge(EdgeData::<Markup>::Output(out));
//         (RefIR::Edge(Some(edge)), GroupVars::Important)
//     }
// }
