use crate::fs::Inode;
use crate::pipe::PipeData;
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
    pub pipe: Option<*mut Spinlock<PipeData>>,
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
            pipe: None,
            ip: None,
            off: 0,
            major: 0,
        }
    }
}

pub struct FileTable {
    pub files: [File; NFILE],
}

pub static FTABLE: Spinlock<FileTable> = Spinlock::new(
    FileTable {
        files: [File::new(); NFILE],
    },
    "FTABLE",
);

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

pub fn filedup(f: &mut File) -> &mut File {
    let _ft = FTABLE.lock(); // Lock table to protect refcnt?
                             // refcnt is not atomic, so we need lock.
                             // wait, FTABLE lock protects file allocation.
                             // manipulating refcnt of allocated file should probably be protected too if we don't have per-file lock.
                             // FTABLE lock is coarse grained but safe.
    if f.refcnt < 1 {
        panic!("filedup");
    }
    f.refcnt += 1;
    f
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

    if f.f_type == FileType::Inode || f.f_type == FileType::Device {
        if let Some(ip) = f.ip {
            crate::fs::iput(ip);
        }
    }

    if f.f_type == FileType::Pipe {
        if let Some(pi) = f.pipe {
            crate::pipe::pipeclose(pi, f.writable);
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
            if let Some(pi) = f.pipe {
                return crate::pipe::piperead(pi, addr, n);
            }
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
            if let Some(pi) = f.pipe {
                return crate::pipe::pipewrite(pi, addr, n);
            }
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
