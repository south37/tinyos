#![allow(dead_code)]

use crate::uart_println;
use crate::util::{IOAPIC_ADDR, IRQ_TIMER, T_IRQ0};

const REG_ID: u32 = 0x00;
const REG_VER: u32 = 0x01;
const REG_TABLE: u32 = 0x10;

const IOREGSEL: usize = 0x00;
const IOWIN: usize = 0x10;

pub fn init() {
    let ioapic_addr = crate::util::io2v(IOAPIC_ADDR);
    uart_println!("IOAPIC address: {:x}", ioapic_addr);

    // Get max entries from version register
    let ver = unsafe { read(ioapic_addr, REG_VER) };
    let maxintr = (ver >> 16) & 0xFF;
    uart_println!("IOAPIC max entries: {}", maxintr);

    // Mark all interrupts edge-triggered, active high, disabled,
    // and not routed to any CPUs.
    for i in 0..=maxintr {
        unsafe {
            write(ioapic_addr, REG_TABLE + 2 * i, 0x10000 | T_IRQ0 + i);
            write(ioapic_addr, REG_TABLE + 2 * i + 1, 0);
        }
    }
}

pub unsafe fn enable(irq: u32, cpu_id: u32) {
    let ioapic_addr = crate::util::io2v(IOAPIC_ADDR);
    // For now assuming CPU 0 or broadcast.
    // Write low 32 bits: vector = T_IRQ0 + irq, Mask = 0 (enabled).
    write(ioapic_addr, REG_TABLE + 2 * irq, T_IRQ0 + irq);

    // Write high 32 bits: destination APIC ID.
    // cpu_id << 24.
    write(ioapic_addr, REG_TABLE + 2 * irq + 1, cpu_id << 24);
}

unsafe fn read(base: usize, reg: u32) -> u32 {
    unsafe {
        core::ptr::write_volatile((base + IOREGSEL) as *mut u32, reg);
        core::ptr::read_volatile((base + IOWIN) as *const u32)
    }
}

unsafe fn write(base: usize, reg: u32, val: u32) {
    unsafe {
        core::ptr::write_volatile((base + IOREGSEL) as *mut u32, reg);
        core::ptr::write_volatile((base + IOWIN) as *mut u32, val);
    }
}
