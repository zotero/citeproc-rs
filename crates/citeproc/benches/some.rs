// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(feature="jemalloc")] {
        use jemallocator::Jemalloc;
        #[global_allocator]
        static A: Jemalloc = Jemalloc;
    } else {
        use std::alloc::System;
        #[global_allocator]
        static A: System = System;
    }
}

#[macro_use]
extern crate criterion;

use criterion::{Bencher, Criterion};
use std::sync::Arc;

use citeproc::prelude::*;
use citeproc_io::{DateOrRange, NumberLike, SmartString};
use csl::variables::*;
use csl::CslType;
// use test_utils::{humans::parse_human_test, yaml::parse_yaml_test};

use std::str::FromStr;

// fn bench_build_tree(c: &mut Criterion) {
//     let formatter = PlainText::new();
//     c.bench_function("Driver::new", move |b| {
//         b.iter(|| {
//             Driver::new(AGLC, &formatter).unwrap();
//         })
//     });
// }

fn common_reference(n: u32) -> Reference {
    let mut refr = Reference::empty(format!("id_{}", n).into(), CslType::LegalCase);
    refr.ordinary
        .insert(Variable::Title, "Title v Title".into());
    refr.ordinary
        .insert(Variable::ContainerTitle, String::from("TASCC"));
    refr.number
        .insert(NumberVariable::Number, NumberLike::Num(55));
    refr.date.insert(
        DateVariable::Issued,
        DateOrRange::from_str("1998-01-04").unwrap(),
    );
    refr
}

fn fetcher() -> Arc<dyn LocaleFetcher> {
    Arc::new(citeproc_db::PredefinedLocales::bundled_en_us())
}

static AGLC: &'static str = include_str!("./data/australian-guide-to-legal-citation.csl");
static APA: &'static str = include_str!("./data/apa.csl");

fn basic_cluster_get_cite_id(proc: &mut Processor, cluster_id: ClusterId, id: &str) -> CiteId {
    let cluster = Cluster {
        id: cluster_id,
        cites: vec![Cite::basic(id)],
        mode: None,
    };
    proc.insert_cluster(cluster);
    let id = proc
        .cluster_cites(cluster_id)
        .iter()
        .cloned()
        .nth(0)
        .unwrap();
    id
}

fn invalidate_rebuild_cluster(
    proc: &mut Processor,
    id: ClusterId,
    cite_id: CiteId,
) -> Arc<SmartString> {
    use citeproc_proc::db;
    db::IrGen0Query.in_db_mut(proc).invalidate(&cite_id);
    db::IrGen2AddGivenNameQuery
        .in_db_mut(proc)
        .invalidate(&cite_id);
    db::IrFullyDisambiguatedQuery
        .in_db_mut(proc)
        .invalidate(&cite_id);
    db::BuiltClusterQuery.in_db_mut(proc).invalidate(&id);
    proc.built_cluster(id)
}

fn bench_build_cluster(b: &mut Bencher, style: &str) {
    let mut proc = Processor::new(InitOptions {
        style,
        test_mode: true,
        ..Default::default()
    })
    .unwrap();
    proc.insert_reference(common_reference(1));
    let cite_id = basic_cluster_get_cite_id(&mut proc, 1, "id_1");
    let cluster_id = ClusterId::new(1);
    proc.set_cluster_order(&[ClusterPosition::note(cluster_id, 1)])
        .unwrap();
    b.iter(move || invalidate_rebuild_cluster(&mut proc, cluster_id, cite_id));
    // b.iter_batched_ref(make, |proc| proc.built_cluster(1), BatchSize::SmallInput)
}

fn bench_clusters(c: &mut Criterion) {
    env_logger::init();
    c.bench_function("Processor::built_cluster(AGLC)", |b| {
        bench_build_cluster(b, AGLC)
    });
    c.bench_function("Processor::built_cluster(APA)", |b| {
        bench_build_cluster(b, APA)
    });
}

criterion_group!(clusters, bench_clusters);
criterion_main!(clusters);
