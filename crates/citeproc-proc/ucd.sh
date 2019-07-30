# cargo install --force ucd-generate

(mkdir -p /tmp/ucd-11.0.0 && \
cd /tmp/ucd-11.0.0 && \
curl -LO https://www.unicode.org/Public/zipped/11.0.0/UCD.zip && \
unzip UCD.zip)
mkdir -p src/input/unicode/
ucd-generate script /tmp/ucd-11.0.0 --include Common,Latin,Cyrillic --trie-set > src/input/unicode/script.rs

