use crate::gdt::{KCODE_SELECTOR, KDATA_SELECTOR, tss_addr};
use crate::util::{
    EFER_SCE, MSR_EFER, MSR_KERNEL_GS_BASE, MSR_LSTAR, MSR_SFMASK, MSR_STAR, rdmsr, wrmsr,
};

pub fn init() {
    unsafe {
        // Syscall Setup
        // 1. Enable EFER.SCE
        let efer = rdmsr(MSR_EFER);
        wrmsr(MSR_EFER, efer | EFER_SCE);

        // 2. Setup STAR
        // Bits 48-63: SYSRET CS and SS (User CS/SS).
        // Bits 32-47: SYSCALL CS and SS (Kernel CS/SS).
        let star = ((KDATA_SELECTOR | 3) as u64) << 48 | (KCODE_SELECTOR as u64) << 32;
        wrmsr(MSR_STAR, star);

        // 3. Setup LSTAR
        wrmsr(MSR_LSTAR, syscall_entry as u64);

        // 4. Setup SFMASK
        // Mask RFLAGS on syscall. Clear Interrupts (IF=0x200).
        wrmsr(MSR_SFMASK, 0x200);

        // 5. Setup KERNEL_GS_BASE
        // Point to TSS to find RSP0.
        wrmsr(MSR_KERNEL_GS_BASE, tss_addr());
    }
}

unsafe extern "C" {
    // Defined in asm/syscall.S
    fn syscall_entry();
}
