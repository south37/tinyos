use crate::syscall;
use core::alloc::{GlobalAlloc, Layout};

#[repr(C)]
// 16 bytes header
struct Header {
    next: *mut Header, // Next block in free list
    nunits: usize,     // Number of units (= size of Header) in this block
}

// Neighbor block address
unsafe fn nbr(p: *mut Header) -> *mut Header {
    p.add((*p).nunits)
}

// Check if p and q are neighbor blocks (p is before q)
unsafe fn is_nbr(p: *mut Header, q: *mut Header) -> bool {
    nbr(p) == q
}

// First block
static mut FIRST_BLOCK: Header = Header {
    next: core::ptr::null_mut(),
    nunits: 0,
};
// Free block list. It is a circular list.
static mut FREE_BLOCK_LIST: *mut Header = core::ptr::null_mut();

pub struct TinyAllocator;

#[global_allocator]
pub static ALLOCATOR: TinyAllocator = TinyAllocator;

unsafe impl GlobalAlloc for TinyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        malloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free(ptr);
    }
}

pub unsafe fn malloc(nbytes: usize) -> *mut u8 {
    // Init FIRST_BLOCK if it is not initialized.
    if FIRST_BLOCK.next.is_null() {
        FIRST_BLOCK.next = &mut FIRST_BLOCK as *mut Header;
    }
    // Init FREE_BLOCK_LIST if it is not initialized.
    if FREE_BLOCK_LIST.is_null() {
        FREE_BLOCK_LIST = &mut FIRST_BLOCK as *mut Header;
    }

    // Require additional 1 unit for Header
    let nunits = (nbytes + core::mem::size_of::<Header>() - 1) / core::mem::size_of::<Header>() + 1;

    let mut prevp: *mut Header = FREE_BLOCK_LIST;
    let mut p: *mut Header = (*prevp).next;

    loop {
        if (*p).nunits >= nunits {
            if (*p).nunits == nunits {
                // Remove this block (= p) from free list.
                (*prevp).next = (*p).next;
            } else {
                (*p).nunits -= nunits;
                p = p.add((*p).nunits);
                (*p).nunits = nunits;
            }
            FREE_BLOCK_LIST = prevp;
            // Skip header
            return p.add(1) as *mut u8;
        }

        if p == FREE_BLOCK_LIST {
            // Checked all blocks in free list, but could not find enough memory.
            // Try to get more memory.
            let success = sbrk(nunits);
            if !success {
                return core::ptr::null_mut();
            }
            // morecore adds a new block to the free list. It will be returned later.
        }

        prevp = p;
        p = (*p).next;
    }
}

unsafe fn sbrk(nunits: usize) -> bool {
    let nunits = if nunits < 4096 { 4096 } else { nunits };
    let p = syscall::sbrk((nunits * core::mem::size_of::<Header>()) as isize);
    if p == -1 {
        return false;
    }

    let p = p as *mut Header;
    (*p).nunits = nunits;
    // sbrk increases memory size, so p should be the last block in free list.
    free((p.add(1)) as *mut u8);
    true
}

pub unsafe fn free(targetp: *mut u8) {
    let mut p: *mut Header = (targetp as *mut Header).offset(-1);
    let mut prevp: *mut Header = FREE_BLOCK_LIST;

    // Find insertion point
    while !(prevp < p && p < (*prevp).next) {
        if (*prevp).next <= prevp && (prevp < p || p < (*prevp).next) {
            break; // At ends of list
        }
        prevp = (*prevp).next;
    }
    // Now p is between prevp and (*prevp).next.
    // Insert p, and merge neighbor blocks if possible.

    let nextp = (*prevp).next;
    if is_nbr(p, nextp) {
        merge(p, nextp);
    } else {
        (*p).next = nextp;
    }

    if is_nbr(prevp, p) {
        merge(prevp, p);
    } else {
        (*prevp).next = p;
    }

    FREE_BLOCK_LIST = prevp;
}

// Merge p and q. Assume p and q are neighbor blocks.
unsafe fn merge(p: *mut Header, q: *mut Header) {
    (*p).nunits += (*q).nunits;
    (*p).next = (*q).next;
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
