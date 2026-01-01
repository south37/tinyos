#!/bin/env bash

set -eux

make -C entry

cargo clean
cargo build --release