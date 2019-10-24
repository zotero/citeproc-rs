// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

// #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
// pub enum SupportedOutputFormat {
//     Pandoc,
//     Html,
//     Plain,
// }
//
// /// An OutputFormat that attempts to downcast before running an inner OutputFormat,
// /// and erases any type info that the inner one produces.
// #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
// pub enum AnyOutputFormat {
//     Pandoc(Pandoc),
//     Plain(PlainText),
// }
//
// impl AnyOutputFormat {
//     pub fn with_format(sof: SupportedOutputFormat) -> Self {
//         match sof {
//             SupportedOutputFormat::Pandoc => AnyOutputFormat::Pandoc(Pandoc::default()),
//             _ => unimplemented!("non-pandoc")
//         }
//     }
// }
//
// impl Default for AnyOutputFormat {
//     fn default() -> Self {
//         AnyOutputFormat::with_format(SupportedOutputFormat::Pandoc)
//     }
// }
//
// use pandoc_types::definition::Inline;
//
// #[derive(Serialize, Clone, Eq, PartialEq, Debug)]
// enum UnionFormat {
//     Pandoc(Vec<Inline>),
//     Plain(String),
//     Empty,
// }
//
// impl Default for UnionFormat {
//     fn default() -> Self { UnionFormat::Empty }
// }
//
// impl UnionFormat {
//     fn unwrap_pandoc(self) -> Vec<Inline> {
//         match self {
//             UnionFormat::Pandoc(x) => x,
//             _ => panic!("unwrapped non-Pandoc UnionFormat value to Pandoc"),
//         }
//     }
//     fn unwrap_pandoc(self) -> Vec<Inline> {
//         match self {
//             UnionFormat::Pandoc(x) => x,
//             _ => panic!("unwrapped non-Pandoc UnionFormat value to Pandoc"),
//         }
//     }
// }
//
// impl OutputFormat for AnyOutputFormat {
//     type Build = UnionFormat;
//     type Output = UnionFormat;
//
//     #[inline]
//     fn text_node(&self, s: String, formatting: Option<Formatting>) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.text_node(s, formatting),
//             _ => unimplemented!()
//         }
//     }
//
//     /// Group some text nodes. You might want to optimise for the case where delimiter is empty.
//     #[inline]
//     fn group(
//         &self,
//         nodes: Vec<Self::Build>,
//         delimiter: &str,
//         formatting: Option<Formatting>,
//     ) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => {
//                 UnionFormat::.group(nodes, delimiter, formatting)
//             }
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn seq(&self, nodes: impl Iterator<Item = Self::Build>) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.seq(nodes),
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn join_delim(&self, a: Self::Build, delim: &str, b: Self::Build) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.join_delim(a, delim, b),
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn output(&self, intermediate: Self::Build) -> Self::Output {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.output(intermediate),
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn plain(&self, s: &str) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.plain(s),
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn quoted(&self, b: Self::Build, quotes: &LocalizedQuotes) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.quoted(b, quotes),
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn with_format(&self, a: Self::Build, f: Option<Formatting>) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.with_format(a, f),
//             _ => unimplemented!()
//         }
//     }
//
//     #[inline]
//     fn hyperlinked(&self, a: Self::Build, target: Option<&str>) -> Self::Build {
//         match self {
//             AnyOutputFormat::Pandoc(p) => p.hyperlinked(a, target),
//             _ => unimplemented!()
//         }
//     }
//
// }
