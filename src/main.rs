#![no_std]
#![no_main]

mod allocator;
mod gdt;
mod ioapic;
mod lapic;
mod proc;
mod trap;
mod uart;
mod util;
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
