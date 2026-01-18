#![no_std]
#![no_main]

use ulib::{entry, println, syscall};

entry!(main);

fn main(_argc: usize, _argv: *const *const u8) {
    println!("init: starting");

    loop {
        let pid = syscall::fork();
        if pid < 0 {
            println!("init: fork failed");
            continue;
        }

        if pid == 0 {
            // Child
            let sh = "sh\0";
            let argv = [sh.as_ptr(), core::ptr::null()];
            syscall::exec(sh.as_ptr(), &argv);
            println!("init: exec sh failed");
            syscall::exit(1);
        } else {
            // Parent
            loop {
                // Wait for shell to exit
                let wpid = syscall::wait(None);
                if wpid == pid {
                    // Shell exited, restart it
                    break;
                } else if wpid < 0 {
                    // Wait failed?
                }
            }
        }
    }
}
