#![no_std]
#![no_main]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use ulib::{entry, print, println, syscall};

entry!(main);

fn main(_argc: usize, _argv: *const *const u8) {
    loop {
        print!("$ ");

        let mut line = String::new();
        loop {
            let mut c = [0u8; 1];
            if syscall::read(0, &mut c) < 1 {
                break;
            }
            if c[0] == b'\n' || c[0] == b'\r' {
                break;
            }
            line.push(c[0] as char);
        }

        if line.is_empty() {
            continue;
        }

        // Split into args
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        if parts[0] == "exit" {
            syscall::exit(0);
        }

        // Create null-terminated strings for exec
        let mut args: Vec<String> = Vec::new();
        for p in parts {
            let mut s = String::from(p);
            s.push('\0');
            args.push(s);
        }

        // Create argv array of pointers
        let mut argv: Vec<*const u8> = Vec::new();
        for arg in &args {
            argv.push(arg.as_ptr());
        }
        argv.push(core::ptr::null());

        // Fork and Exec
        let pid = syscall::fork();
        if pid < 0 {
            println!("fork failed");
        } else if pid == 0 {
            // Child
            let ret = syscall::exec(argv[0], &argv);
            if ret == -1 {
                println!("exec failed");
            }
            syscall::exit(1);
        } else {
            // Parent
            syscall::wait(None);
        }
    }
}
