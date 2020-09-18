// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use std::collections::HashMap;
use std::sync::Arc;

use csl::*;
use crate::prelude::*;

mod position {
    use super::*;
    use crate::prelude::*;
    use csl::Position;

    fn insert_ascending_notes(db: &mut Processor, ref_ids: &[&str]) {
        let len = ref_ids.len();
        let mut clusters = Vec::with_capacity(len);
        let mut order = Vec::with_capacity(len);
        for i in 1..=len {
            clusters.push(Cluster {
                id: i as u32,
                cites: vec![Cite::basic(ref_ids[i - 1])],
            });
            order.push(ClusterPosition {
                id: i as u32,
                note: Some(i as u32),
            });
        }
        db.init_clusters(clusters);
        db.set_cluster_order(&order).unwrap();
    }

    fn test_ibid_1_2(
        ordering: &[ClusterPosition],
        pos1: (Position, Option<u32>),
        pos2: (Position, Option<u32>),
    ) {
        let mut db = Processor::test_db();
        db.init_clusters(vec![
            Cluster {
                id: 1,
                cites: vec![Cite::basic("one")],
            },
            Cluster {
                id: 2,
                cites: vec![Cite::basic("one")],
            },
        ]);
        db.set_cluster_order(ordering).unwrap();
        let poss = db.cite_positions();
        // Get the single cites inside
        let id1 = db.cluster_cites(1)[0];
        let id2 = db.cluster_cites(2)[0];
        // Check their positions
        assert_eq!(poss[&id1], pos1, "position of cite in cluster 1");
        assert_eq!(poss[&id2], pos2, "position of cite in cluster 2");
    }

    #[test]
    fn cite_positions_note_ibid() {
        test_ibid_1_2(
            &[
                ClusterPosition {
                    id: 1,
                    note: Some(1),
                },
                ClusterPosition {
                    id: 2,
                    note: Some(2),
                },
            ],
            (Position::First, None),
            (Position::Ibid, Some(1)),
        )
    }

    #[test]
    fn cite_positions_intext_ibid() {
        test_ibid_1_2(
            &[
                // both in-text
                ClusterPosition { id: 1, note: None },
                ClusterPosition { id: 2, note: None },
            ],
            (Position::First, None),
            // No FRNN as not in a note!
            (Position::Ibid, None),
        );
    }

    #[test]
    fn cite_positions_mixed_noibid() {
        test_ibid_1_2(
            &[
                ClusterPosition { id: 1, note: None },
                ClusterPosition {
                    id: 2,
                    note: Some(1),
                },
            ],
            (Position::First, None),
            (Position::First, None),
        );
    }

    #[test]
    fn cite_positions_mixed_notefirst() {
        test_ibid_1_2(
            &[
                ClusterPosition {
                    id: 1,
                    note: Some(1),
                },
                ClusterPosition { id: 2, note: None },
            ],
            (Position::First, None),
            // XXX: should probably preserve relative ordering of notes and in-text clusters,
            // so that this gets (Position::Subsequent, Some(1))
            (Position::First, None),
        );
    }

    #[test]
    fn cite_positions_near_note() {
        let mut db = Processor::test_db();
        insert_ascending_notes(&mut db, &["one", "other", "one"]);
        let poss = db.cite_positions();
        let id1 = db.cluster_cites(1)[0];
        let id2 = db.cluster_cites(2)[0];
        let id3 = db.cluster_cites(3)[0];
        assert_eq!(poss[&id1], (Position::First, None));
        assert_eq!(poss[&id2], (Position::First, None));
        assert_eq!(poss[&id3], (Position::NearNote, Some(1)));
    }
}

mod terms {
    use super::*;
    use crate::prelude::*;

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
