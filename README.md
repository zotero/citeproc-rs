# `citeproc-rs`

A work-in-progress implementation of [CSL][] and [CSL-M][] in [Rust][]. It is 
geared at:

* replacing `citeproc-js` by providing WebAssembly bindings such that it could 
  be embedded in Zotero or fulfil any other JavaScript-based use;
* replacing much of `pandoc-citeproc`, by running as a Pandoc Filter;
* making it easy to use citeproc from any programming language, and integrate 
  into any use case; and
* correctness and high performance.

[CSL]: https://docs.citationstyles.org/en/stable/specification.html
[CSL-M]: https://citeproc-js.readthedocs.io/en/latest/csl-m/index.html
[Rust]: https://rust-lang.org/

## Supported Rust versions

With a stable Rust compiler, 1.43 or later, you can:

* Build `citeproc` as a library
* Build a WebAssembly package (see below for details)

For development, you need a recent nightly compiler. This is required to run 
the test suite within the `cargo test` harness using `datatest`. Keep your 
nightly up to date, as there are frequent breaking changes in that area at the 
moment, and this repo will track close to the edge.

You should install Rust and keep it up to date with 
[rustup](https://rustup.rs/). 

## Try it out

There is a [demo](https://cormacrelf.github.io/citeproc-wasm-demo/) of 
`citeproc-rs` in action. It includes a graph visualisation of cite ambiguity 
testing. See if you can figure out how it works.

To see how well `citeproc-rs` is doing on the tests, visit
https://cormacrelf.github.io/citeproc-rs-test-viewer/

## WebAssembly usage

The WebAssembly shell lives in `crates/wasm`. It consists of a JavaScript API 
that wraps the `citeproc` crate, and a mechanism for asynchronous locale 
fetching. The API works mostly by serializing to JSON and back, but this is 
invisible. It includes TypeScript definitions, which are the main source of 
documentation. Open up the generated `.d.ts` file or view the doc-comments with 
your editor or IDE. A useful reference is the `js-demo`, which exercises most 
of its functionality and demonstrates correct usage of the 
`batchedUpdates`/`UpdateSummary` API.

Note especially that `Driver` cannot be garbage collected. You will need to 
`.free()` it manually, otherwise the whole engine and its cached data will be 
leaked. Like many things with WebAssembly MVP, this might improve. Same with 
the WASM multithreading proposal -- computing cites is synchronous and 
single-threaded now, but it may be desirable in future to make it async and 
multithreaded. We'll see.

### Building

1. Install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
2. `cd crates/wasm`; ` wasm-pack build -h`

Refer to the 
[docs](https://rustwasm.github.io/docs/wasm-pack/commands/build.html) for on 
how to get the output you need, particularly `--target`. 

## Non-WebAssembly usage

For Rust users, this package will available at some point on crates.io. The 
`csl` crate is already available, if all you wanted to do was parse/validate 
styles and locales. (Hint: `use std::str::FromStr; Style::from_str(xml);`.) For 
the rest, it needs a slightly more stable API, and maybe some of the 
crate-splitting rethought. (I don't want to claim crate names and then change 
my mind later. They are split for compile time reasons.)

It will likely be possible to support communicating via JSON messages over 
stdio: drop me a line over at 
https://github.com/cormacrelf/citeproc-rs/issues/13 if this interests you.
There is currently no way to use `citeproc-rs` via C FFI. This is mostly 
because of the complex structured data involved, which requires significant 
conversion effort and may not be worth it. If this is something you really 
want, file an issue.

## Running the CSL test suite

`citeproc-rs` comes with a full-featured test harness for the CSL test suite, 
based on the Rust testing infrastructure. This includes colourful diffs, and 
support for a new YAML-based test case format. However, given that at the 
moment not all of the tests pass, a more nuanced way of detecting failure and 
comparing results to find regressions between revisions was necessary. So now 
it can store and diff test runs. Pull requests that cause regressions (`Ok => 
Failure`) compared to master will fail.

```sh
# setup once
cargo pull-locales
cargo pull-test-suite

cargo test-suite --help

# the whole suite in parallel
cargo test-suite run

# the whole suite with deterministic test execution order
# this helps show related tests alongside one another in the terminal output
cargo test-suite run -- --test-threads 1

# for a particular test, paste the file name
cargo test-suite run -- name_ParsedDroppingParticleWithApostrophe.txt

# for a subset of tests with some commonality in the name (this runs 8 of them)
cargo test-suite run -- name_Initials

# store a test run for comparison
# this will also save a copy in 'branch_name' and 'commit_hash' if your working 
# directory is clean
cargo test-suite store [name] [-- filter_pattern]
cargo test-suite store disamb -- disamb # all of the disamb tests

# diff two named, commit-named or commit-hash test results
# outputs any regressions and fixed test cases
# exits with code 1 if any regressions
cargo test-suite diff master # compares to ..current

# only checks the intersection of the tests, especially if you're using a filter
cargo test-suite diff master..disamb
# test result: 0 regressions, 0 new passing tests, 0 new ignores, out of 107 intersecting tests

# saves in 'current'
cargo test-suite store
# copies 'current' to 'blessed'
cargo test-suite bless
# (make some changes)
# compares blessed..current
cargo test-suite store && cargo test-suite diff

# in a clean repo, go back in time to a commit and store the captured
# result by its commit SHA and an optional name, then checkout HEAD again
cargo test-suite checkout-store [name]
```

<!--

Hidden because not currently working.

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
repo](https://github.com/citation-style-language/locales), found via
[directories](https://docs.rs/directories) (the cache directory). **Shortcut**:

```sh
# clones the locales repo into place for you
cargo pull-locales
```

Then:

```sh
# currently broken
cd crates/citeproc-cli
cargo run -- parse-locale --lang en-GB
```

### The big end-to-end Pandoc filter (currently broken)

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

-->

