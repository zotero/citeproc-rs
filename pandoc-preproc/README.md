## Pandoc pre-processor for CSL-JSON with Markdown syntax

In a nutshell: read fields on CSL-JSON references as Pandoc Markdown, and write 
them out as parsed Inlines instead of strings. Then you can have italics, 
perfectly alternating quotes, and inline $math$ in your titles!

This is mostly a proof of concept at this stage. If it turns out that 
Pandoc-Lua's `pandoc.read` is roughly as fast as fork/exec the `pandoc` binary, 
then there isn't much point doing it in Lua and making installation more 
complicated. My guess is that creating thousands of processes would be pretty 
darn slow in Windows or Cygwin/git-bash/other weird environments, hence this 
program.

Also recall that cite prefixes and suffixes are already parsed by Pandoc, so 
they're good to go already, minus locators.

`citeproc-rs` doesn't currently accept Pandoc CSL-JSON as input!

### Usage:

```sh
echo "" | pandoc --lua-filter preproc.lua --metadata bibliography="XXX.json" > output.json
```

This program can be fairly slow. It involves one "parse markdown document to 
AST" step for every field of every reference. It is probably worthwhile caching 
this step, as for a large library, it might take a few seconds. On an i7-2677M 
in an old MacBook Air, a dump of 187 references is done in about 560ms. Roughly 
100ms of that is JSON reading/writing. You could easily do this with a 
Makefile, which is already a pretty decent way of using Pandoc, especially for 
larger documents:

```Makefile
pandoc-library.json: original-library.json
	echo "" | pandoc --lua-filter preproc.lua --metadata bibliography="$<" > $@
all: pandoc-library.json
	pandoc -F citeproc-rs --metadata bibliography="$<"
```

It might eventually be worth fork/exec-ing this preprocessing step from 
`citeproc-rs` with Makefile was-file-updated behaviour and a filesystem cache. 
It would activate when you pass regular, non-wrapped CSL-JSON to the engine 
running in Pandoc mode. Then nobody would have to remember. You could then 
explicitly opt-in to providing Pandoc CSL-JSON and manage updates yourself.

### What is Pandoc CSL-JSON?

It's not anything official, and probably never will be. Pandoc's JSON AST is 
not really a long-term storage format, it's more of an interchange format for 
external filter programs. So this is the same. Probably don't rely on stable 
output from this tool, treat it as opaque and tied to the citeproc-rs version. 
The output is shaped like so:

```json
{
  "pandoc-api-version": [1, 17, 5, 4],
  "pandoc-csl-json": [
    {
      "id": "citekey",
      "type": "book",
      "title": [
        {"t":"Str","c":"Marvellous"},
        {"t":"Space"},
        {"t":"Str","c":"Springtime"},
        {"t":"Space"},
        {"t":"Str","c":"Recipes"}
      ]
    }
  ]
}
```

That is:

* Everything is under a `{"pandoc-csl-json": [ ... ]}` wrapper, so you can't 
  read it as CSL-JSON by mistake.
* The version of `pandoc-types` used by the hosting Pandoc executable is 
  attached, just like with Pandoc's document AST.
* Most standard fields are represented as arrays of Pandoc Inlines.
* There are verbatim ones, like URL.
* The same processing could *probably* be done for names. But I'm not sure.

Why not make everything a MetaValue? That's a ton of work converting CSL-JSON 
to add all the MetaMap/etc baggage and the parsing overhead on the other side.

