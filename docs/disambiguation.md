## How to disambiguate

### What is disambiguation?

Let's try to eke out a more detailed definition than the description in the 
spec, which only says: "A cite is ambiguous when it matches multiple 
bibliographic entries" 
[(spec)](https://docs.citationstyles.org/en/stable/specification.html#disambiguation).

Let `reverse(cite)` be a theoretical function

* FROM each rendered cite,
* BACK TO the reference used to render it.

The codomain is all of the references included in the document or bibliography, 
including uncited bibliography entries.

A set of rendered cites is unambiguous iff `reverse` exists, which is to say, 
`reverse` is actually a function. It does not need to be injective or 
surjective; two different cites may be backed by the same reference (that's 
what we have ibids for), and there can be uncited references. In an ambiguous 
document, `reverse` might have multiple outputs for a single cite (and 
therefore not be a function):

> `Doe, 1999.` might point to two different references, possibly by two 
> different Doe, or maybe two different works by the same Doe, or two different 
> works by two different Does. It can happen any number of ways.

We will imagine `reverse` from the perspective of the reader of a document, who 
will be assumed to know, for example, that the same words with different 
brackets around them or in italics would refer to a different type of 
reference. This might have implications for plain-text output modes which 
cannot differentiate between italics and normal text, but that does not 
necessarily need to be implemented.

You might say a *single* cite is ambiguous if it independently causes `reverse` 
not to be a function. This would be the case if the rendered cite could have 
been generated from multiple different references. In fact, a function 
`fn(Cite, [Reference]) -> bool` to determine whether a cite is ambiguous is a 
free function; it does not need to be compared with other cites in the 
document.

> ### Digression
>
> It appears that some implementations are using a very different definition of 
  ambiguous, like `pandoc-citeproc` [(see this issue and related 
  ones)](https://github.com/jgm/pandoc-citeproc/issues/63), which seems to 
  determine ambiguity by comparing outputs of different cites with some 
  normalisations applied. I don't know how `citeproc-js` does it, I haven't 
  ventured that far into the codebase and probably never will.
>
> The phrase "added one by one to all members of a set of ambiguous cites" 
  under step 1 in the spec may have led to some confusion. Ambiguous cites can 
  easily appear on their own if you don't render a bibliography. It appears the 
  spec is trying to say that if you change one cite to make it less ambiguous, 
  you should apply the same change to every cite referencing the same item, so 
  that `Doe 1999a` would appear consistently throughout a document.

If you had two cites to different references which ended up looking identical, 
then by construction they would both be independently ambiguous -- each could 
refer to two references. Both of them would have to be independently 
disambiguated. However, neither _cite_ is actually relevant to the other's 
final result; only the references are. You could say that the two references 
form an ambiguous set, because the way they have been used meant it was not 
possible to discern which was which. The references themselves might have 
fields that are never rendered for a particular style, so they can only be 
determined to be ambiguous in the context of a particular style and situation.

A 'ghost entry' per the spec would be a *cite*, not just a reference, to 
provide exactly this context.

### Implementation

You could implement disambiguation using a procedure that attempts to run 
`reverse`, and takes evasive action if it finds more than one matching 
reference.

1. Start by creating an inverted index of the participating references, such 
   that you can look up `Date(1999)`, `Date(1999, 1)` or `Date(1999, 1, 7)` and 
   get progressively smaller search results, and further narrow the results by 
   taking a set union with another search like `Name(LastName(Doe))`.
2. Collect the variables accessed + rendered in each CiteContext, coerced into 
   the same key format used in the index. Before text-casing or other 
   transforms like ordinals; we're talking pretty raw variable accesses. And if 
   a date block, for example, only shows a year, that will be a broader search, 
   so don't store the whole date, only the parts in use. Names are trickier, 
   but that's the general gist.
3. For each cite, search the index, and if the search turns up more than one 
   matching reference, run progressively more disambiguation on the IR.

'type' and some other variables not actually rendered but used in condition 
checks that impacted the output are a little more difficult, because 
technically that's not a rendering variable:

```xml
<if type="book">
  <text value="(book)" />
</if>
<if type="article-journal">
  <text value="(journal article)" />
</if>
<!-- etc. -->
```

`Doe, 1999 (book)` would not be ambiguous, even if a cite referring to 
`Reference { type: "article-journal", author: "Doe", issued: 1999 }` also 
existed, because if you are familiar with this citation style, they are easily 
discernible.

To implement that, you could use an output equality check on the filtered 
matches. (Maybe excluding reference-independent variables like `locator`, or at 
least ensuring the CiteContext is exactly the same except the reference):

```rust
fn ir_gen(ctx: &CiteContext) -> (IR, Vec<UsedVariable>) { ... }
fn search(vars: &[UsedVariable]) -> Vec<Reference> { ... }
fn hash(ir: &IR) -> i64 { ... }

let (ir, used_vars) = ir_gen(ctx);
let matched_refs = search(&used_vars);
if matched_refs.len() > 1 {
    let narrowed_count = matched_refs
        .iter()
        .filter(|matched| matched != ctx.reference)
        .filter(|matched| {
            let new_ctx = CiteContext {
                reference: matched,
                // ...
            };
            hash(ir_gen(&new_ctx).0) == hash(ir)
        })
        .count();
    // if none of the others match, you're good
}
```

### Ghost entries = ghost cites

Uncited references 'participate in disambiguation' by being given a fake cite 
with `position: "first"` and no locator or affixes, and then being included in 
the above process. It's not clear whether they should be included as 
bibliography entries or as inlines/notes.

### Ibid, subsequent and near-note

All subsequent cite positions have the benefit of the guarantee that the same 
reference ID appears at least once previously in the document, so they can 
simply have disambiguations from the original cite mirrored to any 
variable-rendering elements within them but not perform disambiguation 
themselves. This is helpful because the literal `ibid` doesn't use any 
variables that even *could* be used for disambiguation purposes. But it also 
works for, e.g., `John Doe, supra note 7.` where in note 7 `Doe, 1999` had to 
be disambiguated to `John Doe, 1999`.

### Name disambiguation

`givenname-disambiguation-rule` can also disambiguate names independently of 
the ambiguity of their containing cites. You could create a second inverted 
index of only name-parts, and run a similar procedure.

#### How?

1. The unit of name ambiguity is a single, rendered `PersonName` or `Literal`.
2. The inverted index can be constructed using all individual name parts AND a 
   rendered unit name.
3. If disambiguation adds a name part (e.g. add-givenname), then the new 
   rendered name gets added to the token set.

```
<citation disambiguate-add-givenname="true",
          givenname-disambiguation-rule="all-names">
    <names variable="author">
        <name form="short" initialize-with="." initialize="true" />
    </names>
    <choose>
      <if for-this-example="false">
        <names variable="editor">
            <name form="short" />
        </names>
      </if>
    </choose>
    <date form="long" variable="issued" />
</citation>

Ref: { id: "shortRide",
       author: [{ given: "John", family: "Adams" }]
       issued: { "raw": "1986" } }
Ref: { id: "other",
       author: [{ given: "Jane", family: "Adams" }]
       issued: { "raw": "1986" } }

Name elements:
   Name(<name form="short" initialize-with="." initialize="true" />)
   => TOKENS("ADAMS", "J. ADAMS", "JOHN ADAMS")
   => TOKENS("ADAMS", "J. ADAMS", "JANE ADAMS")

   Name(<name form="short" />)
   => Tokens("ADAMS", "JOHN ADAMS")
   => Tokens("ADAMS", "JANE ADAMS")

Inverted index:
   "ADAMS" => Set("shortRide", "other")
   "J. ADAMS" => Set("shortRide", "other")
   "JOHN ADAMS" => Set("shortRide")
   "JANE ADAMS" => Set("other")
   Date(1986, 0, 0) => Set("shortRide", "other")

1st pass: "Adams, 1986" => either
2nd pass: "J. Adams, 1986" => either
2nd pass: "John Adams, 1986" => Set("shortRide")
```

For sort-separator == delimiter.

```
<citation disambiguate-add-givenname="true",
          givenname-disambiguation-rule="all-names">
    <names variable="author">
        <name prefix="[" suffix="]" form="short"
              name-as-sort-order="all" et-al-min="4"
              initialize-with="." initialize="true" />
    </names>
</citation>

Ref: { id: "one",
       author: [ { given: "John", family: "Adams" } ] }
Ref: { id: "two",
       author: [ { given: "Jane", family: "Adams" } ] }
Ref: { id: "three",
       author: [ { given: "John", family: "Adams" }
               , { given: "Adams", family: "John" } ] }

Names elements:
   (the only one)
       one => TOKENS("Adams", "Adams, John")
       two => TOKENS("Adams", "Adams, Jane")
       three => TOKENS("Adams, John", "Adams, John, John, Adams")

Inverted index:
   "Adams"       => Set(one, two)
   "Adams, John" => Set(one, three)
   "Adams, Jane" => Set(two)
   "Adams, John, John, Adams" => Set(three)
   Date(1986, 0, 0) => Set("shortRide", "other")

Ref(one):
    1st pass:
        "Adams"       => Set(one, two)
    2nd pass:
        "Adams, John" => Set(one, three)
    Still ambiguous

Ref(two):
    1st pass:
        "Adams"       => Set(one, two)
    2nd pass:
        "Adams, Jane" => Set(two)

Ref(three):
    1st pass:
        "Adams, John" => Set(one, three)
    2nd pass:
        "Adams, John, John, Adams" => Set(three)

```

Now we know that `Ref(three)` is referred to as "Adams, John, John, Adams". So 
`Ref(one)`'s lookups into the index for "Adams, John" should no longer include 
`Ref(three)`.

How do you do that? First, keep a map of negatives, like so:

```
if a token appears in this map, you may Set-Minus the ref ids from the usual 
matching set.
{ "Adams" => Set(one, two), "Adams, John" => Set(three) }
```

Split disambiguation into two phases (well, more, but get to that later), one 
that generates IR + negative name matching and waits for all cites to 
disqualify their own negatives before continuing. The second tries again from 
the same point with negative matches excluded. In this case, you would have:

```
Ref(one):
    2nd pass:
        "Adams, John" => Set(one, three) \\ Set(three) = Set(one)
    No longer ambiguous
Ref(two):
    Already unambiguous
Ref(three):
    Already unambiguous
```

Therefore, a cite to `Ref(one)` would not continue to be disambiguated 
unnecessarily. A cite including a *literal* name "Adams" would also be 
unambiguous from that point.

### "all-names" disambiguation rule -- disambiguating names by themselves

You can create another inverted index just for names.

### Year suffixes

You could do a similar but slightly different thing with year-suffixes, 
producing a map like:

```
{ Ref(one) => "a", Ref(two) => "b" }
```

Example implementation:

```rust
fn year_suffixes(db: &impl CiteDatabase, _: ()) -> Arc<HashMap<Atom, u32>> {
    let refs_to_add_suffixes_to = all_cites_ordered
        .map(|cite| (&cite.ref_id, db.ir2(cite.id)))
        .filter_map(|(ref_id, (_, is_date_ambig))| {
            match is_date_ambig {
                true => Some(ref_id),
                _ => None
            }
        });

    let mut suffixes = HashMap::new();
    let mut i = 1; // "a" = 1
    for ref_id in refs_to_add_suffixes_to {
        if !suffixes.contains(ref_id) {
            suffixes.insert(ref_id.clone(), i);
            i += 1;
        }
    }
    Arc::new(suffixes)
}

fn ir3(db: &impl CiteDatabase, cite_id: CiteId) -> Arc<(IrSum<Pandoc>, bool)> {
    let cite = db.cite(cite_id);
    let ir2 = db.ir2(cite_id);
    let suffixes = db.year_suffixes();
    // if unambiguous or not improvable, just return ir2.
    // It's an Arc, so cloning is cheap.
    if !ir2.1 || !suffixes.contains_key(&cite.ref_id) {
        return ir2.clone();
    }
    // Otherwise compute ir3() based on those suffixes
    let ctx = CiteContext {
        year_suffix: suffixes[&cite.ref_id],
        ..etc
    };
}
```

