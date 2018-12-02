<meta charset="utf-8"/>

### How it should work?

#### Arena allocation to replace Vec in style.rs, elements.rs

They are static. Don't need modification at all. So kill it.
All the vec lengths can be computed by exhausting iterators.

#### Multi-threaded?

Would be faster for hundreds of cites. Cargo feature flag.

#### 2-pass compiler

```rust
struct DisambConditional {
    
}
enum DisambNode<'arena> {
    Unambiguous(&'arena str),
    Names(...)
    Conditional(...),
}
struct DisambCluster();

fn pass_1_cite(Cite) -> &[DisambNode]

fn pass_1(arena: Arena) {
    clusters.map(|cluster| {
        cluster.cites.map(|cite| {
            cite_to_disamb_node(cite)
        })
    })
}
fn pass_1(cite: Cite) -> &[DisambNode]
```

* *Pass 1*: `Fn(&[CiteCluster], ...) -> &[DisambNode]`
* *Pass 2*: `&[&[DisambNode]]` ->
  * *render_cite*

### Todo

* Use `typed_arena` over each processing run to make thousands of string 
  allocations quicker and then virtually free to deallocate.
  * Careful of long-lived processes leaking, though.

# 🦀🕸️ Usage with `wasm-pack`

### 🛠️ Build with `wasm-pack build`

```
wasm-pack build
```

### 🔬 Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```

### 🎁 Publish to NPM with `wasm-pack publish`

```
wasm-pack publish
```
