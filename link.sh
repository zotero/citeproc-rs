#!/bin/bash

# You need to run this if you want `wasm-pack build` to execute.

DIR="$(dirname "$0")"
DIR=$(cd $DIR && pwd)

ln -s $DIR/target $DIR/wasm/target
