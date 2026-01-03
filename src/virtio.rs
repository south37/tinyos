use crate::allocator::Allocator;
use crate::pci::PciDevice;
use crate::uart_println;
use crate::util::{PG_SIZE, v2p};
use crate::util::{inb, inl, inw, outb, outl, outw};
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

static mut VIRTIO_BLK_DRIVER: Option<VirtioDriver> = None;
static mut VIRTIO_IO_BASE: u16 = 0;

struct VirtioDriver {
    io_base: u16,
    queue_desc: *mut VRingDesc,
    queue_avail: *mut VRingAvail,
    queue_used: *mut VRingUsed,
    free_head: u16,
    used_idx: u16,
}

pub unsafe fn intr() {
    let io_base = unsafe { VIRTIO_IO_BASE };
    if io_base != 0 {
        let status = unsafe { inb(io_base + VIRTIO_REG_ISR_STATUS) };
        if status & 1 != 0 || status & 3 != 0 {
            // Wakeup waiting process
            unsafe { crate::proc::wakeup(addr_of!(VIRTIO_BLK_DRIVER) as usize) };
        }
    }
}

pub unsafe fn init(dev: &PciDevice, allocator: &mut Allocator) {
    if unsafe { (*addr_of!(VIRTIO_BLK_DRIVER)).is_some() } {
        return;
    }

    let io_base = dev.base_addr as u16;
    unsafe { VIRTIO_IO_BASE = io_base };
    uart_println!("Virtio: io_base={:x}", io_base);

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
    uart_println!("Virtio: Device Queue 0 size {}", q_size);

    // Check if device supports large enough queue
    if q_size < QUEUE_SIZE {
        uart_println!(
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
        uart_println!("Virtio: Failed to allocate pages");
        return;
    }

    // Find Base.
    // We need 3 pages contiguous. kalloc goes high-to-low.
    let pages = [p3 as usize, p2 as usize, p1 as usize];

    if pages[1] != pages[0] + PG_SIZE || pages[2] != pages[1] + PG_SIZE {
        uart_println!(
            "Virtio: Failed to allocate 3 contiguous pages: {:x} {:x} {:x}",
            pages[0],
            pages[1],
            pages[2]
        );
        return;
    }

    let base_addr = pages[0] as *mut u8;

    // Zero out
    unsafe {
        crate::util::stosq(base_addr as *mut u64, 0, PG_SIZE * 3 / 8);
    }

    let paddr_pages = v2p(base_addr as usize);
    uart_println!(
        "Virtio: pages vaddr={:p} paddr={:x}",
        base_addr,
        paddr_pages
    );
    unsafe { outl(io_base + VIRTIO_REG_QUEUE_ADDR, (paddr_pages as u32) >> 12) };

    let desc_ptr = base_addr as *mut VRingDesc;
    let avail_ptr = unsafe { base_addr.add(4096) } as *mut VRingAvail;
    let used_ptr = unsafe { base_addr.add(8192) } as *mut VRingUsed;

    // Initialize Free List in Descriptors
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
    };

    // 5. Driver OK
    status |= VIRTIO_STATUS_DRIVER_OK;
    unsafe { outb(io_base + VIRTIO_REG_DEVICE_STATUS, status) };

    unsafe { *addr_of_mut!(VIRTIO_BLK_DRIVER) = Some(driver) };
    uart_println!("Virtio-blk initialized (Legacy) QSize={}", QUEUE_SIZE);
}

#[repr(C)]
struct VirtioBlkReq {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub fn read_block(sector: u64, buf: &mut [u8]) {
    unsafe {
        if let Some(mut driver) = (*addr_of_mut!(VIRTIO_BLK_DRIVER)).take() {
            driver.submit(sector, buf, false);
            (*addr_of_mut!(VIRTIO_BLK_DRIVER)) = Some(driver);
        }
    }
}

pub fn write_block(sector: u64, buf: &[u8]) {
    unsafe {
        if let Some(mut driver) = (*addr_of_mut!(VIRTIO_BLK_DRIVER)).take() {
            let mut_buf = core::slice::from_raw_parts_mut(buf.as_ptr() as *mut u8, buf.len());
            driver.submit(sector, mut_buf, true);
            (*addr_of_mut!(VIRTIO_BLK_DRIVER)) = Some(driver);
        }
    }
}

impl VirtioDriver {
    unsafe fn submit(&mut self, sector: u64, buf: &mut [u8], write: bool) {
        let head_idx = self.alloc_desc();
        let data_idx = self.alloc_desc();
        let status_idx = self.alloc_desc();

        let req = VirtioBlkReq {
            type_: if write {
                VIRTIO_BLK_T_OUT
            } else {
                VIRTIO_BLK_T_IN
            },
            reserved: 0,
            sector,
        };

        let mut status: u8 = 111;

        let req_paddr = v2p(&req as *const _ as usize);
        let buf_paddr = v2p(buf.as_ptr() as usize);
        let status_paddr = v2p(&status as *const _ as usize);

        let desc_ptr = self.queue_desc;

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

        let avail = self.queue_avail;
        let idx = (*avail).idx;
        (*avail).ring[idx as usize % QUEUE_SIZE] = head_idx;

        (*avail).idx = idx.wrapping_add(1);

        // Memory barrier
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

        outw(self.io_base + VIRTIO_REG_QUEUE_NOTIFY, 0);

        let used = self.queue_used;

        loop {
            let val = core::ptr::read_volatile(&(*used).idx);
            if val != self.used_idx {
                break;
            }
            // Option<Box<T>> is guaranteed to be 0 for None.
            let proc_ptr = addr_of!(crate::proc::CURRENT_PROCESS) as *const usize;
            if unsafe { *proc_ptr != 0 } {
                crate::proc::sleep(
                    addr_of!(VIRTIO_BLK_DRIVER) as usize,
                    None::<crate::spinlock::SpinlockGuard<()>>,
                );
            } else {
                core::arch::asm!("pause");
            }
        }

        self.used_idx = self.used_idx.wrapping_add(1);

        if status != 0 {
            uart_println!("Virtio: IO Error status={}", status);
        }

        self.free_desc(head_idx);
        self.free_desc(data_idx);
        self.free_desc(status_idx);
    }

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
