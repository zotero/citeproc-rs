// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

use super::Info;
use crate::attr::*;
use crate::error::{CslError, InvalidCsl, StyleError};
use crate::from_node::*;
use crate::info::ParentLink;
use crate::{Bibliography, CslVersionReq, Features, Lang, Locale, StyleClass};
use roxmltree::{Document, Node};

/// A stripped-down version of `Style` that can also represent a dependent style.
/// Use this to determine whether a string of XML is a dependent or independent style, and to get
/// the parent link for a dependent style.
#[derive(Debug, Eq, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct StyleMeta {
    pub info: Info,
    pub features: Features,
    pub default_locale: Option<Lang>,
    /// May be absent on a dependent style.
    pub class: Option<StyleClass>,
    pub csl_version_required: CslVersionReq,
    pub independent_meta: Option<IndependentMeta>,
}

#[derive(Debug, Eq, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct IndependentMeta {
    /// A list of languages for which a locale override was specified.
    /// Does not include the language-less final override.
    locale_overrides: Vec<Lang>,
    has_bibliography: bool,
}

impl StyleMeta {
    pub fn is_dependent(&self) -> bool {
        self.info.parent.is_some()
    }
    pub fn independent_parent(&self) -> Option<&ParentLink> {
        self.info.parent.as_ref()
    }
    pub fn independent_parent_id(&self) -> Option<String> {
        self.info.parent.as_ref().map(|p| p.href.to_string())
    }
    /// Parses an XML string. It will successfully parse a valid dependent or independent style,
    /// but will not validate the `cs:citation` (etc) on an independent style.
    pub fn parse(xml: &str) -> Result<Self, StyleError> {
        let doc = Document::parse(xml)?;
        let node = &doc.root_element();
        let parse_info = ParseInfo::default();
        let meta = StyleMeta::from_node(node, &parse_info)?;
        Ok(meta)
    }
}

impl FromNode for StyleMeta {
    fn from_node(node: &Node, parse_info: &ParseInfo) -> FromNodeResult<Self> {
        let csl_version_required = CslVersionReq::from_node(node, parse_info)?;
        let mut errors: Vec<InvalidCsl> = Vec::new();

        let default_locale =
            append_invalid_err!(attribute_option(node, "default-locale", parse_info), errors);
        let class = append_invalid_err!(attribute_option(node, "class", parse_info), errors);
        let info = exactly_one_child::<Info>(node, &parse_info, &mut errors);
        let features = max_one_child::<Features>(node, &parse_info, &mut errors)
            .ok()
            .flatten()
            .unwrap_or_else(|| Features::new());

        if !errors.is_empty() {
            return Err(CslError(errors));
        }
        let info = info?;
        let class = class?;
        let default_locale = default_locale?;
        let independent_meta = if info.parent.is_none() {
            Some(IndependentMeta {
                has_bibliography: node.children().find(Bibliography::select_child).is_some(),
                locale_overrides: node
                    .children()
                    .filter(Locale::select_child)
                    .filter_map(|l| {
                        <Option<Lang> as FromNode>::from_node(&l, parse_info)
                            .ok()
                            .flatten()
                    })
                    .collect(),
            })
        } else {
            None
        };
        Ok(StyleMeta {
            info,
            features,
            class,
            default_locale,
            csl_version_required,
            independent_meta,
        })
    }
}

#[cfg(test)]
mod test {
    use super::StyleMeta;

    macro_rules! assert_meta_parse {
        ($xml:literal) => {
            ::insta::assert_debug_snapshot!(
                StyleMeta::parse(::indoc::indoc!($xml)).expect("should have parsed successfully")
            );
        };
    }

    #[test]
    fn dependent_template_fail() {
        assert_meta_parse!(
            r#"
            <style version="1.0.1" class="in-text">
                <info>
                    <id>https://example.com/mystyle</id>
                    <updated>2020-01-01T00:00:00Z</updated>
                    <title>My Style</title>
                    <link rel="independent-parent" href="parent-uri" />
                </info>
            </style>
        "#
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn info_serialize() {
        use crate::from_node::parse_as;

        insta::assert_json_snapshot!(parse_as::<StyleMeta>(indoc::indoc! {r#"
            <style version="1.0.1" class="in-text" default-locale="en-AU">
                <info>
                    <id>dependent-style</id>
                    <updated>2020-01-01T00:00:00Z</updated>
                    <title>A Dependent Style</title>
                    <link rel="independent-parent" href="http://zotero.org/styles/parent-id" />
                </info>
            </style>
        "#})
        .unwrap());

        insta::assert_json_snapshot!(parse_as::<StyleMeta>(indoc::indoc! {r#"
            <style version="1.0.1" class="in-text" default-locale="en-AU">
                <info>
                    <id>https://example.com/mystyle</id>
                    <updated>2020-01-01T00:00:00Z</updated>
                    <title>My CSL Style</title>
                </info>
            </style>
        "#})
        .unwrap());

        insta::assert_json_snapshot!(parse_as::<StyleMeta>(indoc::indoc! {r#"
            <style version="1.0.1" class="in-text" default-locale="en-AU">
                <info>
                    <id>https://example.com/mystyle</id>
                    <updated>2020-01-01T00:00:00Z</updated>
                    <title>My CSL Style</title>
                </info>
                <locale xml:lang="en-GB">
                </locale>
                <locale xml:lang="fr-FR">
                </locale>
                <citation><locale></locale></citation>
                <bibliography><locale></locale></bibliography>
            </style>
        "#})
        .unwrap());
    }
}
