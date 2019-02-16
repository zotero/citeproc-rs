pandoc-library.json: preproc.json
	echo "" | pandoc --lua-filter preproc.lua --metadata bibliography="$<" > $@
all: pandoc-library.json
	pandoc -F citeproc-rs --metadata bibliography="$<"

