#![allow(static_mut_refs)]

use crate::allocator::Allocator;
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
            name: [0; 16],
        }
    }
}

pub static mut PROCS: [Process; NPROC] = [Process::new(); NPROC];
static mut PID_COUNTER: usize = 0;

unsafe extern "C" {
    fn swtch(old: *mut *mut Context, new: *mut Context);
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

        let sp = p.kstack as usize + KSTACK_SIZE;

        // Setup context
        // Leave space for Context struct on stack
        let context_addr = sp - core::mem::size_of::<Context>();
        p.context = context_addr as *mut Context;

        // Init code (jmp $)
        // 0xEB 0xFE
        let initcode: [u8; 2] = [0xeb, 0xfe];
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

        // Set rip to 0 (where we mapped the code)
        unsafe {
            (*p.context).rip = 0;
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
            for p in PROCS.iter_mut() {
                if p.state == ProcessState::RUNNABLE {
                    p.state = ProcessState::RUNNING;

                    // Switch to user page table
                    vm::uvm_switch(p.pgdir);

                    // Switch to process
                    swtch(core::ptr::addr_of_mut!(SCHEDULER_CONTEXT), p.context);

                    // Process yielded or was preempted (returns here)
                    // Currently our init_code loops forever, so it won't return unless we setup interrupts
                    // and yield.

                    // We assume p.context was updated by swtch (saved there)

                    p.state = ProcessState::RUNNABLE;
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
