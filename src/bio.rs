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

// Buffer cache
pub struct Bcache {
    pub bufs: [Buf; NBUF],
    pub head: usize, // Index of head of LRU list
}

pub static BCACHE: Spinlock<Bcache> = Spinlock::new(
    Bcache {
        bufs: [Buf::new(); NBUF],
        head: 0,
    },
    "BCACHE",
);

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
    // crate::uart_println!("DEBUG: bread dev={} blockno={}", dev, blockno);
    let b = bget(dev, blockno);
    let mut do_read = false;
    {
        let cache = BCACHE.lock();
        if !cache.bufs[b].valid {
            do_read = true;
        }
    }

    if do_read {
        let mut buf_data = [0u8; BSIZE];
        // virtio block driver uses 512 byte sectors, but we use 1024 byte blocks, so
        // we need to specify `blockno * 2` as sector number. Note that the buffer
        // size can be larger than 512 bytes.
        virtio::read_block(blockno as u64 * 2, &mut buf_data);

        let mut cache = BCACHE.lock();
        cache.bufs[b].data.copy_from_slice(&buf_data);
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
}

pub fn bget(dev: u32, blockno: u32) -> usize {
    // crate::uart_println!("DEBUG: bget enter dev={} blockno={}", dev, blockno);
    let mut cache = BCACHE.lock();

    // 1. Look for block
    for i in 0..NBUF {
        if cache.bufs[i].dev == dev && cache.bufs[i].blockno == blockno {
            cache.bufs[i].refcnt += 1;
            return i;
        }
    }

    // 2. Alloc new
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
