pub const KERNBASE: usize = 0xFFFFFFFF80000000; // First kernel virtual address
pub const DEVBASE: usize = 0xFFFFFFFF40000000; // First device virtual address

pub const DEVSPACE: usize = 0xFE000000; // First device physical address
pub const IOAPIC_ADDR: usize = 0xFEC00000;
pub const LAPIC_ADDR: usize = 0xFEE00000;

pub const PG_SIZE: usize = 4096;

pub fn p2v(x: usize) -> usize {
    x + KERNBASE
}

pub fn v2p(x: usize) -> usize {
    x - KERNBASE
}

pub fn io2v(x: usize) -> usize {
    x - DEVSPACE + DEVBASE
}

pub const T_IRQ0: u32 = 32;
pub const IRQ_TIMER: u32 = 0;
pub const IRQ_ERROR: u32 = 19;
