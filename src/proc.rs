#![allow(static_mut_refs)]

use crate::allocator::Allocator;
use crate::gdt::{UCODE_SELECTOR, UDATA_SELECTOR};
use crate::trap::TrapFrame;

use crate::util::PG_SIZE;
use crate::vm::{self, PageTable, PageTableEntry};
use core::arch::global_asm;
use core::sync::atomic::{AtomicBool, Ordering};

pub const NPROC: usize = 64;
pub const KSTACK_SIZE: usize = PG_SIZE;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    rip: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ProcessState {
    UNUSED,
    EMBRYO,
    SLEEPING,
    RUNNABLE,
    RUNNING,
    ZOMBIE,
}

pub const NFILE: usize = 16;
use crate::file::File;

#[derive(Clone, Copy)]
pub struct Process {
    pub state: ProcessState,
    pub kstack: *mut u8,
    pub context: *mut Context,
    pub pgdir: *mut PageTable,
    pub pid: usize,
    pub chan: usize,
    pub name: [u8; 16],
    pub ofile: [Option<*mut File>; NFILE],
    pub parent: Option<*mut Process>,
    pub killed: bool,
}

impl Process {
    pub const fn new() -> Self {
        Self {
            state: ProcessState::UNUSED,
            kstack: core::ptr::null_mut(),
            context: core::ptr::null_mut(),
            pgdir: core::ptr::null_mut(),
            pid: 0,
            chan: 0,
            name: [0; 16],
            ofile: [None; NFILE],
            parent: None,
            killed: false,
        }
    }
}

pub const NCPU: usize = 8;

#[derive(Clone, Copy)]
pub struct Cpu {
    pub lapicid: u32,
    pub scheduler_context: *mut Context,
    pub process: Option<*mut Process>, // Helper to track current process ptr
    pub started: bool,
    pub ncli: usize,
    pub intena: bool,
}

impl Cpu {
    pub const fn new() -> Self {
        Self {
            lapicid: 0,
            scheduler_context: core::ptr::null_mut(),
            process: None,
            started: false,
            ncli: 0,
            intena: false,
        }
    }
}

pub static mut CPUS: [Cpu; NCPU] = [Cpu::new(); NCPU];
pub static mut PROCS: [Process; NPROC] = [Process::new(); NPROC];
pub static PROCS_LOCK: crate::spinlock::Spinlock<()> =
    crate::spinlock::Spinlock::new((), "PROCS_LOCK");
static mut PID_COUNTER: usize = 0;
pub static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn init_cpus() {
    unsafe {
        for (i, cpu) in CPUS.iter_mut().enumerate() {
            cpu.lapicid = i as u32;
        }
        INITIALIZED.store(true, Ordering::Release);
    }
}

pub fn mycpu() -> &'static mut Cpu {
    if !INITIALIZED.load(Ordering::Acquire) {
        return unsafe { &mut CPUS[0] };
    }
    let apic_id = crate::lapic::id();
    unsafe {
        for cpu in CPUS.iter_mut() {
            if cpu.lapicid == apic_id {
                return cpu;
            }
        }
        // Fallback for bootstrap before APIC init? Or just assume index 0?
        // Actually, initial CPU often has ID 0, but not always.
        // For now, let's assume we can always find it.
        // If not found, it's a panic.
        panic!("mycpu: unknown apicid {}", apic_id);
    }
}

use crate::spinlock::SpinlockGuard;

pub fn sleep<T>(chan: usize, guard: Option<SpinlockGuard<T>>) {
    let cpu = mycpu();

    // Acquire ptable lock
    let ptable_guard = PROCS_LOCK.lock();

    // Release guard
    drop(guard);

    unsafe {
        if let Some(p) = cpu.process.as_mut() {
            let p = &mut **p;
            p.chan = chan;
            p.state = ProcessState::SLEEPING;
        }

        sched(ptable_guard);

        if let Some(p) = cpu.process.as_mut() {
            let p = &mut **p;
            p.chan = 0;
        }
    }
    // ptable_guard dropped here
}

pub fn wakeup(chan: usize) {
    let _guard = PROCS_LOCK.lock();
    unsafe {
        for p in PROCS.iter_mut() {
            if p.state == ProcessState::SLEEPING && p.chan == chan {
                p.state = ProcessState::RUNNABLE;
                p.chan = 0;
            }
        }
    }
}

pub unsafe fn sched(guard: SpinlockGuard<()>) {
    let cpu = mycpu();

    if let Some(p) = cpu.process.as_mut() {
        let p = &mut **p;

        if cpu.ncli != 1 {
            crate::error!("PANIC: sched ncli={}", cpu.ncli);
            crate::error!("PROCS_LOCK held: {}", PROCS_LOCK.holding());
            // crate::uart_println!("VIRTIO_LOCK held: {}", crate::virtio::VIRTIO_LOCK.holding());
            // crate::uart_println!("BCACHE held: {}", crate::bio::BCACHE.holding());
            // crate::uart_println!("ALLOCATOR held: {}", crate::allocator::ALLOCATOR.holding());
            panic!("sched: ncli {}", cpu.ncli);
        }
        if p.state == ProcessState::RUNNING {
            panic!("sched: process running");
        }
        if unsafe { crate::util::readeflags() } & 0x200 != 0 {
            panic!("sched: interrupts enabled");
        }

        swtch(&mut p.context as *mut _, cpu.scheduler_context);
    }
    drop(guard);
}

pub fn yield_proc() {
    let guard = PROCS_LOCK.lock();
    let cpu = mycpu();
    unsafe {
        if let Some(p) = cpu.process.as_mut() {
            let p = &mut **p;
            p.state = ProcessState::RUNNABLE;
            sched(guard);
        } else {
            drop(guard);
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn release_procs_lock() {
    unsafe {
        PROCS_LOCK.unlock();
    }
}

unsafe extern "C" {
    fn forkret();
}

global_asm!(
    ".global forkret",
    "forkret:",
    "call release_procs_lock",
    "jmp trapret"
);

unsafe extern "C" {
    fn swtch(old: *mut *mut Context, new: *mut Context);
}

global_asm!(
    ".global swtch",
    "swtch:",
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov [rdi], rsp",
    "mov rsp, rsi",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",
    "ret"
);

pub fn init_process(allocator: &mut Allocator) {
    // Find unused process
    let mut p_option: Option<&mut Process> = None;
    unsafe {
        for proc in PROCS.iter_mut() {
            if proc.state == ProcessState::UNUSED {
                p_option = Some(proc);
                break;
            }
        }
    }

    if let Some(p) = p_option {
        unsafe {
            PID_COUNTER += 1;
            p.pid = PID_COUNTER;
        }
        p.state = ProcessState::EMBRYO;

        // Allocation User Page Table
        p.pgdir = vm::uvm_create(allocator).expect("uvm_create failed");

        // Allocate kernel stack
        p.kstack = allocator.kalloc();
        if p.kstack.is_null() {
            p.state = ProcessState::UNUSED;
            return;
        }
        crate::debug!("kstack: 0x{:x}", p.kstack as usize);

        // Init code
        let initcode: &[u8] = include_bytes!("../asm/initcode");
        let mem = allocator.kalloc();
        if mem.is_null() {
            panic!("init_process: kalloc failed");
        }
        unsafe {
            core::ptr::copy_nonoverlapping(initcode.as_ptr(), mem, initcode.len());
        }
        // Map init code at 0
        vm::map_pages(
            p.pgdir,
            allocator,
            0,
            crate::util::v2p(mem as usize) as u64,
            PG_SIZE as u64,
            PageTableEntry::WRITABLE | PageTableEntry::USER,
        );

        let sp = p.kstack as usize + KSTACK_SIZE;

        // Setup context
        // Reserve space for TrapFrame
        let tf_addr = sp - core::mem::size_of::<TrapFrame>();
        let tf = tf_addr as *mut TrapFrame;

        // Set up TrapFrame
        unsafe {
            (*tf).cs = UCODE_SELECTOR as u64;
            (*tf).ss = UDATA_SELECTOR as u64;
            (*tf).rsp = PG_SIZE as u64; // User stack at top of page
            (*tf).rflags = 0x202; // IF | Reserved
            (*tf).rip = 0; // Entry point
        }

        // Reserve space for Context below TrapFrame
        let context_addr = tf_addr - core::mem::size_of::<Context>();
        p.context = context_addr as *mut Context;

        // Set context to return to forkret
        unsafe {
            (*p.context).rip = forkret as *const () as usize as u64;
            (*p.context).r15 = 0;
            (*p.context).r14 = 0;
            (*p.context).r13 = 0;
            (*p.context).r12 = 0;
            (*p.context).rbx = 0;
            (*p.context).rbp = 0;
        }

        p.state = ProcessState::RUNNABLE;
        p.name[0] = b'i';
        p.name[1] = b'n';
        p.name[2] = b'i';
        p.name[3] = b't';

        for i in 0..3 {
            if let Some(f) = crate::file::filealloc() {
                f.f_type = crate::file::FileType::Device;
                f.major = 1; // Console
                f.readable = true;
                f.writable = true;
                p.ofile[i] = Some(f as *mut _);
            }
        }
    }
}

pub fn scheduler() {
    let cpu = mycpu();
    cpu.process = None; // Ensure no process running

    crate::info!("Scheduler starting on CPU {}", cpu.lapicid);
    loop {
        // Enable interrupts to allow IRQs to wake us up
        unsafe { core::arch::asm!("sti") };

        // Acquire PTABLE LOCK
        // Acquire PTABLE LOCK
        // crate::uart_println!("DEBUG: sched acquiring lock");
        let guard = PROCS_LOCK.lock();
        // crate::uart_println!("DEBUG: sched lock acquired");

        let mut ran_process = false;
        unsafe {
            for i in 0..NPROC {
                let p = &mut PROCS[i];
                if p.state == ProcessState::RUNNABLE {
                    p.state = ProcessState::RUNNING;

                    cpu.process = Some(p as *mut Process);

                    // Switch to user page table
                    vm::switch(p.pgdir);

                    // Set Kernel Stack in TSS
                    let kstack_top = p.kstack as usize + KSTACK_SIZE;
                    crate::gdt::set_kernel_stack(kstack_top as u64, cpu.lapicid as usize);

                    // Switch to process
                    swtch(&mut cpu.scheduler_context as *mut _, p.context);

                    // Back from process
                    vm::switch(crate::vm::kpgdir()); // switch back to kvm

                    cpu.process = None;

                    ran_process = true;
                }
            }
        }
        // Release lock
        drop(guard);

        if !ran_process {
            // unsafe { core::arch::asm!("hlt") };
            // unsafe { core::arch::asm!("sti") }; // Ensure interrupts enabled
            // crate::uart_println!("DEBUG: idle");
            core::hint::spin_loop();
        }
    }
}

pub fn fork() -> isize {
    let mut pid: isize = -1;

    let cpu = mycpu();
    let curproc = unsafe { &mut *cpu.process.unwrap() };

    // Allocate process
    let mut np_opt = None;
    let guard = PROCS_LOCK.lock();
    unsafe {
        for p in PROCS.iter_mut() {
            if p.state == ProcessState::UNUSED {
                np_opt = Some(p);
                // i = idx as isize;
                break;
            }
        }
    }

    if let Some(np) = np_opt {
        unsafe {
            // Allocate kernel stack
            np.kstack = crate::allocator::ALLOCATOR.lock().kalloc();
            if np.kstack.is_null() {
                drop(guard);
                return -1;
            }

            // Copy user memory
            np.pgdir = vm::uvm_create(&mut crate::allocator::ALLOCATOR.lock())
                .expect("fork: uvm_create failed");
            // Assuming simplified uvm_copy for now: size is implicitly managed or we just copy known range?
            // Since we don't track proc size strictly yet, let's assume valid range up to KERNBASE
            // But standard approach is maintaining 'sz' in proc.
            // For this simple text, let's just copy 0..0x40000000 (1GB) if mapped? Too slow.
            // Let's rely on `sz` in process if we added it, or copy what we can.
            // Wait, we didn't add `sz` to Process struct. Let's add it or hack it.
            // Hack: Walk page table and copy present pages. uvm_copy(old, new, 0x80000000).
            if !vm::uvm_copy(
                curproc.pgdir,
                np.pgdir,
                0x80000000,
                &mut crate::allocator::ALLOCATOR.lock(),
            ) {
                // TODO: Free kstack
                drop(guard);
                return -1;
            }

            PID_COUNTER += 1;
            np.pid = PID_COUNTER;
            pid = np.pid as isize;
            np.state = ProcessState::EMBRYO;

            // Copy trap frame
            let sp = np.kstack as usize + KSTACK_SIZE;
            let tf_addr = sp - core::mem::size_of::<TrapFrame>();
            let tf = tf_addr as *mut TrapFrame;
            let cur_tf = ((curproc.kstack as usize) + KSTACK_SIZE
                - core::mem::size_of::<TrapFrame>()) as *const TrapFrame;
            core::ptr::copy_nonoverlapping(cur_tf, tf, 1);

            // Set return value for child
            (*tf).rax = 0;

            // Setup context
            let context_addr = tf_addr - core::mem::size_of::<Context>();
            np.context = context_addr as *mut Context;
            (*np.context).rip = forkret as *const () as usize as u64;
            // Copy registers? No, context is for scheduler.
            (*np.context).r15 = 0;
            (*np.context).r14 = 0;
            (*np.context).r13 = 0;
            (*np.context).r12 = 0;
            (*np.context).rbx = 0;
            (*np.context).rbp = 0;

            // Copy open files
            for fd in 0..NFILE {
                if let Some(f) = curproc.ofile[fd] {
                    // TODO: filedup(f); increment ref count
                    np.ofile[fd] = Some(f);
                }
            }
            // Copy cwd
            // np.cwd = idup(curproc.cwd);

            // Safely copying name
            np.name = curproc.name;

            np.parent = Some(curproc as *mut Process);

            np.state = ProcessState::RUNNABLE;
        }
    } else {
        drop(guard);
        return -1;
    }

    drop(guard);
    pid
}

pub fn exit(status: isize) {
    let cpu = mycpu();
    let curproc = unsafe { &mut *cpu.process.unwrap() };

    crate::info!("Exit: pid={} status={}", curproc.pid, status);

    // Close all open files
    // for fd in 0..NFILE { ... }

    let guard = PROCS_LOCK.lock();

    // Wake up parent
    unsafe {
        wakeup1(curproc.parent);
    }

    curproc.state = ProcessState::ZOMBIE;

    unsafe {
        sched(guard);
    }
    panic!("zombie exit");
}

pub fn wait(_pid: isize) -> isize {
    let cpu = mycpu();
    let curproc = unsafe { &mut *cpu.process.unwrap() };

    let mut guard = PROCS_LOCK.lock();
    loop {
        let mut have_kids = false;
        let mut child_pid: isize = -1;

        unsafe {
            for p in PROCS.iter_mut() {
                if p.parent == Some(curproc as *mut Process) {
                    have_kids = true;
                    if p.state == ProcessState::ZOMBIE {
                        // Found one
                        child_pid = p.pid as isize;

                        // Clean up
                        // kfree(p.kstack)
                        // freevm(p.pgdir)
                        p.kstack = core::ptr::null_mut();
                        p.pgdir = core::ptr::null_mut();
                        p.state = ProcessState::UNUSED;
                        p.pid = 0;
                        p.parent = None;
                        p.name = [0; 16];
                        p.killed = false;

                        break;
                    }
                }
            }
        }

        if child_pid != -1 {
            drop(guard);
            return child_pid;
        }

        if !have_kids || curproc.killed {
            drop(guard);
            return -1;
        }

        // Wait for children to exit (sleep on self)
        unsafe {
            // Manual sleep to avoid deadlock (sleep tries to acquire PROCS_LOCK)
            // We already hold PROCS_LOCK (guard), so just setup state and sched.
            curproc.chan = curproc as *mut Process as usize;
            curproc.state = ProcessState::SLEEPING;
            sched(guard);
            curproc.chan = 0;
            // sleep(curproc as *mut Process as usize, Some(guard));
        }
        guard = PROCS_LOCK.lock();
    }
}

unsafe fn wakeup1(chan: Option<*mut Process>) {
    // Only wake up processes sleeping on chan (in this case, parent pointer for wait)
    // Actually wait uses parent pointer as channel? Or simpler convention.
    // xv6 uses parent ptr.
    if let Some(c) = chan {
        for p in PROCS.iter_mut() {
            if p.state == ProcessState::SLEEPING && p.chan == c as usize {
                p.state = ProcessState::RUNNABLE;
            }
        }
    }
}

pub unsafe fn killed(p: &Process) -> bool {
    p.killed
}
