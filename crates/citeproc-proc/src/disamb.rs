// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use citeproc_io::{Date, DateOrRange, Name, NumericValue, Reference};
use csl::Atom;
use std::collections::HashSet;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum DisambToken {
    // Should this be an Atom, really? There will not typically be that much reuse going on. It
    // might inflate the cache too much. The size of the disambiguation index is reduced, though.
    Str(Atom),

    /// Significantly simplifies things compared to ultra-localized date output strings.
    /// Reference cannot predict what they'll look like.
    /// `Date` itself can encode the lack of day/month with those fields set to zero.
    Date(Date),

    Num(NumericValue),

    YearSuffix(Atom),
}

pub trait AddDisambTokens {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool);
    #[inline]
    fn add_tokens_index(&self, set: &mut HashSet<DisambToken>) {
        self.add_tokens_ctx(set, true);
    }
    #[inline]
    fn add_tokens(&self, set: &mut HashSet<DisambToken>) {
        self.add_tokens_ctx(set, false);
    }
}

impl AddDisambTokens for Reference {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        for val in self.ordinary.values() {
            set.insert(DisambToken::Str(val.as_str().into()));
        }
        for val in self.number.values() {
            set.insert(DisambToken::Num(val.clone()));
        }
        for val in self.name.values() {
            for name in val.iter() {
                name.add_tokens_ctx(set, indexing);
            }
        }
        for val in self.date.values() {
            val.add_tokens_ctx(set, indexing);
        }
    }
}

impl AddDisambTokens for Option<String> {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, _indexing: bool) {
        if let Some(ref x) = self {
            set.insert(DisambToken::Str(x.as_str().into()));
        }
    }
}

impl AddDisambTokens for Name {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        match self {
            Name::Person(ref pn) => {
                pn.family.add_tokens_ctx(set, indexing);
                pn.given.add_tokens_ctx(set, indexing);
                pn.non_dropping_particle.add_tokens_ctx(set, indexing);
                pn.dropping_particle.add_tokens_ctx(set, indexing);
                pn.suffix.add_tokens_ctx(set, indexing);
            }
            Name::Literal { ref literal } => {
                set.insert(DisambToken::Str(literal.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for DateOrRange {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        match self {
            DateOrRange::Single(ref single) => {
                single.add_tokens_ctx(set, indexing);
            }
            DateOrRange::Range(d1, d2) => {
                d1.add_tokens_ctx(set, indexing);
                d2.add_tokens_ctx(set, indexing);
            }
            DateOrRange::Literal(ref lit) => {
                set.insert(DisambToken::Str(lit.as_str().into()));
            }
        }
    }
}

impl AddDisambTokens for Date {
    fn add_tokens_ctx(&self, set: &mut HashSet<DisambToken>, indexing: bool) {
        // when processing a cite, only insert the segments you actually used
        set.insert(DisambToken::Date(*self));
        // for the index, add all possible variations
        if indexing {
            let just_ym = Date {
                year: self.year,
                month: self.month,
                day: 0,
            };
            let just_year = Date {
                year: self.year,
                month: 0,
                day: 0,
            };
            set.insert(DisambToken::Date(just_ym));
            set.insert(DisambToken::Date(just_year));
        }
    }
}

use csl::style::Formatting;
use std::collections::BTreeSet;
use std::collections::HashMap;
// use citeproc_io::output::OutputFormat;

use petgraph::graph::{Graph, NodeIndex};

// // NFA, but only one epsilon transition per node.
// // Two if branches in a row = A -ε-> B(if1) -ε-> C(if2)
#[derive(Clone, PartialEq, Eq, Hash)]
struct NfaToken(String, Formatting);

use std::fmt::{Debug, Formatter};

impl Debug for NfaToken {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        if self.1 == Formatting::default() {
            write!(f, "{}", self.0)
        } else {
            write!(f, "NfaToken({:?}, {:?})", self.0, self.1)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NfaEdge {
    Epsilon,
    Token(NfaToken),
}

impl<'a> From<&'a str> for NfaToken {
    fn from(s: &'a str) -> Self {
        NfaToken(s.to_owned(), Formatting::default())
    }
}

type NfaGraph = Graph<(), NfaEdge>;
type DfaGraph = Graph<(), NfaToken>;

fn epsilon_closure(nfa: &NfaGraph, closure: &mut BTreeSet<NodeIndex>) {
    let mut work: Vec<_> = closure.iter().cloned().collect();
    while !work.is_empty() {
        let s = work.pop().unwrap();
        for node in nfa.neighbors(s) {
            let is_epsilon = nfa
                .find_edge(s, node)
                .and_then(|e| nfa.edge_weight(e))
                .map(|e| e == &NfaEdge::Epsilon)
                .unwrap_or(false);
            if is_epsilon && !closure.contains(&node) {
                work.push(node);
                closure.insert(node);
            }
        }
    }
}

#[derive(Debug)]
struct Nfa {
    graph: NfaGraph,
    accepting: BTreeSet<NodeIndex>,
    start: BTreeSet<NodeIndex>,
}

#[derive(Debug)]
struct Dfa {
    graph: DfaGraph,
    accepting: BTreeSet<NodeIndex>,
    start: NodeIndex,
}

// fn brzozowski_minimise(nfa: Nfa) -> Dfa {
//     // reverse
//     let rev1 = {
//         nfa.graph.reverse();
//         let orig_start = nfa.start;
//         let new_start = nfa.add_node(NfaNode::Normal);
//         nfa
//     };
//     // then convert to dfa via subset construction
//     // reverse again
//     // then convert to dfa via subset construction
//     to_dfa(&mid)
// }


// TODO: find the quotient automaton of the resulting DFA?
// Use Brzozowski's double-reversal algorithm
fn to_dfa(nfa: &Nfa) -> Dfa {
    let mut dfa = DfaGraph::new();

    let mut work = Vec::new();
    let mut start_set = nfa.start.clone();
    epsilon_closure(&nfa.graph, &mut start_set);
    let dfa_start_node = dfa.add_node(());
    let mut dfa_accepting = BTreeSet::new();
    for s in start_set.iter() {
        if nfa.accepting.contains(s) {
            dfa_accepting.insert(dfa_start_node);
            break;
        }
    }
    work.push((start_set.clone(), dfa_start_node));

    let mut dfa_states = HashMap::new();
    dfa_states.insert(start_set, dfa_start_node);

    while !work.is_empty() {
        let (dfa_state, current_node) = work.pop().unwrap();
        let mut by_edge_weight = HashMap::<NfaToken, BTreeSet<NodeIndex>>::new();
        for nfa_node in dfa_state {
            for neigh in nfa.graph.neighbors(nfa_node) {
                let weight = nfa
                    .graph
                    .find_edge(nfa_node, neigh)
                    .and_then(|e| nfa.graph.edge_weight(e))
                    .cloned()
                    .unwrap();
                if let NfaEdge::Token(t) = weight {
                    by_edge_weight
                        .entry(t)
                        .and_modify(|set| {
                            set.insert(neigh);
                        })
                        .or_insert_with(|| {
                            let mut set = BTreeSet::new();
                            set.insert(neigh);
                            set
                        });
                }
            }
        }
        for (k, mut set) in by_edge_weight.drain() {
            epsilon_closure(&nfa.graph, &mut set);
            if !dfa_states.contains_key(&set) {
                let node = dfa.add_node(());
                for s in set.iter() {
                    if nfa.accepting.contains(s) {
                        dfa_accepting.insert(node);
                        break;
                    }
                }
                dfa_states.insert(set.clone(), node);
                dfa.add_edge(current_node, node, k);
                work.push((set, node));
            } else {
                let &node = dfa_states.get(&set).unwrap();
                dfa.add_edge(current_node, node, k);
            }
        }
    }
    Dfa {
        graph: dfa,
        start: dfa_start_node,
        accepting: dfa_accepting,
    }
}

impl Dfa {
    fn accepts(&self, tokens: &[NfaToken]) -> bool {
        let mut cursor = self.start;
        for token in tokens {
            let mut found = false;
            for neighbour in self.graph.neighbors(cursor) {
                let weight = self
                    .graph
                    .find_edge(cursor, neighbour)
                    .and_then(|e| self.graph.edge_weight(e))
                    .map(|w| w == token);
                if let Some(true) = weight {
                    cursor = neighbour;
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        self.accepting.contains(&cursor)
    }
}

#[test]
fn nfa() {
    let andy = NfaEdge::Token("Andy".into());
    let reuben = NfaEdge::Token("Reuben".into());
    let peters = NfaEdge::Token("Peters".into());
    let comma = NfaEdge::Token(",".into());
    let twenty = NfaEdge::Token("2017".into());

    let mut nfa = {
        let mut nfa = NfaGraph::new();
        let initial = nfa.add_node(());
        let forwards1 = nfa.add_node(());
        let backwards1 = nfa.add_node(());
        let backwards2 = nfa.add_node(());
        let last_only = nfa.add_node(());
        let target = nfa.add_node(());
        let abc = nfa.add_node(());
        let acc = nfa.add_node(());
        nfa.add_edge(initial, forwards1, reuben.clone());
        nfa.add_edge(forwards1, target, peters.clone());
        nfa.add_edge(initial, backwards1, peters.clone());
        nfa.add_edge(backwards1, backwards2, comma.clone());
        nfa.add_edge(backwards2, target, reuben.clone());
        nfa.add_edge(initial, target, peters.clone());
        nfa.add_edge(target, abc, comma.clone());
        nfa.add_edge(abc, acc, twenty.clone());
        let mut accepting = BTreeSet::new();
        accepting.insert(acc);
        let mut start = BTreeSet::new();
        start.insert(initial);
        Nfa {
            graph: nfa,
            accepting,
            start,
        }
    };

    let mut nfa2 = {
        let mut nfa = NfaGraph::new();
        let initial = nfa.add_node(());
        let forwards1 = nfa.add_node(());
        let backwards1 = nfa.add_node(());
        let backwards2 = nfa.add_node(());
        let last_only = nfa.add_node(());
        let target = nfa.add_node(());
        let abc = nfa.add_node(());
        let acc = nfa.add_node(());
        nfa.add_edge(initial, forwards1, andy.clone());
        nfa.add_edge(forwards1, target, peters.clone());
        nfa.add_edge(initial, backwards1, peters.clone());
        nfa.add_edge(backwards1, backwards2, comma.clone());
        nfa.add_edge(backwards2, target, andy.clone());
        nfa.add_edge(initial, target, peters.clone());
        nfa.add_edge(target, abc, comma.clone());
        nfa.add_edge(abc, acc, twenty.clone());
        let mut accepting = BTreeSet::new();
        accepting.insert(acc);
        let mut start = BTreeSet::new();
        start.insert(initial);
        Nfa {
            graph: nfa,
            accepting,
            start,
        }
    };

    // You can't do this, because it will overwrite the `middle -ho-> target` edge above
    // nfa.add_edge(middle, target, NfaEdge::Epsilon);

    let dfa = to_dfa(&nfa);
    let dfa2 = to_dfa(&nfa2);

    println!("{:?}", dfa.start);
    use petgraph::dot::Dot;
    println!("{:?}", Dot::with_config(&dfa.graph, &[]));
    println!("{:?}", Dot::with_config(&dfa2.graph, &[]));

    assert!(dfa.accepts(&["Peters".into(), ",".into(), "2017".into()]));
    assert!(dfa2.accepts(&["Peters".into(), ",".into(), "2017".into()]));

    assert!(dfa.accepts(&["Reuben".into(), "Peters".into(), ",".into(), "2017".into()]));
    assert!(dfa.accepts(&[
        "Peters".into(),
        ",".into(),
        "Reuben".into(),
        ",".into(),
        "2017".into()
    ]));
    assert!(!dfa.accepts(&[
        "Peters".into(),
        ",".into(),
        "Andy".into(),
        ",".into(),
        "2017".into()
    ]));
    assert!(!dfa.accepts(&["Andy".into(), "Peters".into(), ",".into(), "2017".into()]));

    assert!(dfa2.accepts(&["Andy".into(), "Peters".into(), ",".into(), "2017".into()]));
    assert!(!dfa2.accepts(&[
        "Peters".into(),
        ",".into(),
        "Reuben".into(),
        ",".into(),
        "2017".into()
    ]));
    assert!(!dfa2.accepts(&["Reuben".into(), "Peters".into(), ",".into(), "2017".into()]));
}

// struct DisambNfa {
//     state: NfaIndex,
//     edges: Vec<HashMap<Option<NfaToken>, NfaIndex>>,
//     accepting: HashSet<NfaIndex>,
// }

// struct DisambiguationState<O: OutputFormat> {
//     format_stack: Vec<Formatting>,
//     format_current: Formatting,
//     dfa: DisambNfa<O>,
// }

// impl DisambiguationState {
//     /// if None, you should exit out of DFADisambiguate
//     fn digest(&mut self, token: DisambToken) -> Option<()> {
//         // ... match against internal DFA, using formatting from the stack.
//     }
//     fn push_fmt(&mut self, fmt: Formatting) {
//         self.format_stack.push(fmt);
//         self.coalesce_fmt();
//     }
//     fn pop_fmt(&mut self) {
//         self.format_stack.pop();
//         self.coalesce_fmt();
//     }
//     fn coalesce_fmt(&mut self) {
//         self.format_current = Formatting::default();
//         for fmt in self.format_stack.iter() {
//             self.format_current.font_style = fmt.font_style;
//             self.format_current.font_weight = fmt.font_weight;
//             self.format_current.font_variant = fmt.font_variant;
//             self.format_current.text_decoration = fmt.text_decoration;
//             self.format_current.vertical_alignment = fmt.vertical_alignment;
//         }
//     }
// }

// trait DFADisambiguate {
//     fn traverse(&self, state: &mut DisambiguationState);
// }

// impl DFADisambiguate for Vec<Node> {
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
