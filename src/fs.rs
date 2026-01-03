// Constants
pub const BSIZE: usize = 1024; // Block size
pub const ROOTINO: u32 = 1; // Root inode number
pub const FSMAGIC: u32 = 0x10203040;
pub const NDIRECT: usize = 12;
pub const NINDIRECT: usize = BSIZE / core::mem::size_of::<u32>();
pub const MAXFILE: usize = NDIRECT + NINDIRECT;

// Inode types
pub const T_DIR: u16 = 1;
pub const T_FILE: u16 = 2;
pub const T_DEV: u16 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SuperBlock {
    pub magic: u32,
    pub size: u32,       // Size of file system image (blocks)
    pub nblocks: u32,    // Number of data blocks
    pub ninodes: u32,    // Number of inodes.
    pub nlog: u32,       // Number of log blocks
    pub logstart: u32,   // Block number of first log block
    pub inodestart: u32, // Block number of first inode block
    pub bmapstart: u32,  // Block number of first free map block
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DiskInode {
    pub type_: u16,                // File type
    pub major: u16,                // Major device number (T_DEV only)
    pub minor: u16,                // Minor device number (T_DEV only)
    pub nlink: u16,                // Number of links to inode in file system
    pub size: u32,                 // Size of file (bytes)
    pub addrs: [u32; NDIRECT + 1], // Data block addresses
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Dirent {
    pub inum: u16,
    pub name: [u8; 14],
}

pub const DIRSIZ: usize = 14;

use crate::sleeplock::{SleepLockGuard, SleepLockSafe};
use crate::spinlock::Spinlock;

pub struct InodeData {
    pub valid: bool, // valid means data loaded from disk
    pub type_: u16,
    pub major: u16,
    pub minor: u16,
    pub nlink: u16,
    pub size: u32,
    pub addrs: [u32; NDIRECT + 1],
}

impl InodeData {
    pub const fn new() -> Self {
        Self {
            valid: false,
            type_: 0,
            major: 0,
            minor: 0,
            nlink: 0,
            size: 0,
            addrs: [0; NDIRECT + 1],
        }
    }
}

impl Default for InodeData {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Inode {
    pub dev: u32,
    pub inum: u32,
    pub refcnt: u32,
    pub lock: SleepLockSafe<InodeData>,
}

pub const IPB: usize = BSIZE / core::mem::size_of::<DiskInode>();

static SB: Spinlock<SuperBlock> = Spinlock::new(SuperBlock {
    magic: 0,
    size: 0,
    nblocks: 0,
    ninodes: 0,
    nlog: 0,
    logstart: 0,
    inodestart: 0,
    bmapstart: 0,
});

pub fn fsinit(dev: u32) {
    let mut sb: SuperBlock = unsafe { core::mem::zeroed() };
    // Read Block 1 (SuperBlock)
    // We use bio::bread calling specific block.
    // SB is at block 1.
    // Assuming 0 is unused/boot?
    // xv6: buf = bread(dev, 1);

    // We don't have a way to copy out from buffer easily yet if bread returns index.
    // bio implementation needs checking.
    // bread returns buffer index.

    let b = crate::bio::bread(dev, 1);
    {
        let cache = crate::bio::BCACHE.lock();
        let buf = &cache.bufs[b];
        let ptr = buf.data.as_ptr() as *const SuperBlock;
        sb = unsafe { *ptr };
    }
    crate::bio::brelse(b);

    if sb.magic != FSMAGIC {
        panic!("invalid file system");
    }

    *SB.lock() = sb;

    // Additional initialization?
}

impl Inode {
    pub const fn new() -> Self {
        Self {
            dev: 0,
            inum: 0,
            refcnt: 0,
            lock: SleepLockSafe::new(InodeData::new()),
        }
    }

    // ilock equivalent
    pub fn ilock(&self) -> SleepLockGuard<InodeData> {
        let mut guard = self.lock.lock();
        if !guard.valid {
            let b = crate::bio::bread(self.dev, self.iblock());
            {
                let cache = crate::bio::BCACHE.lock();
                let buf = &cache.bufs[b];
                let offset = (self.inum as usize % IPB) * core::mem::size_of::<DiskInode>();
                let ptr = unsafe { buf.data.as_ptr().add(offset) } as *const DiskInode;
                let dip = unsafe { &*ptr };

                guard.type_ = dip.type_;
                guard.major = dip.major;
                guard.minor = dip.minor;
                guard.nlink = dip.nlink;
                guard.size = dip.size;
                guard.addrs = dip.addrs;
                guard.valid = true;
            }
            crate::bio::brelse(b);

            if guard.type_ == 0 {
                // If type is 0, it means empty/invalid?
                // But valid=true just means "loaded from disk".
            }
        }
        guard
    }

    // Map logical block number to physical block number
    // Returns 0 if not allocated.
    // If alloc is true, allocate if needed.
    pub fn bmap(&mut self, bn: u32, alloc: bool) -> u32 {
        let mut addr: u32;

        let mut guard = self.lock.lock(); // Use lock() for mutable access to InodeData

        if (bn as usize) < NDIRECT {
            addr = guard.addrs[bn as usize];
            if addr == 0 {
                if !alloc {
                    return 0;
                }
                addr = balloc(self.dev);
                if addr == 0 {
                    return 0;
                }
                guard.addrs[bn as usize] = addr;
            }
            return addr;
        }

        // Indirect block
        panic!("bmap: indirect not supported yet");
    }

    // Update inode to disk
    pub fn iupdate(&self) {
        let guard = self.lock.lock();
        let b = crate::bio::bread(self.dev, self.iblock());
        {
            let mut cache = crate::bio::BCACHE.lock(); // Need mutable access to BCACHE to get mutable buf
            let buf = &mut cache.bufs[b]; // Need &mut Buf? crate::bio should allow it?
            // bread currently returns usize index.
            // Bcache lock gives &mut Bcache.
            // But we need to lock Buffer?
            // For now, assume exclusive access to buffer via index.

            let offset = (self.inum as usize % IPB) * core::mem::size_of::<DiskInode>();
            let ptr = unsafe { buf.data.as_mut_ptr().add(offset) } as *mut DiskInode;
            let dip = unsafe { &mut *ptr };

            dip.type_ = guard.type_;
            dip.major = guard.major;
            dip.minor = guard.minor;
            dip.nlink = guard.nlink;
            dip.size = guard.size;
            dip.addrs = guard.addrs;
        }
        crate::bio::bwrite(b);
        crate::bio::brelse(b);
    }

    fn iblock(&self) -> u32 {
        let sb = SB.lock();
        (self.inum / IPB as u32) + sb.inodestart
    }
}

const NINODE: usize = 10;

struct ICache {
    inodes: [Inode; NINODE],
}

static ICACHE: Spinlock<ICache> = Spinlock::new(ICache {
    inodes: [
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
        Inode::new(),
    ],
});

pub fn iinit() {
    // Initialized by static
}

pub fn iget(dev: u32, inum: u32) -> &'static Inode {
    let mut guard = ICACHE.lock();
    let cache = &mut *guard;

    // Is the inode already cached?
    let mut empty: Option<usize> = None;
    for (i, ip) in cache.inodes.iter_mut().enumerate() {
        if ip.refcnt > 0 && ip.dev == dev && ip.inum == inum {
            ip.refcnt += 1;
            return unsafe { &*(ip as *const Inode) };
        }
        if empty.is_none() && ip.refcnt == 0 {
            empty = Some(i);
        }
    }

    // Recycle an inode entry.
    if let Some(idx) = empty {
        let ip = &mut cache.inodes[idx];
        ip.dev = dev;
        ip.inum = inum;
        ip.refcnt = 1;
        // Reset InodeData validation
        let data = ip.lock.get_mut();
        data.valid = false;

        return unsafe { &*(ip as *const Inode) };
    }

    panic!("iget: no inodes");
}

pub fn iput(ip: &Inode) {
    let mut guard = ICACHE.lock();
    let cache = &mut *guard;

    // We need to find the mutable inode corresponding to ip.
    // ip is a pointer to one of cache.inodes.
    // We can assume ip points into cache.inodes.
    // Safety: we trust ip was returned by iget.

    // Check if ip is inside the slice range.
    let base = cache.inodes.as_ptr();
    let ptr = ip as *const Inode;

    let offset = unsafe { ptr.offset_from(base) };
    if offset >= 0 && (offset as usize) < NINODE {
        let idx = offset as usize;
        let ip_mut = &mut cache.inodes[idx];

        if ip_mut.refcnt == 1 {
            // refcnt dropping to 0
            // In xv6, release triggers nothing special, just free slot.
            // But we should ensure validity is cleared if we want?
            // Actually xv6 clears valid in iget when recycling.
        }
        ip_mut.refcnt -= 1;
    } else {
        panic!("iput: invalid inode pointer");
    }
}

// Allocate a zeroed disk block.
fn balloc(dev: u32) -> u32 {
    let sb = SB.lock();
    // iterate bitmap
    let sz = sb.size;
    let bmap_start = sb.bmapstart;

    // Logic for bitmap allocator needed.
    // For now, fail or implement minimal.
    // Let's defer full allocator.
    0
}

pub fn readi(ip: &Inode, mut dst: *mut u8, off: u32, mut n: u32) -> u32 {
    let mut guard = ip.ilock();

    if off > guard.size {
        return 0;
    }
    if off + n > guard.size {
        n = guard.size - off;
    }

    let mut tot = 0;
    let mut offset = off;
    let mut m = n;

    // We need to release guard to call bmap?
    // bmap uses get_mut(), so requires &mut Inode or exclusive access?
    // bmap modifies InodeData (allocates blocks).
    // `guard` gives &mut InodeData.
    // So we can implement bmap on `InodeData`?
    // bmap needs `balloc`.
    // Let's implement bmap on Inode (requires &mut Inode or locking).
    // But `ilock` gives guard.
    // `bmap` is internal.
    // In xv6, bmap takes `struct inode*`.

    // Let's put bmap logic inside here or use `ip` if possible.
    // But `bmap` might need to sleep (read indirect block).
    // If we hold sleep-lock on inode, it's fine to sleep for other locks.

    // Actually, `bmap` on `ip` is fine.
    // But wait, `bmap` needs to modify `ip->addrs`.
    // `ip->addrs` is inside `ip->lock` which `guard` holds.
    // So `guard` has mutable access to `addrs`.
    // So `bmap` should operate on `guard` (InodeData) + `dev`?
    // But `bmap` also updates `ip`.

    // Let's extract bmap logic to work on InodeData.

    while m > 0 {
        let b = bmap_on_data(&mut guard, ip.dev, offset / BSIZE as u32);
        if b == 0 {
            break;
        }
        let buf_idx = crate::bio::bread(ip.dev, b);
        let start = (offset % BSIZE as u32) as usize;
        let len = core::cmp::min(m as usize, BSIZE - start);

        unsafe {
            let cache = crate::bio::BCACHE.lock();
            let src = cache.bufs[buf_idx].data.as_ptr().add(start);
            core::ptr::copy_nonoverlapping(src, dst, len);
        }
        crate::bio::brelse(buf_idx);

        tot += len as u32;
        offset += len as u32;
        m -= len as u32;
        dst = unsafe { dst.add(len) };
    }
    tot
}

pub fn writei(ip: &Inode, src: *const u8, off: u32, mut n: u32) -> u32 {
    let mut src = src;
    let mut guard = ip.ilock();

    if off > guard.size {
        return 0;
    }
    // writei can grow file?
    if off + n > guard.size {
        // guard.size = off + n; // Only if we support growing
        // For now, minimal.
    }

    let mut tot = 0;
    let mut offset = off;
    let mut m = n;

    while m > 0 {
        let b = bmap_on_data(&mut guard, ip.dev, offset / BSIZE as u32);
        if b == 0 {
            break;
        }
        let buf_idx = crate::bio::bread(ip.dev, b);
        let start = (offset % BSIZE as u32) as usize;
        let len = core::cmp::min(m as usize, BSIZE - start);

        unsafe {
            let mut cache = crate::bio::BCACHE.lock();
            let dst = cache.bufs[buf_idx].data.as_mut_ptr().add(start);
            core::ptr::copy_nonoverlapping(src, dst, len);
        }
        crate::bio::bwrite(buf_idx);
        crate::bio::brelse(buf_idx);

        tot += len as u32;
        offset += len as u32;
        m -= len as u32;
        src = unsafe { src.add(len) };
    }

    if n > 0 && offset > guard.size {
        guard.size = offset;
        ip.iupdate(); // Update inode size on disk
    }

    tot
}

// Allocate a new inode with the given type.
pub fn ialloc(dev: u32, type_: u16) -> Option<&'static Inode> {
    let sb = SB.lock();
    for inum in 1..sb.ninodes {
        let b = crate::bio::bread(dev, iblock_of(inum, sb.inodestart));
        {
            let mut cache = crate::bio::BCACHE.lock(); // Need mutable access to BCACHE
            let buf = &mut cache.bufs[b];
            let offset = (inum as usize % IPB) * core::mem::size_of::<DiskInode>();
            let ptr = unsafe { buf.data.as_mut_ptr().add(offset) } as *mut DiskInode;
            let dip = unsafe { &mut *ptr };
            if dip.type_ == 0 {
                // Found free inode
                unsafe {
                    core::ptr::write_bytes(ptr as *mut u8, 0, core::mem::size_of::<DiskInode>())
                }; // memset 0
                dip.type_ = type_;
                // dip.nlink = 0; // default?
                // dip.major = 0; ...
                // Mark buffer dirty? bwrite assumes we modify.
            } else {
                drop(cache); // Drop the lock before continuing the loop
                crate::bio::brelse(b);
                continue;
            }
        }
        crate::bio::bwrite(b);
        crate::bio::brelse(b);

        return Some(iget(dev, inum));
    }
    None
}

// Moved helper
const fn iblock_of(i: u32, start: u32) -> u32 {
    (i / IPB as u32) + start
}

fn bmap_on_data(data: &mut InodeData, dev: u32, bn: u32) -> u32 {
    if (bn as usize) < NDIRECT {
        let mut addr = data.addrs[bn as usize];
        if addr == 0 {
            addr = balloc(dev);
            if addr != 0 {
                data.addrs[bn as usize] = addr;
            }
        }
        return addr;
    }
    0
}
