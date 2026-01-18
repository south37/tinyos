# Makefile for tinyos

# Configuration
PROFILE ?= release
PHYS_MEM ?= 256M
CARGO ?= cargo
QEMU ?= qemu-system-x86_64
MKFS ?= mkfs.ext2
LOG ?= info
export LOG_LEVEL := $(LOG)

# Paths
TARGET_DIR := target/x86_64-unknown-none/$(PROFILE)
KERNEL_BIN := $(TARGET_DIR)/kernel
DISK_IMG := disk.img

# Flags
ifeq ($(PROFILE),release)
	CARGO_FLAGS := --release
else
	CARGO_FLAGS :=
endif

QEMUOPTS := -m $(PHYS_MEM) -smp 2 -net none -nographic -serial mon:stdio
# Default QEMU debug flags (can be overridden)
QEMU_DEBUG ?= guest_errors

# GDB Support
ifdef GDB
	QEMUOPTS += -S -gdb tcp::1234
endif

.PHONY: all build kernel asm user fs run clean qemu

all: build

build: kernel fs

# 1. Assembly Objects (Required for linking kernel)
asm:
	$(MAKE) -C kernel/asm

# 2. Kernel Build
kernel: asm
	cd kernel && $(CARGO) build $(CARGO_FLAGS)

# 3. User Programs (Required for fs)
user:
	$(MAKE) -C user

# 4. Filesystem Image
fs: user
	mkdir -p build/fs
	echo "Hello Ext2" > build/fs/hello.txt
	cp user/build/init build/fs/
	cp user/build/sh build/fs/
	cp user/build/echo build/fs/
	dd if=/dev/zero of=$(DISK_IMG) bs=1M count=32
	$(MKFS) -E revision=0 -b 1024 -d build/fs -F $(DISK_IMG)

# 5. Run QEMU
run: kernel fs
	$(QEMU) \
		-kernel $(KERNEL_BIN) \
		$(QEMUOPTS) \
		-d $(QEMU_DEBUG) \
		-D qemu.log \
		-drive file=$(DISK_IMG),if=none,format=raw,id=x0 \
		-device virtio-blk-pci,drive=x0,bus=pci.0,addr=0x3

# 6. GDB
gdb:
	gdb -x .gdbinit $(KERNEL_BIN)

# Clean
clean:
	$(MAKE) -C kernel/asm clean
	$(MAKE) -C user clean
	$(CARGO) clean
	rm -rf build $(DISK_IMG) qemu.log
