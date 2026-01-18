use crate::syscall;

#[repr(C)]
struct Header {
    ptr: *mut Header,
    size: usize,
}

static mut BASE: Header = Header {
    ptr: core::ptr::null_mut(),
    size: 0,
};
static mut FREEP: *mut Header = core::ptr::null_mut();

pub unsafe fn malloc(nbytes: usize) -> *mut u8 {
    let mut p: *mut Header;
    let mut prevp: *mut Header;
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
            // morecore returns a block that is freed.
            // The free will insert it into the list, and next loop iteration will find it.
            // Wait, free puts it in list. p is reset?
            // standard K&R malloc resets p to FREEP because free updates FREEP?
            // Let's analyze morecore.
        }

        prevp = p;
        p = (*p).ptr;
    }
}

unsafe fn morecore(nu: usize) -> *mut Header {
    let mut n = nu;
    if n < 4096 {
        n = 4096;
    }
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
