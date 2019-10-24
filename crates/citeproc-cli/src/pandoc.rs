// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use pandoc_types::definition::CitationMode;
pub fn suppression_from_pandoc_mode(mode: CitationMode) -> Option<Suppression> {
    match mode {
        CitationMode::AuthorInText => Some(Suppression::InText),
        CitationMode::SuppressAuthor => Some(Suppression::Rest),
        CitationMode::NormalCitation => None,
    }
}

use pandoc_types::{
    definition::{Block, Citation, Inline, Pandoc as PandocDocument},
    walk::MutVisitor,
};

use citeproc::input::{Cite, CiteId, Cluster, ClusterId, Suppression};
use citeproc::output::Pandoc;
use citeproc::Processor;
use csl::StyleClass;

struct GetClusters {
    next_cluster_id: ClusterId,
    next_cite_id: CiteId,
    clusters: Vec<Cluster<Pandoc>>,
}

pub fn get_clusters(pandoc: &mut PandocDocument) -> Vec<Cluster<Pandoc>> {
    // pandoc-citeproc starts at 1
    let mut gc = GetClusters {
        next_cluster_id: 1,
        next_cite_id: 1,
        clusters: vec![],
    };
    gc.walk_pandoc(pandoc);
    gc.clusters
}

impl MutVisitor for GetClusters {
    fn walk_inline(&mut self, inline: &mut Inline) {
        match *inline {
            Inline::Note(_) => {
                // just trying to mirror the cite hashes of pandoc-citeproc
                self.next_cite_id += 1;
            }
            Inline::Cite(ref p_cites, ref _literal) => {
                let mut note_number = 0;
                let cites = p_cites
                    .iter()
                    .map(|p| {
                        note_number = p.citation_note_num;
                        let id = self.next_cite_id;
                        self.next_cite_id += 1;
                        Cite {
                            id,
                            ref_id: p.citation_id.clone().into(),
                            suppression: suppression_from_pandoc_mode(p.citation_mode.clone()),
                            prefix: p.citation_prefix.clone(),
                            suffix: p.citation_suffix.clone(),
                            // XXX: parse these out of the suffix, and drop the rest in "suffix"
                            locators: vec![],
                            locator_extra: None,
                            locator_date: None,
                        }
                    })
                    .collect();
                let cluster = Cluster {
                    // id == note_number for Pandoc, but not necessarily with user-supplied
                    // ids.
                    id: self.next_cluster_id,
                    note_number: self.next_cluster_id as u32,
                    cites,
                };
                self.clusters.push(cluster);
                self.next_cluster_id += 1;
            }
            _ => {}
        }
    }
}

struct WriteClusters<'a> {
    next_cluster_id: ClusterId,
    next_cite_id: CiteId,
    db: &'a Processor,
}

/// Only works if you run it on a PandocDocument that hasn't been modified since you ingested the
/// clusters into the database. The Inline::Cite-s & Inline::Note-s have to be in the same order.
/// If you're adding a bibliography, do it after a get_clusters/write_clusters pair.
pub fn write_clusters(pandoc: &mut PandocDocument, db: &Processor) {
    let mut wc = WriteClusters {
        next_cluster_id: 1,
        next_cite_id: 1,
        db,
    };
    wc.walk_pandoc(pandoc);
}

impl<'a> MutVisitor for WriteClusters<'a> {
    fn walk_inline(&mut self, inline: &mut Inline) {
        match *inline {
            Inline::Note(_) => {
                // just trying to mirror the cite hashes of pandoc-citeproc
                self.next_cite_id += 1;
            }
            Inline::Cite(ref mut p_cites, ref mut literal) => {
                let cites = p_cites
                    .iter()
                    .map(|p| {
                        let id = self.next_cite_id;
                        self.next_cite_id += 1;
                        let rust_cite = self.db.get_cite(id);
                        Citation {
                            citation_hash: id as i32,
                            citation_prefix: rust_cite.prefix.clone(),
                            citation_suffix: rust_cite.suffix.clone(),
                            ..p.clone()
                        }
                    })
                    .collect();
                *p_cites = cites;
                let built = (*self.db.get_cluster(self.next_cluster_id)).clone();
                if self.db.get_style().class == StyleClass::Note {
                    *literal = vec![Inline::Note(vec![Block::Para(built)])];
                } else {
                    *literal = built;
                }
                self.next_cluster_id += 1;
            }
            _ => {}
        }
    }
}

// TODO: parse locators!
