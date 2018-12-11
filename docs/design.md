## Design

### 3-pass compiler (maybe even 4!)

In order to support parallel processing of a large document with many cites, 
the processing is split into many passes.

#### Pass 1: Cite to IR ('Intermediate Representation').

This computes every piece of output, but represents it as a tree where branches 
can be either unambiguous rendered text, or a possible point of ambiguity to be 
resolved in pass 2. These ambiguous nodes include the rendered version of 
themselves so that two IRs can be compared for 'output equality', and a 
reference to the rendering element that produced them and any other state 
required to recompute. Because Pass 1 requires no coordination between 
different cites, it is embarrassingly parallel.

#### Pass 2: Resolve ambiguities in 4 stages (according to the spec).

Compare IRs, and produce new ones based on the old ones (cheaply; most of the 
IR tree is unambiguous and reusable, and any potentially ambiguous parts not 
involved in a disambiguation transform can be returned verbatim) with 
increasing levels of disambiguation.

#### Pass 3 Flatten each IR.

This is cheap, really only a few hundred instructions per cite to pull the 
already-rendered content out into its final output form. ("Flatten" may not 
produce a single string, it just removes the IR's tree structure. Output 
formats may themselves be tree-structured.)

#### Potential optimisations

There is potential for a "Pass 0", before the rest, which would optimise a 
Style tree before it meets any particular cite. It helps to think of CSL as a 
low-powered, non-Turing-complete programming language which cannot set 
variables or recurse, and `citeproc` as an optimising compiler. Most compiler 
authors would be thrilled to find out those were the restrictions!

You could optimise by by macro-inlining, constant-propagating a common variable 
and running conditional branch elimination (CBE). While in regular compilers, 
such an optimisation flow is expensive due to the contextual interaction 
between inlined code and a call site, and its superlinear impact on analysing 
the resulting larger function, there is no such interaction in CSL and little 
rewriting is necessary; the contents of an unconditional `if` branch can simply 
be taken by reference without affecting correctness, and it would only incur a 
duplication (space) cost if any child elements are changed. (You would probably 
need everything in an `Rc` or `Arc` to make it work.) I would expect the cost 
of optimising a style for a concrete type not to greatly exceed the cost of 
running a single un-optimised IR generation, and for the cost to be recouped 
reasonably quickly, within normal document sizes. Optimised ASTs would be 
cached for long-lived Drivers, and optimisation could be avoided on a heuristic 
(like `cites_of_type(type).count() < 5`). If it turns out to be slow, the 
optimization could be pruned to serve only the most common cases, like macros 
with only a single `<choose>` that reduces to a single branch.

The most cost-effective variable to run this on would be `type`:

* A document typically contains a small number of distinct types (like 
  `legal_case` + `legislation` + `article-journal`), and the total time 
  optimising is bounded to the number of types that exist.
* Types are mutually exclusive, so propagating one as `true` allows propagating 
  all of the other types as `false` at the same time, so you actually get ~20 
  constants, not just one.
* The `type` variable is frequently matched against in the archetypal CSL 
  authoring mode, in which macros pretty much all try to be generic and start 
  with long set of `<if/else-if type="...">`s, and are then called from 
  multiple branches of other `choose` blocks which are already performing this 
  check.

```xml
<!-- Note that this would be a confusing, difficult to maintain and yet
     fairly typical CSL style. It isn't easy to write it better. If you can do 
     better, you should! Here, merging the two conditional blocks together 
     would make much more sense when you read it back later. -->
<macro name="GenericMacro">
  <!-- Often, these use a different set of type matches than the call sites -->
  <choose>
    <if type="bbb">
      <text variable="variable-for-bbb" />
    </if>
    <else-if type="aaa ccc ddd" match="any">
      <text value="for any matched type" />
      <choose>
        <!-- This condition matcher can be simplified under the "all" 
             scheme -->
        <if type="aaa" variable="some-other-variable" match="all">
          <text variable="variable-for-normal-case" />
        </if>
      </choose>
    </else-if>
  </choose>
</macro>
...
<choose>
  <if type="aaa bbb ccc" match="any">
    <text macro="GenericMacro" />
  </if>
  <else-if type="ddd">
    <text macro="GenericMacro" prefix="(specialisation for ddd " suffix=")" />
  </else-if>
</choose>
```

Let's say your document has references to type `aaa`. An optimal style 
structure specialised for `aaa` would look something like this:

```xml
<text value="for any matched type" />
<choose>
  <if variable="some-other-variable">
    <text variable="variable-for-normal-case" />
  </if>
</choose>
```

Such a style would save:

* Macro lookup, macro recursion avoidance
* Multiple sequence folds (macros and branches can contain multiple child 
  elements that need to be merged)
* The overhead of two choose blocks, either of which in general might be long 
  chains of `else-if` with many conditions to match against

You could construct a specialised style in this case by:

```xml
<!-- Inlining (using <seq> to represent <group> without its delimiter or 
     variable-collapsing semantics just for demonstration, because this 
     wouldn't be transformed at the XML level, it would happen internally): -->
<choose>
  <if type="aaa bbb ccc" match="any"> <!-- constant = true -->
    <seq>
      <choose>
        <if type="bbb"> <!-- constant = false; sever branch -->
            <text variable="variable-for-bbb" />
        </if>
        <else-if type="aaa ccc ddd" match="any"> <!-- constant = true -->
          <text value="for any matched type" />
          <choose>
            <!-- This condition matcher can be simplified under the "all" 
                 scheme -->
            <if type="aaa" variable="some-other-variable" match="all">
              <text variable="variable-for-normal-case" />
            </if>
          </choose>
        </else-if>
      </choose>
    <seq>
  </if>
  <else-if type="ddd"> <!-- ignored because first branch was constant true -->
    <seq prefix="(specialisation for ddd " suffix=")" >
      <choose>
        <if type="bbb">
          <text variable="variable-for-bbb" />
        </if>
        <else-if type="aaa ccc ddd" match="any">
          <text variable="variable-for-normal-case" />
        </else-if>
      </choose>
    <seq>
  </else-if>
</choose>

<!-- And constant-propagating:
         * type="aaa" => true
         * type="anything else" => false

     And then performing CBE (taking unconditional branches and severing 
     constant false ones); also collapsing any single-length seq blocks, 
     resulting in: -->
<seq>
  <text value="for any matched type" />
  <choose>
    <if variable="some-other-variable">
      <text variable="variable-for-normal-case" />
    </if>
  </choose>
</seq>

```

Which is what was desired. The `<seq>` surrounding the items protects them from 
having a `group delimiter="..."` from interfering with what was originally a 
unitary `<choose>` block.


### Modular output formats

