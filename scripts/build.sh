#!/bin/env bash

set -eux

make -C asm

cargo clean
cargo build --release