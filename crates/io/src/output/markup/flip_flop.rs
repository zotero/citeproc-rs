// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use crate::output::markup::InlineElement;
use crate::output::micro_html::MicroNode;
use crate::output::FormatCmd;
use csl::{FontStyle, FontVariant, FontWeight, Formatting, TextDecoration, VerticalAlignment};

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct FlipFlopState {
    in_inner_quotes: bool,
    font_style: FontStyle,
    text_decoration: TextDecoration,
    font_weight: FontWeight,
    font_variant: FontVariant,
    vertical_alignment: VerticalAlignment,
}

impl FlipFlopState {
    pub fn from_formatting(f: Formatting) -> Self {
        FlipFlopState {
            in_inner_quotes: false,
            font_weight: f.font_weight.unwrap_or_default(),
            font_style: f.font_style.unwrap_or_default(),
            font_variant: f.font_variant.unwrap_or_default(),
            text_decoration: f.text_decoration.unwrap_or_default(),
            vertical_alignment: f.vertical_alignment.unwrap_or_default(),
        }
    }
    pub fn flip_flop_inlines(&self, inlines: &[InlineElement]) -> Vec<InlineElement> {
        let mut new = Vec::with_capacity(inlines.len());
        inlines.iter().for_each(|inl| match flip_flop(inl, self) {
            Ok(x) => new.push(x),
            Err(vec) => new.extend(vec.into_iter()),
        });
        new
    }
    /// Retval is whether any change resulted
    pub fn push_cmd(&mut self, cmd: FormatCmd) -> bool {
        let old = self.clone();
        match cmd {
            FormatCmd::FontStyleItalic => self.font_style = FontStyle::Italic,
            FormatCmd::FontStyleOblique => self.font_style = FontStyle::Oblique,
            FormatCmd::FontStyleNormal => self.font_style = FontStyle::Normal,
            FormatCmd::FontWeightBold => self.font_weight = FontWeight::Bold,
            FormatCmd::FontWeightNormal => self.font_weight = FontWeight::Normal,
            FormatCmd::FontWeightLight => self.font_weight = FontWeight::Light,
            FormatCmd::FontVariantSmallCaps => self.font_variant = FontVariant::SmallCaps,
            FormatCmd::FontVariantNormal => self.font_variant = FontVariant::Normal,
            FormatCmd::TextDecorationUnderline => self.text_decoration = TextDecoration::Underline,
            FormatCmd::TextDecorationNone => self.text_decoration = TextDecoration::None,
            FormatCmd::VerticalAlignmentSuperscript => {
                self.vertical_alignment = VerticalAlignment::Superscript
            }
            FormatCmd::VerticalAlignmentSubscript => {
                self.vertical_alignment = VerticalAlignment::Subscript
            }
            FormatCmd::VerticalAlignmentBaseline => {
                self.vertical_alignment = VerticalAlignment::Baseline
            }
            _ => return false,
            // FormatCmd::DisplayBlock,
            // FormatCmd::DisplayIndent,
            // FormatCmd::DisplayLeftMargin,
            // FormatCmd::DisplayRightInline,
        }
        *self != old
    }
}

fn flip_flop(
    inline: &InlineElement,
    state: &FlipFlopState,
) -> Result<InlineElement, Vec<InlineElement>> {
    match *inline {
        InlineElement::Micro(ref nodes) => {
            let nodes = flip_flop_nodes(nodes, state);
            Ok(InlineElement::Micro(nodes))
        }
        InlineElement::Formatted(ref ils, f) => {
            let mut flop = state.clone();
            let mut new_f = f;
            if let Some(fs) = f.font_style {
                if fs == state.font_style {
                    new_f.font_style = None;
                }
                flop.font_style = fs;
            }
            if let Some(fw) = f.font_weight {
                if fw == state.font_weight {
                    new_f.font_weight = None;
                }
                flop.font_weight = fw;
            }
            if let Some(fv) = f.font_variant {
                if fv == state.font_variant {
                    new_f.font_variant = None;
                }
                flop.font_variant = fv;
            }
            let nodes = flop.flip_flop_inlines(ils);
            Ok(InlineElement::Formatted(nodes, new_f))
        }

        InlineElement::Quoted {
            is_inner: _,
            ref localized,
            ref inlines,
        } => {
            let mut flop = state.clone();
            flop.in_inner_quotes = !flop.in_inner_quotes;
            let nodes = flop.flip_flop_inlines(inlines);
            Ok(InlineElement::Quoted {
                is_inner: flop.in_inner_quotes,
                localized: localized.clone(),
                inlines: nodes,
            })
        }

        InlineElement::Div(dm, ref inlines) => {
            let nodes = state.flip_flop_inlines(inlines);
            Ok(InlineElement::Div(dm, nodes))
        }

        InlineElement::Text(ref string) if string.is_empty() => Err(vec![]),

        _ => Ok(inline.clone()),
    }

    // a => a
}
fn flip_flop_nodes(nodes: &[MicroNode], state: &FlipFlopState) -> Vec<MicroNode> {
    let mut new = Vec::with_capacity(nodes.len());
    nodes
        .iter()
        .for_each(|node| match flip_flop_node(node, state) {
            Ok(x) => new.push(x),
            Err(vec) => new.extend(vec.into_iter()),
        });
    new
}

fn flip_flop_node(node: &MicroNode, state: &FlipFlopState) -> Result<MicroNode, Vec<MicroNode>> {
    match node {
        MicroNode::Quoted {
            ref children,
            ref localized,
            ..
        } => {
            let mut flop = state.clone();
            flop.in_inner_quotes = !state.in_inner_quotes;
            let nodes = flip_flop_nodes(children, &flop);
            Ok(MicroNode::Quoted {
                is_inner: flop.in_inner_quotes,
                localized: localized.clone(),
                children: nodes,
            })
        }
        MicroNode::Formatted(ref nodes, cmd) => {
            let mut flop = state.clone();
            match cmd {
                FormatCmd::FontStyleItalic
                | FormatCmd::FontStyleNormal
                | FormatCmd::FontStyleOblique => {
                    let is_italic = |x| x != FontStyle::Normal;
                    let outer = state.font_style;
                    flop.push_cmd(*cmd);
                    let inner = flop.font_style;
                    if is_italic(outer) && is_italic(inner) {
                        flop.font_style = FontStyle::Normal;
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Ok(MicroNode::Formatted(nodes, FormatCmd::FontStyleNormal))
                    } else if outer == inner {
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Err(nodes)
                    } else {
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Ok(MicroNode::Formatted(nodes, *cmd))
                    }
                }
                FormatCmd::FontWeightBold
                | FormatCmd::FontWeightLight
                | FormatCmd::FontWeightNormal => {
                    let outer = state.font_weight;
                    flop.push_cmd(*cmd);
                    let inner = flop.font_weight;
                    if outer == inner && inner != FontWeight::Normal {
                        flop.font_style = FontStyle::Normal;
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Ok(MicroNode::Formatted(nodes, FormatCmd::FontWeightNormal))
                    } else if outer == inner {
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Err(nodes)
                    } else {
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Ok(MicroNode::Formatted(nodes, *cmd))
                    }
                }
                FormatCmd::FontVariantSmallCaps | FormatCmd::FontVariantNormal => {
                    let outer = state.font_variant;
                    flop.push_cmd(*cmd);
                    let inner = flop.font_variant;
                    if outer == inner && inner != FontVariant::Normal {
                        flop.font_variant = FontVariant::Normal;
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Ok(MicroNode::Formatted(nodes, FormatCmd::FontVariantNormal))
                    } else if outer == inner {
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Err(nodes)
                    } else {
                        let nodes = flip_flop_nodes(nodes, &flop);
                        Ok(MicroNode::Formatted(nodes, *cmd))
                    }
                }
                // i.e. sup and sub
                _ => {
                    let nodes = flip_flop_nodes(nodes, state);
                    Ok(MicroNode::Formatted(nodes, *cmd))
                }
            }
        }
        MicroNode::Text(_) => Ok(node.clone()),
        MicroNode::NoCase(ref nodes) => {
            let nodes = flip_flop_nodes(nodes, state);
            Ok(MicroNode::NoCase(nodes))
        }
        MicroNode::NoDecor(ref nodes) => {
            let mut flop = state.clone();
            flop.font_style = FontStyle::Normal;
            let fs = state.font_style == flop.font_style;
            flop.font_weight = FontWeight::Normal;
            let fw = state.font_weight == flop.font_weight;
            flop.font_variant = FontVariant::Normal;
            let fv = state.font_variant == flop.font_variant;
            flop.text_decoration = TextDecoration::None;
            let td = state.text_decoration == flop.text_decoration;
            let nodes = flip_flop_nodes(nodes, &flop);
            if fs && fw && fv && td {
                Err(nodes)
            } else {
                let mut out = nodes;
                if !fs {
                    out = vec![MicroNode::Formatted(out, FormatCmd::FontStyleNormal)]
                }
                if !fw {
                    out = vec![MicroNode::Formatted(out, FormatCmd::FontWeightNormal)]
                }
                if !fv {
                    out = vec![MicroNode::Formatted(out, FormatCmd::FontVariantNormal)]
                }
                if !td {
                    out = vec![MicroNode::Formatted(out, FormatCmd::TextDecorationNone)]
                }
                Err(out)
            }
        }
    }
}
