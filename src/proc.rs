#![allow(static_mut_refs)]

use crate::allocator::Allocator;
use crate::gdt::{UCODE_SELECTOR, UDATA_SELECTOR};
use crate::trap::TrapFrame;
use crate::uart_println;
use crate::util::PG_SIZE;
use crate::vm::{self, PageTable, PageTableEntry};
use core::arch::global_asm;

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

#[derive(Clone, Copy)]
pub struct Process {
    pub state: ProcessState,
    pub kstack: *mut u8,
    pub context: *mut Context,
    pub pgdir: *mut PageTable,
    pub pid: usize,
    pub chan: usize,
    pub name: [u8; 16],
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
        }
    }
}

pub static mut PROCS: [Process; NPROC] = [Process::new(); NPROC];
static mut PID_COUNTER: usize = 0;
pub static mut CURRENT_PROCESS: Option<&mut Process> = None;

use crate::spinlock::SpinlockGuard;

pub fn sleep<T>(chan: usize, guard: Option<SpinlockGuard<T>>) {
    unsafe {
        if let Some(p) = CURRENT_PROCESS.as_deref_mut() {
            p.chan = chan;
            p.state = ProcessState::SLEEPING;
        }

        // Release lock if provided
        if let Some(g) = guard {
            drop(g);
        }

        // Swtch needs scheduler context.
        if let Some(p) = CURRENT_PROCESS.as_mut() {
            swtch(&mut p.context as *mut _, SCHEDULER_CONTEXT);
        }
    }
}

pub fn wakeup(chan: usize) {
    unsafe {
        for p in PROCS.iter_mut() {
            if p.state == ProcessState::SLEEPING && p.chan == chan {
                p.state = ProcessState::RUNNABLE;
                p.chan = 0;
            }
        }
    }
}

unsafe extern "C" {
    fn swtch(old: *mut *mut Context, new: *mut Context);
    fn trapret();
}

// Save callee-saved registers and switch stack
// rdi -> old context
// rsi -> new context
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
    let mut p: Option<&mut Process> = None;
    unsafe {
        for proc in PROCS.iter_mut() {
            if proc.state == ProcessState::UNUSED {
                p = Some(proc);
                break;
            }
        }
    }

    if let Some(p) = p {
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

        // Init code (jmp $)
        // int 0x40; jmp $
        // 0xCD 0x40 0xEB 0xFE
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

        // Set context to return to trapret
        unsafe {
            (*p.context).rip = trapret as *const () as usize as u64;
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
    }
}

// Scheduler context (per-cpu). For now just a static variable?
// Since we are single core and running on kstack of current process or scheduler loop.
// We need a place to save the scheduler's own context when we switch TO a process.
static mut SCHEDULER_CONTEXT: *mut Context = core::ptr::null_mut();

pub fn scheduler() {
    uart_println!("Scheduler starting...");
    loop {
        let mut ran_process = false;
        unsafe {
            for i in 0..NPROC {
                let p = &mut PROCS[i];
                if p.state == ProcessState::RUNNABLE {
                    p.state = ProcessState::RUNNING;

                    CURRENT_PROCESS = Some(p);

                    // Switch to user page table
                    let p_ptr = CURRENT_PROCESS.as_mut().unwrap();
                    vm::switch(p_ptr.pgdir);

                    // Set Kernel Stack in TSS
                    let kstack_top = p_ptr.kstack as usize + KSTACK_SIZE;
                    crate::gdt::set_kernel_stack(kstack_top as u64);

                    // Switch to process
                    swtch(core::ptr::addr_of_mut!(SCHEDULER_CONTEXT), p_ptr.context);

                    // Back from process
                    CURRENT_PROCESS = None;

                    ran_process = true;
                }
            }
        }
        if !ran_process {
            // Enable interrupts to allow IRQs to wake us up
            unsafe { core::arch::asm!("sti") };
            // Wait for interrupt
            unsafe { core::arch::asm!("hlt") };
        }
    }
}
