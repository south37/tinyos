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

use crate::proc::CURRENT_PROCESS;
use crate::trap::TrapFrame;
use crate::uart_println;

pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_EXEC: u64 = 59; // Linux execve is 59

pub fn syscall() {
    #[allow(static_mut_refs)]
    let p = unsafe { CURRENT_PROCESS.as_mut().unwrap() };
    let tf = unsafe {
        &mut *(((p.kstack as usize) + crate::proc::KSTACK_SIZE - core::mem::size_of::<TrapFrame>())
            as *mut TrapFrame)
    };

    let num = tf.rax;
    uart_println!("DEBUG: Syscall: {}", num);

    let ret = match num {
        SYS_READ => sys_read(tf),
        SYS_WRITE => sys_write(tf),
        SYS_EXEC => sys_exec(tf),
        _ => {
            uart_println!("Unknown syscall {}", num);
            -1
        }
    };

    tf.rax = ret as u64;
}

fn argraw(n: usize, tf: &TrapFrame) -> u64 {
    match n {
        0 => tf.rdi,
        1 => tf.rsi,
        2 => tf.rdx,
        3 => tf.r10,
        4 => tf.r8,
        5 => tf.r9,
        _ => panic!("argraw"),
    }
}

fn argint(n: usize, tf: &TrapFrame) -> usize {
    argraw(n, tf) as usize
}

fn argptr(n: usize, tf: &TrapFrame) -> u64 {
    argraw(n, tf)
}

fn argfd(n: usize, tf: &TrapFrame) -> Result<&'static mut crate::file::File, ()> {
    let fd = argint(n, tf);
    #[allow(static_mut_refs)]
    let p = unsafe { CURRENT_PROCESS.as_mut().unwrap() };
    if fd >= p.ofile.len() {
        return Err(());
    }
    match p.ofile[fd] {
        Some(f_ptr) => unsafe { Ok(&mut *f_ptr) },
        None => Err(()),
    }
}

fn argstr(n: usize, tf: &TrapFrame) -> Result<&str, ()> {
    // Fetch nth argument as string pointer
    let ptr_val = argptr(n, tf);

    // Verify pointer (very basic verification)
    if ptr_val == 0 {
        return Err(());
    }

    // Find length
    let mut len = 0;
    loop {
        let b = unsafe { *((ptr_val + len) as *const u8) };
        if b == 0 {
            break;
        }
        len += 1;
        if len > 1024 {
            return Err(());
        } // Max string length
    }

    let slice = unsafe { core::slice::from_raw_parts(ptr_val as *const u8, len as usize) };
    core::str::from_utf8(slice).map_err(|_| ())
}

fn sys_exec(tf: &TrapFrame) -> isize {
    let path = match argstr(0, tf) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    // Argv ignored for now
    crate::exec::exec(path, &[])
}

fn sys_read(tf: &TrapFrame) -> isize {
    let f = match argfd(0, tf) {
        Ok(f) => f,
        Err(_) => return -1,
    };
    let ptr = argptr(1, tf);
    let n = argint(2, tf);
    crate::file::fileread(f, ptr, n)
}

fn sys_write(tf: &TrapFrame) -> isize {
    let f = match argfd(0, tf) {
        Ok(f) => f,
        Err(_) => return -1,
    };
    let ptr = argptr(1, tf);
    let n = argint(2, tf);
    crate::file::filewrite(f, ptr, n)
}
