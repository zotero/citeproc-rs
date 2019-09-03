// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::prelude::*;
use citeproc_io::Reference;
use csl::style::{Cond, CslType, Position};
use csl::variables::{AnyVariable, NumberVariable, Variable};
use csl::IsIndependent;
use fnv::FnvHashSet;

use super::ConditionStack;

#[cfg(test)]
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
