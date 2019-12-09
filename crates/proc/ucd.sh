# cargo install --force ucd-generate
VERSION=12.1.0

PROC=$(pwd)

mkdir -p /tmp/ucd && \
cd /tmp/ucd && \
curl -LO https://www.unicode.org/Public/zipped/$VERSION/UCD.zip && \
unzip -o UCD.zip && \
mkdir -p $PROC/src/unicode && \
ucd-generate script /tmp/ucd --include Common,Latin,Cyrillic --trie-set > $PROC/src/unicode/script.rs

