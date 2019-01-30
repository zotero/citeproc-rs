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

use citeproc::input::*;
use citeproc::output::*;
use csl::style::CslType;
use citeproc::style::variables::*;
use citeproc::Driver;

use std::fs::File;
use std::io::prelude::*;
use std::str::FromStr;

fn bench_build_tree(c: &mut Criterion) {
    let formatter = PlainText::new();
    c.bench_function("Driver::new", move |b| {
        b.iter(|| {
            Driver::new(&aglc(), &formatter).unwrap();
        })
    });
}

fn common_reference() -> Reference {
    let mut refr = Reference::empty("id".into(), CslType::LegalCase);
    refr.ordinary
        .insert(Variable::ContainerTitle, String::from("TASCC"));
    refr.number
        .insert(NumberVariable::Number, NumericValue::num(55));
    refr.date.insert(
        DateVariable::Issued,
        DateOrRange::from_str("1998-01-04").unwrap(),
    );
    refr
}

fn aglc() -> String {
    let path = "./benches/data/australian-guide-to-legal-citation.csl";
    let mut f = File::open(path).expect("no file at path");
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    contents
}

// fn bench_flatten<O: OutputFormat + std::fmt::Debug>(
//     b: &mut Bencher,
//     formatter: &O,
// ) {
//     let driver = Driver::new(&aglc(), formatter).unwrap();
//     driver.bench_flatten(b, &common_reference());
// }

fn bench_ir_gen<O: OutputFormat>(b: &mut Bencher, style: &str, formatter: &O) {
    let cite = Cite::basic(0, "ok");
    let refr = common_reference();
    let driver = Driver::new(style, formatter).unwrap();
    b.iter(move || driver.pair(&cite, &refr))
}

fn bench_ir_gen_multi<O: OutputFormat>(b: &mut Bencher, style: &str, formatter: &O) {
    let cite = Cite::basic(0, "ok");
    let refr = common_reference();
    let pairs: Vec<_> = std::iter::repeat((&cite, &refr)).take(40).collect();
    let driver = Driver::new(style, formatter).unwrap();
    b.iter(move || driver.multiple(&pairs))
}

fn bench_pandoc(c: &mut Criterion) {
    // c.bench_function("flatten", |b| bench_flatten(b, format));
    c.bench_function("pandoc_ir_gen", |b| {
        bench_ir_gen(b, &aglc(), &Pandoc::new())
    });
    c.bench_function("pandoc_ir_gen_multi", |b| {
        bench_ir_gen_multi(b, &aglc(), &Pandoc::new())
    });
}

fn bench_plain(c: &mut Criterion) {
    // c.bench_function("flatten", |b| bench_flatten(b, format));
    c.bench_function("plain_ir_gen", |b| {
        bench_ir_gen(b, &aglc(), &PlainText::new())
    });
    c.bench_function("plain_ir_gen_multi", |b| {
        bench_ir_gen_multi(b, &aglc(), &PlainText::new())
    });
}

criterion_group!(tree, bench_build_tree);
criterion_group!(plain, bench_pandoc);
criterion_group!(pandoc, bench_plain);
criterion_main!(tree, plain, pandoc);

// pub fn bench_flatten(&self, b: &mut Bencher, refr: &Reference) {
//     let ctx = CiteContext {
//         style: &self.style,
//         reference: refr,
//         cite: &Cite::basic("ok", &self.formatter.output(self.formatter.plain(""))),
//         position: Position::First,
//         format: self.formatter,
//         citation_number: 1,
//     };
//     let i = self.style.intermediate(&ctx);
//     b.iter(|| {
//         i.flatten(self.formatter);
//     });
// }
