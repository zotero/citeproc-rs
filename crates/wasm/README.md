# `@citeproc-rs/wasm`

This is a front-end to 
[`citeproc-rs`](https://github.com/cormacrelf/citeproc-rs), a citation 
processor written in Rust and compiled to WebAssembly.

It contains builds appropriate for:

- Node.js
- Browsers, using a bundler like Webpack.js
- Browsers directly importing an ES Module from a webserver

## Installation / Release channels

There are two release channels:

**Stable** is each versioned release. (*At the time of writing, there are no 
versioned releases.*) Install with:

```sh
yarn add @citeproc-rs/wasm
```

**Canary** tracks the master branch [on 
GitHub](https://github.com/cormacrelf/citeproc-rs). Its version numbers follow 
the format `0.0.0-canary-GIT_COMMIT_SHA`, so version ranges in your 
`package.json` are not meaningful. But you can install the latest one with:

```sh
yarn add @citeproc-rs/wasm@canary
# alternatively, a specific commit
yarn add @citeproc-rs/wasm@0.0.0-canary-COMMIT_SHA
```

If you use NPM, replace `yarn add` with `npm install`.

### Including in your project

For Node.js, simply import the package as normal. Typescript definitions are 
provided, though parts of the API that cannot have auto-generated type 
definitions are alluded to in doc comments with an accompanying type you can 
import.

```
// Node.js
const { Driver } = require("@citeproc-rs/wasm");
```

##### Microsoft Edge

Note the caveats in around Microsoft Edge's TextEncoder/TextDecoder support in 
[the wasm-bindgen 
tutorial](https://rustwasm.github.io/docs/wasm-bindgen/examples/hello-world.html).

#### Using Webpack

When loading on the web, for technical reasons and because the compiled 
WebAssembly is large, you must load the package asynchronously. Webpack comes 
with the ability to import packages asynchronously like so:

```javascript
// Webpack
import("@citeproc-rs/wasm")
    .then(go)
    .catch(console.error);

function go(wasm) {
    const { Driver } = wasm;
    // use Driver
}
```

When you do this, your code will trigger a download (and streaming parse) of 
the binary, and when that is complete, your `go` function will be called. The 
download can of course be cached if your web server is set up correctly, making 
the whole process very quick.

You can use the regular-import Driver as a TypeScript type anywhere, just don't 
use it to call `new Driver()`.

##### React

If you're writing a React app, you may wish to use `React.lazy` like so:

```typescript
// App.tsx
import React, { Suspense } from "react";
const AsyncCiteprocEnabledComponent = React.lazy(async () => {
    await import("@citeproc-rs/wasm");
    return await import("./CiteprocEnabledComponent");
});
const App = () => (
    <Suspense
        fallback={<div>Loading citation formatting engine...</div>}>
        <AsyncCiteprocEnabledComponent />
    </Suspense>
);

// CiteprocEnabledComponent
import { Driver } from "@citeproc-rs/wasm";
// ...
```

#### Importing it in a script tag (`web` target)

To directly import it without a bundler in a (modern) web browser with ES 
modules support, the procedure is different. You must:

1. Make the `_web` subdirectory of the published NPM package available in a 
   content directory on your webserver, or use a CDN like [unpkg](unpkg.com).
2. Include a `<script type="module">` tag in your page's `<body>`, like so:

```html
<script type="module">
    import init, { Driver } from './path/to/_web/citeproc_rs_wasm.js';
    async function run() {
        await init();
        // use Driver
    }
    run()
</script>
```

**Careful**: This method does not ensure the package is loaded only once. If 
you call init again, it will invalidate any previous Drivers you created.

#### Importing it in a script tag (`no-modules` target)

This is *based on* the [wasm-bindgen guide 
entry](https://rustwasm.github.io/docs/wasm-bindgen/examples/without-a-bundler.html?highlight=no-modules#using-the-older---target-no-modules), 
noting the caveats. You will, similarly to the `web` target, need to make the 
contents of the `_no_modules` subdirectory of the published NPM package 
available on a webserver or via a CDN. But it has **ONE ADDITIONAL FILE** to 
import via a script tag.

**Careful**: This method does not ensure the package is loaded only once. If 
you call init again, it will invalidate any previous Drivers you created.

```
<html>
  <head>
    <meta content="text/html;charset=utf-8" http-equiv="Content-Type"/>
  </head>
  <body>
    <!-- Include these TWO JS files -->
    <script src='path/to/@citeproc-rs/wasm/_no_modules/citeproc_rs_wasm_include.js'></script>
    <script src='path/to/@citeproc-rs/wasm/_no_modules/citeproc_rs_wasm.js'></script>

    <script>
      // Like with the `--target web` output the exports are immediately
      // available but they won't work until we initialize the module. Unlike
      // `--target web`, however, the globals are all stored on a
      // `wasm_bindgen` global. The global itself is the initialization
      // function and then the properties of the global are all the exported
      // functions.
      //
      // Note that the name `wasm_bindgen` will at some point be configurable with the
      // `--no-modules-global` CLI flag (https://github.com/rustwasm/wasm-pack/issues/729)
      const { Driver } = wasm_bindgen;

      async function run() {
        // Note the _bg.wasm ending
        await wasm_bindgen('path/to/@citeproc-rs/wasm/_no_modules/citeproc_rs_wasm_bg.wasm');

        // Use Driver
      }

      run();

    </script>
  </body>
</html>
```

#### Usage in Zotero

There is a special build for Zotero and the legacy Firefox ESR extensions API, 
which wants a CommonJS module format but without the Node.js `fs` APIs, and 
`no-modules`' loading mechanisms but without the use of `window` as a global as 
it doesn't exist. The files are in the `_zotero` directory of the NPM package. 
Usage is essentially the same as no-modules; you'll need all three files:

* `@citeproc-rs/wasm/_zotero/citeproc_rs_wasm_include.js`
* `@citeproc-rs/wasm/_zotero/citeproc_rs_wasm.js`
* `@citeproc-rs/wasm/_zotero/citeproc_rs_wasm_bg.wasm`

Apart from the CommonJS shims, the main difference is that the API will be 
loaded onto the `Zotero.CiteprocRs` object, in order for it all to be linked 
together.

**Careful**: This method does not ensure the package is loaded only once. If 
you call `initWasmModule` again, it will invalidate any previous Drivers you 
created.


```javascript
require("citeproc_rs_wasm_include");
const initWasmModule = require("citeproc_rs_wasm");
const wasmBinaryPromise = Zotero.HTTP
    .request('GET',
             'resource://zotero/citeproc_rs_wasm_bg.wasm',
             { responseType: "arraybuffer" })
    .then(xhr => xhr.response);
await initWasmModule(wasmBinaryPromise);

let driver;
try {
    driver = new Zotero.CiteprocRs.Driver({...});
} catch (e) {
    if (e instanceof Zotero.CiteprocRs.CslStyleError) {
        // ...
    }
}
```

## Usage


### Overview

The basic pattern of interactive use is:

1. Create a driver instance with your style
2. Edit the references or the citation clusters as you please
3. **Call `driver.batchedUpdates()`**
4. Apply the updates to your document (e.g. GUI)
5. Go to step 2 when a user makes a change

Step three is the important one. Each time you edit a cluster or a reference, 
it is common for only one or two visible modifications to result. Therefore, 
the driver only gives you those clusters or bibliography entries that have 
changed, or have been caused to change by an edit elsewhere. You can submit any 
number of edits between each call.

The API also allows for non-interactive use. See below.

### Error handling

Many Driver methods can throw errors.

If you want to handle the errors from this library specifically, you can, and
this is mainly useful for showing style parse or validation errors. Some error
types have structured data attached to them.

```typescript
try {
    let driver = new Driver({ ... });
    // do stuff with driver
} catch (error) {
    if (error instanceof CslStyleError) {
        console.error("Could not parse CSL, error:", error);
    } else if (error instanceof CiteprocRsDriverError) {
        console.error("Error in usage of Driver", error);
    } else if (error instanceof CiteprocRsError) {
        // CslStyleError and CiteprocRsDriverError are both subclasses of
        // CiteprocRsError, so this branch would catch them too had they not
        // been checked already.
        //
        // There may be errors that are not a subclass, but directly an
        // instance of CitprocRsError, so for completeness one should test for
        // this too.
        console.error("Catch-all error", error);
    } else {
        throw error;
    }
} finally {
    // Driver is only undefined if `new Driver` threw an error.
    if (driver) {
        driver.free()
    }
}
```

The error types must unfortunately be global exports, on window/global/self.

### 1. Creating a driver instance

First, create a driver. Note that for now, you must also call `.free()` on the 
Driver when you are finished with it to deallocate its memory, but [there is a TC39 
proposal](https://rustwasm.github.io/docs/wasm-bindgen/reference/weak-references.html) 
in the implementation phase that will make this unnecessary.

A driver needs at least an XML style string, a fetcher (below), and an output 
format (one of `"html"`, `"rtf"` or `"plain"`).

```javascript
let fetcher =  ...; // see below
let driver = new Driver({
    style: "<style version=\"1.0\" class=\"note\" ... > ... </style>",
    format: "html", // optional, html is the default
    formatOptions: { // optional
        linkAnchors: true, // optional, default true
    },
    localeOverride: "de-DE", // optional, like setting default-locale on the style
    // bibliographyNoSort: true // disables sorting on the bibliography
    fetcher,
});
// Fetch the chain of locale files required to use the specified locale
await driver.fetchLocales();
// ... use the driver ...
driver.free()
```

The library parses and validates the CSL style input. Any validation errors are 
reported, with byte offsets to find the CSL fragment responsible, a descriptive 
and useful message (in English) and sometimes even a hint for how to fix it. 
See [Error Handling](#error-handling) for how to access this.

#### Fetcher

There are hundreds of locales, and the locales you need depend on the style
default, any overrides and any fallback locales defined, so the procedure for
retrieving one is asynchronous to allow for fetching one over HTTP. There's not
much more to it than this:

```javascript
class Fetcher {
    async fetchLocale(lang) {
        return await fetch("https://some-cdn-with-locales.com/locales-${lang}.xml")
            .then(res => res.text());

        // or just
        // return "<locale> ... </locale>";
        // return LOCALES_PRELOADED[lang];

        // or if you don't support locales other than the bundled en-US!
        // return null;
    }
}

let fetcher = new Fetcher();
let driver = new Driver({ ..., fetcher });
// Make sure you actually fetch them!
await driver.fetchLocales();
```

Unless you don't have `async` syntax, in which case, return a `Promise` 
directly, e.g. `return Promise.resolve("<locale> ... </locale>")`.

Declining to provide a locale fetcher in `new Driver` or forgetting to call
`await driver.fetchLocales()` results in use of the bundled `en-US` locale. You
should also never attempt to use the driver instance while it is fetching locales.

### 2. Edit the references or the citation clusters

#### References

You can insert a reference like so. This is a [CSL-JSON][schema] object.

[schema]: https://github.com/citation-style-language/schema

```javascript
driver.insertReference({ id: "citekey", type: "book", title: "Title" });
driver.insertReferences([ ... many references ... ]);
driver.resetReferences([ ... deletes any others ... ]);
driver.removeReference("citekey");
```

#### Citation Clusters and their Cites

A document consists of a series of clusters, each with a series of cites. Each 
cluster has an `id`, which is any old string.

```javascript
// initClusters is like booting up an existing document and getting up to speed
driver.initClusters([
    { id: "one", cites: [ {id: "citekey"} ] },
    { id: "two", cites: [ {id: "citekey", locator: "56", label: "page" } ] },
]);
// Update or insert any one of them like so
driver.insertCluster({ id: "one", cites: [ { id: "updated_citekey" } ] });
// (You can use `driver.randomClusterId()` to generate a new one at random.)
let three = driver.randomClusterId();
driver.insertCluster({ id: three, cites: [ { id: "new_cluster_here" } ] });
```

These clusters do not contain position information, so reordering is a separate 
procedure. **Without calling setClusterOrder, the driver considers the document 
to be empty.**

So, `setClusterOrder` expresses the ordering of the clusters within the 
document. Each one in the document should appear in this list. You can skip 
note numbers, which means there were non-citing footnotes in between. Omitting 
`note` means it's an in-text reference. Note numbers must be monotonic, but you 
can have more than one cluster in the same footnote.

```javascript
driver.setClusterOrder([ { id: "one", note: 1 }, { id: "two", note: 4 } ]);
```

You will notice that if an interactive user cuts and pastes a paragraph 
containing citation clusters, the whole reordering operation can be expressed 
in two calls, one after the cut (with some clusters omitted) and one after the 
paste (with those same clusters placed somewhere else). No calls to 
`insertCluster` need be made.

#### Uncited items

Sometimes a user wishes to include references in the bibliography even though 
they are not mentioned in a citation anywhere in the document.

```javascript
driver.includeUncited("None"); // Default
driver.includeUncited("All");
driver.includeUncited({ Specific: ["citekeyA", "citekeyB"] });
```

The "All" is based on which references your driver knows about. If you have
this set to "All", simply calling `driver.insertReference()` with a new
reference ID will result in an entry being added to the bibliography. Entries
in Specific mode do not have to exist when they are provided here; they can be,
for instance, the citekeys of collection of references in a reference library
which are subsequently provided in full to the driver, at which point they
appear in the bibliography, but not items from elsewhere in the library.

### 3. Call `driver.batchedUpdates()` and apply the diff

This gets you a diff to apply to your document UI. It includes both clusters 
that have changed, and bibliography entries that have changed.

```javascript
// Get the diff since last time batchedUpdates, fullRender or drain was called.
let diff = driver.batchedUpdates();

// apply cluster changes to the UI.

// ("myDocument" is an imaginary API.)

for (let changedCluster of diff.clusters) {
    let [id, html] = changedCluster;
    myDocument.updateCluster(id, html);
}

// Null? No change to the bibliography.
if (diff.bibliography != null) {
    let bib = diff.bibliography;
    // Save the entries that have actually changed
    for (let key of Object.keys(bib.updatedEntries)) {
        let rendered = bib.updatedEntries[key];
        myDocument.updateBibEntry(key, rendered);
    }
    // entryIds is the full list of entries in the bibliography.
    // If a citekey isn't in there, it should be removed.
    // It is non-null when it has changed.
    if (bib.entryIds != null) {
        myDocument.setBibliographyOrder(bib.entryIds);
    }
}
```

Note, for some intuition, if you call `batchedUpdates()` again immediately, the 
diff will be empty.

### Bibliographies

Beyond the interactive batchedUpdates method, there are two functions for
producing a bibliography statically.

```javascript
// returns BibliographyMeta, with information about how a library consumer should
// lay out the bibliography. There is a similar API in citeproc-js.
let meta = driver.bibliographyMeta();

// This is an array of BibEntry
let bibliography = driver.makeBibliography();
for (let entry of bibliography) {
    console.log(entry.id, entry.value);
}
```

### Preview citation clusters

Sometimes, a user wants to see how a cluster will look while they are editing 
it, before confirming the change.

```javascript
let cluster = { cites: [ { id: "citekey", locator: "45" }, { ... } ] };
let positions = [ ... before, { note: 34 }, ... after ];
let preview = driver.previewCluster(cluster, positions);
let plainPreview = driver.previewCluster(cluster, positions, "plain");
```

The cluster argument is just a cluster, without an `id` field, since it's
ephemeral. The lack of `id` field is reflected in the `positions` argument as
well.

The positions array is exactly like a call to `setClusterOrder`, except exactly 
one of the positions omits the id field. This could either:

- Replace an existing cluster's position, and preview a cluster replacement; or
- Represent the position a cluster is hypothetically inserted.

If you passed only one position, it would be like previewing an operation like 
"delete the entire document and replace it with this one cluster". **That would 
mean you would never see "ibid" in a preview.** So for maximum utility, 
assemble the positions array as you would a call to `setClusterOrder` with 
exactly the operation you're previewing applied.

The format argument is optional, and works like the format passed to
`new Driver`: one of `"html"`, `"rtf"` or `"plain"`. The driver will use that
instead of its normal output format.


### `AuthorOnly`, `SuppressAuthor` & `Composite`

`@citeproc-rs/wasm` supports these flags on clusters (all 3) and cites (except
`Composite`), in a similar way to `citeproc-js`. See the [`citeproc-js`
documentation on Special Citation
Forms](https://citeproc-js.readthedocs.io/en/latest/running.html#special-citation-forms)
for reference.

```javascript
// only two modes for cites
let citeAO = { id: "jones2006", mode: "AuthorOnly" };
let citeSA = { id: "jones2006", mode: "SuppressAuthor" };

// additional options for clusters
let clusterAO       = { id: "one", cites: [...], mode: "AuthorOnly" };
let clusterSA       = { id: "one", cites: [...], mode: "SuppressAuthor" };
let clusterSA_First = { id: "one", cites: [...], mode: "SuppressAuthor", suppressFirst: 3 };
let clusterC        = { id: "one", cites: [...], mode: "Composite" };
let clusterC_Infix  = { id: "one", cites: [...], mode: "Composite", infix: ", whose book" };
let clusterC_Full   = { id: "one", cites: [...], mode: "Composite", infix: ", whose books", suppressFirst: 0 };
```

It does support one extra option with `SuppressAuthor` and `Composite` on
clusters: `suppressFirst`, which limits the effect to the first N name groups
(or if cite grouping is disabled, first N names). Setting it to 0 means
unlimited.

#### `<intext>` element with `AuthorOnly` etc.

`citeproc-rs` supports the `<intext>` element described in the `citeproc-js`
docs linked above, but it is not enabled by default. It also supports `<intext
and="symbol">` or `and="text"`, which will swap out the last intext layout
delimiter (`<layout delimiter="; ">`) for either the ampersand or the `and`
term.

If you want to use the `<intext>` element in CSL, you may either:

##### Option 1: Add a feature flag to the style wishing to use it

```xml
<style class="in-text">
    <features>
        <feature name="custom-intext" />
    </features>
    ...
</style>
```

AFAIK no other processors support this syntax yet.

##### Option 2: Enable the `custom-intext` feature for all styles via `new Driver`

```javascript
let driver = new Driver({ ..., cslFeatures: ["custom-intext"] });
// ... driver.free();
```

### Non-Interactive use, or re-hydrating a previously created document

If you are working non-interactively, or re-hydrating a previously created 
document for interactive use, you may want to do one pass over all the clusters 
in the document, so that each cluster and bibliography entry reflects the 
correct value.

```javascript
// Get the clusters from your document (example)
let allNotes = myDocument.footnotes.map(fn => {
    return { cluster: getCluster(fn), number: fn.number }
});

// Re-hydrate the entire document based on the reference library and your
// document's clusters
driver.resetReferences(myDocument.allReferences);
driver.initClusters(allNotes.map(fn => fn.cluster));
driver.setClusterOrder(allNotes.map(fn => { id: fn.cluster.id, note: fn.number }));

// Render every cluster and bibliography item.
// It then drains the update queue, leaving the diff empty for the next edit.
// see the FullRender typescript type
let render = driver.fullRender();

// Write out the rendered clusters into the doc
for (let fn of allNotes) {
    fn.renderedHtml = render.allClusters[fn.cluster.id];
}

// Write out the bibliography entries as well
let allBibKeys = render.bibEntries.map(entry => entry.id);
for (let bibEntry of render.bibEntries) {
    myDocument.bibliographyMap[entry.id] = entry.value;
}

// Update your (example) UI
updateUserInterface(allNotes, myDocument, whatever);
```

### `parseStyleMetadata`

Sometimes you want information about a CSL style without actually booting up a
whole driver. One important use case is a dependent style, which can't be used
with `new Driver()` because it doesn't have the ability to render citations on
its own, and is essentially just a container for three pieces of information:

- A journal name
- An independent parent style
- A possible default-locale override

`@citeproc-rs/wasm` provides an API for finding out what's in a CSL style file.

```typescript
let styleMeta = parseStyleMetadata("<style ...> ... </style>");
```

This function can still throw a `CslStyleError`, but this is less likely than
with new Driver() as it will not actually attempt to parse and validate all the
parts of a style. It will throw if the XML is malformed or if the `<info>`
block is too invalid to salvage.

Here's how to use `parseStyleMetadata` to parse and use a dependent style.

```typescript
let dependentStyle = "<style ...> ... </style>";
let meta = parseStyleMetadata(dependentStyle);
let isDependent = meta.info.parent != null;
let parentStyleId = isDependent && meta.info.parent.href;
let localeOverride = meta.defaultLocale;

// ...
let parentStyle = await downloadStyleWithId(parentStyleId);
let driver = new Driver({
    style: parentStyle,
    localeOverride,
    ...
});
await driver.fetchLocales();

// Here you might also want to know if the style can render a bibliography or not
let parentMeta = parseStyleMetadata(parentStyle);
if (parentMeta.independentMeta.hasBibliography) {
    let bib = driver.makeBibliography();
    // ...
}

// ...
driver.free();
```

### `setOutputFormat` and `setStyle`

If you wish to change the output format of the entire driver, you can use 
`setOutputFormat(format, formatOptions)`. The format is a string, one of `"html" | 
"rtf" | "plain"` just like the `new Driver` method. The options is an optional
argument with the same value as `formatOptions` in `new Driver`.

`setStyle(xmlString)` will change the CSL style used by the driver.

Both of these methods will require throwing out almost all cached computation,
so use sparingly.

If you need to render a preview in a different format, there is an argument on
`previewCluster` for doing just that. It does not throw out all the
computation. `citeproc-rs`' disambiguation procedures do take formatting into
account, so `<i>Title</i>` can be distinct from `<b>Title</b>` in HTML and RTF,
but not if the whole driver's output format is `"plain"`, since they both look
identical in plain text. `previewCluster` will simply translate the formatting
into another format, without re-computing all the disambiguation.
