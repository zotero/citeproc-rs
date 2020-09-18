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

use criterion::{Bencher, BenchmarkGroup, Criterion, measurement::WallTime};
use std::sync::Arc;

use citeproc::prelude::*;
use citeproc_io::{DateOrRange, NumberLike};
use csl::variables::*;
use csl::CslType;
use test_utils::{TestCase, humans::parse_human_test, yaml::parse_yaml_test};

use std::str::FromStr;
use std::path::{Path, PathBuf};

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

use salsa::Database;

static AGLC: &'static str = include_str!("./data/australian-guide-to-legal-citation.csl");
static APA: &'static str = include_str!("./data/apa.csl");

fn basic_cluster_get_cite_id(proc: &mut Processor, cluster_id: u32, id: &str) -> CiteId {
    let cluster = Cluster {
        id: cluster_id,
        cites: vec![Cite::basic(id)],
    };
    proc.insert_cluster(cluster);
    let id = proc.cluster_cites(cluster_id).iter().cloned().nth(0).unwrap();
    id
}

fn invalidate_rebuild_cluster(proc: &mut Processor, id: u32, cite_id: CiteId) -> Arc<String> {
    use citeproc_proc::db;
    proc.query_mut(db::IrGen0Query).invalidate(&cite_id);
    proc.query_mut(db::IrGen1AddNamesQuery).invalidate(&cite_id);
    proc.query_mut(db::IrGen2AddGivenNameQuery).invalidate(&cite_id);
    proc.query_mut(db::IrGen3AddYearSuffixQuery).invalidate(&cite_id);
    proc.query_mut(db::IrGen4ConditionalsQuery).invalidate(&cite_id);
    proc.query_mut(db::BuiltClusterQuery).invalidate(&id);
    proc.built_cluster(id)
}

fn bench_build_cluster(b: &mut Bencher, style: &str) {
    let mut proc = Processor::new(style, fetcher(), false, SupportedFormat::Html).unwrap();
    proc.insert_reference(common_reference(1));
    let cite_id = basic_cluster_get_cite_id(&mut proc, 1, "id_1");
    proc.set_cluster_order(&[ClusterPosition { id: 1, note: Some(1) }]).unwrap();
    b.iter(move || invalidate_rebuild_cluster(&mut proc, 1, cite_id));
    // b.iter_batched_ref(make, |proc| proc.built_cluster(1), BatchSize::SmallInput)
}

fn bench_clusters(c: &mut Criterion) {
    // env_logger::init();
    c.bench_function("Processor::built_cluster(AGLC)", |b| bench_build_cluster(b, AGLC));
    c.bench_function("Processor::built_cluster(APA)", |b| bench_build_cluster(b, APA));
}

fn test_case(b: &mut Bencher, case: &TestCase) {
    b.iter_batched(move || case.clone(), |mut case| case.execute(), criterion::BatchSize::SmallInput);
}

fn test_case_txt(g: &mut BenchmarkGroup<'_, WallTime>, name: &str) {
    let mut path = PathBuf::new();
    path.push("tests");
    path.push("data");
    path.push("test-suite");
    path.push("processor-tests");
    path.push("humans");
    path.push(name);
    let input = std::fs::read_to_string(path).unwrap();
    let case = parse_human_test(&input);
    g.bench_function(name, |b| test_case(b, &case));
}

fn bench_test_cases(c: &mut Criterion) {
    let mut grp = c.benchmark_group("test_cases");
    test_case_txt(&mut grp, "disambiguate_BasedOnSubsequentFormWithBackref.txt");
    test_case_txt(&mut grp, "disambiguate_AndreaEg2.txt");
    grp.finish();
}

criterion_group!(clusters, bench_clusters);
criterion_group!(test_cases, bench_test_cases);
criterion_main!(clusters, test_cases);
