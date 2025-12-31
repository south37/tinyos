#![no_std]
#![no_main]

mod uart;

use core::{cell::OnceCell, panic::PanicInfo};

unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

fn kernel_range() -> (usize, usize) {
    let start = unsafe { &__kernel_start as *const u8 as usize };
    let end = unsafe { &__kernel_end as *const u8 as usize };
    (start, end)
}

const KERNBASE: usize = 0xFFFFFFFF80100000;

fn p2v(x: usize) -> usize {
    x + KERNBASE
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
    let freelist = unsafe { &*(kernel.allocator.freelist) };
    let addr2 = freelist.next as *const u8;
    uart_println!("freelist->next: {:x}", addr2 as usize);

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

struct Allocator {
    freelist: *const Run,
}

struct Run {
    next: *const Run,
}

impl Allocator {
    fn new() -> Self {
        Self {
            freelist: core::ptr::null(),
        }
    }

    fn init1(&mut self, vstart: usize, vend: usize) {
        let mut p = pgroundup(vstart);
        while p + PG_SIZE <= vend {
            self.kfree(p);
            p += PG_SIZE;
        }
    }

    fn kfree(&mut self, addr: usize) {
        unsafe {
            core::ptr::write_bytes(addr as *mut u8, 1u8, PG_SIZE);
        }
        let run: &mut Run = unsafe { &mut *(addr as *mut Run) };
        run.next = self.freelist;
        self.freelist = run;
    }
}

const PG_SIZE: usize = 4096;

fn pgroundup(sz: usize) -> usize {
    (sz + PG_SIZE - 1) & !(PG_SIZE - 1)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}
