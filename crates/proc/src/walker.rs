// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::choose::CondChecker;
use csl::*;

pub enum WalkerFoldType<'a> {
    Group(&'a Group),
    Layout(&'a Layout),
    IfThen,
    Else,
    Substitute,
    Macro(&'a TextElement),
}

pub trait StyleWalker {
    type Output;
    type Checker: CondChecker;
    fn default(&mut self) -> Self::Output;
    fn fold(&mut self, elements: &[Element], _fold_type: WalkerFoldType) -> Self::Output {
        for el in elements {
            let _ = self.element(el);
        }
        self.default()
    }
    /// Default returns None, but if you implement & return Some, you get condition checking for
    /// free
    fn get_checker(&self) -> Option<&Self::Checker> {
        None
    }
    /// Default impl tries to use `self.get_checker()`, otherwise false.
    fn should_take_branch(&mut self, conditions: &Conditions) -> bool {
        if let Some(ck) = self.get_checker() {
            let (eval_true, _is_disambiguate) =
                crate::choose::eval_conditions(conditions, ck, std::u32::MAX);
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
        _text: &TextElement,
        _svar: StandardVariable,
        _form: VariableForm,
    ) -> Self::Output {
        self.default()
    }
    fn text_value(&mut self, _text: &TextElement, _value: &SmartString) -> Self::Output {
        self.default()
    }
    fn text_macro(&mut self, _source: &TextElement, _name: &SmartString) -> Self::Output {
        self.default()
    }
    fn text_term(
        &mut self,
        _source: &TextElement,
        _sel: TextTermSelector,
        _plural: bool,
    ) -> Self::Output {
        self.default()
    }
    fn label(&mut self, _label: &LabelElement) -> Self::Output {
        self.default()
    }
    fn number(&mut self, _source: &NumberElement) -> Self::Output {
        self.default()
    }
    fn names(&mut self, _name: &Names) -> Self::Output {
        self.default()
    }
    fn date(&mut self, _date: &BodyDate) -> Self::Output {
        self.default()
    }
    fn group(&mut self, group: &Group) -> Self::Output {
        self.fold(&group.elements, WalkerFoldType::Group(group))
    }
    fn layout(&mut self, layout: &Layout) -> Self::Output {
        self.fold(&layout.elements, WalkerFoldType::Layout(layout))
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
    fn bibliography(&mut self, bib: &Bibliography) -> Self::Output {
        self.layout(&bib.layout)
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
