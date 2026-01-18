use crate::allocator::Allocator;
use crate::proc::mycpu;
use crate::vm;

pub fn growproc(n: isize) -> Result<(), ()> {
    let cpu = mycpu();
    let p = unsafe { &mut *cpu.process.unwrap() };
    let sz = p.sz;

    if n > 0 {
        // Lazy allocation (= demand paging): just increment sz.
        // Physical memory will be allocated in page fault handler.
        p.sz += n as usize;
    } else if n < 0 {
        let new_sz = (sz as isize + n) as usize;
        let new_sz = vm::uvm_dealloc(p.pgdir, &mut crate::allocator::ALLOCATOR.lock(), sz, new_sz);
        p.sz = new_sz;
    }

    vm::switch(p.pgdir);
    Ok(())
}
