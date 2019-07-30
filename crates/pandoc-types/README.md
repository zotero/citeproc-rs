# Rust port of pandoc-types [![Build Status](https://travis-ci.org/elliottslaughter/rust-pandoc-types.svg?branch=master)](https://travis-ci.org/elliottslaughter/rust-pandoc-types)

This library provides a Rust port of the [pandoc-types Haskell
package](https://hackage.haskell.org/package/pandoc-types).

To install, add the following to your `Cargo.toml`:

```
[dependencies]
pandoc_types = "0.2"
```

## What this library is for

The purpose of pandoc-types is to allow Rust programs to natively
manipulate [Pandoc](http://pandoc.org/) documents. Using this library,
Rust programs should be able to create and modify Pandoc documents in
a principled way (i.e. via ASTs, not text). This library can also be
used along with [serde_json](https://github.com/serde-rs/json) to
serialize and deserialize Pandoc documents to and from Pandoc's JSON
format.

## What this library is NOT for

This library does *not* provide a way of calling the Pandoc executable
itself. If that's what you're looking for, consider the
[rust-pandoc](https://github.com/oli-obk/rust-pandoc) library.

## Compatibility

The current version is **compatible with Haskell pandoc-types
1.17**. This is the most recent version at the time of writing.

## Supported modules

The following modules from pandoc-types are supported:

  * Haskell `Text.Pandoc.Definition` (as `pandoc_types::definition` in Rust)

Note that `Text.Pandoc.JSON` is unnecessary in Rust because all types
implement `Serialize` and `Deserialize` from
[serde](https://github.com/serde-rs/serde) and can be used directly
with [serde_json](https://github.com/serde-rs/json).

## Example usage

```rust
let para = Block::Para(vec![Inline::Str("b".to_owned())]);

let s = serde_json::to_string(&para)?;
println!("serialized = {}", s);

let d: Block = serde_json::from_str(&s)?;
println!("deserialized = {:?}", d);
```

For a full example, see [examples/definition.rs](examples/definition.rs).

## License

This library is licensed under the Apache License, Version 2.0 (see
[LICENSE.txt](LICENSE.txt)).

