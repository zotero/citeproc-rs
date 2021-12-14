// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::db::IrDatabase;
use citeproc_io::output::{markup::Markup, OutputFormat};
use petgraph::dot::Dot;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::BTreeSet;
use std::collections::HashMap;

// XXX(pandoc): maybe force this to be a string and coerce pandoc output into a string
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EdgeData<O = <Markup as OutputFormat>::Output> {
    Output(O),

    // The rest are synchronised with fields on CiteContext and IR.
    Locator,
    NotUsed,
    LocatorLabel,

    /// TODO: add a parameter to Dfa::accepts_data to supply the actual year suffix for the particular reference.
    YearSuffix,

    /// Not for DFA matching, must be turned into YearSuffix via `RefIR::keep_first_ysh` before DFA construction
    YearSuffixExplicit,
    /// Not for DFA matching, must be turned into YearSuffix via `RefIR::keep_first_ysh` before DFA construction
    YearSuffixPlain,

    CitationNumber,
    CitationNumberLabel,

    // TODO: treat this specially? Does it help you disambiguate back-referencing cites?
    Frnn,
    FrnnLabel,

    /// The accessed date, which should not help disambiguate cites.
    Accessed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NfaEdge {
    Epsilon,
    Token(EdgeData),
}

impl From<EdgeData> for NfaEdge {
    fn from(edge: EdgeData) -> Self {
        NfaEdge::Token(edge)
    }
}

pub type NfaGraph = Graph<(), NfaEdge>;
pub type DfaGraph = Graph<(), EdgeData>;

fn epsilon_closure(nfa: &NfaGraph, closure: &mut BTreeSet<NodeIndex>) {
    let mut work: Vec<_> = closure.iter().cloned().collect();
    while !work.is_empty() {
        let s = work.pop().unwrap();
        for edge in nfa.edges(s) {
            let is_epsilon = *edge.weight() == NfaEdge::Epsilon;
            let target = edge.target();
            if is_epsilon && !closure.contains(&target) {
                work.push(target);
                closure.insert(target);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Nfa {
    pub graph: NfaGraph,
    pub accepting: BTreeSet<NodeIndex>,
    pub start: BTreeSet<NodeIndex>,
}

const NFA_INITIAL_CAPACITY: usize = 40;

impl Default for Nfa {
    fn default() -> Self {
        Nfa {
            graph: NfaGraph::with_capacity(NFA_INITIAL_CAPACITY, NFA_INITIAL_CAPACITY),
            accepting: Default::default(),
            start: Default::default(),
        }
    }
}

/// https://github.com/petgraph/petgraph/issues/199#issuecomment-484077775
fn graph_eq<N, E, Ty, Ix>(
    a: &petgraph::graph::Graph<N, E, Ty, Ix>,
    b: &petgraph::graph::Graph<N, E, Ty, Ix>,
) -> bool
where
    N: PartialEq,
    E: PartialEq,
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType + PartialEq,
{
    let a_ns = a.raw_nodes().iter().map(|n| &n.weight);
    let b_ns = b.raw_nodes().iter().map(|n| &n.weight);
    let a_es = a
        .raw_edges()
        .iter()
        .map(|e| (e.source(), e.target(), &e.weight));
    let b_es = b
        .raw_edges()
        .iter()
        .map(|e| (e.source(), e.target(), &e.weight));
    a_ns.eq(b_ns) && a_es.eq(b_es)
}

/// We have to have an Eq impl for Nfa so RefIR can have one
impl std::cmp::PartialEq for Nfa {
    fn eq(&self, other: &Self) -> bool {
        self.accepting == other.accepting
            && self.start == other.start
            && graph_eq(&self.graph, &other.graph)
    }
}

#[derive(Clone)]
pub struct Dfa {
    pub graph: DfaGraph,
    pub accepting: BTreeSet<NodeIndex>,
    pub start: NodeIndex,
}

/// We have to have an Eq impl so Dfa can be returned from a salsa query.
/// this compares each graph bit-for-bit.
impl std::cmp::Eq for Dfa {}
impl std::cmp::PartialEq for Dfa {
    fn eq(&self, other: &Self) -> bool {
        self.accepting == other.accepting
            && self.start == other.start
            && graph_eq(&self.graph, &other.graph)
    }
}

#[derive(Debug)]
enum DebugNode {
    Node,
    Start,
    Accepting,
    StartAndAccepting,
}

impl fmt::Display for DebugNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node => f.write_str(""),
            _ => <Self as fmt::Debug>::fmt(self, f),
        }
    }
}

impl fmt::Display for EdgeData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Output(o) => <str as fmt::Debug>::fmt(&o, f),
            _ => <Self as fmt::Debug>::fmt(self, f),
        }
    }
}

impl Dfa {
    pub fn debug_graph(&self, _db: &dyn IrDatabase) -> String {
        let g = self.graph.map(
            |node, _| {
                let start = node == self.start;
                let accept = self.accepting.contains(&node);
                if start && accept {
                    DebugNode::StartAndAccepting
                } else if start {
                    DebugNode::Start
                } else if accept {
                    DebugNode::Accepting
                } else {
                    DebugNode::Node
                }
            },
            |_, edge| edge.clone(),
        );
        format!(
            "{}",
            Dot::with_attr_getters(
                &g,
                &[],
                &|g, e| {
                    match &g[e.id()] {
                        EdgeData::Output(o) => {
                            return if o.starts_with(" ") || o.ends_with(" ") {
                                format!("label=<&quot;{}&quot;>", o)
                            } else {
                                format!("label=<{}>", o)
                            }
                        }
                        _ => r##"fontname="monospace" style="dashed" fontsize="12.0""##,
                    }
                    .to_string()
                },
                &|g, (node, _)| {
                    match &g[node] {
                        DebugNode::Start => {
                            r##"shape="invhouse" style="filled" fillcolor="#b6d6e2""##
                        }
                        DebugNode::Node => {
                            r##"shape="circle" width="0.2" style=filled fillcolor="#e2ecdf""##
                        }
                        DebugNode::Accepting => r##"shape="box" style=filled fillcolor="#A2B29F""##,
                        // reddish, because it's basically an error
                        DebugNode::StartAndAccepting => r##"style=filled fillcolor="#efb8a5""##,
                    }
                    .to_string()
                }
            )
        )
    }
}

impl Nfa {
    pub fn new() -> Self {
        Nfa::default()
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.accepting
    }

    pub fn add_complete_sequence(&mut self, tokens: Vec<EdgeData>) {
        let mut cursor = self.graph.add_node(());
        self.start.insert(cursor);
        for token in tokens {
            let next = self.graph.add_node(());
            self.graph.add_edge(cursor, next, NfaEdge::Token(token));
            cursor = next;
        }
        self.accepting.insert(cursor);
    }

    /// A Names block, for instance, is given start & end nodes, and simply fills in a segment a
    /// few times with increasing given-name counts etc.
    pub fn add_sequence_between(&mut self, a: NodeIndex, b: NodeIndex, tokens: Vec<EdgeData>) {
        let mut cursor = self.graph.add_node(());
        self.graph.add_edge(a, cursor, NfaEdge::Epsilon);
        for token in tokens {
            let next = self.graph.add_node(());
            self.graph.add_edge(cursor, next, NfaEdge::Token(token));
            cursor = next;
        }
        self.graph.add_edge(cursor, b, NfaEdge::Epsilon);
    }

    pub fn brzozowski_minimise(mut self: Nfa) -> Dfa {
        use std::mem;
        // reverse
        let rev1 = {
            self.graph.reverse();
            mem::swap(&mut self.start, &mut self.accepting);
            self
        };
        let mut dfa1 = to_dfa(&rev1);
        let rev2 = {
            dfa1.graph.reverse();
            let mut start_set = BTreeSet::new();
            start_set.insert(dfa1.start);
            Nfa {
                // TODO: implement into_map, so this doesn't need to clone all the edges and
                // simply throw dfa1 away
                graph: dfa1.graph.map(|_, _| (), |_, e| NfaEdge::Token(e.clone())),
                accepting: start_set,
                start: dfa1.accepting,
            }
        };
        to_dfa(&rev2)
    }
}

use std::fmt::{self, Formatter};

impl fmt::Debug for Dfa {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "start:{:?}\naccepting:{:?}\n{:?}\n---\n",
            &self.start,
            &self.accepting,
            Dot::with_config(&self.graph, &[])
        )
    }
}

pub fn to_dfa(nfa: &Nfa) -> Dfa {
    let mut dfa = DfaGraph::with_capacity(nfa.graph.node_count(), nfa.graph.edge_count());

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
        let mut by_edge_weight = HashMap::<EdgeData, BTreeSet<NodeIndex>>::new();
        for nfa_node in dfa_state {
            for edge in nfa.graph.edges(nfa_node) {
                let weight = edge.weight();
                let target = edge.target();
                if let NfaEdge::Token(t) = weight {
                    match by_edge_weight.get_mut(t) {
                        None => {
                            let mut set = BTreeSet::new();
                            set.insert(target);
                            by_edge_weight.insert(t.clone(), set);
                        }
                        Some(set) => {
                            set.insert(target);
                        }
                    }
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
    pub fn accepts_data(&self, data: &[EdgeData]) -> bool {
        let mut cursors = Vec::new();
        cursors.push((self.start, None, data));
        while !cursors.is_empty() {
            let (cursor, prepended, chunk) = cursors.pop().unwrap();
            let first = prepended.as_ref().or_else(|| chunk.get(0));
            if first == None && self.accepting.contains(&cursor) {
                // we did it!
                return true;
            }
            if let Some(token) = first {
                for edge in self.graph.edges(cursor) {
                    let weight = edge.weight();
                    let target = edge.target();
                    use std::cmp::min;
                    match (weight, token) {
                        // TODO: add an output check that EdgeData::YearSuffix contains the RIGHT
                        (w, t) if w == t => {
                            cursors.push((target, None, &chunk[min(1, chunk.len())..]));
                        }
                        (EdgeData::Output(w), EdgeData::Output(t)) => {
                            if w == t {
                                cursors.push((target, None, &chunk[min(1, chunk.len())..]));
                            } else if t.starts_with(w.as_str()) {
                                let next = if prepended.is_some() {
                                    // already have split this one
                                    0
                                } else {
                                    1
                                };
                                let t_rest = &t[w.len()..];
                                let c_rest = &chunk[min(next, chunk.len())..];
                                cursors.push((
                                    target,
                                    Some(EdgeData::Output(t_rest.into())),
                                    c_rest,
                                ));
                            }
                        }
                        _ => {} // Don't continue down this path
                    }
                }
            }
        }
        false
    }

    pub fn accepts(&self, tokens: &[EdgeData]) -> bool {
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
    let andy = || EdgeData::Output("andy".into());
    let reuben = || EdgeData::Output("reuben".into());
    let peters = || EdgeData::Output("peters".into());
    let comma = || EdgeData::Output(", ".into());
    let twenty = || EdgeData::Output("20".into());

    let nfa = {
        let mut nfa = NfaGraph::new();
        let initial = nfa.add_node(());
        let forwards1 = nfa.add_node(());
        let backwards1 = nfa.add_node(());
        let backwards2 = nfa.add_node(());
        let target = nfa.add_node(());
        let abc = nfa.add_node(());
        let acc = nfa.add_node(());
        nfa.add_edge(initial, forwards1, reuben().into());
        nfa.add_edge(forwards1, target, peters().into());
        nfa.add_edge(initial, backwards1, peters().into());
        nfa.add_edge(backwards1, backwards2, comma().into());
        nfa.add_edge(backwards2, target, reuben().into());
        nfa.add_edge(initial, target, peters().into());
        nfa.add_edge(target, abc, comma().into());
        nfa.add_edge(abc, acc, twenty().into());
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

    let nfa2 = {
        let mut nfa = NfaGraph::new();
        let initial = nfa.add_node(());
        let forwards1 = nfa.add_node(());
        let backwards1 = nfa.add_node(());
        let backwards2 = nfa.add_node(());
        let target = nfa.add_node(());
        let abc = nfa.add_node(());
        let acc = nfa.add_node(());
        nfa.add_edge(initial, forwards1, andy().into());
        nfa.add_edge(forwards1, target, peters().into());
        nfa.add_edge(initial, backwards1, peters().into());
        nfa.add_edge(backwards1, backwards2, comma().into());
        nfa.add_edge(backwards2, target, andy().into());
        nfa.add_edge(initial, target, peters().into());
        nfa.add_edge(target, abc, comma().into());
        nfa.add_edge(abc, acc, twenty().into());
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

    let dfa = to_dfa(&nfa);
    let dfa2 = to_dfa(&nfa2);

    let dfa_brz = nfa.brzozowski_minimise();
    let dfa2_brz = nfa2.brzozowski_minimise();

    println!("{:?}", dfa.start);
    println!("dfa {:?}", Dot::with_config(&dfa.graph, &[]));
    println!("dfa2 {:?}", Dot::with_config(&dfa2.graph, &[]));
    println!("dfa_brz {:?}", Dot::with_config(&dfa2_brz.graph, &[]));
    println!("dfa2_brz {:?}", Dot::with_config(&dfa2_brz.graph, &[]));

    let test_dfa = |dfa: &Dfa| {
        assert!(dfa.accepts(&[peters(), comma(), twenty()]));
        assert!(dfa.accepts(&[reuben(), peters(), comma(), twenty()]));
        assert!(dfa.accepts(&[peters(), comma(), reuben(), comma(), twenty()]));
        assert!(!dfa.accepts(&[peters(), comma(), andy(), comma(), twenty()]));
        assert!(!dfa.accepts(&[andy(), comma(), peters(), comma(), twenty()]));
    };

    let test_dfa2 = |dfa2: &Dfa| {
        assert!(dfa2.accepts(&[peters(), comma(), twenty()]));
        assert!(dfa2.accepts(&[andy(), peters(), comma(), twenty()]));
        assert!(!dfa2.accepts(&[peters(), comma(), reuben(), comma(), twenty()]));
        assert!(!dfa2.accepts(&[reuben(), peters(), comma(), twenty()]));
    };

    test_dfa(&dfa);
    test_dfa(&dfa_brz);
    test_dfa2(&dfa2);
    test_dfa2(&dfa2_brz);
}

#[test]
fn test_brzozowski_minimise() {
    let a = || EdgeData::Output("a".into());
    let b = || EdgeData::Output("b".into());
    let c = || EdgeData::Output("c".into());
    let d = || EdgeData::Output("d".into());
    let e = || EdgeData::Output("e".into());
    let nfa = {
        let mut nfa = Nfa::new();
        nfa.add_complete_sequence(vec![a(), b(), c(), e()]);
        nfa.add_complete_sequence(vec![a(), b(), e()]);
        nfa.add_complete_sequence(vec![b(), c(), d(), e()]);
        nfa.add_complete_sequence(vec![b(), d(), e()]);
        nfa
    };

    let dfa = nfa.brzozowski_minimise();
    println!("abcde {:?}", Dot::with_config(&dfa.graph, &[]));

    assert!(dfa.accepts(&[a(), b(), e()]));
    assert!(!dfa.accepts(&[a(), b(), c(), d(), e()]));
}
