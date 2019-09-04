// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use super::free::{FreeCond, FreeCondSets};
use super::DisambiguationOld;
use crate::prelude::*;
use citeproc_io::Reference;
use csl::style::{Cond, CslType, Position};
use csl::variables::{AnyVariable, NumberVariable, Variable};
use csl::IsIndependent;
use fnv::FnvHashSet;

use super::ConditionStack;

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
fn independent_conds() {
    use crate::test::MockProcessor;
    let db = MockProcessor::new();
    let style = style_layout!(
        r#"
    <group delimiter=" ">
        <text variable="locator" />
        <text variable="title" font-style="italic"/>
        <choose>
            <if disambiguate="true">
                <text value="..." />
            </if>
            <else-if position="first" type="book">
                <text value="..." />
            </else-if>
            <else-if position="subsequent" variable="title">
                <text value="..." />
            </else-if>
            <else>
                <text variable="first-reference-note-number" />
                <text variable="ISBN" />
            </else>
        </choose>
    </group>
    "#
    );
    let mut refr = Reference::empty("id".into(), CslType::Book);
    refr.ordinary.insert(Variable::Title, "Title".into());
    let mut stack = ConditionStack::from(&refr);
    style.independent_conds(&db, &mut stack);
    let mut result_set = FnvHashSet::default();
    result_set.insert(Cond::Variable(AnyVariable::Number(NumberVariable::Locator)));
    result_set.insert(Cond::Position(Position::First));
    result_set.insert(Cond::Disambiguate(true));
    result_set.insert(Cond::Variable(AnyVariable::Number(
        NumberVariable::FirstReferenceNoteNumber,
    )));
    // does not contain Position::Subsequent as we knew variable="title" was true
    assert_eq!(
        stack.output.into_iter().collect::<FnvHashSet<_>>(),
        result_set,
    )
}

#[test]
fn whole_apa() {
    use crate::test::MockProcessor;
    let mut db = MockProcessor::new();
    use csl::style::Style;
    use std::fs;
    db.set_style_text(include_str!("../../tests/data/apa.csl"));
    // let style = Style::from_str(&).unwrap();
    let fcs = db.style().get_free_conds(&db);
    dbg!(&fcs);
}

use crate::test::MockProcessor;
use csl::style::Style;

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
    use super::Disambiguation;
    use crate::test::MockProcessor;
    let mut db = MockProcessor::new();
    use csl::style::Style;
    use std::fs;
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
