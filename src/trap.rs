use crate::gdt::KCODE_SELECTOR;
use crate::uart_println;
use crate::util::{IRQ_TIMER, IRQ_UART, IRQ_VIRTIO, T_IRQ0, T_SYSCALL};

pub fn init() {
    unsafe {
        for i in 0..256 {
            let addr = vectors[i];
            IDT[i] = GateDesc {
                offset_low: (addr & 0xFFFF) as u16,
                selector: KCODE_SELECTOR,
                ist: 0,
                type_attr: 0x8E, // Type=0xE (Interrupt Gate), DPL=0, P=1 => 0x8E
                offset_mid: ((addr >> 16) & 0xFFFF) as u16,
                offset_high: (addr >> 32) as u32,
                reserved: 0,
            };
        }
        // Syscall gate (DPL=3)
        // For now, let's keep it as interrupt gate but allow user (DPL=3)
        // Type=0xE (Interrupt Gate), DPL=3, P=1 => 0xEE.
        // TODO: Use 64-bit Trap Gate (= 0xF).
        IDT[T_SYSCALL as usize].type_attr = 0xEE;

        let idtr = Idtr {
            limit: (core::mem::size_of::<[GateDesc; 256]>() - 1) as u16,
            base: core::ptr::addr_of!(IDT) as u64,
        };
        core::arch::asm!("lidt [{}]", in(reg) &idtr, options(nostack));
    }
}

#[repr(C)]
pub struct TrapFrame {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub trap_num: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct GateDesc {
    offset_low: u16,
    selector: u16, // code segment selector
    ist: u8,
    type_attr: u8, // type and attributes
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

static mut IDT: [GateDesc; 256] = [GateDesc {
    offset_low: 0,
    selector: 0,
    ist: 0,
    type_attr: 0,
    offset_mid: 0,
    offset_high: 0,
    reserved: 0,
}; 256];

unsafe extern "C" {
    // Trap handler vector table. Defined in asm/vectors.S.
    static vectors: [u64; 256];
}

#[unsafe(no_mangle)]
extern "C" fn trap_handler(tf: &mut TrapFrame) {
    match tf.trap_num {
        n if n == (T_IRQ0 + IRQ_TIMER) as u64 => {
            crate::lapic::eoi();
            crate::proc::yield_proc();
        }
        n if n == (T_IRQ0 + IRQ_UART) as u64 => {
            crate::uart::uartintr();
            crate::lapic::eoi();
        }
        n if n == (T_IRQ0 + IRQ_VIRTIO) as u64 => {
            unsafe { crate::virtio::intr() };
            crate::lapic::eoi();
        }
        n if n == T_SYSCALL as u64 => {
            crate::syscall::syscall();
        }
        _ => {
            uart_println!("Trap {} on CPU {}", tf.trap_num, crate::lapic::id());
            uart_println!("Error Code: {:x}", tf.error_code);
            uart_println!("RIP: {:x}", tf.rip);
            uart_println!("CS: {:x}", tf.cs);
            uart_println!("CR2: {:x}", unsafe { crate::util::rcr2() });
            // Infinite loop on unhandled trap
            loop {}
        }
    }
}
