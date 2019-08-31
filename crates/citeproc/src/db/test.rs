// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2019 Corporation for Digital Scholarship

use std::collections::HashMap;
use std::sync::Arc;

use csl::locale::*;
use csl::terms::*;

use super::Processor;

mod position {
    use super::*;
    use crate::prelude::*;
    use csl::style::Position;

    #[test]
    fn cite_positions_ibid() {
        let mut db = Processor::test_db();
        db.init_clusters(vec![
            Cluster2::Note {
                id: 1,
                note: IntraNote::Single(1),
                cites: vec![Cite::basic("one")],
            },
            Cluster2::Note {
                id: 2,
                note: IntraNote::Single(2),
                cites: vec![Cite::basic("one")],
            },
        ]);
        let poss = db.cite_positions();
        let id1 = db.cluster_cites(1)[0];
        let id2 = db.cluster_cites(2)[0];
        assert_eq!(poss[&id1], (Position::First, None));
        assert_eq!(poss[&id2], (Position::Ibid, Some(1)));
    }

    #[test]
    fn cite_positions_near_note() {
        let mut db = Processor::test_db();
        db.init_clusters(vec![
            Cluster2::Note {
                id: 1,
                note: IntraNote::Single(1),
                cites: vec![Cite::basic("one")],
            },
            Cluster2::Note {
                id: 2,
                note: IntraNote::Single(2),
                cites: vec![Cite::basic("other")],
            },
            Cluster2::Note {
                id: 3,
                note: IntraNote::Single(3),
                cites: vec![Cite::basic("one")],
            },
        ]);
        use citeproc_db::CiteDatabase;
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
