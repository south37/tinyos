#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)] // For static mut context

mod allocator;
mod bio;
mod elf;
mod exec;
mod fs;
mod gdt;
mod ioapic;
mod lapic;
mod pci;
mod proc;
mod sleeplock;
mod spinlock;
mod syscall;
mod trap;
mod uart;
mod util;
mod virtio;
mod vm;

use allocator::Allocator;
use core::panic::PanicInfo;
use util::*;

unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

fn kernel_range() -> (usize, usize) {
    let start = unsafe { &__kernel_start as *const u8 as usize };
    let end = unsafe { &__kernel_end as *const u8 as usize };
    (start, end)
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    uart_println!("Hello from tinyos!");
    uart_println!(
        "INFO: kernel range: {:x} - {:x}",
        kernel_range().0,
        kernel_range().1
    );

    crate::allocator::ALLOCATOR
        .lock()
        .init(kernel_range().1, p2v(PHYS_MEM));

    // Debug
    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        debug_freelist(&mut allocator);
    }

    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        vm::init(&mut allocator);
    }
    uart_println!("INFO: Page table loaded");

    gdt::init();
    uart_println!("INFO: GDT loaded");

    lapic::init();
    uart_println!("INFO: LAPIC initialized");

    ioapic::init();
    uart_println!("INFO: IOAPIC initialized");

    trap::init();
    uart_println!("INFO: Traps initialized");

    syscall::init();
    uart_println!("INFO: Syscalls initialized");

    bio::binit();
    uart_println!("INFO: Buffer cache initialized");

    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        proc::init_process(&mut allocator);
    }
    uart_println!("INFO: Init process initialized");

    let device = pci::scan_pci(virtio::VIRTIO_LEGACY_DEVICE_ID);
    if let Some(dev) = device {
        uart_println!("INFO: Device found, initializing virtio (legacy)...");
        // Initialize Virtio
        unsafe {
            let mut allocator = crate::allocator::ALLOCATOR.lock();
            virtio::init(&dev, &mut allocator);
        }

        // Enable Virtio IRQ (11) on CPU 0
        unsafe {
            ioapic::enable(IRQ_VIRTIO, 0);
        }

        // Enable Interrupts
        unsafe { core::arch::asm!("sti") };

        // Initialize Filesystem
        fs::fsinit(1);
        uart_println!("INFO: Filesystem initialized");

        // Verify Root Inode
        {
            let ip = fs::iget(1, fs::ROOT_INO);
            let guard = ip.ilock();
            let mode = guard.i_mode;
            let nlink = guard.i_links_count;
            let size = guard.i_size;
            uart_println!("DEBUG: Root Inode:");
            uart_println!("  mode: {:x}", mode);
            uart_println!("  nlinks: {}", nlink);
            uart_println!("  size: {}", size);
            // guard is dropped here, unlocking inode.
        }

        // Read 'hello.txt' file
        {
            let root = fs::iget(1, fs::ROOT_INO);
            if let Some(inum) = fs::dirlookup(root, "init") {
                uart_println!("DEBUG: Found 'hello.txt' inode: {}", inum);
                let ip = fs::iget(1, inum);
                let mut buf = [0u8; 128];
                let n = fs::readi(ip, buf.as_mut_ptr(), 0, 128);
                if n > 0 {
                    let len = if n as usize > buf.len() {
                        buf.len()
                    } else {
                        n as usize
                    };
                    let s = core::str::from_utf8(&buf[0..len]).unwrap_or("invalid utf8");
                    uart_println!("DEBUG: Content: {}", s);
                } else {
                    uart_println!("DEBUG: Read 0 bytes");
                }
            } else {
                uart_println!("ERROR: 'hello.txt' not found in root");
            }
        }
    }

    // Enable interrupts
    unsafe {
        core::arch::asm!("sti");
    }

    proc::scheduler();

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

fn debug_freelist(allocator: &mut Allocator) {
    let addr = allocator.freelist as *const u8 as usize;
    uart_println!("DEBUG: freelist: 0x{:x}", addr);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}
