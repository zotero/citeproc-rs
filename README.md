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
* Incremental computation using [`salsa`](https://github.com/salsa-rs/salsa)
* Parallel processing using [`rayon`](https://github.com/rayon-rs/rayon)'s 
  work-stealing queues
* JSON IO using [`serde_json`](https://github.com/serde-rs/json)
* Pandoc-JSON interop using (currently) an internal fork of
  [`pandoc_types`](https://github.com/elliottslaughter/rust-pandoc-types/).



## Try it out!

Currently it can:

* parse a CSL style (ignoring `<info>`) with built-in validation, 
  type-checking, error reporting, and semantic versioning,
* parse a CSL-M style (ignoring `<info>`, probably still missing many 
  undocumented `citeproc-js` extensions to the spec),
* parse locale files and perform locale fallback and merging for 
  terms/dates/etc inside it
* parse a CSL-JSON file into references
* pluck out a particular reference, and execute the style against only that one
* read and write cites for an entire Pandoc JSON document

### Parse a style

```sh
git clone https://github.com/cormacrelf/citeproc-rs
cd citeproc-rs/crates/citeproc-cli
cargo run -- --csl ../example.csl # runs on a predefined single ref
cargo run -- --csl ../example.csl --library path/to/csl-json/file.json
```

To test it across the entire styles repo:

```sh
cd citeproc-rs/crates/citeproc-cli
cargo install --path . --force
cd ../..
git clone https://github.com/citation-style-language/styles
for style in styles/*.csl; do citeproc-rs --csl $style | pandoc -f json -t html; done
```

* Some styles in the repo are possibly invalid (mostly for using terms that 
  don't exist).
* Some will successfully output HTML!

### Parse a locale

You can also parse a locale to check for errors. It can find a locale in a 
locales directory assuming it is structured like [the official CSL locales 
repo](https://github.com/citation-style-language/locales), using e.g. 
`$HOME/Library/Caches/net.cormacrelf.citeproc-rs/locales` on a Mac, or 
`$HOME/.cache/citeproc-rs/locales` on Linux. It's best to just clone that repo 
into place.
See the [directories](https://docs.rs/directories) crate for more.

```sh
git clone https://github.com/citation-style-language/locales $DIR_FROM_ABOVE
cd crates/citeproc-cli
cargo run -- parse-locale --lang en-GB
```

### The big end-to-end Pandoc filter

#### Step 1: export a CSL-JSON library somewhere, with Zotero for example

#### Step 2: create a markdown file

It must contain inline `csl`/`bibliography` metadata. Currently, and contrary 
to its documentation, Pandoc will automatically add `-F pandoc-citeproc` 
whenever you add command line `--metadata csl=XXX` or `--metadata 
bibliography=XXX` flags. (That is, as far as I know, only supposed to happen if 
you use shorthand `--csl XXX` or `--bibliography XXX`.)

    ---
    csl: path-to-my-csl.csl
    bibliography: path-to-my-csl-json-library.json
    ---

    First paragraph.[@knownCitekey]

    Second paragraph.[@knownCitekey; @anotherOne]

#### Step 4: Run as a filter!

```sh
# much quicker than `build --release` or `install --path .`
cargo build

pandoc -F ../target/debug/citeproc-rs input.md -s -o out.html

open out.html
```

## Running the CSL test suite

```sh
# setup once
cargo pull-test-suite

cd crates/citeproc

# the whole suite in parallel
cargo test

# for a particular test, paste the file name
cargo test name_ParsedDroppingParticleWithApostrophe.txt

# for a subset of tests with some commonality in the name (this runs 8 of them)
cargo test name_Initials
```

Run `cargo test -- --test-threads 1` to have the tests run in a deterministic 
order (i.e. alphabetically); this helps show related tests alongside one 
another in the terminal output. Run `cargo test -- --help` for more options.
