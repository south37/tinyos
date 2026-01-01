#![no_std]
#![no_main]

mod allocator;
mod gdt;
mod ioapic;
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

pub const KERNBASE: usize = 0xFFFFFFFF80000000; // First kernel virtual address
pub const DEVBASE: usize = 0xFFFFFFFF40000000; // First device virtual address

pub const DEVSPACE: usize = 0xFE000000; // First device physical address
pub const IOAPIC: usize = 0xFEC00000;

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
        .init1(kernel_range().1, p2v(128 * 1024 * 1024));

    // Debug
    let addr = kernel.allocator.freelist as *const u8;
    uart_println!("freelist: {:x}", addr as usize);
    if !kernel.allocator.freelist.is_null() {
        let freelist = unsafe { &*(kernel.allocator.freelist) };
        let addr2 = freelist.next as *const u8;
        uart_println!("freelist->next: {:x}", addr2 as usize);
    }

    // Kernel virtual memory
    let mut kvm = vm::Kvm::new();
    kvm.init(&mut kernel.allocator);
    // Linear map
    make_linear(&mut kvm, &mut kernel.allocator);
    // Load page table. Switch cr3.
    kvm.load();
    uart_println!("Page table loaded");

    // Test paging
    unsafe {
        test_paging();
    }

    gdt::init();
    uart_println!("GDT loaded");

    ioapic::init();
    uart_println!("IOAPIC initialized");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

fn make_linear(kvm: &mut vm::Kvm, allocator: &mut Allocator) {
    // Linear map. Virtual: [0, 0 + 1GiB) -> Physical: [0, 1GiB)
    let r = kvm.map(
        allocator,
        0,
        0,
        0x40000000, // 1GiB
        vm::PageTableEntry::WRITABLE,
    );
    if !r {
        uart_println!("Linear map [0, 0 + 1GiB) failed");
    }
    // Linear map. Virtual: [KERNBASE, KERNBASE + 1GiB) -> Physical: [0, 1GiB)
    let r = kvm.map(
        allocator,
        KERNBASE as u64,
        0,
        0x40000000, // 1GiB
        vm::PageTableEntry::WRITABLE,
    );
    if !r {
        uart_println!("Linear map [KERNBASE, KERNBASE + 1GiB) failed");
    }
    // Linear map. Virtual: [DEVBASE, DEVBASE + 512MiB) -> Physical: [DEVSPACE, DEVSPACE + 512MiB)
    let r = kvm.map(
        allocator,
        DEVBASE as u64,
        DEVSPACE as u64,
        0x20000000, // 512MiB
        vm::PageTableEntry::WRITABLE
            | vm::PageTableEntry::WRITE_THROUGH
            | vm::PageTableEntry::CACHE_DISABLE,
    );
    if !r {
        uart_println!("Linear map [DEVBASE, DEVBASE + 512MiB) failed");
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
