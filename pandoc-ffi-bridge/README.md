# pandoc-ffi-bridge

## Compiling

At the moment, everything Haskell is dynamically linked. It's not ideal, but 
it's a start.

```
stack build -j4 && cp $(find $(stack path --local-install-root)/lib -name '*.dylib') ../target/debug
cargo run
```
`
