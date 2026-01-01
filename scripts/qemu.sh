#!/bin/env bash

set -eux

QEMUOPTS="-m 128M -net none -nographic -serial mon:stdio"
for arg in "$@"; do
    case "$arg" in  
        "gdb")
            QEMUOPTS="${QEMUOPTS} -S -gdb tcp::1234"
            ;;
        *)
            ;;
    esac
done

qemu-system-x86_64 \
  -kernel ./target/x86_64-unknown-none/release/tinyos \
  -d int,mmu,guest_errors \
  -D qemu.log \
  $QEMUOPTS
