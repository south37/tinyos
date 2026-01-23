#![allow(unsafe_op_in_unsafe_fn)]
use crate::allocator::Allocator;
use crate::pci::PciDevice;

use crate::util::{inb, inl, inw, outb, outl, outw};
use crate::util::{v2p, PG_SIZE};
use core::mem::size_of;
use core::ptr::{addr_of, addr_of_mut};

pub const VIRTIO_LEGACY_DEVICE_ID: u16 = 0x1001;

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

// Offsets for Legacy Virtio Header (IO Space)
const VIRTIO_REG_HOST_FEATURES: u16 = 0;
const VIRTIO_REG_GUEST_FEATURES: u16 = 4;
const VIRTIO_REG_QUEUE_ADDR: u16 = 8;
const VIRTIO_REG_QUEUE_SIZE: u16 = 12;
const VIRTIO_REG_QUEUE_SELECT: u16 = 14;
const VIRTIO_REG_QUEUE_NOTIFY: u16 = 16;
const VIRTIO_REG_DEVICE_STATUS: u16 = 18;
const VIRTIO_REG_ISR_STATUS: u16 = 19;

// Status Bits
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;

// VirtQueue sizes: QEMU defaults to 256
const QUEUE_SIZE: usize = 256;

#[repr(C)]
struct VRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct VRingAvail {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE],
    event: u16,
}

#[repr(C)]
struct VRingUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
struct VRingUsed {
    flags: u16,
    idx: u16,
    ring: [VRingUsedElem; QUEUE_SIZE],
    event: u16,
}

#[repr(C)]
struct VirtioBlkOutHeader {
    type_: u32,
    priority: u32,
    sector: u64,
}

pub struct VirtioDriver {
    io_base: u16,
    queue_desc: *mut VRingDesc,
    queue_avail: *mut VRingAvail,
    queue_used: *mut VRingUsed,
    free_head: u16,
    used_idx: u16,
    avail_idx: u16,
}

use crate::spinlock::Spinlock;

pub static VIRTIO_BLK_DRIVER: Spinlock<Option<VirtioDriver>> =
    Spinlock::new(None, "VIRTIO_BLK_DRIVER");

pub unsafe fn intr() {
    let guard = VIRTIO_BLK_DRIVER.lock();
    if let Some(driver) = guard.as_ref() {
        let status = unsafe { inb(driver.io_base + VIRTIO_REG_ISR_STATUS) };
        if status & 1 != 0 || status & 3 != 0 {
            // Wakeup waiting process
            // We wake up the VIRTIO_BLK_DRIVER address (global static address)
            crate::proc::wakeup(addr_of!(VIRTIO_BLK_DRIVER) as usize);
        }
    }
}

pub unsafe fn init(dev: &PciDevice, allocator: &mut Allocator) {
    let mut guard = VIRTIO_BLK_DRIVER.lock();
    if guard.is_some() {
        return;
    }

    let io_base = dev.base_addr as u16;
    crate::info!("Virtio: io_base={:x}", io_base);

    // 1. Reset device
    unsafe { outb(io_base + VIRTIO_REG_DEVICE_STATUS, 0) };

    // 2. Set ACKNOWLEDGE and DRIVER
    let mut status = VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER;
    unsafe { outb(io_base + VIRTIO_REG_DEVICE_STATUS, status) };

    // 3. Negotiate Features
    let features = unsafe { inl(io_base + VIRTIO_REG_HOST_FEATURES) };
    unsafe { outl(io_base + VIRTIO_REG_GUEST_FEATURES, features) };

    // 4. Setup Virtqueues
    unsafe { outw(io_base + VIRTIO_REG_QUEUE_SELECT, 0) };

    let q_size = unsafe { inw(io_base + VIRTIO_REG_QUEUE_SIZE) } as usize;
    crate::info!("Virtio: Device Queue 0 size {}", q_size);

    if q_size < QUEUE_SIZE {
        crate::error!(
            "Virtio: Warning device queue size {} < compiled {}",
            q_size,
            QUEUE_SIZE
        );
    }

    // Allocate 3 contiguous pages manually
    let p1 = allocator.kalloc();
    let p2 = allocator.kalloc();
    let p3 = allocator.kalloc();

    if p1.is_null() || p2.is_null() || p3.is_null() {
        crate::error!("Virtio: Failed to allocate pages");
        return;
    }

    // Find Base (kalloc goes high-to-low)
    let pages = [p3 as usize, p2 as usize, p1 as usize];

    if pages[1] != pages[0] + PG_SIZE || pages[2] != pages[1] + PG_SIZE {
        crate::error!("Virtio: Failed to allocate 3 contiguous pages");
        return;
    }

    let base_addr = pages[0] as *mut u8;

    unsafe {
        crate::util::stosq(base_addr as *mut u64, 0, PG_SIZE * 3 / 8);
    }

    let paddr_pages = v2p(base_addr as usize);
    crate::info!(
        "Virtio: pages vaddr={:p} paddr={:x}",
        base_addr,
        paddr_pages
    );
    unsafe { outl(io_base + VIRTIO_REG_QUEUE_ADDR, (paddr_pages as u32) >> 12) };

    let desc_ptr = base_addr as *mut VRingDesc;
    let avail_ptr = unsafe { base_addr.add(4096) } as *mut VRingAvail;
    let used_ptr = unsafe { base_addr.add(8192) } as *mut VRingUsed;

    for i in 0..(QUEUE_SIZE - 1) {
        unsafe { (*desc_ptr.add(i)).next = (i + 1) as u16 };
    }

    let driver = VirtioDriver {
        io_base,
        queue_desc: desc_ptr,
        queue_avail: avail_ptr,
        queue_used: used_ptr,
        free_head: 0,
        used_idx: 0,
        avail_idx: 0,
    };

    // 5. Driver OK
    status |= VIRTIO_STATUS_DRIVER_OK;
    unsafe { outb(io_base + VIRTIO_REG_DEVICE_STATUS, status) };

    *guard = Some(driver);
    crate::info!("Virtio-blk initialized (Legacy) QSize={}", QUEUE_SIZE);
}

#[repr(C)]
struct VirtioBlkReq {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub fn read_block(sector: u64, buf: &mut [u8]) {
    do_block_io(sector, buf, false);
}

pub fn write_block(sector: u64, buf: &[u8]) {
    // cast const buf to mut for common helper, but we won't write to it if write=true
    let mut_buf = unsafe { core::slice::from_raw_parts_mut(buf.as_ptr() as *mut u8, buf.len()) };
    do_block_io(sector, mut_buf, true);
}

fn do_block_io(sector: u64, buf: &mut [u8], write: bool) {
    let mut guard = VIRTIO_BLK_DRIVER.lock();
    let mut status_val: u8 = 111;
    let req = VirtioBlkReq {
        type_: if write {
            VIRTIO_BLK_T_OUT
        } else {
            VIRTIO_BLK_T_IN
        },
        reserved: 0,
        sector,
    };

    // 1. Submit Request
    let head_idx = {
        let driver = match guard.as_mut() {
            Some(d) => d,
            None => return,
        };

        let head_idx = driver.alloc_desc();
        let data_idx = driver.alloc_desc();
        let status_idx = driver.alloc_desc();

        let req_paddr = v2p(&req as *const _ as usize);
        let buf_paddr = v2p(buf.as_ptr() as usize);
        let status_paddr = v2p(&status_val as *const _ as usize);

        let desc_ptr = driver.queue_desc;

        unsafe {
            // Desc 1: Header
            (*desc_ptr.add(head_idx as usize)).addr = req_paddr as u64;
            (*desc_ptr.add(head_idx as usize)).len = size_of::<VirtioBlkReq>() as u32;
            (*desc_ptr.add(head_idx as usize)).flags = 1; // NEXT
            (*desc_ptr.add(head_idx as usize)).next = data_idx;

            // Desc 2: Data
            (*desc_ptr.add(data_idx as usize)).addr = buf_paddr as u64;
            (*desc_ptr.add(data_idx as usize)).len = buf.len() as u32;
            (*desc_ptr.add(data_idx as usize)).flags = 1; // NEXT
            if !write {
                (*desc_ptr.add(data_idx as usize)).flags |= 2; // WRITE
            }
            (*desc_ptr.add(data_idx as usize)).next = status_idx;

            // Desc 3: Status
            (*desc_ptr.add(status_idx as usize)).addr = status_paddr as u64;
            (*desc_ptr.add(status_idx as usize)).len = 1;
            (*desc_ptr.add(status_idx as usize)).flags = 2; // WRITE
            (*desc_ptr.add(status_idx as usize)).next = 0;

            let avail = driver.queue_avail;
            let idx = driver.avail_idx;

            // 2. Update Avail Ring
            // Use volatile write to ensure it happens before idx update
            core::ptr::write_volatile(&mut (*avail).ring[idx as usize % QUEUE_SIZE], head_idx);

            // Barrier to ensure ring update is visible before idx update
            // (Processor barrier shouldn't be needed for TSO x86, but compiler barrier is essential)
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

            // 3. Update Avail Idx
            driver.avail_idx = idx.wrapping_add(1);
            core::ptr::write_volatile(&mut (*avail).idx, driver.avail_idx);

            // Barrier to ensure idx update is visible before notify
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

            outw(driver.io_base + VIRTIO_REG_QUEUE_NOTIFY, 0);
        }

        // crate::uart_println!("Virtio: submit sector={} head={}", sector, head_idx);

        head_idx
    };

    // 2. Wait for completion
    loop {
        let driver = guard.as_mut().unwrap(); // Safe unwrap as checked above

        let used = driver.queue_used;
        let used_idx = unsafe { core::ptr::read_volatile(&(*used).idx) };

        // Ensure we read the index before reading the ring entry (load-load barrier)
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        if driver.used_idx != used_idx {
            let entry_idx = driver.used_idx as usize % QUEUE_SIZE;
            let id = unsafe { (*used).ring[entry_idx].id };

            // crate::uart_println!(
            //     "Virtio: check used_idx={} driver_used={} id={} head={}",
            //     used_idx,
            //     driver.used_idx,
            //     id,
            //     head_idx
            // );

            if id as u16 == head_idx {
                break;
            }
        }

        // Use yield to avoid lost wakeup race conditions
        if crate::proc::mycpu().process.is_some() {
            crate::proc::sleep(addr_of!(VIRTIO_BLK_DRIVER) as usize, Some(guard));
            guard = VIRTIO_BLK_DRIVER.lock();
        } else {
            drop(guard);
            unsafe { core::arch::asm!("pause") };
            guard = VIRTIO_BLK_DRIVER.lock();
        }
    }

    // 3. Cleanup
    {
        let driver = guard.as_mut().unwrap();
        driver.used_idx = driver.used_idx.wrapping_add(1);

        // Wake up others because used_idx changed, so the next pending request (if any)
        // is now at the head of the driver's process queue.
        crate::proc::wakeup(addr_of!(VIRTIO_BLK_DRIVER) as usize);

        unsafe {
            let desc_ptr = driver.queue_desc;
            let data_idx = (*desc_ptr.add(head_idx as usize)).next;
            let status_idx = (*desc_ptr.add(data_idx as usize)).next;

            driver.free_desc(head_idx);
            driver.free_desc(data_idx);
            driver.free_desc(status_idx);
        }
    }
}

impl VirtioDriver {
    fn alloc_desc(&mut self) -> u16 {
        let idx = self.free_head;
        unsafe {
            self.free_head = (*self.queue_desc.add(idx as usize)).next;
        }
        idx
    }

    fn free_desc(&mut self, idx: u16) {
        unsafe {
            (*self.queue_desc.add(idx as usize)).next = self.free_head;
            self.free_head = idx;
        }
    }
}
