// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use std::collections::HashMap;
use std::sync::{Arc, Once};

use crate::prelude::*;
use csl::*;

macro_rules! assert_cluster {
    ($arcstring:expr, $optstr:expr) => {
        let built = $arcstring;
        assert_eq!(
            built.as_deref().map(|string_ref| string_ref.as_str()),
            $optstr
        );
    };
}

fn test_db(style: Option<&str>) -> Processor {
    static INIT_ONCE: Once = Once::new();
    INIT_ONCE.call_once(|| {
        env_logger::init();
    });
    Processor::new(InitOptions {
        style: style.unwrap_or(
            r#"<style version="1.0" class="in-text">
                                    <citation><layout></layout></citation>
                                  </style>"#,
        ),
        format: SupportedFormat::Plain,
        test_mode: true,
        ..Default::default()
    })
    .unwrap()
}

fn insert_basic_refs(db: &mut Processor, ref_ids: &[&str]) {
    for &id in ref_ids {
        let mut refr = Reference::empty(Atom::from(id), CslType::Book);
        let title = "Book ".to_string() + id;
        refr.ordinary.insert(Variable::Title, title);
        db.insert_reference(refr);
    }
}

fn cid(db: &mut Processor, n: u32) -> ClusterId {
    db.cluster_id(n.to_string())
}

fn insert_ascending_notes(db: &mut Processor, ref_ids: &[&str]) {
    let len = ref_ids.len();
    let mut clusters = Vec::with_capacity(len);
    let mut order = Vec::with_capacity(len);
    for i in 1..=len {
        let id = cid(db, i as u32);
        clusters.push(Cluster::new(id, vec![Cite::basic(ref_ids[i - 1])], None));
        order.push(ClusterPosition::note(id, i as u32));
    }
    db.init_clusters(clusters);
    db.set_cluster_order(&order).unwrap();
}

mod cluster_order {
    use super::*;

    #[test]
    fn set_cluster_order_twice() {
        let one = ClusterId(1);
        let two = ClusterId(2);

        let mut db = test_db(None);
        db.insert_cluster(Cluster::new(one, vec![Cite::basic("r1")], None));
        db.insert_cluster(Cluster::new(two, vec![Cite::basic("r2")], None));
        // inserting a cluster should not affect the cluster_ids.
        db.set_cluster_order(&[ClusterPosition::in_text(one)])
            .unwrap();
        db.set_cluster_order(&[ClusterPosition::in_text(one), ClusterPosition::in_text(two)])
            .unwrap();
    }
}

mod position {
    use super::*;

    use csl::Position;

    fn test_ibid_1_2(
        ordering: impl Fn(ClusterId, ClusterId) -> Vec<ClusterPosition>,
        pos1: (Position, Option<u32>),
        pos2: (Position, Option<u32>),
    ) {
        let mut db = test_db(None);
        let one = cid(&mut db, 1);
        let two = cid(&mut db, 2);
        db.init_clusters(vec![
            Cluster {
                id: one,
                cites: vec![Cite::basic("one")],
                mode: None,
            },
            Cluster {
                id: two,
                cites: vec![Cite::basic("one")],
                mode: None,
            },
        ]);
        db.set_cluster_order(&ordering(one, two)).unwrap();
        let poss = db.cite_positions();
        // Get the single cites inside
        let id1 = db.cluster_cites(one.raw())[0];
        let id2 = db.cluster_cites(two.raw())[0];
        // Check their positions
        assert_eq!(poss[&id1], pos1, "position of cite in cluster 1");
        assert_eq!(poss[&id2], pos2, "position of cite in cluster 2");
    }

    #[test]
    fn cite_positions_note_ibid() {
        test_ibid_1_2(
            |one, two| vec![ClusterPosition::note(one, 1), ClusterPosition::note(two, 2)],
            (Position::First, None),
            (Position::IbidNear, Some(1)),
        )
    }

    #[test]
    fn cite_positions_intext_ibid() {
        test_ibid_1_2(
            |one, two| {
                vec![
                    // both in-text
                    ClusterPosition::in_text(one),
                    ClusterPosition::in_text(two),
                ]
            },
            (Position::First, None),
            // No FRNN as not in a note!
            (Position::Ibid, None),
        );
    }

    #[test]
    fn cite_positions_mixed_noibid() {
        test_ibid_1_2(
            |one, two| vec![ClusterPosition::in_text(one), ClusterPosition::note(two, 1)],
            (Position::First, None),
            (Position::First, None),
        );
    }

    #[test]
    fn cite_positions_mixed_notefirst() {
        test_ibid_1_2(
            |one, two| vec![ClusterPosition::note(one, 1), ClusterPosition::in_text(two)],
            (Position::First, None),
            // XXX: should probably preserve relative ordering of notes and in-text clusters,
            // so that this gets (Position::Subsequent, Some(1))
            (Position::First, None),
        );
    }

    #[test]
    fn cite_positions_near_note() {
        let mut db = test_db(None);
        insert_ascending_notes(&mut db, &["one", "other", "one"]);
        let one = cid(&mut db, 1);
        let two = cid(&mut db, 2);
        let three = cid(&mut db, 3);
        let poss = db.cite_positions();
        let id1 = db.cluster_cites(one.raw())[0];
        let id2 = db.cluster_cites(two.raw())[0];
        let id3 = db.cluster_cites(three.raw())[0];
        assert_eq!(poss[&id1], (Position::First, None));
        assert_eq!(poss[&id2], (Position::First, None));
        assert_eq!(poss[&id3], (Position::NearNote, Some(1)));
    }
}

mod preview {
    use super::*;

    const STYLE: &'static str = r##"
    <style class="note" version="1.0.1">
        <citation>
            <layout delimiter="; ">
                <group delimiter=", ">
                    <text variable="title" />
                    <choose>
                        <if position="ibid"><text value="ibid" /></if>
                        <else-if position="subsequent"><text value="subsequent" /></else-if>
                    </choose>
                </group>
            </layout>
        </citation>
    </style>
"##;

    fn mk_db() -> Processor {
        let mut db = test_db(Some(STYLE));
        insert_basic_refs(&mut db, &["one", "two", "three"]);
        insert_ascending_notes(&mut db, &["one", "two"]);
        db
    }

    #[test]
    fn preview_cluster_replace() {
        let mut db = mk_db();
        let c = cid(&mut db, 1);
        assert_cluster!(db.get_cluster(c), Some("Book one"));
        let cites = vec![Cite::basic("two")];
        let preview = db.preview_citation_cluster(
            PreviewCluster::new(cites, None),
            PreviewPosition::ReplaceCluster(c),
            None,
        );
        assert_cluster!(db.get_cluster(c), Some("Book one"));
        assert_cluster!(preview, Ok("Book two"));
    }

    #[test]
    fn preview_cluster_replace_ibid() {
        let mut db = mk_db();
        let two = cid(&mut db, 2);
        assert_cluster!(db.get_cluster(two), Some("Book two"));
        let cites = vec![Cite::basic("one")];
        let preview = db.preview_citation_cluster(
            PreviewCluster::new(cites, None),
            PreviewPosition::ReplaceCluster(two),
            None,
        );
        assert_cluster!(db.get_cluster(two), Some("Book two"));
        assert_cluster!(preview, Ok("Book one, ibid"));
    }

    #[test]
    fn preview_cluster_reorder_append() {
        let mut db = mk_db();
        let cites = vec![Cite::basic("one")];
        let one = cid(&mut db, 1);
        let two = cid(&mut db, 2);
        let positions = &[
            ClusterPosition::note(one, 1),
            ClusterPosition::note(two, 2),
            ClusterPosition::preview_note(3), // Append at the end
        ];
        let preview = db.preview_citation_cluster(
            PreviewCluster::new(cites, None),
            PreviewPosition::MarkWithZero(positions),
            None,
        );
        assert_cluster!(preview, Ok("Book one, subsequent"));
        assert_cluster!(db.get_cluster(one), Some("Book one"));
        assert_cluster!(db.get_cluster(two), Some("Book two"));
    }

    #[test]
    fn preview_cluster_reorder_insert() {
        let mut db = mk_db();
        let cites = vec![Cite::basic("one"), Cite::basic("three")];
        let one = cid(&mut db, 1);
        let two = cid(&mut db, 2);
        let positions = &[
            ClusterPosition::preview_note(1), // Insert into the first note, at the start.
            ClusterPosition::note(one, 1),
            ClusterPosition::note(two, 2),
        ];
        let preview = db.preview_citation_cluster(
            PreviewCluster::new(cites, None),
            PreviewPosition::MarkWithZero(positions),
            None,
        );
        assert_cluster!(preview, Ok("Book one; Book three"));
        assert_cluster!(db.get_cluster(one), Some("Book one"));
        assert_cluster!(db.get_cluster(two), Some("Book two"));
    }

    #[test]
    fn preview_cluster_reorder_replace() {
        let mut db = mk_db();
        let cites = vec![Cite::basic("three")];
        let one = cid(&mut db, 1);
        let two = cid(&mut db, 2);
        let positions = &[
            ClusterPosition::preview_note(1), // Replace cluster #1
            ClusterPosition::note(two, 2),
        ];
        let preview = db.preview_citation_cluster(
            PreviewCluster::new(cites, None),
            PreviewPosition::MarkWithZero(positions),
            None,
        );
        assert_cluster!(preview, Ok("Book three"));
        assert_cluster!(db.get_cluster(one), Some("Book one"));
        assert_cluster!(db.get_cluster(two), Some("Book two"));
    }
}

mod terms {
    use super::*;

    fn en_au() -> Lang {
        Lang::Iso(IsoLang::English, Some(IsoCountry::AU))
    }

    fn terms(xml: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
        <locale xmlns="http://purl.org/net/xbiblio/csl" version="1.0" xml:lang="en-US">
        <terms>{}</terms></locale>"#,
            xml
        )
    }

    use citeproc_db::PredefinedLocales;
    fn predefined_xml(pairs: &[(Lang, &str)]) -> PredefinedLocales {
        let mut map = HashMap::new();
        for (lang, ts) in pairs {
            map.insert(lang.clone(), terms(ts));
        }
        PredefinedLocales(map)
    }

    fn term_and(form: TermFormExtended) -> SimpleTermSelector {
        SimpleTermSelector::Misc(MiscTerm::And, form)
    }

    fn test_simple_term(term: SimpleTermSelector, langs: &[(Lang, &str)], expect: Option<&str>) {
        let db = Processor::safe_default(Arc::new(predefined_xml(langs)));
        // use en-AU so it has to do fallback to en-US
        let locale = db.merged_locale(en_au());
        assert_eq!(
            locale.get_text_term(TextTermSelector::Simple(term), false),
            expect
        )
    }

    #[test]
    fn term_override() {
        test_simple_term(
            term_and(TermFormExtended::Long),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (en_au(), r#"<term name="and">Australia</term>"#),
            ],
            Some("Australia"),
        )
    }

    #[test]
    fn term_form_refine() {
        test_simple_term(
            term_and(TermFormExtended::Long),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (en_au(), r#"<term name="and" form="short">Australia</term>"#),
            ],
            Some("USA"),
        );
        test_simple_term(
            term_and(TermFormExtended::Short),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (en_au(), r#"<term name="and" form="short">Australia</term>"#),
            ],
            Some("Australia"),
        );
    }

    #[test]
    fn term_form_fallback() {
        test_simple_term(
            term_and(TermFormExtended::Short),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (en_au(), r#"<term name="and">Australia</term>"#),
            ],
            Some("Australia"),
        );
        test_simple_term(
            // short falls back to long and skips the "symbol" in a later locale
            term_and(TermFormExtended::Short),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (
                    en_au(),
                    r#"<term name="and" form="symbol">Australia</term>"#,
                ),
            ],
            Some("USA"),
        );
        test_simple_term(
            term_and(TermFormExtended::VerbShort),
            &[
                (Lang::en_us(), r#"<term name="and" form="long">USA</term>"#),
                (
                    en_au(),
                    r#"<term name="and" form="symbol">Australia</term>"#,
                ),
            ],
            Some("USA"),
        );
    }

    #[test]
    fn term_locale_fallback() {
        test_simple_term(
            term_and(TermFormExtended::Long),
            &[
                (Lang::en_us(), r#"<term name="and">USA</term>"#),
                (en_au(), r#""#),
            ],
            Some("USA"),
        )
    }
}
