use pandoc_types::{
    definition::{Block, Inline, Pandoc as PandocDocument},
    walk::MutVisitor,
};

use citeproc::db::ReferenceDatabase;
use citeproc::input::{Cite, Cluster, Suppression};
use citeproc::output::Pandoc;

#[derive(Default)]
struct GetClusters {
    next_cluster_id: u64,
    next_cite_id: u64,
    clusters: Vec<Cluster<Pandoc>>,
}

pub fn get_clusters(pandoc: &mut PandocDocument) -> Vec<Cluster<Pandoc>> {
    let mut gc = GetClusters::default();
    gc.walk_pandoc(pandoc);
    gc.clusters
}

impl MutVisitor for GetClusters {
    fn walk_inline(&mut self, inline: &mut Inline) {
        match *inline {
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
    db: &'a DB,
}

pub fn write_clusters(pandoc: &mut PandocDocument, db: &impl ReferenceDatabase) {
    let mut wc = WriteClusters {
        next_cluster_id: 0,
        db,
    };
    wc.walk_pandoc(pandoc);
}

impl<'a, DB: ReferenceDatabase> MutVisitor for WriteClusters<'a, DB> {
    fn walk_inline(&mut self, inline: &mut Inline) {
        match *inline {
            Inline::Cite(_, ref mut literal) => {
                *literal = vec![Inline::Note(vec![Block::Para(
                    (*self.db.built_cluster(self.next_cluster_id)).clone(),
                )])];
                self.next_cluster_id += 1;
            }
            _ => {}
        }
    }
}
