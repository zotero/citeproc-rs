# `citeproc-rs`

An early-stage work-in-progress implementation of [CSL][] and [CSL-M][] in 
[Rust][]. It is geared at:

* replacing `citeproc-js` by providing WebAssembly bindings such that it could 
  be embedded in Zotero or fulfil any other JavaScript-based use;
* replacing much of `pandoc-citeproc`, by running as a Pandoc Filter;
* providing a compiled static library to replace other divergent or incomplete 
  implementations and make it easy to integrate citeproc into document 
  processing pipelines and web services; and
* correctness and high performance.
 
Nearly every programming language in existence can link against a static C 
library; this effort is therefore also aimed at reducing the number of 
implementations to 1, thereby reducing the burden on implementers and making 
CSL evolution more nimble.

[CSL]: https://docs.citationstyles.org/en/stable/specification.html
[CSL-M]: https://citeproc-js.readthedocs.io/en/latest/csl-m/index.html
[Rust]: https://rust-lang.org/

Currently, the codebase is evolving rapidly and if you submit a PR, chances are 
I've either already done it or made a change that breaks the subsystem you were 
working on. Or force-pushed on master. If you really want to contribute, let me 
know and we can sort something out.

[Status tracker issue here](https://github.com/cormacrelf/citeproc-rs/issues/1).

## Technology overview

Compiling requires Rust 2018 Edition with Cargo, i.e. stable version `1.31` or 
later, or a nightly compiler. You should install it with 
[rustup](https://rustup.rs/).

* XML parsing with [`roxmltree`](https://github.com/RazrFalcon/roxmltree)
* Error reporting with [`codespan`](https://github.com/brendanzab/codespan)
* Little utility parsers written with [`nom`](https://github.com/Geal/nom)
* Parallel processing using [`rayon`](https://github.com/rayon-rs/rayon)'s 
  work-stealing queues
* JSON IO using [`serde_json`](https://github.com/serde-rs/json)
* Pandoc-JSON interop using 
  [`pandoc_types`](https://github.com/elliottslaughter/rust-pandoc-types/).

## Try it out!

Currently it can:

* parse a CSL style (ignoring `<info>`, `<style-options>`, and 
  `<bibliography>`) with built-in validation, type-checking, error reporting, 
  and semantic versioning,
* parse a CSL-M style (ignoring `<info>`, `<style-options>`, and 
  `<bibliography>`, missing many undocumented `citeproc-js` extensions to the 
  spec),
* parse a CSL-JSON file
* pluck out the first reference, and execute the style against only that one
* output valid Pandoc JSON that can be converted to HTML

```sh
git clone https://github.com/cormacrelf/citeproc-rs
cd citeproc-rs/citeproc
cargo run -- --csl ../example.csl # runs on a predefined single ref
cargo run -- --csl ../example.csl --library path/to/csl-json/file.json
```

To test it across the entire styles repo:

```sh
cd citeproc-rs/citeproc
cargo install --path . --force
cd ../..
git clone https://github.com/citation-style-language/styles
for style in styles/*.csl; do citeproc-rs --csl $style | pandoc -f json -t html; done
```

* Some styles in the repo are possibly invalid (mostly for using terms that 
  aren't actually listed in the terms in the spec, namely number variables).
* Some will successfully output HTML!
* Many styles encounter unimplemented name blocks and render pretty much 
  nothing.
* Many styles will panic when accessing locales, as most locale lookups are 
  just `.get("en-GB").unwrap()` on the inline locales instead of using a merged 
  locale.

