// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

#![allow(dead_code)]

use csl::style::Formatting;

mod finite_automata;
pub mod old;

pub use finite_automata::{Dfa, Edge, EdgeData, Nfa};

pub trait Disambiguation {
    fn construct_nfa(&self, state: &mut DisambiguationState);
}

pub struct DisambiguationState {
    format_stack: Vec<Formatting>,
    format_current: Formatting,
    nfa: Nfa,
}

impl DisambiguationState {
    pub fn new() -> Self {
        DisambiguationState {
            format_stack: vec![],
            format_current: Default::default(),
            nfa: Nfa::new(),
        }
    }
    /// if None, you should exit out of DFADisambiguate
    fn digest(&mut self, _token: EdgeData) -> Option<()> {
        // ... match against internal DFA, using formatting from the stack.
        None
    }
    pub fn push_fmt(&mut self, fmt: Formatting) {
        self.format_stack.push(fmt);
        self.coalesce_fmt();
    }
    pub fn pop_fmt(&mut self) {
        self.format_stack.pop();
        self.coalesce_fmt();
    }
    fn coalesce_fmt(&mut self) {
        self.format_current = Formatting::default();
        for fmt in self.format_stack.iter() {
            self.format_current.font_style = fmt.font_style;
            self.format_current.font_weight = fmt.font_weight;
            self.format_current.font_variant = fmt.font_variant;
            self.format_current.text_decoration = fmt.text_decoration;
            self.format_current.vertical_alignment = fmt.vertical_alignment;
        }
    }
}

// impl ConstructNfa for Vec<Node> {
//     fn traverse(&self, state: &mut DisambiguationState) -> Option<()> {
//         match self {
//             Text(s) => for token in s.split(" ") { state.digest(token)?; },
//             Fmt(f, ts) => {
//                 state.push_fmt(f);
//                 for token in ts {
//                     state.digest(token)?;
//                 }
//                 state.pop_fmt();
//             }
//         }
//     }
// }
