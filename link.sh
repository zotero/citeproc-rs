#!/bin/bash

DIR="$(dirname "$0")"
DIR=$(cd $DIR && pwd)

ln -s $DIR/target $DIR/citeproc-wasm/target
