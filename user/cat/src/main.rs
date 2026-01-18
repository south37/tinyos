#![no_std]
#![no_main]

extern crate alloc;
use ulib::{entry, syscall};

entry!(main);

fn main(argc: usize, argv: *const *const u8) {
    let args = unsafe { ulib::env::args(argc, argv) };

    if args.len() <= 1 {
        cat(0);
    } else {
        for arg in &args[1..] {
            let fd = syscall::open(arg.to_str().unwrap(), 0);
            if fd < 0 {
                ulib::print!("cat: cannot open {}\n", arg.to_str().unwrap());
                continue;
            }
            cat(fd);
            syscall::close(fd);
        }
    }
    syscall::exit(0);
}

fn cat(fd: i32) {
    let mut buf = [0u8; 512];
    loop {
        let n = syscall::read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        syscall::write(1, &buf[0..n as usize]);
    }
}
