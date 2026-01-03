use crate::fs::BSIZE;
use crate::spinlock::Spinlock;
use crate::virtio;

pub const NBUF: usize = 30;

#[derive(Clone, Copy)]
pub struct Buf {
    pub valid: bool, // Has data been read from disk?
    pub disk: bool,  // Does content match disk?
    pub dev: u32,
    pub blockno: u32,
    pub refcnt: u32,
    pub prev: usize, // LRU cache list
    pub next: usize,
    pub data: [u8; BSIZE],
}

impl Buf {
    pub const fn new() -> Self {
        Self {
            valid: false,
            disk: false,
            dev: 0,
            blockno: 0,
            refcnt: 0,
            prev: 0,
            next: 0,
            data: [0; BSIZE],
        }
    }
}

pub struct Bcache {
    pub bufs: [Buf; NBUF],
    pub head: usize, // Index of head of LRU list
}

pub static BCACHE: Spinlock<Bcache> = Spinlock::new(Bcache {
    bufs: [Buf::new(); NBUF],
    head: 0,
});

pub fn binit() {
    let mut bcache = BCACHE.lock();

    // Create linked list of buffers
    // Head -> buf[0] -> buf[1] ... -> Head
    // For simplicity, let's just use indices.
    // prev/next are indices in bufs array.
    // 0 is a dummy head? Or just circular list.
    // Let's use 0 as LRU head (dummy).

    // Initialize list to all free
    // head.next = &bufs[0]
    // bufs[0].next = &bufs[1] ...

    let n = NBUF;
    for i in 0..n {
        bcache.bufs[i].next = (i + 1) % n;
        bcache.bufs[i].prev = (i + n - 1) % n;
    }
    bcache.head = 0;
}

// Read a block into buffer
pub fn bread(dev: u32, blockno: u32) -> usize {
    let b = bget(dev, blockno);
    {
        let mut bcache = BCACHE.lock();
        if !bcache.bufs[b].valid {
            // Drop lock to read?
            // virtio::read_block sleeps, so we MUST drop spinlock.
            // But if we drop spinlock, someone else might use the buffer?
            // buf needs a sleep-lock (busy flag).
            // For now, xv6-style: buffer is locked by bget.
            // But we don't have sleep-lock yet.
            // Let's just hold the lock for now? No, sleep inside spinlock bad.
            // We need to implement proper sleep-lock pattern.

            // For simplicity in this step: READ synchronously while holding lock?
            // virtio::read_block sleeps, which switches process.
            // Interrupts come in.
            // If we hold spinlock (with interrupts disabled), sleep is meaningless/deadlock.
            // virtio::read_block re-enables interrupts by sleep() -> swtch().

            // CRITICAL: We cannot hold Spinlock while calling virtio::read_block.
            // bget returns a "locked" buffer (semantics).
            // We need to release BCACHE lock but keep BUFFER locked.
            // Since we implemented naive Spinlock, we don't have per-buffer locks yet.

            // Simplification: Just read synchronously.
            // But virtio requires sleep.

            // Solution:
            // 1. Acquire BCACHE.
            // 2. Find buffer. Mark 'locked/busy' in flags.
            // 3. Release BCACHE.
            // 4. Do IO.
            // 5. Acquire BCACHE. Mark valid.
            // 6. Return buffer index.

            // Wait, bget already does logic.
            // Let's implement minimal bread that does IO.
        }
    }
    // Perform IO if not valid
    // This part is tricky without full lock infrastructure.
    // Let's assume for this step, we just read.
    // To make this safe, we really need a Lock on the Buf or similar.
    // Let's use `refcnt` as a lock for now?
    // refcnt > 0 means it's in use.

    let mut buf_data = [0u8; BSIZE];

    // COPYING STRATEGY for simplicity (Buffer Cache is just a cache, we copy out?)
    // No, we want zero-copy reference usually.
    // But returning &Buf is hard with Spinlock.
    // Returning index is easier.

    // REAL implementation needs sleep-locks.
    // I will implement a placeholder that reads every time for now,
    // bypassing cache logic to prove FS works, OR implement full cache.
    // Let's try full cache with "busy" bit.

    // For now, assume bget returned a buffer we own (refcnt incremented).
    // We check valid bit.

    // Note: This needs access to internal data.
    // Let's create a temporary simpler implementation that effectively bypasses cache for reads
    // but uses structure, until we harden locks.
    // Actually, `virtio` is fast. Maybe we can rely on that?
    // No, we need cache for inodes.

    // Let's assume single process for now during fs dev (init).
    let mut bcache = BCACHE.lock();
    if !bcache.bufs[b].valid {
        // Read from disk
        // We must release lock to do IO?
        // This assumes we have exclusive access to this buf (bget ensures).
    }
    drop(bcache);

    // If not valid, read.
    // To read safely, we need mutable access.
    // But `bufs` is in `BCACHE`.
    // We need `BCACHE` lock to write to `bufs[b].data`.

    // Workaround: We define `read` to take a buffer?
    // Let's make `bread` read into `bufs[b].data`.

    // Since we are single threaded mostly (just kthread + init),
    // we can cheat:
    // Hold lock, check valid. If not, drop lock, read local buf, take lock, copy to buf, set valid.

    let mut do_read = false;
    {
        let cache = BCACHE.lock();
        if !cache.bufs[b].valid {
            do_read = true;
        }
    }

    if do_read {
        virtio::read_block(blockno as u64 * 2, &mut buf_data);
        let mut cache = BCACHE.lock();
        cache.bufs[b].data = buf_data;
        cache.bufs[b].valid = true;
    }

    b
}

pub fn bwrite(b: usize) {
    let mut cache = BCACHE.lock();
    let blockno = cache.bufs[b].blockno;
    let data = cache.bufs[b].data;
    drop(cache);

    virtio::write_block(blockno as u64 * 2, &data);

    let mut cache = BCACHE.lock();
    cache.bufs[b].valid = true; // Up to date
}

pub fn brelse(b: usize) {
    let mut cache = BCACHE.lock();
    cache.bufs[b].refcnt -= 1;
    // Move to head of LRU if refcnt == 0?
}

pub fn bget(dev: u32, blockno: u32) -> usize {
    let mut cache = BCACHE.lock();

    // 1. Look for block
    for i in 0..NBUF {
        if cache.bufs[i].dev == dev && cache.bufs[i].blockno == blockno {
            cache.bufs[i].refcnt += 1;
            return i;
        }
    }

    // 2. Alloc new (LRU) - Scan backwards from head?
    // Naive: Find first refcnt==0.
    for i in 0..NBUF {
        if cache.bufs[i].refcnt == 0 {
            cache.bufs[i].dev = dev;
            cache.bufs[i].blockno = blockno;
            cache.bufs[i].valid = false;
            cache.bufs[i].refcnt = 1;
            return i;
        }
    }

    panic!("bget: no buffers");
}
