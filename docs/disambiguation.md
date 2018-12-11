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
