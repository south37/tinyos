#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)] // For static mut context

mod allocator;
mod bio;
mod console;
mod elf;
mod exec;
mod file;
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

    gdt::init(0);
    uart_println!("INFO: GDT loaded");

    proc::init_cpus();

    lapic::init();
    uart_println!("INFO: LAPIC initialized");

    ioapic::init();
    uart_println!("INFO: IOAPIC initialized");

    trap::init();
    uart_println!("INFO: Traps initialized");

    syscall::init(0);
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

    start_aps();

    proc::scheduler();

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

fn start_aps() {
    uart_println!("INFO: Starting APs...");
    let entry_code = include_bytes!("../asm/entryother");
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
            uart_println!("ERROR: Failed to allocate stack for CPU {}", i);
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

    uart_println!("INFO: CPU {} started!", cpuid);

    // Mark started
    // unsafe { proc::CPUS[cpuid as usize].started = true; }
    // We need to access CPUS.

    crate::proc::scheduler();

    loop {
        unsafe { core::arch::asm!("hlt") }
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
