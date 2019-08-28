// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::db::IrDatabase;
use petgraph::graph::{Graph, NodeIndex};
use salsa::{InternId, InternKey};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

use citeproc_io::output::{html::Html, OutputFormat};

#[cfg(test)]
use petgraph::dot::Dot;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Edge(u32);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EdgeData(<Html as OutputFormat>::Output);

impl Edge {
    // Adding this method is often convenient, since you can then
    // write `path.lookup(db)` to access the data, which reads a bit better.
    pub fn lookup(self, db: &impl IrDatabase) -> EdgeData {
        IrDatabase::lookup_edge(db, self)
    }
}

impl InternKey for Edge {
    fn from_intern_id(v: InternId) -> Self {
        Edge(u32::from(v))
    }
    fn as_intern_id(&self) -> InternId {
        InternId::from(self.0)
    }
}

impl Debug for EdgeData {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NfaEdge {
    Epsilon,
    Token(Edge),
}

impl From<Edge> for NfaEdge {
    fn from(edge: Edge) -> Self {
        NfaEdge::Token(edge)
    }
}

pub type NfaGraph = Graph<(), NfaEdge>;
pub type DfaGraph = Graph<(), Edge>;

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
pub struct Nfa {
    graph: NfaGraph,
    accepting: BTreeSet<NodeIndex>,
    start: BTreeSet<NodeIndex>,
}

#[derive(Debug)]
pub struct Dfa {
    graph: DfaGraph,
    accepting: BTreeSet<NodeIndex>,
    start: NodeIndex,
}

impl Nfa {
    pub fn new() -> Self {
        Nfa {
            graph: NfaGraph::new(),
            start: BTreeSet::new(),
            accepting: BTreeSet::new(),
        }
    }

    pub fn add_complete_sequence(&mut self, tokens: Vec<Edge>) {
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
    pub fn add_sequence_between(&mut self, a: NodeIndex, b: NodeIndex, tokens: Vec<Edge>) {
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
                graph: dfa1.graph.map(|_, _| (), |_, e| NfaEdge::Token(e.clone())),
                accepting: start_set,
                start: dfa1.accepting,
            }
        };
        to_dfa(&rev2)
    }
}

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
        let mut by_edge_weight = HashMap::<Edge, BTreeSet<NodeIndex>>::new();
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
    pub fn accepts(&self, tokens: &[Edge]) -> bool {
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
    let andy = Edge(1);
    let reuben = Edge(2);
    let peters = Edge(3);
    let comma = Edge(4);
    let twenty = Edge(5);

    let nfa = {
        let mut nfa = NfaGraph::new();
        let initial = nfa.add_node(());
        let forwards1 = nfa.add_node(());
        let backwards1 = nfa.add_node(());
        let backwards2 = nfa.add_node(());
        let target = nfa.add_node(());
        let abc = nfa.add_node(());
        let acc = nfa.add_node(());
        nfa.add_edge(initial, forwards1, reuben.into());
        nfa.add_edge(forwards1, target, peters.into());
        nfa.add_edge(initial, backwards1, peters.into());
        nfa.add_edge(backwards1, backwards2, comma.into());
        nfa.add_edge(backwards2, target, reuben.into());
        nfa.add_edge(initial, target, peters.into());
        nfa.add_edge(target, abc, comma.into());
        nfa.add_edge(abc, acc, twenty.into());
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
        nfa.add_edge(initial, forwards1, andy.into());
        nfa.add_edge(forwards1, target, peters.into());
        nfa.add_edge(initial, backwards1, peters.into());
        nfa.add_edge(backwards1, backwards2, comma.into());
        nfa.add_edge(backwards2, target, andy.into());
        nfa.add_edge(initial, target, peters.into());
        nfa.add_edge(target, abc, comma.into());
        nfa.add_edge(abc, acc, twenty.into());
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
        assert!(dfa.accepts(&[peters, comma, twenty]));
        assert!(dfa.accepts(&[reuben, peters, comma, twenty]));
        assert!(dfa.accepts(&[peters, comma, reuben, comma, twenty]));
        assert!(!dfa.accepts(&[peters, comma, andy, comma, twenty]));
        assert!(!dfa.accepts(&[andy, comma, peters, comma, twenty]));
    };

    let test_dfa2 = |dfa2: &Dfa| {
        assert!(dfa2.accepts(&[peters, comma, twenty]));
        assert!(dfa2.accepts(&[andy, peters, comma, twenty]));
        assert!(!dfa2.accepts(&[peters, comma, reuben, comma, twenty]));
        assert!(!dfa2.accepts(&[reuben, peters, comma, twenty]));
    };

    test_dfa(&dfa);
    test_dfa(&dfa_brz);
    test_dfa2(&dfa2);
    test_dfa2(&dfa2_brz);
}

#[test]
fn test_brzozowski_minimise() {
    let a = Edge(1);
    let b = Edge(2);
    let c = Edge(3);
    let d = Edge(4);
    let e = Edge(5);
    let nfa = {
        let mut nfa = Nfa::new();
        nfa.add_complete_sequence(vec![a, b, c, e]);
        nfa.add_complete_sequence(vec![a, b, e]);
        nfa.add_complete_sequence(vec![b, c, d, e]);
        nfa.add_complete_sequence(vec![b, d, e]);
        nfa
    };

    let dfa = nfa.brzozowski_minimise();
    println!("abcde {:?}", Dot::with_config(&dfa.graph, &[]));

    assert!(dfa.accepts(&[a, b, e]));
    assert!(!dfa.accepts(&[a, b, c, d, e]));
}
