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

    let mut kernel = Kernel::new();
    kernel.allocator.init(kernel_range().1, p2v(PHYS_MEM));

    // Debug
    debug_freelist(&mut kernel.allocator);

    // Kernel virtual memory
    vm::init(&mut kernel.allocator);
    uart_println!("Page table loaded");

    // Test paging
    unsafe {
        test_paging();
    }

    gdt::init();
    uart_println!("GDT loaded");

    lapic::init();
    uart_println!("LAPIC initialized");

    ioapic::init();
    uart_println!("IOAPIC initialized");

    trap::init();
    uart_println!("Traps initialized");

    proc::init_process(&mut kernel.allocator);
    uart_println!("Init process initialized");

    // Enable interrupts
    unsafe {
        core::arch::asm!("sti");
    }

    proc::scheduler();

    // Debug
    debug_freelist(&mut kernel.allocator);
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

unsafe fn test_paging() {
    let virt_addr1 = KERNBASE as *mut u32;

    uart_println!("Testing paging...");

    // Save original value
    let original_value = unsafe { *virt_addr1 };
    uart_println!("Original value at KERNBASE: 0x{:x}", original_value);

    // Write to KERNBASE
    uart_println!("Writing 0xDEADBEEF to KERNBASE");
    unsafe { *virt_addr1 = 0xDEADBEEF };

    // Read from address 0 using assembly to avoid Rust null pointer check/panic
    let val: u32;
    unsafe {
        core::arch::asm!("mov {0:e}, [0]", out(reg) val);
    }
    uart_println!("Read 0x{:x} from address 0", val);

    if val == 0xDEADBEEF {
        uart_println!("Paging test passed!");
    } else {
        uart_println!("Paging test failed!");
    }

    // Restore
    unsafe { *virt_addr1 = original_value };
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}

fn panic_loop(message: &str) -> ! {
    uart_println!("panicked: {}", message);
    loop {}
}
