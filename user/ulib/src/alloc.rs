use crate::syscall;
use core::alloc::{GlobalAlloc, Layout};

#[repr(C)]
// 16 bytes header
struct Header {
    ptr: *mut Header, // Next block in list
    size: usize,      // Number of units (= size of Header) in this block
}

static mut BASE: Header = Header {
    ptr: core::ptr::null_mut(),
    size: 0,
};
static mut FREEP: *mut Header = core::ptr::null_mut();

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
    let mut p: *mut Header;
    let mut prevp: *mut Header;
    // Require additional 1 unit for Header
    let nunits = (nbytes + core::mem::size_of::<Header>() - 1) / core::mem::size_of::<Header>() + 1;

    if FREEP.is_null() {
        BASE.ptr = &mut BASE as *mut Header; // Circular list
        BASE.size = 0;
        FREEP = &mut BASE as *mut Header;
    }

    prevp = FREEP;
    p = (*prevp).ptr;

    loop {
        if (*p).size >= nunits {
            if (*p).size == nunits {
                // Remove this block (= p) from list.
                (*prevp).ptr = (*p).ptr;
            } else {
                (*p).size -= nunits;
                p = p.add((*p).size);
                (*p).size = nunits;
            }
            FREEP = prevp;
            return p.add(1) as *mut u8;
        }

        if p == FREEP {
            // Need more memory
            let p_new = morecore(nunits);
            if p_new.is_null() {
                return core::ptr::null_mut();
            }
        }

        prevp = p;
        p = (*p).ptr;
    }
}

unsafe fn morecore(nu: usize) -> *mut Header {
    let n = if nu < 4096 { 4096 } else { nu };
    let p = syscall::sbrk((n * core::mem::size_of::<Header>()) as isize);
    if p == -1 {
        return core::ptr::null_mut();
    }

    let hp = p as *mut Header;
    (*hp).size = n;
    free((hp.add(1)) as *mut u8);
    FREEP
}

pub unsafe fn free(ap: *mut u8) {
    let mut bp: *mut Header = (ap as *mut Header).offset(-1);
    let mut p: *mut Header = FREEP;

    // Find insertion point
    while !(bp > p && bp < (*p).ptr) {
        if p >= (*p).ptr && (bp > p || bp < (*p).ptr) {
            break; // At ends of list
        }
        p = (*p).ptr;
    }

    // Join to upper nbr
    if bp.add((*bp).size) == (*p).ptr {
        (*bp).size += (*(*p).ptr).size;
        (*bp).ptr = (*(*p).ptr).ptr;
    } else {
        (*bp).ptr = (*p).ptr;
    }

    // Join to lower nbr
    if p.add((*p).size) == bp {
        (*p).size += (*bp).size;
        (*p).ptr = (*bp).ptr;
    } else {
        (*p).ptr = bp;
    }

    FREEP = p;
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
