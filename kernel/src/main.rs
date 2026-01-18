#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)] // For static mut context

mod allocator;
mod bio;
mod console;
mod elf;
mod exec;
pub mod file;
pub mod fs;
mod gdt;
pub mod growproc;
mod ioapic;
mod lapic;
mod log;
mod pci;
mod pipe;
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
    crate::info!("Hello from tinyos!");

    crate::allocator::ALLOCATOR
        .lock()
        .init(kernel_range().1, p2v(PHYS_MEM));

    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        vm::init(&mut allocator);
    }
    crate::info!("Page table loaded");

    gdt::init(0);
    crate::info!("GDT loaded");

    proc::init_cpus();
    crate::info!("CPUs initialized");

    lapic::init();
    crate::info!("LAPIC initialized");

    ioapic::init();
    crate::info!("IOAPIC initialized");

    trap::init();
    crate::info!("Traps initialized");

    uart::init();
    crate::info!("UART initialized");

    unsafe {
        ioapic::enable(IRQ_UART, 0);
    }

    syscall::init(0);
    crate::info!("Syscalls initialized");

    bio::binit();
    crate::info!("Buffer cache initialized");

    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        proc::init_process(&mut allocator);
    }
    crate::info!("Init process initialized");

    let device = pci::scan_pci(virtio::VIRTIO_LEGACY_DEVICE_ID);
    if let Some(dev) = device {
        crate::info!("Device found, initializing virtio (legacy)...");
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
        crate::info!("Filesystem initialized");
    }

    // Enable interrupts
    unsafe {
        core::arch::asm!("sti");
    }

    start_aps();

    crate::debug!("DEBUG: kernel initialized");

    proc::scheduler();

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

fn start_aps() {
    crate::info!("Starting APs...");
    let entry_code = include_bytes!("../asm/build/entryother");
    let code_ptr = p2v(0x7000) as *mut u8;

    unsafe {
        core::ptr::copy_nonoverlapping(entry_code.as_ptr(), code_ptr, entry_code.len());
    }

    for i in 0..proc::NCPU {
        if i == 0 {
            continue;
        } // Skip BSP (assumed 0)

        let mut allocator = crate::allocator::ALLOCATOR.lock();
        let stack = allocator.kalloc();
        if stack.is_null() {
            crate::error!("Failed to allocate stack for CPU {}", i);
            continue;
        }

        let stack_top = stack as usize + proc::KSTACK_SIZE;

        // Pass parameters to entryother at 0x7000 - ...
        unsafe {
            let code_phys = 0x7000;
            *(p2v(code_phys - 8) as *mut u64) = stack_top as u64;
            *(p2v(code_phys - 16) as *mut u32) = util::rcr3() as u32; // CR3
            *(p2v(code_phys - 24) as *mut u64) = mpenter as *const () as u64;
        }

        let lapicid = i as u32; // Assuming linear mapping for now.

        // Send INIT IPI
        unsafe {
            lapic::write_reg(lapic::ICRHI, lapicid << 24);
            lapic::write_reg(
                lapic::ICRLO,
                lapic::ICR_INIT | lapic::ICR_LEVEL | lapic::ICR_ASSERT,
            );
            util::micro_delay(200);
            lapic::write_reg(lapic::ICRLO, lapic::ICR_INIT | lapic::ICR_LEVEL);
            util::micro_delay(10000); // 10ms

            // Send Startup IPI (twice)
            for _ in 0..2 {
                lapic::write_reg(lapic::ICRHI, lapicid << 24);
                lapic::write_reg(lapic::ICRLO, lapic::ICR_STARTUP | (0x7000 >> 12));
                util::micro_delay(200);
            }
        }

        // Wait for CPU to start?
        // We can check proc::CPUS[i].started if we had it exposed.
        // For now just wait and hope.
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mpenter() -> ! {
    // AP Entry Point
    // Get CPUID first
    let cpuid = crate::lapic::id() as usize;

    // 1. Enable paging (already done in entryother)
    // 2. Load GDT (per-CPU)
    crate::gdt::init(cpuid);

    // 3. Init LAPIC
    crate::lapic::init();

    // 4. Init Traps (IDT)
    crate::trap::init();

    // 5. Init Syscall (MSRs)
    crate::syscall::init(cpuid);

    crate::info!("CPU {} started!", cpuid);

    // Mark started
    // unsafe { proc::CPUS[cpuid as usize].started = true; }
    // We need to access CPUS.

    crate::proc::scheduler();

    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}
