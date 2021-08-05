// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::free::{FreeCond, FreeCondSets};
use super::get_free_conds;
use crate::prelude::*;
use crate::test::MockProcessor;
use citeproc_db::{ClusterNumber, IntraNote};
use citeproc_io::output::markup::Markup;
use citeproc_io::{Cite, Reference};

use csl::CslType;
use csl::Variable;

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

#[test]
fn whole_apa() {
    let mut db = MockProcessor::new();

    db.set_style_text(include_str!("../../tests/data/apa.csl"));
    // let style = Style::from_str(&).unwrap();
    let fcs = get_free_conds(&db);
    dbg!(&fcs);
}

#[test]
fn whole_agcl() {
    let mut db = MockProcessor::new();

    db.set_style_text(include_str!("../../tests/data/aglc.csl"));
    // let style = Style::from_str(&).unwrap();
    let fcs = get_free_conds(&db);
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
    let fcs = get_free_conds(&db);
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

    let vec = create_ref_ir::<Markup>(db, &refr);
    for (fc, ir) in &vec {
        println!("{:?}:\n    {}", fc, ir.debug(db));
    }
    let dfa = create_dfa::<Markup>(db, &refr);
    println!("{}", dfa.debug_graph(db));

    let _vec = create_ref_ir::<Markup>(db, &refr2);
    let dfa2 = create_dfa::<Markup>(db, &refr2);
    println!("{}", dfa2.debug_graph(db));

    db.insert_references(vec![refr, refr2]);
    use citeproc_db::ClusterId;
    let mut interner = string_interner::StringInterner::<ClusterId>::new();
    let id = interner.get_or_intern("1");
    db.init_clusters(vec![(
        id,
        ClusterNumber::Note(IntraNote::Single(1)),
        vec![Cite::basic("ref_id")],
    )]);
    let cite_ids = db.cluster_cites(id);

    let get_stream = |ind: usize| {
        let id = cite_ids[ind];
        let gen0 = db.ir_gen0(id);
        gen0.tree_ref().to_edge_stream(&db.get_formatter())
    };

    let cite_edges = get_stream(0);
    dbg!(&cite_edges);
    assert!(dfa.accepts_data(&cite_edges));
    println!("dfa2?");
    assert!(dfa2.accepts_data(&cite_edges));
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
//     let value = EdgeData("value".into());
//     let italic = EdgeData("<i>italic</i>".to_string());
//     assert!(dfa.accepts(&[value, italic]));
// }
