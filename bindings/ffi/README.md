# ffi interface to `citeproc-rs`

```sh
# prerequisite (using a fork at the moment)
git clone https://github.com/cormacrelf/cbindgen.git
cd cbindgen
cargo install --force --path .

# from this ffi directory
make

# or more explicitly
cargo build
cbindgen -o include/citeproc_rs.h
make ./examples/client

# linked to to the dylib in <repo root>/target/debug/libciteproc_rs.dylib
./examples/client
```
