#![no_std]
#![no_main]

mod allocator;
mod bio;
mod fs;
mod gdt;
mod ioapic;
mod lapic;
mod pci;
mod proc;
mod sleeplock;
mod spinlock;
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
    uart_println!("Hello, world!");
    uart_println!(
        "kernel range: {:x} - {:x}",
        kernel_range().0,
        kernel_range().1
    );

    let mut allocator = Allocator::new();
    allocator.init(kernel_range().1, p2v(PHYS_MEM));

    // Debug
    debug_freelist(&mut allocator);

    vm::init(&mut allocator);
    uart_println!("Page table loaded");

    gdt::init();
    uart_println!("GDT loaded");

    lapic::init();
    uart_println!("LAPIC initialized");

    ioapic::init();
    uart_println!("IOAPIC initialized");

    trap::init();
    uart_println!("Traps initialized");

    bio::binit();
    uart_println!("Buffer cache initialized");

    proc::init_process(&mut allocator);
    uart_println!("Init process initialized");

    let device = pci::scan_pci(virtio::VIRTIO_LEGACY_DEVICE_ID);
    if let Some(dev) = device {
        uart_println!("Device found, initializing virtio (legacy)...");
        // Initialize Virtio
        unsafe {
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
        uart_println!("Filesystem initialized");

        // Verify Root Inode
        {
            let ip = fs::iget(1, fs::ROOTINO);
            let guard = ip.ilock();
            uart_println!("Root Inode:");
            uart_println!("  type: {}", guard.type_);
            uart_println!("  nlink: {}", guard.nlink);
            uart_println!("  size: {}", guard.size);
            // guard is dropped here, unlocking inode.
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
    uart_println!("freelist: 0x{:x}", addr);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}
