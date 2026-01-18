use crate::syscall;
use core::alloc::{GlobalAlloc, Layout};

#[repr(C)]
// 16 bytes header for each block
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

// Bins for small sizes.
const MIN_UNITS: usize = 2; // Minimum units (1 header + 1 data) => 16 bytes data
const MAX_BIN_UNITS: usize = 31; // Max units for bins => 30*16 = 480 bytes data
static mut BINS: [*mut Header; MAX_BIN_UNITS + 1] = [core::ptr::null_mut(); MAX_BIN_UNITS + 1];

// First block for Large List sentinel
static mut FIRST_BLOCK: Header = Header {
    next: core::ptr::null_mut(),
    nunits: 0,
};
// Large free block list (circular, sorted by address).
static mut LARGE_LIST: *mut Header = core::ptr::null_mut();

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
    // Init LARGE_LIST if it is not initialized.
    if LARGE_LIST.is_null() {
        FIRST_BLOCK.next = &mut FIRST_BLOCK as *mut Header;
        LARGE_LIST = &mut FIRST_BLOCK as *mut Header;
    }

    // Require additional 1 unit for Header
    let nunits = (nbytes + core::mem::size_of::<Header>() - 1) / core::mem::size_of::<Header>() + 1;
    let nunits = if nunits < MIN_UNITS {
        MIN_UNITS
    } else {
        nunits
    };

    // Check bins if small request
    if nunits <= MAX_BIN_UNITS {
        if !BINS[nunits].is_null() {
            let p = BINS[nunits];
            BINS[nunits] = (*p).next;
            return p.add(1) as *mut u8;
        }
    }

    // Allocate from LARGE_LIST
    let mut prevp: *mut Header = LARGE_LIST;
    let mut p: *mut Header = (*prevp).next;

    loop {
        if (*p).nunits >= nunits {
            if (*p).nunits == nunits {
                // Exact match: Remove this block (= p) from free list.
                (*prevp).next = (*p).next;
                LARGE_LIST = prevp;
                return p.add(1) as *mut u8;
            } else {
                // Split: Use tail part (shrinking p stays in list)
                (*p).nunits -= nunits;
                let ret_p = nbr(p);
                (*ret_p).nunits = nunits;
                // Since prevp is still in list, LARGE_LIST can point to prevp.
                LARGE_LIST = prevp;
                return ret_p.add(1) as *mut u8;
            }
        }

        if p == LARGE_LIST {
            // Checked all blocks in free list. Get more memory.
            let success = sbrk(nunits);
            if !success {
                return core::ptr::null_mut();
            }
            // sbrk inserts a new block to LARGE_LIST, and may set LARGE_LIST to prev of inserted block.
            // To check the inserted block immediately, we reset prevp and p.
            prevp = LARGE_LIST;
            p = (*LARGE_LIST).next;
        } else {
            prevp = p;
            p = (*p).next;
        }
    }
}

unsafe fn sbrk(nunits: usize) -> bool {
    let alloc_units = if nunits < 4096 { 4096 } else { nunits };
    let p = syscall::sbrk((alloc_units * core::mem::size_of::<Header>()) as isize);
    if p == -1 {
        return false;
    }

    let p = p as *mut Header;
    (*p).nunits = alloc_units;
    // Insert into LARGE_LIST
    free_large(p);
    true
}

pub unsafe fn free(targetp: *mut u8) {
    let p = (targetp as *mut Header).offset(-1);
    let nunits = (*p).nunits;

    if nunits <= MAX_BIN_UNITS {
        (*p).next = BINS[nunits];
        BINS[nunits] = p;
    } else {
        free_large(p);
    }
}

unsafe fn free_large(p: *mut Header) {
    let mut prevp: *mut Header = LARGE_LIST;

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

    LARGE_LIST = prevp;
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
