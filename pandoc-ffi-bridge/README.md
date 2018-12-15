# pandoc-ffi-bridge

## Compiling

At the moment, everything Haskell is dynamically linked. It's not ideal, but 
it's a start.

Also, go make a cup of coffee, because `stack` is about to compile all of 
Pandoc for you.

```sh
stack build -j4 && cp $(find $(stack path --local-install-root)/lib -name '*.dylib') ../target/debug
cargo run
```

This works because `cargo run` sets the linker path in the environment before 
it execs your binary. You can emulate that with:

```sh
env LD_LIBRARY_PATH=../target/debug ../target/debug/ffi-bridge

# or on macOS
env DYLD_LIBRARY_PATH=../target/debug ../target/debug/ffi-bridge
```

