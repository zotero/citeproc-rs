set -euxo pipefail

VERSION=13.0.0

mkdir -p /tmp/ucd && \
cd /tmp/ucd && \
curl -LO https://www.unicode.org/Public/zipped/$VERSION/UCD.zip || (echo failed to download $VERSION/UCD.zip && exit 1)
unzip -o UCD.zip || (echo failed to unzip UCD.zip && exit 1)
grep '<super>' UnicodeData.txt > Superscript.txt || (echo failed to write Superscript.txt && exit 1)
grep '<sub>' UnicodeData.txt > Subscript.txt || (echo failed to write Subscript.txt && exit 1)
ucd-generate script /tmp/ucd --include Common,Latin,Cyrillic,Greek,Arabic --trie-set > "$UCD_RUST_OUT_DIR/latin_cyrillic.rs" || exit 1
