#!/bin/bash

# Run this script from the crates/wasm folder.

CLEAR='\033[0m'
RED='\033[0;31m'

function usage() {
  if [ -n "$1" ]; then
    echo -e "${RED}👉 $1${CLEAR}\n";
  fi
  echo "Usage: $0"
  echo "  --dest DEST              Write output to this folder (rm -rf/deletes it first!)"
  echo "                           The default is './dist'"
  echo "  --canary-sha SHA_COMMIT  Set version to '0.0.0-canary-\$SHA_COMMIT'"
  echo "  --set-version VERSION    Use this version specifically"
  echo "  --cargo-version          Use the version from Cargo.toml"
  echo "  --set-name NAME          Use this package name"
  echo "  --package-only           Don't rebuild, just rewrite the package.json"
  echo "  --github-packages        Configure for publishing to GitHub packages"
  echo
  exit 1
}

DEST=./dist
SET_VERSION=
USE_CARGO_VERSION=
SET_NAME=
PACKAGE_ONLY=
GITHUB_PACKAGES=
CANARY_SHA=

# parse params
while [[ "$#" > 0 ]]; do case $1 in
  --dest) DEST="$2"; shift;shift;;
  --set-name) SET_NAME="$2";shift;shift;;
  --canary-sha) CANARY_SHA="$2";shift;shift;;
  --set-version) SET_VERSION="$2";shift;shift;;
  --cargo-version) USE_CARGO_VERSION=true; shift;;
  --package-only) PACKAGE_ONLY=true;shift;;
  --github-packages) GITHUB_PACKAGES=true;shift;;
  *) usage "Unknown parameter passed: $1"; shift; shift;;
esac; done

# make sure it doesn't have a v prefix
SET_VERSION=${SET_VERSION#v}

echo "Running: $0 --dest $DEST --cargo-version? ${USE_CARGO_VERSION:-false} --set-version ${SET_VERSION:-<none>} --set-name ${SET_NAME:-<none>}"

# default destination location is called 'dist' i.e. distribution
CARGO_VERSION=$(cargo metadata --format-version 1 --no-deps | jq -r '.packages  | .[] | select(.name=="wasm") | .version')

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

if [ -z "$PACKAGE_ONLY" ]; then
  mkdir -p pkg-scratch
  rm -rf $DEST
  mkdir -p $DEST

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
fi


cp pkg-scratch/browser/README.md $DEST/
# cp scripts/model-package.json $DEST/package.json

TARGET_VERSION=${SET_VERSION:-$CARGO_VERSION}

JQ_ARGS=""
JQ_FILTERS=". "
function append() {
  JQ_ARGS="$JQ_ARGS $1"
  JQ_FILTERS="$JQ_FILTERS | $2"
}

if [ -n "$CANARY_SHA" ]; then
  append "--arg version 0.0.0-canary-$CANARY_SHA" '.version = $version'
elif [ -n "$SET_VERSION" ]; then
  append "--arg version $SET_VERSION" '.version = $version'
elif [ -n "$USE_CARGO_VERSION" ]; then
  append "--arg version $CARGO_VERSION" '.version = $version'
fi

if [ -n "$SET_NAME" ]; then
  append "--arg name $SET_NAME" '.name = $name'
fi

if [ -n "$GITHUB_PACKAGES" ]; then
  append ' ' '.publishConfig = {repository: "https://npm.pkg.github.com"}'
fi

# echo $JQ_ARGS \'$JQ_FILTERS\'

jq $JQ_ARGS "$JQ_FILTERS" < scripts/model-package.json > $DEST/package.json

# cat $DEST/package.json | jq

