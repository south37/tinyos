use crate::allocator::Allocator;

use crate::util::{p2v, v2p, PG_SIZE};

static mut KPGDIR: *mut PageTable = core::ptr::null_mut();

pub fn init(allocator: &mut Allocator) {
    let pgdir = kvm_create(allocator).expect("kvm_create failed");
    unsafe {
        KPGDIR = pgdir;
    }
    switch(pgdir);
}

pub fn kpgdir() -> *mut PageTable {
    unsafe { KPGDIR }
}

pub fn kvm_create(allocator: &mut Allocator) -> Option<*mut PageTable> {
    let pgdir = allocator.kalloc() as *mut PageTable;
    if pgdir.is_null() {
        return None;
    }

    // Linear map. Virtual: [0, 0 + 1GiB) -> Physical: [0, 1GiB)
    let r = map_pages(
        pgdir,
        allocator,
        0,
        0,
        0x40000000, // 1GiB
        PageTableEntry::WRITABLE,
    );
    if !r {
        crate::error!("Linear map [0, 0 + 1GiB) failed");
        return None;
    }
    if !map_highmem(pgdir, allocator) {
        return None;
    }

    Some(pgdir)
}

fn map_highmem(pgdir: *mut PageTable, allocator: &mut Allocator) -> bool {
    // Linear map. Virtual: [KERNBASE, KERNBASE + 1GiB) -> Physical: [0, 1GiB)
    let r = map_pages(
        pgdir,
        allocator,
        crate::util::KERNBASE as u64,
        0,
        0x40000000, // 1GiB
        PageTableEntry::WRITABLE,
    );
    if !r {
        crate::error!("Linear map [KERNBASE, KERNBASE + 1GiB) failed");
        return false;
    }
    // Linear map. Virtual: [DEVBASE, DEVBASE + 512MiB) -> Physical: [DEVSPACE, DEVSPACE + 512MiB)
    let r = map_pages(
        pgdir,
        allocator,
        crate::util::DEVBASE as u64,
        crate::util::DEVSPACE as u64,
        0x20000000, // 512MiB
        PageTableEntry::WRITABLE | PageTableEntry::WRITE_THROUGH | PageTableEntry::CACHE_DISABLE,
    );
    if !r {
        crate::error!("Linear map [DEVBASE, DEVBASE + 512MiB) failed");
        return false;
    }
    true
}

const PG_SIZE_2M: u64 = 0x200000;

pub fn uvm_create(allocator: &mut Allocator) -> Option<*mut PageTable> {
    let pgdir = allocator.kalloc() as *mut PageTable;
    if pgdir.is_null() {
        return None;
    }

    // Only map high memory
    if !map_highmem(pgdir, allocator) {
        return None;
    }

    Some(pgdir)
}

pub fn switch(pgdir: *mut PageTable) {
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) v2p(pgdir as usize));
    }
}

pub fn map_pages(
    pgdir: *mut PageTable,
    allocator: &mut Allocator,
    va: u64,
    pa: u64,
    sz: u64,
    perm: u64,
) -> bool {
    let mut addr = pgrounddown(va);
    let end = pgrounddown(va + sz - 1);
    let mut pa = pa;

    while addr <= end {
        // Check if we can map a 2MB page
        let use_2m = (addr % PG_SIZE_2M == 0)
            && (pa % PG_SIZE_2M == 0)
            && (addr + PG_SIZE_2M <= end + PG_SIZE as u64);

        let level = if use_2m { 1 } else { 0 };

        let pte = walk(pgdir, allocator, addr, true, level);
        if pte.is_none() {
            crate::error!("Failed to map address: {:x}", addr);
            return false;
        }
        let pte = pte.unwrap();
        if pte.is_present() {
            crate::error!("Address {:x} already mapped", addr);
            return false;
        }

        let mut flags = perm | PageTableEntry::PRESENT;
        if use_2m {
            flags |= PageTableEntry::HUGE_PAGE;
        }
        *pte = PageTableEntry::new(pa, flags);

        if use_2m {
            addr += PG_SIZE_2M;
            pa += PG_SIZE_2M;
        } else {
            addr += PG_SIZE as u64;
            pa += PG_SIZE as u64;
        }
    }
    true
}

pub fn walk(
    pgdir: *mut PageTable,
    allocator: &mut Allocator,
    va: u64,
    alloc: bool,
    target_level: u8,
) -> Option<&'static mut PageTableEntry> {
    let mut table = pgdir;

    // Level 4, 3, 2
    for level in (1..4).rev() {
        if level <= target_level {
            break;
        }
        let idx = (va >> (12 + 9 * level)) & 0x1FF;
        let pte = unsafe { &mut (*table).entries[idx as usize] };

        if pte.is_present() {
            table = p2v(pte.addr() as usize) as *mut PageTable;
        } else {
            if !alloc {
                return None;
            }
            let new_table = allocator.kalloc() as *mut PageTable;
            if new_table.is_null() {
                return None;
            }
            let pa = v2p(new_table as usize) as u64;
            *pte = PageTableEntry::new(
                pa,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER,
            );
            table = new_table;
        }
    }

    let shift = 12 + 9 * target_level;
    let idx = (va >> shift) & 0x1FF;
    unsafe { Some(&mut (*table).entries[idx as usize]) }
}

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
const FLAGS_MASK: u64 = 0xfff;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

#[allow(dead_code)]
impl PageTableEntry {
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const CACHE_DISABLE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE_PAGE: u64 = 1 << 7;
    pub const GLOBAL: u64 = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;

    pub fn new(addr: u64, flags: u64) -> Self {
        Self((addr & ADDR_MASK) | (flags & FLAGS_MASK))
    }

    pub fn addr(&self) -> u64 {
        self.0 & ADDR_MASK
    }

    pub fn flags(&self) -> u64 {
        self.0 & FLAGS_MASK
    }

    pub fn is_present(&self) -> bool {
        self.0 & Self::PRESENT != 0
    }
}

pub fn uvm_copy(
    old_pgdir: *mut PageTable,
    new_pgdir: *mut PageTable,
    sz: u64,
    allocator: &mut Allocator,
) -> bool {
    let mut i = 0;
    while i < sz {
        let pte = walk(old_pgdir, allocator, i, false, 0);
        if let Some(pte) = pte {
            if pte.is_present() {
                let pa = pte.addr();
                let flags = pte.flags();

                let mem = allocator.kalloc();
                if mem.is_null() {
                    return false;
                }
                unsafe {
                    core::ptr::copy_nonoverlapping(p2v(pa as usize) as *const u8, mem, PG_SIZE);
                }

                if !map_pages(
                    new_pgdir,
                    allocator,
                    i,
                    v2p(mem as usize) as u64,
                    PG_SIZE as u64,
                    flags,
                ) {
                    return false;
                }
            }
        }
        i += PG_SIZE as u64;
    }
    true
}

pub fn pgrounddown(x: u64) -> u64 {
    x & !(PG_SIZE as u64 - 1)
}

fn pgroundup(x: u64) -> u64 {
    (x + PG_SIZE as u64 - 1) & !(PG_SIZE as u64 - 1)
}

pub fn uvm_alloc(
    pgdir: *mut PageTable,
    allocator: &mut Allocator,
    old_sz: usize,
    new_sz: usize,
) -> Option<usize> {
    if new_sz < old_sz {
        return Some(old_sz);
    }
    let mut a = pgroundup(old_sz as u64);
    while a < new_sz as u64 {
        let mem = allocator.kalloc();
        if mem.is_null() {
            uvm_dealloc(pgdir, allocator, a as usize, old_sz);
            return None;
        }
        unsafe {
            core::ptr::write_bytes(mem, 0, PG_SIZE);
        }
        if !map_pages(
            pgdir,
            allocator,
            a,
            v2p(mem as usize) as u64,
            PG_SIZE as u64,
            PageTableEntry::WRITABLE | PageTableEntry::USER,
        ) {
            allocator.kfree(mem as usize);
            uvm_dealloc(pgdir, allocator, a as usize, old_sz);
            return None;
        }
        a += PG_SIZE as u64;
    }
    Some(new_sz)
}

pub fn uvm_dealloc(
    pgdir: *mut PageTable,
    allocator: &mut Allocator,
    old_sz: usize,
    new_sz: usize,
) -> usize {
    if new_sz >= old_sz {
        return old_sz;
    }

    let mut a = pgroundup(new_sz as u64);
    let old = pgroundup(old_sz as u64);
    while a < old {
        let pte = walk(pgdir, allocator, a, false, 0);
        if let Some(pte) = pte {
            if pte.is_present() {
                let pa = pte.addr();
                if pa != 0 {
                    allocator.kfree(p2v(pa as usize));
                }
                unsafe { *pte = PageTableEntry::new(0, 0) };
            }
        }
        a += PG_SIZE as u64;
    }
    new_sz
}
