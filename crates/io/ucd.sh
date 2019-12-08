VERSION=12.1.0

(mkdir -p /tmp/ucd-$VERSION && \
cd /tmp/ucd-$VERSION && \
curl -LO https://www.unicode.org/Public/zipped/$VERSION/UCD.zip && \
unzip -o UCD.zip && \
grep '<super>' UnicodeData.txt > Superscript.txt && \
grep '<sub>' UnicodeData.txt > Subscript.txt)

