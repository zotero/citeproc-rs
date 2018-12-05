FROM rust:1.30.1

RUN apt-get update && apt-get install -y linux-perf

# create a new empty shell project
RUN USER=root mkdir -p /citeproc-rs
RUN USER=root cargo new --bin /citeproc-rs/citeproc
WORKDIR /citeproc-rs

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./citeproc/Cargo.toml ./citeproc/Cargo.toml

RUN echo "[workspace]" > Cargo.toml
RUN echo "members = [ \"citeproc\" ]" >> Cargo.toml
RUN echo "[profile.release]" >> Cargo.toml
RUN echo "lto = true" >> Cargo.toml
RUN echo "debug = true" >> Cargo.toml

RUN touch citeproc/src/lib.rs

# this build step will cache your dependencies
RUN cargo build --release
RUN rm citeproc/src/*.rs

WORKDIR /citeproc-rs/citeproc

# copy your source tree
COPY ./citeproc/src ./src

# build for release
RUN rm ../target/release/deps/citeproc*
RUN cargo build --release --features alloc_system

COPY ../example.csl .
COPY /Users/cormac/Zotero/styles/australian-guide-to-legal-citation.csl .

RUN perf record -g ./target/release/citeproc --csl ./example.csl
RUN perf script | stackcollapse-perf | rust-unmangle | flamegraph > perf.svg

