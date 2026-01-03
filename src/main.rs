#![no_std]
#![no_main]

mod allocator;
mod gdt;
mod ioapic;
mod lapic;
mod pci;
mod proc;
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

    proc::init_process(&mut allocator);
    uart_println!("Init process initialized");

    let device = pci::scan_pci();
    if let Some(dev) = device {
        uart_println!("Device found, initializing virtio...");
        unsafe {
            virtio::init(&dev, &mut allocator);
        }

        // Test Read/Write
        let mut buf = [0u8; 512];
        virtio::read_block(0, &mut buf);
        uart_println!("Read block 0");
        // Check magic? disk.img is empty (zeros).

        // Write something
        buf[0] = 0xDE;
        buf[1] = 0xAD;
        buf[2] = 0xBE;
        buf[3] = 0xEF;
        virtio::write_block(0, &buf); // sector 0
        uart_println!("Wrote block 0");

        // Read back
        let mut buf2 = [0u8; 512];
        virtio::read_block(0, &mut buf2);
        uart_println!(
            "Read back block 0: {:x} {:x} {:x} {:x}",
            buf2[0],
            buf2[1],
            buf2[2],
            buf2[3]
        );

        if buf2[0] == 0xDE && buf2[1] == 0xAD && buf2[2] == 0xBE && buf2[3] == 0xEF {
            uart_println!("Virtio test PASSED");
        } else {
            uart_println!("Virtio test FAILED");
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
