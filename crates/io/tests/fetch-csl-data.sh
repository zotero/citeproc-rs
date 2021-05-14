#!/usr/bin/env bash
set -euxo pipefail
URL="https://raw.githubusercontent.com/citation-style-language/schema/master/schemas/input/csl-data.json"

curl -Lo json/csl-data.json "$URL"
