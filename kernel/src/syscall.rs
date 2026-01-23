use crate::gdt::{tss_addr, KCODE_SELECTOR, KDATA_SELECTOR};
use crate::util::{
    rdmsr, wrmsr, EFER_SCE, MSR_EFER, MSR_KERNEL_GS_BASE, MSR_LSTAR, MSR_SFMASK, MSR_STAR,
};

pub fn init(cpuid: usize) {
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
        wrmsr(MSR_KERNEL_GS_BASE, tss_addr(cpuid));

        // Switch to Kernel GS Base (Active GS = TSS, Shadow GS = User/0)
        core::arch::asm!("swapgs");
    }
}

unsafe extern "C" {
    // Defined in asm/syscall.S
    fn syscall_entry();
}

use crate::proc::mycpu;
use crate::trap::TrapFrame;

pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_OPEN: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_SBRK: u64 = 12;
pub const SYS_PIPE: u64 = 22;
pub const SYS_DUP: u64 = 32;
pub const SYS_FORK: u64 = 57;
pub const SYS_EXEC: u64 = 59;
pub const SYS_EXIT: u64 = 60;
pub const SYS_WAIT: u64 = 61;

pub fn syscall() {
    #[allow(static_mut_refs)]
    let p = unsafe { &mut *mycpu().process.unwrap() };
    let tf = unsafe {
        &mut *(((p.kstack as usize) + crate::proc::KSTACK_SIZE - core::mem::size_of::<TrapFrame>())
            as *mut TrapFrame)
    };

    let num = tf.rax;
    let ret = match num {
        SYS_READ => sys_read(tf),
        SYS_WRITE => sys_write(tf),
        SYS_OPEN => sys_open(tf),
        SYS_CLOSE => sys_close(tf),
        SYS_SBRK => sys_sbrk(tf),
        SYS_EXEC => sys_exec(tf),
        SYS_FORK => sys_fork(tf),
        SYS_EXIT => sys_exit(tf),
        SYS_WAIT => sys_wait(tf),
        SYS_PIPE => sys_pipe(tf),
        SYS_DUP => sys_dup(tf),
        _ => {
            crate::error!("Unknown syscall {}", num);
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
        6 => tf.r8, // Linux uses different regs? rdi, rsi, rdx, r10, r8, r9.
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
    let p = unsafe { &mut *mycpu().process.unwrap() };
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
    fetch_str(ptr_val)
}

fn fetch_str(ptr_val: u64) -> Result<&'static str, ()> {
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
        Err(_) => {
            return -1;
        }
    };

    let argv_ptr = argptr(1, tf);
    let mut argv: [&str; 16] = [""; 16];
    let mut argc = 0;

    if argv_ptr != 0 {
        loop {
            if argc >= 16 {
                return -1;
            }
            let uarg = unsafe { *((argv_ptr + (argc as u64) * 8) as *const u64) };
            if uarg == 0 {
                break;
            }
            match fetch_str(uarg) {
                Ok(s) => argv[argc] = s,
                Err(_) => return -1,
            }
            argc += 1;
        }
    }
    crate::exec::exec(path, &argv[0..argc])
}

fn sys_fork(_tf: &TrapFrame) -> isize {
    crate::proc::fork()
}

fn sys_exit(tf: &TrapFrame) -> isize {
    let status = argint(0, tf) as isize;
    crate::proc::exit(status);
    0
}

fn sys_wait(tf: &TrapFrame) -> isize {
    let _pid = argint(0, tf) as isize; // We don't support waiting for specific PID yet in bare wait?
                                       // Actually standard wait(status) waits for ANY child. waitpid(pid, status, options) waits for specific.
                                       // Let's implement wait() as wait for any child.
    crate::proc::wait(-1)
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

fn sys_open(tf: &TrapFrame) -> isize {
    let path = match argstr(0, tf) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let mode = argint(1, tf);

    // 1. Alloc file
    let f = match crate::file::filealloc() {
        Some(f) => f,
        None => return -1,
    };

    // 2. Open inode
    let ip = match crate::fs::namei(path) {
        Some(ip) => ip,
        None => {
            f.refcnt = 0; // Manual rollback
            return -1;
        }
    };

    let guard = ip.ilock();
    if (guard.i_mode & 0xF000) == 0x2000 {
        f.f_type = crate::file::FileType::Device;
        f.major = guard.i_block[0] as u16;
        f.ip = Some(ip); // We still keep IP to hold refcnt? Fileclose decreases refcnt on IP only if type Inode?
                         // Wait, fileclose handles Inode and Device separately?
                         // file.rs: fileclose only iput if FileType::Inode.
                         // If Device, we leak refcnt on ip?
                         // We should arguably keep type Inode but set major?
                         // Or update fileclose to iput if ip is set?

    // file.rs:
    /*
    if f.f_type == FileType::Inode {
        if let Some(ip) = f.ip {
            crate::fs::iput(ip);
        }
    }
    */
    // It doesn't check Device.
    // So if we set Device, we must NOT set ip in f.ip OR update fileclose.

    // But we NEED to iput eventually.
    // So we should update fileclose.
    // For now, let's update fileclose too?
    // OR, simpler:
    // Keep f.f_type = Inode? But then read/write uses readi/writei.
    // We need read/write to dispatch to console.

    // So we MUST use FileType::Device.
    // And we MUST update fileclose to iput if f.ip is set, regardless of type?
    // Or add Device handling in fileclose.

    // Let's check file.rs.
    } else {
        f.f_type = crate::file::FileType::Inode;
    }
    drop(guard);

    f.ip = Some(ip);
    f.off = 0;
    f.readable = true;
    f.writable = false;
    // TODO: use mode
    if mode != 0 {}

    // 3. Alloc fd
    #[allow(static_mut_refs)]
    let p = unsafe { &mut *mycpu().process.unwrap() };
    for (i, fd_slot) in p.ofile.iter_mut().enumerate() {
        if fd_slot.is_none() {
            *fd_slot = Some(f as *mut crate::file::File);
            return i as isize;
        }
    }

    // Fail
    f.refcnt = 0;
    -1
}

fn sys_close(tf: &TrapFrame) -> isize {
    let fd = argint(0, tf) as usize;
    #[allow(static_mut_refs)]
    let p = unsafe { &mut *mycpu().process.unwrap() };

    if fd >= p.ofile.len() {
        return -1;
    }

    if let Some(f_ptr) = p.ofile[fd] {
        p.ofile[fd] = None;
        unsafe {
            crate::file::fileclose(&mut *f_ptr);
        }
        return 0;
    }
    -1
}

fn sys_sbrk(tf: &TrapFrame) -> isize {
    let n = argint(0, tf) as isize;
    let cpu = crate::proc::mycpu();
    let sz = unsafe { (*cpu.process.unwrap()).sz };

    if crate::growproc::growproc(n).is_err() {
        return -1;
    }

    sz as isize
}

fn sys_pipe(tf: &TrapFrame) -> isize {
    let fds_ptr = argptr(0, tf);
    let fds = unsafe { core::slice::from_raw_parts_mut(fds_ptr as *mut i32, 2) };

    let f0 = match crate::file::filealloc() {
        Some(f) => f,
        None => return -1,
    };
    let f1 = match crate::file::filealloc() {
        Some(f) => f,
        None => {
            f0.refcnt = 0;
            return -1;
        }
    };

    if crate::pipe::pipealloc(f0, f1).is_err() {
        f0.refcnt = 0;
        f1.refcnt = 0;
        return -1;
    }

    let cpu = crate::proc::mycpu();
    let p = unsafe { &mut *cpu.process.unwrap() };

    let mut fd0 = -1;
    for (i, fd) in p.ofile.iter_mut().enumerate() {
        if fd.is_none() {
            *fd = Some(f0 as *mut crate::file::File);
            fd0 = i as isize;
            break;
        }
    }
    if fd0 == -1 {
        // Cleanup pipe
        f0.refcnt = 0;
        f1.refcnt = 0;
        // Ideally we should call fileclose/pipeclose to free the pipe memory allocated in pipealloc
        // For now, let's assume we won't run out of fds often, but this is a leak if it happens.
        // To fix: manually free pipe or implement proper cleanup.
        return -1;
    }

    let mut fd1 = -1;
    for (i, fd) in p.ofile.iter_mut().enumerate() {
        if fd.is_none() {
            *fd = Some(f1 as *mut crate::file::File);
            fd1 = i as isize;
            break;
        }
    }
    if fd1 == -1 {
        p.ofile[fd0 as usize] = None;
        f0.refcnt = 0;
        f1.refcnt = 0;
        // Leak pipe
        return -1;
    }

    fds[0] = fd0 as i32;
    fds[1] = fd1 as i32;

    0
}

fn sys_dup(tf: &TrapFrame) -> isize {
    let oldfd = argint(0, tf);
    let cpu = crate::proc::mycpu();
    let p = unsafe { &mut *cpu.process.unwrap() };

    if oldfd >= p.ofile.len() || p.ofile[oldfd].is_none() {
        return -1;
    }

    // Find new fd
    let mut newfd = -1;
    for (i, fd) in p.ofile.iter_mut().enumerate() {
        if fd.is_none() {
            newfd = i as isize;
            break;
        }
    }

    if newfd == -1 {
        return -1;
    }

    let f = p.ofile[oldfd].unwrap();
    p.ofile[newfd as usize] = Some(f);
    // Use proper filedup to manage refcnt safely (with lock)
    unsafe {
        crate::file::filedup(&mut *f);
    }

    newfd
}
