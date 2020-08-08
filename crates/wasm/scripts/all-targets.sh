#!/bin/sh

# default location is called 'dist' i.e. distribution
DEST=${1:-dist}

# Run this script from the crates/wasm folder.


# What's going on here?
#
# It's a replacement while we wait on https://github.com/rustwasm/wasm-pack/pull/705
#
# Webpack knows what its target is. If you are using it (or similar), which
# entry point in package.json to select will be determined by this target.
# There are fallbacks, so it tries "browser", "module", and then "main".
# Node.js, on the other hand, when executing require('...'), simply looks for
# "main" and loads the file it points to.
#
# For us:
# - "main" points to _cjs, which uses Node.js-specific features to load a raw
#   binary file, and load it as WASM. The JS modules are also in CommonJS
#   (`module.exports = {}`) format.
# - "browser" points to _esm, which uses window and other browser-based
#   things to load wasm. The JS modules are in ES Modules (`export { blah }`)
#   format, which WebPack et al can understand.
#
# Each of the methods/formats is written out for us by wasm-pack. What about
# the "web" (i.e. "can be loaded by a browser with a script tag") format? That
# one is included in the `_web` folder, but it does not need an entry in package.json!
# 
# The PR linked above uses different folder names, but these ones accurately
# describe why we need different builds in different environments.

mkdir -p pkg-scratch
mkdir -p pkg

## e.g. build into pkg-scratch/target; move files to $DEST/_target/
target() {
  TARGET=$1
  OUT=$2
  wasm-pack build --release --out-name citeproc_rs_wasm --scope citeproc-rs --target $TARGET --out-dir pkg-scratch/$TARGET
  mkdir -p $DEST/$OUT
  cp pkg-scratch/$TARGET/citeproc_rs_wasm* $DEST/$OUT/
}

target nodejs _cjs
target browser _esm
target web _web
cp scripts/model-package.json $DEST/package.json

cp pkg-scratch/browser/README.md $DEST/

