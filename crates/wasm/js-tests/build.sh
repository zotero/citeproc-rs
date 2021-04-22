#!/usr/bin/env bash
set -euo pipefail

# jest can only load an environment from node_modules
mkdir -p node_modules/webdriver-environment
cp webdriver-environment.js node_modules/webdriver-environment/index.js

# yarn will erase unrecognized node_modules if you run `yarn install` after `yarn build`
# so
# $ yarn
# $ yarn build
# $ yarn test
# in that order
../scripts/npm-pkg-config.sh --dev --targets nodejs --dest ./node_modules/@citeproc-rs/wasm
../scripts/npm-pkg-config.sh --dev --targets no-modules --dest ./browser/pkg-nomod
