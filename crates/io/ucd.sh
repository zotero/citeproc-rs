VERSION=12.1.0

(mkdir -p /tmp/ucd && \
cd /tmp/ucd && \
curl -LO https://www.unicode.org/Public/zipped/$VERSION/UCD.zip && \
unzip -o UCD.zip && \
grep '<super>' UnicodeData.txt > Superscript.txt && \
grep '<sub>' UnicodeData.txt > Subscript.txt)

