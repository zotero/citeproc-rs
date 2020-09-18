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

For Node.js or Webpack, simply import the package as normal. Typescript 
definitions are provided, though parts of the API that cannot have 
auto-generated type definitions are alluded to in doc comments with an 
accompanying type you can import.

```
// Node.js
const { Driver } = require("@citeproc-rs/wasm");

// Webpack, anything using compiled ES Modules
import { Driver } from "@citeproc-rs/wasm";
```

To directly import it in a (modern) web browser, you must:

1. Make the `_web` subdirectory of the published NPM package available in a 
   content directory on your webserver, or use a CDN like [unpkg](unpkg.com).
2. Include a `<script type="module">` tag on your page, like so:

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


### 1. Creating a driver instance

First, create a driver. Note that for now, you must also call `.free()` on the 
Driver when you are finished with it to deallocate its memory, but [there is a TC39 
proposal](https://rustwasm.github.io/docs/wasm-bindgen/reference/weak-references.html) 
in the implementation phase that will make this unnecessary.

A driver needs an XML style string, a fetcher (below), and an output format 
(one of `"html"`, `"rtf"` or `"plain"`).

```javascript
let driver = Driver.new(cslStyleTextAsXML, fetcher, "html");
// ... use the driver ...
driver.free()
```

The library parses and validates the CSL style input. Any validation errors are 
reported, with line/column positions, the text at that location, a descriptive 
and useful message (only in English at the moment) and sometimes even a hint 
for how to fix it. This is thrown as an error, which you can catch in a `try {} 
catch (e) {}` block.

#### Fetcher

There are hundreds of locales, and the locales you need change depending on the 
references that are active in your document, so the procedure for retrieving 
one is asynchronous to allow for fetching one over HTTP. There's not much more 
to it than this:

```javascript
class Fetcher {
    async fetchLocale(lang) {
        return fetch("https://some-cdn-with-locales.com/locales-${lang}.xml")
            .then(res => res.text());

        // or just
        // return "<locale> ... </locale>";
        // return LOCALES_PRELOADED[lang];

        // or if you don't support locales other than the bundled en-US!
        // return null;
    }
}

let fetcher = new Fetcher(); // Pass to Driver.new()
```

Unless you don't have `async` syntax, in which case, return a `Promise` 
directly, e.g. `return Promise.resolve("<locale> ... </locale>")`.

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

When you do insert a reference, it may have locale information in it. This 
should be done after updating the references, so any new locales can be 
fetched.

```javascript
// May call your Fetcher instance.
await driver.fetchAll();
```

#### Citation Clusters and their Cites

A document consists of a series of clusters, each with a series of cites. Each 
cluster has an `id`, which is any integer except zero.

```javascript
// initClusters is like booting up an existing document and getting up to speed
driver.initClusters([
    { id: 1, cites: [ {id: "citekey"} ] },
    { id: 2, cites: [ {id: "citekey", locator: "56", label: "page" } ] },
]);
// Update or insert any one of them like so
driver.insertCluster({ id: 1, cites: [ { id: "updated_citekey" } ] });
driver.insertCluster({ id: 3, cites: [ { id: "new_cluster_here" } ] });
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
driver.setClusterOrder([ { id: 1, note: 1 }, { id: 2, note: 4 } ]);
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

### 3. Call `driver.batchedUpdates()` and apply the diff

This gets you a diff to apply to your document UI. It includes both clusters 
that have changed, and bibliography entries that have changed.

```javascript
import { UpdateSummary } from "@citeproc-rs/wasm"; // typescript users, annotate with this

let diff = driver.batchedUpdates();

// apply to the UI
diff.clusters.forEach(changedCluster => {
    let [id, html] = changedCluster;
    myDocument.updateCluster(id, html);
});
diff.bibliography.entryIds.forEach(citekey => {
    let html = diff.updatedEntries[citekey];
    myDocument.updateBibEntry(citekey, html);
});
```

Note, for some intuition, if you call `batchedUpdates()` again immediately, the 
diff will be empty.


### Preview citation clusters

Sometimes, a user wants to see how a cluster will look while they are editing 
it, before confirming the change.

```javascript
let cites = [ { id: "citekey", locator: "45" }, { ... } ];
let positions = [ ... before, { id: 0, note: 34 }, ... after ];
let preview = driver.previewCitationCluster(cites, positions, "html");
```

The positions array is exactly like a call to `setClusterOrder`, except exactly 
one of the positions has an id of 0. This could either:

- Replace an existing cluster's position, and preview a cluster replacement; or
- Represent the position a cluster is hypothetically inserted.

If you passed only one position, it would be like previewing an operation like 
"delete the entire document and replace it with this one cluster". **That would 
mean you would never see "ibid" in a preview.** So for maximum utility, 
assemble the positions array as you would a call to `setClusterOrder` with 
exactly the operation you're previewing applied.



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

// Re-hydrate the entire document
driver.resetReferences(allReferences);
driver.initClusters(allNotes.map(fn => fn.cluster));
driver.setClusterOrder(allNotes.map(fn => { id: note.cluster.id, note: note.number }));

// Build every cluster, only after the driver knows about all of them
allNotes.forEach(fn => {
    fn.renderedHtml = driver.builtCluster(fn.cluster.id);
});

let bibliography = driver.makeBibliography();

// Drain the update queue, so the driver knows you're up to date and won't send 
// you a whole-document diff
driver.drain();

// Update the UI
updateUserInterface(allNotes, bibliography);
```

