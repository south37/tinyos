use core::arch::asm;

pub const SYS_READ: usize = 0;
pub const SYS_WRITE: usize = 1;
pub const SYS_OPEN: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_SBRK: u64 = 12;
pub const SYS_FORK: usize = 57;
pub const SYS_EXEC: usize = 59;
pub const SYS_EXIT: usize = 60;
pub const SYS_WAIT: usize = 61;
pub const SYS_PIPE: usize = 22;
pub const SYS_DUP: usize = 32;

#[inline(always)]
pub unsafe fn syscall0(num: usize) -> usize {
    let ret: usize;
    asm!(
        "syscall",
        inout("rax") num => ret,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall1(num: usize, a1: usize) -> usize {
    let ret: usize;
    asm!(
        "syscall",
        inout("rax") num => ret,
        in("rdi") a1,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall2(num: usize, a1: usize, a2: usize) -> usize {
    let ret: usize;
    asm!(
        "syscall",
        inout("rax") num => ret,
        in("rdi") a1,
        in("rsi") a2,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall3(num: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    asm!(
        "syscall",
        inout("rax") num => ret,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        out("rcx") _,
        out("r11") _,
        options(nostack, preserves_flags)
    );
    ret
}

pub fn exit(status: i32) -> ! {
    unsafe {
        syscall1(SYS_EXIT, status as usize);
    }
    loop {}
}

pub fn write(fd: i32, buf: &[u8]) -> isize {
    unsafe { syscall3(SYS_WRITE, fd as usize, buf.as_ptr() as usize, buf.len()) as isize }
}

pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    unsafe { syscall3(SYS_READ, fd as usize, buf.as_mut_ptr() as usize, buf.len()) as isize }
}

pub fn fork() -> i32 {
    unsafe { syscall0(SYS_FORK) as i32 }
}

pub fn wait(status: Option<&mut i32>) -> i32 {
    unsafe {
        let ptr = status.map(|s| s as *mut i32 as usize).unwrap_or(0);
        syscall1(SYS_WAIT, ptr) as i32
    }
}

pub fn exec(path: *const u8, argv: &[*const u8]) -> i32 {
    // We need to convert &[&str] to null-terminated C-style array of pointers
    // This is tricky without allocation. User has to provide the buffer or we use variable stack.
    // However, existing exec takes char** argv.
    // The kernel likely expects pointers to null-terminated strings.
    // Since we are in no_std without heap, this is annoying.
    // For now, let's assume the user passes a proper layout or we construct it on stack if small?
    // The `init` and `sh` use static or stack arrays.

    // Actually, `exec` implementation in `init.c` passes `char *argv[] = {"sh", 0};`.
    // Rust strings are not null terminated.
    // We can provide a helper that just takes raw pointers if we want to be safe, or
    // we make the user create CStr-like references.

    // Let's expose a unsafe exec first that takes pointers.
    unsafe {
        // path must be null terminated
        syscall2(SYS_EXEC, path as usize, argv.as_ptr() as usize) as i32
    }
}

// Safer exec is hard without alloc.

pub fn open(path: &str, mode: i32) -> i32 {
    // We need null-terminated path.
    // Ideally we assume path is null-terminated or we copy to stack buffer.
    // For now, let's just pass pointer and hope user provided null-terminated or use a small buffer.
    // Since we can't allocate, we can't easily append null.
    // BUT we can check if it ends with null?
    // Rust strings are not null terminated.
    // Let's implement a small stack buffer copy for open since paths are usually short.
    let mut buf = [0u8; 128];
    if path.len() >= 128 {
        return -1;
    }
    for (i, b) in path.bytes().enumerate() {
        buf[i] = b;
    }
    buf[path.len()] = 0;

    unsafe { syscall2(SYS_OPEN as usize, buf.as_ptr() as usize, mode as usize) as i32 }
}

pub fn close(fd: i32) -> i32 {
    unsafe { syscall1(SYS_CLOSE as usize, fd as usize) as i32 }
}

pub fn sbrk(n: isize) -> isize {
    unsafe { syscall1(SYS_SBRK as usize, n as usize) as isize }
}

pub fn dup(fd: i32) -> i32 {
    unsafe { syscall1(SYS_DUP as usize, fd as usize) as i32 }
}

pub fn pipe(fds: &mut [i32; 2]) -> i32 {
    unsafe { syscall1(SYS_PIPE as usize, fds.as_mut_ptr() as usize) as i32 }
}
