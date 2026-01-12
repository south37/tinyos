#!/bin/bash

set -eux

# Create staging directory
mkdir -p build/fs
echo "Hello Ext2" > build/fs/hello.txt
cp ./user/init build/fs/

# Create disk.img (size 32M approx)
# -d build/fs: populate with files from directory
# -r 1: revision 1 (dynamic inode sizes) -- actually for tiny OS revocation 0 might be simpler but 1 is standard.
# -N 100: inodes
# blocks-count: 32768 (1k blocks -> 32MB) or just size.
# 32M = 32768 blocks of 1K.

# We use -b 1024 for 1KB blocks to match our likely buffer size, simplermath.
dd if=/dev/zero of=disk.img bs=1M count=32
mkfs.ext2 -E revision=0 -b 1024 -d build/fs -F disk.img

echo "disk.img created."
