#![allow(dead_code)]

use crate::util::{IRQ_ERROR, IRQ_TIMER, LAPIC_ADDR, T_IRQ0};

// Local APIC registers
const ID: u32 = 0x0020; // ID
const VER: u32 = 0x0030; // Version
const TPR: u32 = 0x0080; // Task Priority
const EOI: u32 = 0x00B0; // EOI
const SVR: u32 = 0x00F0; // Spurious Interrupt Vector
const ESR: u32 = 0x0280; // Error Status
const ICRLO: u32 = 0x0300; // Interrupt Command
const ICRHI: u32 = 0x0310; // Interrupt Command [63:32]
const TIMER: u32 = 0x0320; // Local Vector Table 0 (TIMER)
const PCINT: u32 = 0x0340; // Performance Counter LVT
const LINT0: u32 = 0x0350; // Local Vector Table 1 (LINT0)
const LINT1: u32 = 0x0360; // Local Vector Table 2 (LINT1)
const ERROR: u32 = 0x0370; // Local Vector Table 3 (ERROR)
const TICR: u32 = 0x0380; // Timer Initial Count
const TCCR: u32 = 0x0390; // Timer Current Count
const TDCR: u32 = 0x03E0; // Timer Divide Configuration

const MASKED: u32 = 0x10000;

pub fn init() {
    let lapic = crate::util::io2v(LAPIC_ADDR);

    unsafe {
        // Enable local APIC; set spurious interrupt vector.
        write(lapic, SVR, 0x100 | (T_IRQ0 + 255));

        // The timer repeatedly counts down at bus frequency
        // from lapic[TICR] and then issues an interrupt.
        // If we weren't driven by interrupt (e.g. context switch),
        // we would need to tune this.
        write(lapic, TDCR, 0x0B); // Divide by 1
        write(lapic, TIMER, 0x20000 | (T_IRQ0 + IRQ_TIMER)); // Periodic
        write(lapic, TICR, 10000000);

        // Disable logical interrupt lines.
        write(lapic, LINT0, MASKED);
        write(lapic, LINT1, MASKED);

        // Disable performance counter overflow interrupts
        // on machines that provide that interrupt entry.
        if ((read(lapic, VER) >> 16) & 0xFF) >= 4 {
            write(lapic, PCINT, MASKED);
        }

        // Map error interrupt to IRQ_ERROR.
        write(lapic, ERROR, T_IRQ0 + IRQ_ERROR);

        // Clear error status register (requires back-to-back writes).
        write(lapic, ESR, 0);
        write(lapic, ESR, 0);

        // Ack any outstanding interrupts.
        write(lapic, EOI, 0);

        // Send an Init Level De-Assert to synchronise arbitration ID's.
        write(lapic, ICRHI, 0);
        write(lapic, ICRLO, 0x80000 | 0x0500 | 0x8000); // BCAST | INIT | LEVEL

        // Wait for the send to finish.
        while read(lapic, ICRLO) & 0x1000 != 0 {}

        // Enable interrupts on the APIC (but not on the processor).
        write(lapic, TPR, 0);
    }
}

pub fn eoi() {
    let lapic = crate::util::io2v(LAPIC_ADDR);
    unsafe {
        write(lapic, EOI, 0);
    }
}

unsafe fn write(lapic: usize, reg: u32, val: u32) {
    unsafe {
        core::ptr::write_volatile((lapic + reg as usize) as *mut u32, val);
        core::ptr::read_volatile((lapic + ID as usize) as *const u32); // Wait for write to finish
    }
}

unsafe fn read(lapic: usize, reg: u32) -> u32 {
    unsafe { core::ptr::read_volatile((lapic + reg as usize) as *const u32) }
}
