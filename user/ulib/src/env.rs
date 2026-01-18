use core::ffi::CStr;
use rust_alloc::vec::Vec;

pub unsafe fn args(argc: usize, argv: *const *const u8) -> Vec<&'static CStr> {
    let mut args = Vec::with_capacity(argc);
    for i in 0..argc {
        let ptr = *argv.add(i);
        let cstr = CStr::from_ptr(ptr as *const i8);
        args.push(cstr);
    }
    args
}
