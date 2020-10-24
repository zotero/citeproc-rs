// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2020 Corporation for Digital Scholarship

//! The cs:info element contains the style’s metadata. Its structure is based on the Atom
//! Syndication Format. In independent styles, cs:info has the following child elements:
//! - cs:id (mandatory) - a URI (but in practice only a URL)
//! - cs:title (mandatory) [xml:lang]
//! - cs:title-short
//! - cs:updated (mandatory)
//! - cs:category
//! - cs:author and cs:contributor
//!     > Within these elements, the child element cs:name must appear once, while cs:email and
//!     cs:uri each may appear once. These child elements should contain respectively the name,
//!     email address and URI of the author or contributor.
//! - cs:issn, cs:eissn, cs:issnl
//! - cs:link rel="self | template | documentation" href="URI" [xml:lang]
//! - cs:rights [xml:lang]
//! - cs:summary
//!
//! In dependent styles:
//! - cs:link must be used with rel set to 'independent-parent' and href pointing to the URI (id) of the
//! independent parent style.
//! - cs:link should not be used with rel set to template

use crate::terms::Category;
use crate::Lang;
use chrono::{DateTime, FixedOffset};
#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::marker::PhantomData;
use url::Url;

/// The spec says URI in a great many places, but suggests that these be actual URLs. We attempt to parse them as URLs so we can emit warnings when they're not.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Uri {
    Url(Url),
    Identifier(String),
}

impl Uri {
    fn parse(s: &str) -> Self {
        if let Ok(url) = Url::parse(s) {
            Uri::Url(url)
        } else {
            Uri::Identifier(s.to_owned())
        }
    }
}

impl<'a> From<&'a str> for Uri {
    fn from(s: &'a str) -> Self {
        Self::parse(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct LocalizedString {
    pub content: String,
    pub lang: Option<Lang>,
}

struct StringTag<H: LSVariant>(String, PhantomData<H>);

impl<H: LSVariant> From<StringTag<H>> for String {
    fn from(other: StringTag<H>) -> Self {
        other.0
    }
}

struct LSHelper<H: LSVariant> {
    string: LocalizedString,
    _marker: PhantomData<H>,
}

trait LSVariant {
    const TAG: &'static str;
    const HINT: Option<&'static str>;
}

impl<H: LSVariant> From<LSHelper<H>> for LocalizedString {
    fn from(other: LSHelper<H>) -> Self {
        other.string
    }
}

macro_rules! mk_hint {
    ($t:ident, $tag:literal, $h:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        struct $t;
        impl LSVariant for $t {
            const TAG: &'static str = $tag;
            const HINT: Option<&'static str> = $h;
        }
    };
}
mk_hint!(
    TitleHint,
    "title",
    Some("enter a full title for this style, like \"My Example Citation Style, 3rd Edition\"")
);
mk_hint!(
    TitleShortHint,
    "title-short",
    Some("enter a short title for this style, like \"MECS 3\"")
);
mk_hint!(
    SummaryHint,
    "summary",
    Some("give a short description of this style")
);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Rights {
    pub content: String,
    pub lang: Option<Lang>,
    pub license: Option<Uri>,
}

impl FromNode for Rights {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(Rights {
            content: node.text().unwrap_or("").to_owned(),
            license: attribute_option(node, "license", info)?,
            lang: attribute_option(node, LANG_ATTR, info)?,
        })
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("rights")
    }
    const CHILD_DESC: &'static str = "rights";
}

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab-case")]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum CitationFormat {
    AuthorDate,
    Author,
    Numeric,
    Label,
    Note,
}

#[derive(AsRefStr, EnumString, EnumProperty, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab-case")]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum Rel {
    #[strum(serialize = "self")]
    RelSelf,
    Documentation,
    /// Not allowed in dependent styles
    Template,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Link {
    pub href: Uri,
    pub rel: Rel,
    pub lang: Option<Lang>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ParentLink {
    pub href: Uri,
    pub lang: Option<Lang>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Info {
    /// Mandatory
    pub id: Uri,
    /// Mandatory
    pub updated: DateTime<FixedOffset>,
    /// Mandatory
    pub title: LocalizedString,

    pub title_short: Option<LocalizedString>,
    pub rights: Option<Rights>,
    pub summary: Option<LocalizedString>,
    pub parent: Option<ParentLink>,
    pub links: Vec<Link>,

    /// `<category citation-format="...">`
    pub citation_format: Option<CitationFormat>,
    /// `<category field="...">`*
    pub categories: Vec<Category>,

    pub issn: Option<String>,
    pub eissn: Option<String>,
    pub issnl: Option<String>,
}

use crate::attr::*;
use crate::locale::LANG_ATTR;
use crate::version::Features;
use crate::{
    exactly_one_child, many_children, max_one_child, FromNode, FromNodeResult, ParseInfo,
    UnknownAttributeValue,
};
use roxmltree::Node;

#[cfg(test)]
use roxmltree::Document;

#[cfg(test)]
fn parse_as<T>(s: &str) -> FromNodeResult<T>
where
    T: FromNode,
{
    let doc = Document::parse(s).unwrap();
    T::from_node(&doc.root_element(), &ParseInfo::default())
}

impl GetAttribute for Uri {
    fn get_attr(s: &str, _: &Features) -> Result<Self, UnknownAttributeValue> {
        Ok(Self::parse(s))
    }
}

#[test]
fn test_link() {
    assert_eq!(
        parse_as(r#"<link rel="documentation" href="https://example.com" />"#),
        Ok(Link {
            rel: Rel::Documentation,
            href: Uri::Url("https://example.com".parse().unwrap()),
            lang: None
        }),
    );
    assert_eq!(
        parse_as(r#"<link rel="documentation" href="https://example.com" />"#),
        Ok(Link {
            rel: Rel::Documentation,
            href: Uri::Url("https://example.com".parse().unwrap()),
            lang: None
        }),
    );
}

impl<H: LSVariant> FromNode for LSHelper<H> {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let content = node
            .text()
            .filter(|x| !x.is_empty())
            .ok_or_else(|| InvalidCsl::no_content(node, "text", H::HINT))?;
        Ok(LSHelper {
            string: LocalizedString {
                content: content.to_owned(),
                lang: attribute_option(node, LANG_ATTR, info)?,
            },
            _marker: PhantomData,
        })
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name(H::TAG)
    }
    const CHILD_DESC: &'static str = H::TAG;
}

impl<H: LSVariant> FromNode for StringTag<H> {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        let content = node
            .text()
            .filter(|x| !x.is_empty())
            .ok_or_else(|| InvalidCsl::no_content(node, "text", H::HINT))?;
        Ok(StringTag(content.to_owned(), PhantomData))
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name(H::TAG)
    }
    const CHILD_DESC: &'static str = H::TAG;
}

use crate::error::{CslError, InvalidCsl};

const DATETIME_HINT: &str = "e.g. \"2019-11-26T19:32:52Z\"";

struct UpdatedNode(DateTime<FixedOffset>);
impl FromNode for UpdatedNode {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        let txt = node.text().filter(|x| !x.is_empty()).ok_or_else(|| {
            InvalidCsl::no_content(
                node,
                "a DateTime representing when the style was last updated",
                Some(DATETIME_HINT),
            )
        })?;
        let dt = DateTime::parse_from_rfc3339(txt).map_err(|e| {
            InvalidCsl::new(
                node,
                format!(
                    "Could not parse DateTime, expected {} ({})",
                    DATETIME_HINT, e
                ),
            )
        })?;
        Ok(UpdatedNode(dt))
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("updated")
    }
    const CHILD_DESC: &'static str = "updated";
}

struct IdNode(Uri);
impl FromNode for IdNode {
    fn from_node(node: &Node, _info: &ParseInfo) -> FromNodeResult<Self> {
        let txt = node.text().filter(|x| !x.is_empty()).ok_or_else(|| {
            InvalidCsl::no_content(
                node,
                "a URI (preferably a URL) uniquely identifying the style",
                None,
            )
        })?;
        Ok(IdNode(Uri::parse(txt)))
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("id")
    }
    const CHILD_DESC: &'static str = "id";
}

impl<'a> From<&'a str> for LocalizedString {
    fn from(other: &'a str) -> Self {
        LocalizedString {
            content: other.into(),
            lang: None,
        }
    }
}

impl FromNode for CitationFormat {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(attribute_required(node, "citation-format", info)?)
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("category") && node.has_attribute("citation-format")
    }
    const CHILD_DESC: &'static str = "category citation-format=\"...\"";
}

impl FromNode for Category {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(attribute_required(node, "field", info)?)
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("category") && node.has_attribute("field")
    }
    const CHILD_DESC: &'static str = "category field=\"...\"";
}

impl FromNode for Link {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(Link {
            rel: attribute_required(node, "rel", info)?,
            href: attribute_required(node, "href", info)?,
            lang: attribute_option(node, LANG_ATTR, info)?,
        })
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("link") && node.attribute("rel") != Some("independent-parent")
    }
    const CHILD_DESC: &'static str = "link";
}

impl FromNode for ParentLink {
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        Ok(ParentLink {
            href: attribute_required(node, "href", info)?,
            lang: attribute_option(node, LANG_ATTR, info)?,
        })
    }
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("link") && node.attribute("rel") == Some("independent-parent")
    }
    const CHILD_DESC: &'static str = "link rel=\"independent-parent\"";
}

impl FromNode for Info {
    fn select_child(node: &Node) -> bool {
        node.has_tag_name("info")
    }
    const CHILD_DESC: &'static str = "info";
    fn from_node(node: &Node, info: &ParseInfo) -> FromNodeResult<Self> {
        let mut errors: Vec<InvalidCsl> = Vec::new();
        let id = exactly_one_child::<IdNode>(node, info, &mut errors);
        let updated = exactly_one_child::<UpdatedNode>(node, info, &mut errors);
        let title = exactly_one_child::<LSHelper<TitleHint>>(node, info, &mut errors);
        let title_short = max_one_child::<LSHelper<TitleShortHint>>(node, info, &mut errors);
        let summary = max_one_child::<LSHelper<SummaryHint>>(node, info, &mut errors);
        let rights = max_one_child(node, info, &mut errors);
        let citation_format = max_one_child::<CitationFormat>(node, info, &mut errors);
        let categories = many_children::<Category>(node, info, &mut errors);
        let parent_link = max_one_child::<ParentLink>(node, info, &mut errors);
        let links = many_children::<Link>(node, info, &mut errors);
        if parent_link.as_ref().map_or(false, |x| x.is_some()) {
            if let Ok(links) = links.as_ref() {
                for link in links {
                    if link.rel == Rel::Template {
                        errors.push(InvalidCsl::new(
                            node,
                            "link rel=\"template\" not permitted in a dependent style",
                        ))
                    }
                }
            }
        }
        mk_hint!(IssnHint, "issn", None);
        mk_hint!(EIssnHint, "eissn", None);
        mk_hint!(IssnLHint, "issnl", None);
        let issn = max_one_child::<StringTag<IssnHint>>(node, info, &mut errors);
        let eissn = max_one_child::<StringTag<EIssnHint>>(node, info, &mut errors);
        let issnl = max_one_child::<StringTag<IssnLHint>>(node, info, &mut errors);
        if !errors.is_empty() {
            return Err(CslError(errors));
        }
        Ok(Info {
            id: id?.0,
            updated: updated?.0,
            title: title?.into(),
            title_short: title_short?.map(Into::into),
            summary: summary?.map(Into::into),
            rights: rights?,
            citation_format: citation_format?,
            categories: categories?,
            parent: parent_link?,
            links: links?,
            issn: issn?.map(|x| x.0),
            eissn: eissn?.map(|x| x.0),
            issnl: issnl?.map(|x| x.0),
        })
    }
}

impl Default for Info {
    fn default() -> Self {
        Info {
            id: Uri::parse("default"),
            updated: DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").unwrap(),
            title: "".into(),
            title_short: None,
            summary: None,
            rights: None,
            parent: None,
            links: Vec::new(),
            citation_format: None,
            categories: Vec::new(),
            issn: None,
            eissn: None,
            issnl: None,
        }
    }
}

#[cfg(test)]
use crate::error::Severity;

#[test]
fn test_info() {
    assert_eq!(
        parse_as(
            r#"<info>
                 <id>https://example.com/mystyle</id>
                 <updated>2020-01-01T00:00:00Z</updated>
                 <title>My Style</title>
                 </info>"#
        ),
        Ok(Info {
            id: "https://example.com/mystyle".into(),
            updated: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap(),
            title: "My Style".into(),
            ..Default::default()
        }),
    );
    assert_eq!(
        parse_as(
            r#"<info>
                 <id>https://example.com/kitchen-sink</id>
                 <updated>2020-01-01T00:00:00Z</updated>
                 <title xml:lang="en-AU">My Style</title>
                 <title-short xml:lang="en-AU">MS</title-short>
                 <summary xml:lang="en-AU">Sum</summary>
                 <rights license="license-uri" xml:lang="en-AU">Rights to use</rights>
                 <link rel="self" href="https://example.com/self" xml:lang="en-AU" />
                 <link rel="documentation" href="https://example.com/documentation" xml:lang="en-AU" />
                 <link rel="template" href="https://example.com/template" xml:lang="en-AU" />
                 <!-- link rel = independent-parent -->
                 <category citation-format="author-date"/>
                 <category field="medicine"/>
                 <issn>issn</issn>
                 <eissn>eissn</eissn>
                 <issnl>issnl</issnl>
                 </info>"#
        ),
        Ok(Info {
            id: "https://example.com/kitchen-sink".into(),
            updated: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap(),
            title: LocalizedString {
                content: "My Style".into(),
                lang: Some(Lang::en_au())
            },
            title_short: Some(LocalizedString {
                content: "MS".into(),
                lang: Some(Lang::en_au())
            }),
            summary: Some(LocalizedString {
                content: "Sum".into(),
                lang: Some(Lang::en_au())
            }),
            citation_format: Some(CitationFormat::AuthorDate),
            categories: vec![Category::Medicine],
            rights: Some(Rights {
                content: "Rights to use".into(),
                lang: Some(Lang::en_au()),
                license: Some(Uri::parse("license-uri")),
            }),
            parent: None,
            links: vec![
                Link {
                    rel: Rel::RelSelf,
                    href: Uri::parse("https://example.com/self"),
                    lang: Some(Lang::en_au()),
                },
                Link {
                    rel: Rel::Documentation,
                    href: Uri::parse("https://example.com/documentation"),
                    lang: Some(Lang::en_au()),
                },
                Link {
                    rel: Rel::Template,
                    href: Uri::parse("https://example.com/template"),
                    lang: Some(Lang::en_au()),
                },
            ],
            issn: Some("issn".into()),
            eissn: Some("eissn".into()),
            issnl: Some("issnl".into()),
        }),
    );
}

#[test]
fn test_dependent() {
    assert_eq!(
        parse_as(
            r#"<info>
               <id>https://example.com/mystyle</id>
               <updated>2020-01-01T00:00:00Z</updated>
               <title>My Style</title>
               <link rel="independent-parent" href="parent-uri" />
               </info>"#
        ),
        Ok(Info {
            id: "https://example.com/mystyle".into(),
            updated: DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap(),
            title: "My Style".into(),
            parent: Some(ParentLink {
                href: Uri::parse("parent-uri"),
                lang: None
            }),
            ..Default::default()
        }),
    );
    assert_eq!(
        parse_as::<Info>(
            r#"<info>
               <id>https://example.com/mystyle</id>
               <updated>2020-01-01T00:00:00Z</updated>
               <title>My Style</title>
               <link rel="independent-parent" href="parent-uri" />
               <link rel="template" href="not-permitted" />
               </info>"#
        ),
        Err(CslError(vec![InvalidCsl {
            severity: Severity::Error,
            range: 0..302,
            message: "link rel=\"template\" not permitted in a dependent style".into(),
            hint: "".into()
        },])),
    );
}

#[test]
fn test_info_empty_is_error() {
    assert_eq!(
        parse_as::<Info>(r#"<info></info>"#),
        Err(CslError(vec![
            InvalidCsl {
                severity: Severity::Error,
                range: 0..13,
                message: "Must have exactly one <id>".into(),
                hint: "".into()
            },
            InvalidCsl {
                severity: Severity::Error,
                range: 0..13,
                message: "Must have exactly one <updated>".into(),
                hint: "".into()
            },
            InvalidCsl {
                severity: Severity::Error,
                range: 0..13,
                message: "Must have exactly one <title>".into(),
                hint: "".into()
            }
        ])),
    );
}
