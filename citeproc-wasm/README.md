# `citeproc-wasm`

(Not yet implemented.)

This is a build of `citeproc` that is suitable for use in Node.js, a browser or 
a Firefox/Chromium-based application like Zotero. It consists of a WebAssembly 
(WASM) binary of roughly 100kB gzipped, and a fairly lightweight JavaScript 
wrapper for that binary. If your programming language can link to C libraries 
either natively or through FFI, it is preferable to use that instead.

The wrapper consists of:

* A JS class to wrap `citeproc::Driver`. The core processing code is 
  synchronous, but it will be sufficiently fast to run in any interactive 
  context.
* A JS interface for library consumers to asynchronously fetch locales and 
  style modules at specific points in Driver's lifecycle. The fetching must 
  happen and complete before the processing actually begins, because processing 
  cannot be paused to wait for the JS library consumer to fetch something. So 
  the processor will analyse which locales and modules will be needed, and 
  request all of them at once.
* Style, locale and style-module inputs are all XML strings.
* The library does its own parsing, and validates it at the same time. Any 
  validation errors are reported, with line/column positions, the text at that 
  location, a descriptive and useful message (only in English at the moment) 
  and sometimes even a hint for how to fix it, for common errors.
* Input libraries are only CSL-JSON, serialized as a string. The other input 
  formats that `citeproc` may recognise will be disabled, to save bundle size.
* Output is native JS objects. These are pretty much just the `serde_json` 
  outputs from `citeproc` for your chosen format.
* A TypeScript definition file auto-generated from the Rust types and 
  interfaces via `wasm-bindgen` and 
  [this](https://github.com/tcr/wasm-typescript-definition).

The library is intended to replace `citeproc-js`, but the interface will be 
different.

* All fetching is via Promises, but if you have a string already you can just 
  `return Promise.resolve(string)`.

* Not all features are supported (at least for now).
  * The `citeproc-js` abbreviations API currently requires a huge 
    implementation by library consumers in order to get the behaviour 
    envisioned by the spec. Some of this functionality needs to be made more 
    consistent by having the processor take responsibility for it. (Like 
    tokenizing/slugifying strings and requesting My, My_Abbreviated, 
    My_Abbreviated_Name in order, etc. Maybe feed all your abbreviations to the 
    processor and allow it to build its own 
    [trie](https://en.wikipedia.org/wiki/Trie) for much faster lookup.)
  * Some of it doesn't make much sense, like requiring users to import Juris-M 
    abbreviation lists into each document rather than linking abbreviations to 
    styles or jurisdictions or the Courts that each item in those huge lists 
    refers to. Some of the functionality just isn't documented enough to 
    re-implement.

* You cannot deserialize your own XML. Any tree modifications you want to do 
  should be implemented as transforms on the Style AST within `citeproc`, 
  which, it should be noted, is dramatically different from the XML such that 
  it is not possible to represent invalid CSL. There isn't any API for such 
  transforms at present, but it could be done. It's possible but unlikely that 
  such an API could be bridged to JavaScript.

Drop-in API compatibility will not be a goal, and given that, the whole 
interface may as well be improved.



# 🦀🕸️ Usage with `wasm-pack`

`wasm-pack` has a bug with Cargo workspaces. Run `../link.sh` to make it work 
for now.

### 🛠️ Build with `wasm-pack build`

```
wasm-pack build
```

### 🔬 Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```

### 🎁 Publish to NPM with `wasm-pack publish`

```
wasm-pack publish
```
