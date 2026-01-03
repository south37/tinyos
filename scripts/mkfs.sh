#!/bin/bash

set -eux

# Compile and run mkfs to create disk.img
cargo run --bin mkfs --release --target x86_64-unknown-linux-gnu

echo "disk.img created."
