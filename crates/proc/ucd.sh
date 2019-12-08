# cargo install --force ucd-generate
VERSION=12.1.0

(mkdir -p /tmp/ucd && \
cd /tmp/ucd && \
curl -LO https://www.unicode.org/Public/zipped/$VERSION/UCD.zip && \
unzip -o UCD.zip && \
mkdir -p src/input/unicode/ && \
ucd-generate script /tmp/ucd --include Common,Latin,Cyrillic --trie-set > src/input/unicode/script.rs)

