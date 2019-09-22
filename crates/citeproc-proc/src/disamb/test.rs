// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::free::{FreeCond, FreeCondSets};
use crate::prelude::*;
use crate::test::MockProcessor;
use citeproc_io::output::html::Html;
use citeproc_io::Reference;
use csl::style::Style;
use csl::style::{Cond, CslType, Position};
use csl::variables::{AnyVariable, NumberVariable, Variable};
use csl::IsIndependent;
use fnv::FnvHashSet;

macro_rules! style_text_layout {
    ($ex:expr) => {{
        &format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
    <style class="in-text" version="1.0.1">
        <citation>
            <layout>
                {}
            </layout>
        </citation>
    </style>"#,
            $ex
        )
    }};
}

macro_rules! style_layout {
    ($ex:expr) => {{
        use std::str::FromStr;
        ::csl::style::Style::from_str(&format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
    <style class="in-text" version="1.0.1">
        <citation>
            <layout>
                {}
            </layout>
        </citation>
    </style>"#,
            $ex
        ))
        .unwrap()
    }};
}

macro_rules! style {
    ($ex:expr) => {{
        use std::str::FromStr;
        ::csl::style::Style::from_str($ex).unwrap()
    }};
}

#[test]
fn whole_apa() {
    let mut db = MockProcessor::new();
    use csl::style::Style;
    use std::fs;
    db.set_style_text(include_str!("../../tests/data/apa.csl"));
    // let style = Style::from_str(&).unwrap();
    let fcs = db.style().get_free_conds(&db);
    dbg!(&fcs);
}

#[test]
fn whole_agcl() {
    let mut db = MockProcessor::new();
    use std::fs;
    db.set_style_text(include_str!("../../tests/data/aglc.csl"));
    // let style = Style::from_str(&).unwrap();
    let fcs = db.style().get_free_conds(&db);
    dbg!(&fcs);
}

#[test]
fn test_locator_macro() {
    let mut db = MockProcessor::new();
    db.set_style_text(style_text_layout!(
        r#"<choose>
      <if locator="page">
        <text variable="locator"/>
      </if>
      <else-if variable="locator">
        <group delimiter=" " >
          <label variable="locator" form="short" />
          <text variable="locator" />
        </group>
      </else-if>
    </choose>"#
    ));
    let fcs = db.style().get_free_conds(&db);
    let mut correct = FreeCondSets::empty();
    correct.0.insert(FreeCond::LOCATOR | FreeCond::LT_PAGE);
    correct
        .0
        .insert(FreeCond::LOCATOR | FreeCond::LT_PAGE_FALSE);
    correct
        .0
        .insert(FreeCond::LOCATOR_FALSE | FreeCond::LT_PAGE_FALSE);
    assert_eq!(fcs, correct);
}

use crate::disamb::{create_dfa, create_ref_ir};
use std::sync::Arc;

#[test]
fn test() {
    let db = &mut MockProcessor::new();
    db.set_style_text(style_text_layout!(
        r#"<group delimiter=", ">
          <group delimiter=" ">
              <text variable="title" />
              <text variable="container-title" />
          </group>
          <group delimiter=" ">
          <text variable="locator" />
          <label variable="locator" form="short" />
          </group>
        </group>"#
    ));
    let mut refr = Reference::empty("ref_id".into(), CslType::Book);
    refr.ordinary.insert(Variable::Title, "The Title".into());
    // let's be intentionally deceptive
    let mut refr2 = Reference::empty("other_ref".into(), CslType::Book);
    refr2.ordinary.insert(Variable::Title, "The".into());
    refr2
        .ordinary
        .insert(Variable::ContainerTitle, "Title".into());

    let vec = create_ref_ir::<Html, MockProcessor>(db, &refr);
    for (fc, ir) in &vec {
        println!("{:?}:\n    {}", fc, ir.debug(db));
    }
    let dfa = create_dfa::<Html, MockProcessor>(db, &refr);
    println!("{}", dfa.debug_graph(db));

    let vec = create_ref_ir::<Html, MockProcessor>(db, &refr2);
    let dfa2 = create_dfa::<Html, MockProcessor>(db, &refr2);
    println!("{}", dfa2.debug_graph(db));

    use citeproc_io::{Cite, Cluster2, IntraNote};

    db.set_references(vec![refr, refr2]);
    let cluster = Cluster2::Note {
        id: 1,
        note: IntraNote::Single(1),
        cites: vec![Cite::basic("ref_id")],
    };
    db.init_clusters(vec![cluster]);
    let cite_ids = db.cluster_cites(1);

    let get_stream = |ind: usize| {
        let id = cite_ids[ind];
        let gen0 = db.ir_gen0(id);
        let (ir, bo, st) = &*gen0;
        let fmt = db.get_formatter();
        ir.to_edge_stream(&fmt)
    };

    let cite_edges = get_stream(0);
    dbg!(&cite_edges);
    assert!(dfa.accepts_data(db, &cite_edges));
    println!("dfa2?");
    assert!(dfa2.accepts_data(db, &cite_edges));
}

// #[test(ignore)]
// fn element_disamb() {
//     use crate::test::MockProcessor;
//     let db = MockProcessor::new();
//     let style = style_layout!(
//         r#"
//     <group delimiter=" ">
//         <text value="value" />
//         <text value="italic" font-style="italic"/>
//     </group>
//     "#
//     );
//     let group = &style.citation.layout.elements[0];
//     let mut state = DisambiguationState::new();
//     group.construct_nfa(&db, &mut state);
//     let dfa = state.nfa.brzozowski_minimise();
//     let value = db.edge(EdgeData("value".to_string()));
//     let italic = db.edge(EdgeData("<i>italic</i>".to_string()));
//     assert!(dfa.accepts(&[value, italic]));
// }
