-- This Source Code Form is subject to the terms of the Mozilla Public
-- License, v. 2.0. If a copy of the MPL was not distributed with this
-- file, You can obtain one at http://mozilla.org/MPL/2.0/.
--
-- Copyright © 2019 Corporation for Digital Scholarship

local pandoc = require("pandoc")
local json = require("json")
local io = require("io")

function attr_as_json(attr)
  local identifier = attr.identifier
  local classes = attr.classes
  local kvs = {}
  for i,kv in ipairs(attr.attributes) do
    kvs[i] = { kv[1], kv[2] }
  end
  return {identifier, classes, kvs}
end

function to_pandoc_json(inline)
  local obj = { t = inline.t }
  local tag = inline.tag;
  if inline.tag == "Space" or tag == "SoftBreak" or tag == "LineBreak" then
    return obj
  elseif inline.tag == "Str" then
    obj.c = inline.text
    return obj
  elseif tag == "Emph" or tag == "Strong" or tag == "Strikeout" or tag == "Superscript" or tag == "Subscript" or tag == "SmallCaps" then
    obj.c = inline.content:map(to_pandoc_json)
  elseif tag == "Span" then
    local inls = inline.content:map(to_pandoc_json)
    obj.c = { attr_as_json(inline.attr), inls }
  elseif tag == "Code" then
    obj.c = { attr_as_json(inline.attr), inline.text }
  elseif tag == "Quoted" then
    obj.c = { inline.quotetype, inline.content:map(to_pandoc_json) }
  elseif tag == "Math" then
    obj.c = { inline.mathtype, inline.text }
  elseif tag == "Link" or tag == "Image" then
    obj.c = { attr_as_json(inline.attr), inline.content:map(to_pandoc_json), {inline.target, ""} }
  elseif tag == "RawInline" then
    obj.c = { inline.format, inline.text }
  else
    -- basically if tag == "Note" or tag == "Cite"
    print("pandoc element not supported: " + tag)
    os.exit(1)
  end
  return obj
end

function parse_ordinary(refr, field)
  local content = refr[field]
  if content ~= nil then
    -- TODO: do not allow even parsing [^notes], [@citations] and some other select markdown features
    -- so that you can use more special chars by default.
    local parsed = pandoc.read(content, "markdown")
    if parsed ~= nil then
      local inlines = pandoc.utils.blocks_to_inlines(parsed.blocks)
      -- blocks_to_inlines returns a Lua table, not a pandoc.List.
      local jsonified = {}
      for i, inl in ipairs(inlines) do
        jsonified[i] = to_pandoc_json(inl)
      end
      refr[field] = jsonified
    end
  end
end

local ordinary_fields = {
  -- these should probably be parsed as Blocks
  "annote",
  "note",
  "abstract",

  -- these must be verbatim, so don't parse them
  -- "URL",
  -- "PMCID",
  -- "PMID",
  -- "ISBN",
  -- "ISSN",
  -- "DOI",

  "archive",
  "archive-location",
  "archive-place",
  "authority",
  "call-number",
  "citation-label",
  "collection-title",
  "container-title",
  "container-title-short",
  "dimensions",
  "event",
  "event-place",
  "genre",
  "keyword",
  "medium",
  "original-publisher",
  "original-publisher-place",
  "original-title",
  "publisher",
  "publisher-place",
  "references",
  "reviewed-title",
  "scale",
  "section",
  "source",
  "status",
  "title",
  "title-short",
  "version",

  -- virtual-only variables
  "year-suffix",

  -- CSL-M
  "available-date",
  "volume-title",
  "committee",
  "document-name",
  "gazette-flag",

  -- CSL-M verbatim
  -- "language",
  -- "jurisdiction",

  -- virtual-only variables
  -- "hereinafter",

}

function preprocess(library_path)
  local file = assert(io.open(library_path, "r"))
  local library_json = file:read("*all")
  local library = assert(json.decode(library_json))

  for i,refr in ipairs(library) do
    for i,field in ipairs(ordinary_fields) do
      parse_ordinary(refr, field)
    end
  end

  local output = {}
  output["pandoc-api-version"] = PANDOC_API_VERSION
  output["pandoc-csl-json"] = library
  io.stdout:write(json.encode(output))
end

function get_meta_string(meta, key)
  local value = meta["bibliography"]
  if value == nil then
    return nil
  elseif type(value) == "string" then
    return value
  elseif value.tag == "MetaInlines" or value.tag == "RawInlines" then
    return pandoc.utils.stringify(value)
  end
  return nil
end

function Meta(meta)
  local library_path = get_meta_string(meta, "bibliography")
  if library_path ~= nil then
    preprocess(library_path)
    os.exit(0)
  else
    os.exit(1)
  end
end

