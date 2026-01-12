use crate::fs::Inode;
use crate::spinlock::Spinlock;

pub const NFILE: usize = 100; // Open files per system

#[derive(Clone, Copy, PartialEq)]
pub enum FileType {
    None,
    Pipe,
    Inode,
    Device,
}

#[derive(Clone, Copy)]
pub struct File {
    pub f_type: FileType,
    pub refcnt: usize,
    pub readable: bool,
    pub writable: bool,
    pub pipe: usize, // Placeholder for pipe
    pub ip: Option<&'static Inode>,
    pub off: u32,
    pub major: u16, // For devices
}

impl File {
    pub const fn new() -> Self {
        Self {
            f_type: FileType::None,
            refcnt: 0,
            readable: false,
            writable: false,
            pipe: 0,
            ip: None,
            off: 0,
            major: 0,
        }
    }
}

pub struct FileTable {
    pub files: [File; NFILE],
}

pub static FTABLE: Spinlock<FileTable> = Spinlock::new(FileTable {
    files: [File::new(); NFILE],
});

pub fn filealloc() -> Option<&'static mut File> {
    let mut ft = FTABLE.lock();
    for f in ft.files.iter_mut() {
        if f.refcnt == 0 {
            f.refcnt = 1;
            return Some(unsafe { &mut *(f as *mut File) });
        }
    }
    None
}

pub fn fileclose(f: &mut File) {
    let mut ft = FTABLE.lock();
    if f.refcnt < 1 {
        panic!("fileclose");
    }
    f.refcnt -= 1;
    if f.refcnt > 0 {
        return;
    }

    if f.f_type == FileType::Inode {
        if let Some(ip) = f.ip {
            crate::fs::iput(ip);
        }
    }

    f.f_type = FileType::None;
    f.ip = None;
    drop(ft);
}

pub fn filestat(_f: &File, _addr: u64) -> isize {
    // TODO: Implement
    -1
}

pub fn fileread(f: &mut File, addr: u64, n: usize) -> isize {
    if !f.readable {
        return -1;
    }

    match f.f_type {
        FileType::Pipe => {
            // TODO
            -1
        }
        FileType::Device => {
            if f.major == 1 {
                // Console
                return crate::console::consoleread(addr, n) as isize;
            }
            -1
        }
        FileType::Inode => {
            if let Some(ip) = f.ip {
                // We need to implement writei/readi that takes user address?
                // Currently readi takes kernel address.
                // For now, let's assume we can copy traits or something.
                // Actually readi takes *mut u8. We need to check user buffer validity.

                // For simplicity, let's just use readi with a temporary kernel buffer call copyout,
                // OR we trust the address for now (since we don't have user/kernel separation fully enforced yet with map_pages for user buffers mapped in kernel).
                // Wait, user pages are accessible if we are in kernel and they are mapped.
                // But typically we use `copyout`/`copyin`.

                let res = crate::fs::readi(ip, addr as *mut u8, f.off, n as u32);
                if res > 0 {
                    f.off += res;
                }
                res as isize
            } else {
                -1
            }
        }
        _ => -1,
    }
}

pub fn filewrite(f: &mut File, addr: u64, n: usize) -> isize {
    if !f.writable {
        return -1;
    }

    match f.f_type {
        FileType::Pipe => {
            // TODO
            -1
        }
        FileType::Device => {
            if f.major == 1 {
                // Console
                return crate::console::consolewrite(addr, n) as isize;
            }
            -1
        }
        FileType::Inode => {
            if let Some(ip) = f.ip {
                // TODO include Transaction?
                let res = crate::fs::writei(ip, addr as *const u8, f.off, n as u32);
                if res > 0 {
                    f.off += res;
                }
                res as isize
            } else {
                -1
            }
        }
        _ => -1,
    }
}
