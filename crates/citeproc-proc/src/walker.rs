// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::prelude::*;
use citeproc_db::LocaleFetcher;
use csl::locale::Lang;
use csl::style::{Name as NameEl, *};
use csl::terms::*;
use csl::variables::*;
use csl::Atom;
use std::sync::Arc;

use crate::choose::{CondChecker, UselessCondChecker};

pub enum WalkerFoldType {
    Group,
    Layout,
    IfThen,
    Else,
}

pub trait StyleWalker {
    type Output: Default;
    type Checker: CondChecker;
    fn fold(&mut self, elements: &[Element], fold_type: WalkerFoldType) -> Self::Output {
        for el in elements {
            let _ = self.element(el);
        }
        Self::Output::default()
    }
    /// Default returns None, but if you implement & return Some, you get condition checking for
    /// free
    fn get_checker(&self) -> Option<&Self::Checker> {
        None
    }
    /// Default impl tries to use `self.get_checker()`, otherwise false.
    fn should_take_branch(&mut self, conditions: &Conditions) -> bool {
        if let Some(ck) = self.get_checker() {
            let (eval_true, _is_disambiguate) = crate::choose::eval_conditions(conditions, ck);
            eval_true
        } else {
            false
        }
    }
    /// Default impl only walks branches for which should_take_branch is true
    fn ifthen(&mut self, ifthen: &IfThen) -> Option<Self::Output> {
        if self.should_take_branch(&ifthen.0) {
            Some(self.fold(&ifthen.1, WalkerFoldType::IfThen))
        } else {
            None
        }
    }
    /// Default impl only walks branches for which ifthen returns Some
    fn choose(&mut self, choose: &Choose) -> Self::Output {
        use std::iter;
        let Choose(head, rest, last) = choose;
        let iter = std::iter::once(head).chain(rest.iter());
        for branch in iter {
            if let Some(out) = self.ifthen(branch) {
                return out;
            }
        }
        self.fold(&last.0, WalkerFoldType::Else)
    }
    fn text_all(&mut self, text: &TextElement) -> Self::Output {
        match text.source {
            TextSource::Variable(svar, form) => self.text_variable(text, svar, form),
            TextSource::Value(ref atom) => self.text_value(text, atom),
            TextSource::Term(sel, plural) => self.text_term(text, sel, plural),
            TextSource::Macro(ref name) => self.text_macro(text, name),
        }
    }
    fn text_variable(
        &mut self,
        text: &TextElement,
        svar: StandardVariable,
        form: VariableForm,
    ) -> Self::Output {
        Self::Output::default()
    }
    fn text_value(&mut self, text: &TextElement, value: &Atom) -> Self::Output {
        Self::Output::default()
    }
    fn text_macro(&mut self, source: &TextElement, name: &Atom) -> Self::Output {
        Self::Output::default()
    }
    fn text_term(
        &mut self,
        source: &TextElement,
        sel: TextTermSelector,
        plural: bool,
    ) -> Self::Output {
        Self::Output::default()
    }
    fn text_label(&mut self, source: &TextElement) -> Self::Output {
        Self::Output::default()
    }
    fn label(&mut self, label: &LabelElement) -> Self::Output {
        Self::Output::default()
    }
    fn number(&mut self, source: &NumberElement) -> Self::Output {
        Self::Output::default()
    }
    fn names(&mut self, name: &Names) -> Self::Output {
        Self::Output::default()
    }
    fn date(&mut self, date: &BodyDate) -> Self::Output {
        Self::Output::default()
    }
    fn group(&mut self, group: &Group) -> Self::Output {
        self.fold(&group.elements, WalkerFoldType::Group)
    }
    fn layout(&mut self, layout: &Layout) -> Self::Output {
        self.fold(&layout.elements, WalkerFoldType::Layout)
    }
    fn walk_citation(&mut self, style: &Style) -> Self::Output {
        self.layout(&style.citation.layout)
    }
    fn walk_bibliography(&mut self, style: &Style) -> Option<Self::Output> {
        style
            .bibliography
            .as_ref()
            .map(|bib| self.layout(&bib.layout))
    }
    fn element(&mut self, element: &Element) -> Self::Output {
        match element {
            Element::Text(t) => self.text_all(t),
            Element::Group(g) => self.group(g),
            Element::Label(l) => self.label(l),
            Element::Number(n) => self.number(n),
            Element::Names(n) => self.names(n),
            Element::Date(bd) => self.date(&*bd),
            Element::Choose(c) => self.choose(&*c),
        }
    }
}
