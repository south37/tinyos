pub const KERNBASE: usize = 0xFFFFFFFF80000000; // First kernel virtual address
pub const DEVBASE: usize = 0xFFFFFFFF40000000; // First device virtual address

pub const DEVSPACE: usize = 0xFE000000; // First device physical address
pub const IOAPIC_ADDR: usize = 0xFEC00000;
pub const LAPIC_ADDR: usize = 0xFEE00000;

pub const PHYS_MEM: usize = 256 * 1024 * 1024; // 256MB

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

pub const T_SYSCALL: u32 = 64; // system call

pub const T_IRQ0: u32 = 32;
pub const IRQ_TIMER: u32 = 0;
pub const IRQ_VIRTIO: u32 = 11;
pub const IRQ_ERROR: u32 = 19;

pub unsafe fn stosq(addr: *mut u64, val: u64, count: usize) {
    unsafe {
        core::arch::asm!(
            "rep stosq",
            inout("rdi") addr => _,
            inout("rcx") count => _,
            in("rax") val,
        );
    }
}

pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") val);
    }
}

pub unsafe fn inb(port: u16) -> u8 {
    let mut ret;
    unsafe {
        core::arch::asm!("in al, dx", out("al") ret, in("dx") port);
    }
    ret
}

pub unsafe fn outw(port: u16, val: u16) {
    unsafe {
        core::arch::asm!("out dx, ax", in("dx") port, in("ax") val);
    }
}

pub unsafe fn inw(port: u16) -> u16 {
    let mut ret;
    unsafe {
        core::arch::asm!("in ax, dx", out("ax") ret, in("dx") port);
    }
    ret
}

pub unsafe fn outl(port: u16, val: u32) {
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") val);
    }
}

pub unsafe fn inl(port: u16) -> u32 {
    let mut ret;
    unsafe {
        core::arch::asm!("in eax, dx", out("eax") ret, in("dx") port);
    }
    ret
}
