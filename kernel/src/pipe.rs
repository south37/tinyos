use crate::spinlock::Spinlock;

pub const PIPESIZE: usize = 512;

pub struct Pipe {
    pub data: [u8; PIPESIZE],
    pub nread: usize,  // number of bytes read
    pub nwrite: usize, // number of bytes written
    pub readopen: bool,
    pub writeopen: bool,
}

impl Pipe {
    pub const fn new() -> Self {
        Self {
            data: [0; PIPESIZE],
            nread: 0,
            nwrite: 0,
            readopen: true,
            writeopen: true,
        }
    }
}

pub static PIPE_ALLOCATOR: Spinlock<()> = Spinlock::new((), "PIPE_ALLOCATOR");

pub fn pipealloc(f0: &mut crate::file::File, f1: &mut crate::file::File) -> Result<(), ()> {
    let mut allocator = crate::allocator::ALLOCATOR.lock();
    let p_ptr = allocator.kalloc();
    if p_ptr.is_null() {
        return Err(());
    }

    unsafe {
        *(p_ptr as *mut Spinlock<PipeData>) = Spinlock::new(PipeData::new(), "pipe");
    }

    f0.f_type = crate::file::FileType::Pipe;
    f0.readable = true;
    f0.writable = false;
    f0.pipe = Some(p_ptr as *mut Spinlock<PipeData>);

    f1.f_type = crate::file::FileType::Pipe;
    f1.readable = false;
    f1.writable = true;
    f1.pipe = Some(p_ptr as *mut Spinlock<PipeData>);

    Ok(())
}

pub struct PipeData {
    pub data: [u8; PIPESIZE],
    pub nread: usize,
    pub nwrite: usize,
    pub readopen: bool,
    pub writeopen: bool,
}

impl PipeData {
    pub const fn new() -> Self {
        Self {
            data: [0; PIPESIZE],
            nread: 0,
            nwrite: 0,
            readopen: true,
            writeopen: true,
        }
    }
}

pub fn pipeclose(pi: *mut Spinlock<PipeData>, writable: bool) {
    if pi.is_null() {
        return;
    }
    let mut p = unsafe { (*pi).lock() };

    if writable {
        p.writeopen = false;
        crate::proc::wakeup(pi as usize + 1); // Wakeup readers (nwrite changed effectively)
    } else {
        p.readopen = false;
        crate::proc::wakeup(pi as usize + 1); // Wakeup writers
    }

    if !p.readopen && !p.writeopen {
        // Free pipe
        drop(p);
        // We need to call kfree(pi).
        // BUT `fileclose` calls this.
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        unsafe {
            allocator.kfree(pi as usize);
        }
    } else {
        drop(p);
    }
}

pub fn pipewrite(pi: *mut Spinlock<PipeData>, addr: u64, mut n: usize) -> isize {
    if pi.is_null() {
        return -1;
    }

    crate::debug!("pipewrite: entry pi={:?} n={}", pi, n);
    let mut p = unsafe { (*pi).lock() };
    let mut written = 0;
    let pgdir = unsafe { (*crate::proc::mycpu().process.unwrap()).pgdir };

    while n > 0 {
        if !p.readopen {
            crate::debug!("pipewrite: read closed");
            return -1; // memory leak? user process problem
        }

        if p.nwrite == p.nread + PIPESIZE {
            // Full
            crate::debug!("pipewrite: full, sleeping");
            crate::proc::wakeup(pi as usize + 1); // Wakeup readers
            crate::proc::sleep(pi as usize + 1, Some(p)); // Sleep on nwrite/nread change
            crate::debug!("pipewrite: woke up");
            p = unsafe { (*pi).lock() }; // Reacquire
        } else {
            // Write chunk
            let idx = p.nwrite % PIPESIZE;
            let space = PIPESIZE - (p.nwrite - p.nread);
            let chunk = core::cmp::min(n, space);
            let chunk = core::cmp::min(chunk, PIPESIZE - idx); // Handle wrapping

            {
                let mut allocator = crate::allocator::ALLOCATOR.lock();
                if !crate::vm::copyin(
                    pgdir,
                    &mut allocator,
                    &mut p.data[idx] as *mut u8,
                    addr + written as u64,
                    chunk,
                ) {
                    return -1;
                }
            }

            p.nwrite += chunk;
            written += chunk;
            n -= chunk;
        }
    }
    crate::proc::wakeup(pi as usize + 1);
    crate::debug!("pipewrite: exit written={}", written);
    written as isize
}

pub fn piperead(pi: *mut Spinlock<PipeData>, addr: u64, mut n: usize) -> isize {
    if pi.is_null() {
        return -1;
    }

    crate::debug!("piperead: entry pi={:?} n={}", pi, n);
    let mut p = unsafe { (*pi).lock() };
    let pgdir = unsafe { (*crate::proc::mycpu().process.unwrap()).pgdir };

    while p.nread == p.nwrite && p.writeopen {
        crate::debug!("piperead: empty, sleeping");
        let process_ptr = crate::proc::mycpu().process.unwrap() as *const crate::proc::Process;
        // Convert *const Process to &Process unsafe
        if unsafe { crate::proc::killed(&*process_ptr) } {
            return -1;
        }
        crate::proc::sleep(pi as usize + 1, Some(p));
        crate::debug!("piperead: woke up");
        p = unsafe { (*pi).lock() };
    }

    let mut read = 0;
    while n > 0 && p.nread < p.nwrite {
        // Read chunk
        let idx = p.nread % PIPESIZE;
        let available = p.nwrite - p.nread;
        let chunk = core::cmp::min(n, available);
        let chunk = core::cmp::min(chunk, PIPESIZE - idx); // Handle wrapping

        {
            let mut allocator = crate::allocator::ALLOCATOR.lock();
            if !crate::vm::copyout(
                pgdir,
                &mut allocator,
                addr + read as u64,
                &p.data[idx] as *const u8,
                chunk,
            ) {
                return -1;
            }
        }

        p.nread += chunk;
        read += chunk;
        n -= chunk;
    }

    crate::proc::wakeup(pi as usize + 1);
    crate::debug!("piperead: exit read={}", read);
    read as isize
}
