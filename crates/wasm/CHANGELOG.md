# Changelog (@citeproc-rs/wasm)

## [wasm-v0.2.0](https://github.com/zotero/citeproc-rs/tree/wasm-v0.2.0) (2021-12-14)

[Full Changelog](https://github.com/zotero/citeproc-rs/compare/wasm-v0.1.0...wasm-v0.2.0)

#### Breaking changes:

- Add a way to specify the ClusterMode of a previewed cluster [\#119](https://github.com/zotero/citeproc-rs/issues/119)
- Remove .unwrap\(\) from JS API; `Driver.new` =\> `new Driver` [\#142](https://github.com/zotero/citeproc-rs/pull/142) [[A-ci](https://github.com/zotero/citeproc-rs/labels/A-ci)]

#### Implemented enhancements:

- Set output format after driver initialization [\#122](https://github.com/zotero/citeproc-rs/issues/122)
- previewCitationCluster =\> previewCluster [\#141](https://github.com/zotero/citeproc-rs/pull/141)
- setOutputFormat API [\#140](https://github.com/zotero/citeproc-rs/pull/140)

#### Fixed bugs:

- PMID and PMCID urls are backwards [\#131](https://github.com/zotero/citeproc-rs/issues/131) [[I-bug](https://github.com/zotero/citeproc-rs/labels/I-bug)] [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]

#### Closed issues:

- CI errors downloading test results from S3 when PRs are submitted from people who aren't me [\#134](https://github.com/zotero/citeproc-rs/issues/134)
- "\<" is ignored in prefix [\#129](https://github.com/zotero/citeproc-rs/issues/129) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- In some scenarios prefix needs to be ignored when wrapping DOI [\#127](https://github.com/zotero/citeproc-rs/issues/127) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Invalid HTML produced when item URL contains a quotation symbol `"`. [\#123](https://github.com/zotero/citeproc-rs/issues/123)
- BibliographyMeta wrongly cased/documented [\#77](https://github.com/zotero/citeproc-rs/issues/77)

#### Merged pull requests:

- reimplement name parsing [\#139](https://github.com/zotero/citeproc-rs/pull/139) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Solve year-suffix / no-date interaction [\#137](https://github.com/zotero/citeproc-rs/pull/137) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Fix "line-spacing" attribute on \<bibliography\> in CSL is ignored [\#133](https://github.com/zotero/citeproc-rs/pull/133) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Fix doi urls in prefix duplication, and pmids/pmcids [\#132](https://github.com/zotero/citeproc-rs/pull/132) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Fix erroneous parsing of HTML in prefix="..."/suffix [\#130](https://github.com/zotero/citeproc-rs/pull/130)
- Test case for \#127 [\#128](https://github.com/zotero/citeproc-rs/pull/128)
- Test ESR wasm support [\#125](https://github.com/zotero/citeproc-rs/pull/125)
- Escape urls, implement auto-prefixing and configurable link anchors [\#124](https://github.com/zotero/citeproc-rs/pull/124) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]

## [wasm-v0.1.0](https://github.com/zotero/citeproc-rs/tree/wasm-v0.1.0) (2021-10-21)

[Full Changelog](https://github.com/zotero/citeproc-rs/compare/wasm-v0.0.0...wasm-v0.1.0)

#### Breaking changes:

- `author-only`, `suppress-author`, `composite` [\#117](https://github.com/zotero/citeproc-rs/pull/117) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Breaking JS changes: Driver.new\(InitOptions\); WasmResult wrapper/.unwrap\(\); parseStyleMetadata [\#94](https://github.com/zotero/citeproc-rs/pull/94)
- Bring the CSL test suite to 100% [\#82](https://github.com/zotero/citeproc-rs/pull/82) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Remove unclear setReferences, replace with insertReferences + resetReferences [\#69](https://github.com/zotero/citeproc-rs/pull/69)

#### Implemented enhancements:

- suppress-author and friends  [\#114](https://github.com/zotero/citeproc-rs/issues/114) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Publish prerelease builds of the wasm driver [\#7](https://github.com/zotero/citeproc-rs/issues/7)
- CSL-JSON hardening [\#109](https://github.com/zotero/citeproc-rs/pull/109) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Integration testing on Zotero's Firefox ESR [\#108](https://github.com/zotero/citeproc-rs/pull/108) [[A-ci](https://github.com/zotero/citeproc-rs/labels/A-ci)]
- Return an error on unrecognised output format \(JS\) [\#107](https://github.com/zotero/citeproc-rs/pull/107)
- Use strings as cluster ids [\#80](https://github.com/zotero/citeproc-rs/pull/80)
- Queue-draining fullRender\(\) API; better diffing & bibliography docs [\#76](https://github.com/zotero/citeproc-rs/pull/76)
- Throw proper JS errors [\#75](https://github.com/zotero/citeproc-rs/pull/75)
- Write a new wasm README [\#70](https://github.com/zotero/citeproc-rs/pull/70) [[A-docs](https://github.com/zotero/citeproc-rs/labels/A-docs)]
- Preview citation cluster [\#68](https://github.com/zotero/citeproc-rs/pull/68)
- Uncited Items API [\#67](https://github.com/zotero/citeproc-rs/pull/67) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]

#### Fixed bugs:

- npmjs.com repo is not updating [\#110](https://github.com/zotero/citeproc-rs/issues/110) [[I-bug](https://github.com/zotero/citeproc-rs/labels/I-bug)] [[A-ci](https://github.com/zotero/citeproc-rs/labels/A-ci)]
- CiteprocRsDriverError: JSON Deserialization Error: unknown field `year`, expected one of `date-parts`, `season`, `circa`, `literal`, `raw` [\#99](https://github.com/zotero/citeproc-rs/issues/99) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)] [[I-spec](https://github.com/zotero/citeproc-rs/labels/I-spec)]
- Fatal failure with unexpected page field data [\#93](https://github.com/zotero/citeproc-rs/issues/93) [[I-bug](https://github.com/zotero/citeproc-rs/labels/I-bug)] [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Error: unknown field `shortTitle`, expected `any CSL variable` [\#92](https://github.com/zotero/citeproc-rs/issues/92) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)] [[I-schema](https://github.com/zotero/citeproc-rs/labels/I-schema)]
- Plaintext output of citations does not handle unicode characters properly [\#91](https://github.com/zotero/citeproc-rs/issues/91) [[I-bug](https://github.com/zotero/citeproc-rs/labels/I-bug)] [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- CompileErrror when initializing the wasm driver [\#84](https://github.com/zotero/citeproc-rs/issues/84) [[I-bug](https://github.com/zotero/citeproc-rs/labels/I-bug)]
- Wasm driver throws string errors [\#25](https://github.com/zotero/citeproc-rs/issues/25) [[I-packaging](https://github.com/zotero/citeproc-rs/labels/I-packaging)]
- Fix npm publishing breakage due to wasm-opt segfault [\#111](https://github.com/zotero/citeproc-rs/pull/111) [[A-ci](https://github.com/zotero/citeproc-rs/labels/A-ci)] [[I-packaging](https://github.com/zotero/citeproc-rs/labels/I-packaging)]
- Don't exclude the no-modules target from NPM builds [\#81](https://github.com/zotero/citeproc-rs/pull/81) [[I-packaging](https://github.com/zotero/citeproc-rs/labels/I-packaging)]
- Configure wasm-opt to avoid 'exported global cannot be mutable' [\#66](https://github.com/zotero/citeproc-rs/pull/66) [[I-packaging](https://github.com/zotero/citeproc-rs/labels/I-packaging)]

#### Merged pull requests:

- Fix & test overflowing integer parsing [\#95](https://github.com/zotero/citeproc-rs/pull/95) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)]
- Downgrade to wasm-bindgen 0.2.62 [\#86](https://github.com/zotero/citeproc-rs/pull/86)
- Use Rust 1.43 and fix wasm mutable globals [\#85](https://github.com/zotero/citeproc-rs/pull/85) [[A-ci](https://github.com/zotero/citeproc-rs/labels/A-ci)]
- Build the no-modules target for wasm [\#73](https://github.com/zotero/citeproc-rs/pull/73)
- Run Rust library test suite as well as integration tests [\#71](https://github.com/zotero/citeproc-rs/pull/71) [[A-core](https://github.com/zotero/citeproc-rs/labels/A-core)] [[A-ci](https://github.com/zotero/citeproc-rs/labels/A-ci)]

## [wasm-canary](https://github.com/zotero/citeproc-rs/tree/wasm-v0.0.0) (2020-09-19)

[Full Changelog](https://github.com/zotero/citeproc-rs/commit/wasm-v0.0.0)

This marks the first time `@citeproc-rs/wasm` was published to NPM, but it was
only a canary release. Everything after that is summarised above as "v0.1.0"
but in fact there were many canary releases in between.


\* *This Changelog was automatically generated by [github_changelog_generator](https://github.com/github-changelog-generator/github-changelog-generator)*
