#![feature(test)]

extern crate cfg_if;
use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(test)] {
        use std::alloc::System;
        #[global_allocator]
        static A: System = System;
    }
}

// #[macro_use]
extern crate failure;

mod driver;
pub mod input;
pub mod output;
pub mod style;
pub use self::driver::Driver;
mod utils;

pub use self::style::error::StyleError;

pub mod proc;

#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
#[cfg(test)]
extern crate test;

#[cfg(test)]
mod tests {
    use crate::input::*;
    use crate::output::*;
    use crate::style::element::CslType;
    use crate::style::variables::*;
    use crate::Driver;
    use crate::StyleError;

    use std::fs::File;
    use std::io::prelude::*;
    use std::str::FromStr;
    use test::Bencher;

    use pandoc_types::definition::Inline;

    #[bench]
    fn bench_build_tree(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        let formatter = PlainText::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        b.iter(|| {
            Driver::new(&contents, &formatter).unwrap();
        });
        Ok(())
    }

    fn bench_single<O: OutputFormat + std::fmt::Debug>(
        b: &mut Bencher,
        path: &str,
        formatter: O,
    ) -> Result<(), StyleError> {
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        let driver = Driver::new(&contents, &formatter)?;
        let mut refr = Reference::empty("id", CslType::LegalCase);
        refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
        refr.number
            .insert(NumberVariable::Number, NumericValue::num(55));
        refr.date.insert(
            DateVariable::Issued,
            DateOrRange::from_str("1998-01-04").unwrap(),
        );
        driver.bench_single(b, &refr);
        Ok(())
    }

    fn bench_ir_gen<O: OutputFormat + std::fmt::Debug>(
        b: &mut Bencher,
        path: &str,
        formatter: O,
    ) -> Result<(), StyleError> {
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        let driver = Driver::new(&contents, &formatter)?;
        let mut refr = Reference::empty("id", CslType::LegalCase);
        refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
        refr.number
            .insert(NumberVariable::Number, NumericValue::num(55));
        refr.date.insert(
            DateVariable::Issued,
            DateOrRange::from_str("1998-01-04").unwrap(),
        );
        driver.bench_intermediate(b, &refr);
        Ok(())
    }

    #[bench]
    fn bench_ir_gen_plain(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let format = PlainText::new();
        bench_ir_gen(b, path, format)
    }

    #[bench]
    fn bench_ir_gen_pandoc(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let format = Pandoc::new();
        bench_ir_gen(b, path, format)
    }

    fn bench_ir_gen_multi<O: OutputFormat + std::fmt::Debug>(
        b: &mut Bencher,
        path: &str,
        formatter: O,
    ) -> Result<(), StyleError> {
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        let driver = Driver::new(&contents, &formatter)?;
        let mut refr = Reference::empty("id", CslType::LegalCase);
        refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
        refr.number
            .insert(NumberVariable::Number, NumericValue::num(55));
        refr.date.insert(
            DateVariable::Issued,
            DateOrRange::from_str("1998-01-04").unwrap(),
        );
        driver.bench_intermediate_multi(b, &refr);
        Ok(())
    }

    #[bench]
    fn bench_ir_gen_pandoc_multi(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let format = Pandoc::new();
        bench_ir_gen_multi(b, path, format)
    }

    #[bench]
    fn bench_ir_gen_plain_multi(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let format = PlainText::new();
        bench_ir_gen_multi(b, path, format)
    }

    // #[bench]
    // fn bench_single_plain(b: &mut Bencher) -> Result<(), StyleError> {
    //     let path = "./australian-guide-to-legal-citation.csl";
    //     let format = PlainText::new();
    //     bench_single(b, path, format)
    // }

    // #[bench]
    // fn bench_single_pandoc(b: &mut Bencher) -> Result<(), StyleError> {
    //     let path = "./australian-guide-to-legal-citation.csl";
    //     let format = Pandoc::new();
    //     bench_single(b, path, format)
    // }

    fn bench_flatten<O: OutputFormat + std::fmt::Debug>(
        b: &mut Bencher,
        path: &str,
        formatter: O,
    ) -> Result<(), StyleError> {
        let mut f = File::open(path).expect("no file at path");
        let mut contents = String::new();
        f.read_to_string(&mut contents)
            .expect("something went wrong reading the file");
        let driver = Driver::new(&contents, &formatter)?;
        let mut refr = Reference::empty("id", CslType::LegalCase);
        refr.ordinary.insert(Variable::ContainerTitle, "TASCC");
        refr.number
            .insert(NumberVariable::Number, NumericValue::num(55));
        refr.date.insert(
            DateVariable::Issued,
            DateOrRange::from_str("1998-01-04").unwrap(),
        );
        driver.bench_flatten(b, &refr);
        Ok(())
    }

    #[bench]
    fn bench_flatten_plain(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let format = PlainText::new();
        bench_flatten(b, path, format)
    }

    #[bench]
    fn bench_flatten_pandoc(b: &mut Bencher) -> Result<(), StyleError> {
        let path = "./australian-guide-to-legal-citation.csl";
        let format = Pandoc::new();
        bench_flatten(b, path, format)
    }

    // #[bench]
    // fn bench_fail(b: &mut Bencher) {
    //     let path = "path";
    //     let contents = "<content></content>".to_owned();
    //     b.iter(|| {
    //         drive_style(path, &contents);
    //     });
    // }

}
