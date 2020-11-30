// We export to some global, because without linking, there is no way for the rust
// code to find these items
// For use in no-modules builds for the browser, which have no linking
// Also because wasm-bindgen is not yet capable of exporting JS items defined here
// to the wasm library consumer, so they also need a way to get at it to check if
// an error is an instanceof CiteprocRsError (eg)

// doExport defined in include.js

let env_global;
if (typeof self !== "undefined") {
    env_global = self;
} else if (typeof global !== "undefined") {
    env_global = global;
} else if (typeof window !== "undefined") {
    env_global = window;
}
if (typeof env_global !== "undefined") {
    doExport(env_global)
}

// Then we do one little commonjs hack
if (typeof module !== "undefined") {
    module.exports = {};
    doExport(module.exports);
}
