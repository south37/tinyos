#![no_std]
#![no_main]

mod allocator;
mod uart;
mod vm;

use allocator::Allocator;
use core::panic::PanicInfo;

unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

fn kernel_range() -> (usize, usize) {
    let start = unsafe { &__kernel_start as *const u8 as usize };
    let end = unsafe { &__kernel_end as *const u8 as usize };
    (start, end)
}

const KERNBASE: usize = 0xFFFFFFFF80000000;

pub fn p2v(x: usize) -> usize {
    x + KERNBASE
}

pub fn v2p(x: usize) -> usize {
    x - KERNBASE
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    uart_println!("Hello, world!");
    uart_println!(
        "kernel range: {:x} - {:x}",
        kernel_range().0,
        kernel_range().1
    );

    let mut kernel = Kernel::new();
    kernel
        .allocator
        .init1(kernel_range().1, p2v(4 * 1024 * 1024));

    // Debug
    let addr = kernel.allocator.freelist as *const u8;
    uart_println!("freelist: {:x}", addr as usize);
    if !kernel.allocator.freelist.is_null() {
        let freelist = unsafe { &*(kernel.allocator.freelist) };
        let addr2 = freelist.next as *const u8;
        uart_println!("freelist->next: {:x}", addr2 as usize);
    }

    // Page Table
    let mut kvm = vm::Kvm::new();
    kvm.init(&mut kernel.allocator);
    // Map kernel [KERNBASE, KERNBASE + 128MB) -> [0, 128MB)
    kvm.map(
        &mut kernel.allocator,
        KERNBASE as u64,
        0,
        128 * 1024 * 1024,
        vm::PageTableEntry::WRITABLE | vm::PageTableEntry::PRESENT,
    );
    unsafe {
        kvm.load();
    }
    uart_println!("Page table loaded");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

struct Kernel {
    allocator: Allocator,
}

impl Kernel {
    fn new() -> Self {
        Self {
            allocator: Allocator::new(),
        }
    }
}

pub const PG_SIZE: usize = 4096;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}
