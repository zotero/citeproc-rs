// We export to some global, because without linking, there is no way for the rust
// code to find these items
// For use in the _zotero output
// Also because wasm-bindgen is not yet capable of exporting JS items defined here
// to the wasm library consumer, so they also need a way to get at it to check if
// an error is an instanceof CiteprocRsError (eg)
//
// Note that the generated glue code uses CITEPROC_RS_ZOTERO_GLOBAL, which
// needs replacement in the Zotero build script to work.

// doExport defined in include.js

if (typeof Zotero !== "undefined" && typeof Zotero.CiteprocRs !== "undefined") {
    doExport(Zotero.CiteprocRs)
}

// Then we do one little commonjs hack so const { CiteprocRsError } = require("...") works.
if (typeof module !== "undefined") {
    module.exports = {};
    doExport(module.exports);
}
