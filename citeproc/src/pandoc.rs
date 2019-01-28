use pandoc_types::{
    definition::{Block, Inline, Pandoc as PandocDocument, Citation},
    walk::MutVisitor,
};

use citeproc::db::ReferenceDatabase;
use citeproc::input::{Cite, Cluster, Suppression};
use citeproc::output::Pandoc;

struct GetClusters {
    next_cluster_id: u64,
    next_cite_id: u64,
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
                            suppression: Suppression::from_pandoc_mode(p.citation_mode.clone()),
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
                    id: self.next_cluster_id,
                    cites,
                    // XXX: note_number,
                };
                self.clusters.push(cluster);
                self.next_cluster_id += 1;
            }
            _ => {}
        }
    }
}

struct WriteClusters<'a, DB: ReferenceDatabase> {
    next_cluster_id: u64,
    next_cite_id: u64,
    db: &'a DB,
}

/// Only works if you run it on a PandocDocument that hasn't been modified since you ingested the
/// clusters into the database. The Inline::Cite-s & Inline::Note-s have to be in the same order.
/// If you're adding a bibliography, do it after a get_clusters/write_clusters pair.
pub fn write_clusters(pandoc: &mut PandocDocument, db: &impl ReferenceDatabase) {
    let mut wc = WriteClusters {
        next_cluster_id: 1,
        next_cite_id: 1,
        db,
    };
    wc.walk_pandoc(pandoc);
}

impl<'a, DB: ReferenceDatabase> MutVisitor for WriteClusters<'a, DB> {
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
                        let rust_cite = self.db.cite(id);
                        Citation {
                            citation_hash: id as i32,
                            citation_prefix: rust_cite.prefix.clone(),
                            citation_suffix: rust_cite.suffix.clone(),
                            ..p.clone()
                        }
                    })
                    .collect();
                *p_cites = cites;
                *literal = vec![Inline::Note(vec![Block::Para(
                    (*self.db.built_cluster(self.next_cluster_id)).clone(),
                )])];
                self.next_cluster_id += 1;
            }
            _ => {}
        }
    }
}
