#!/bin/sh

set -eux

qemu-system-x86_64 -kernel ./target/x86_64-unknown-none/debug/tinyos -nographic -serial mon:stdio