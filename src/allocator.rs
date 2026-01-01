use crate::PG_SIZE;

pub struct Allocator {
    pub freelist: *const Run,
}

pub struct Run {
    pub next: *const Run,
}

impl Allocator {
    pub fn new() -> Self {
        Self {
            freelist: core::ptr::null(),
        }
    }

    pub fn init(&mut self, vstart: usize, vend: usize) {
        let mut p = pgroundup(vstart);

        while p + PG_SIZE <= vend {
            self.kfree(p);
            p += PG_SIZE;
        }
    }

    pub fn kfree(&mut self, addr: usize) {
        let run: &mut Run = unsafe { &mut *(addr as *mut Run) };
        run.next = self.freelist;
        self.freelist = run;
    }

    pub fn kalloc(&mut self) -> *mut u8 {
        let run = self.freelist;
        if run.is_null() {
            return core::ptr::null_mut();
        }
        unsafe {
            self.freelist = (*run).next;
            // Zero out run
            crate::util::stosq(run as *mut u64, 0, PG_SIZE / 8);
        }
        run as *mut u8
    }
}

fn pgroundup(sz: usize) -> usize {
    (sz + PG_SIZE - 1) & !(PG_SIZE - 1)
}
