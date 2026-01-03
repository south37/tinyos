#!/bin/env bash

set -eux

# Must be same as PHYS_MEM in util.rs
PHYS_MEM=256M

QEMUOPTS="-m $PHYS_MEM -net none -nographic -serial mon:stdio"
for arg in "$@"; do
    case "$arg" in  
        "gdb")
            QEMUOPTS="${QEMUOPTS} -S -gdb tcp::1234"
            ;;
        *)
            ;;
    esac
done

if [ ! -f disk.img ]; then
    qemu-img create -f raw disk.img 128M
fi

qemu-system-x86_64 \
  -kernel ./target/x86_64-unknown-none/release/tinyos \
  -d int,mmu,guest_errors \
  -D qemu.log \
  -drive file=disk.img,if=none,format=raw,id=x0 \
  -device virtio-blk-pci,drive=x0,bus=pci.0,addr=0x3 \
  $QEMUOPTS
