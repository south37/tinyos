#![no_std]
#![no_main]

use ulib::{entry, print, println, syscall};

entry!(main);

fn main(argc: usize, argv: *const *const u8) {
    for i in 1..argc {
        let arg_ptr = unsafe { *argv.add(i) };
        let len = unsafe {
            let mut l = 0;
            while *arg_ptr.add(l) != 0 {
                l += 1;
            }
            l
        };
        let arg = unsafe {
            core::str::from_utf8(core::slice::from_raw_parts(arg_ptr, len)).unwrap_or("")
        };

        print!("{}", arg);
        if i < argc - 1 {
            print!(" ");
        }
    }
    println!();
    syscall::exit(0);
}
