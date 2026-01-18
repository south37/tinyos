#![no_std]
#![no_main]

use ulib::{entry, env, print, println, syscall};

entry!(main);

fn main(argc: usize, argv: *const *const u8) {
    let args = unsafe { env::args(argc, argv) };

    for (i, arg) in args.iter().skip(1).enumerate() {
        print!("{}", arg.to_str().unwrap());
        if i < args.len() - 2 {
            print!(" ");
        }
    }
    println!();
    syscall::exit(0);
}
