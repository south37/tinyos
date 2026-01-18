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

        // Parse pipe |
        let mut pipe_cmd_strs: Vec<Vec<&str>> = Vec::new();
        let mut current_cmd_strs: Vec<&str> = Vec::new();

        for p in parts {
            if p == "|" {
                if !current_cmd_strs.is_empty() {
                    pipe_cmd_strs.push(current_cmd_strs);
                    current_cmd_strs = Vec::new();
                }
            } else {
                current_cmd_strs.push(p);
            }
        }
        if !current_cmd_strs.is_empty() {
            pipe_cmd_strs.push(current_cmd_strs);
        }

        if pipe_cmd_strs.is_empty() {
            continue;
        }

        if pipe_cmd_strs.len() == 1 {
            // Normal command
            run_cmd_strs(&pipe_cmd_strs[0]);
        } else if pipe_cmd_strs.len() == 2 {
            // Pipe command
            let fds: &mut [i32; 2] = &mut [0, 0];
            if syscall::pipe(fds) < 0 {
                println!("pipe failed");
                continue;
            }

            let pid1 = syscall::fork();
            if pid1 < 0 {
                println!("fork failed");
            } else if pid1 == 0 {
                // Left child
                syscall::close(1);
                syscall::dup(fds[1]);
                syscall::close(fds[0]);
                syscall::close(fds[1]);

                run_cmd_strs(&pipe_cmd_strs[0]);
                syscall::exit(0);
            }

            let pid2 = syscall::fork();
            if pid2 < 0 {
                println!("fork failed");
            } else if pid2 == 0 {
                // Right child
                syscall::close(0);
                syscall::dup(fds[0]);
                syscall::close(fds[0]);
                syscall::close(fds[1]);

                run_cmd_strs(&pipe_cmd_strs[1]);
                syscall::exit(0);
            }

            syscall::close(fds[0]);
            syscall::close(fds[1]);
            syscall::wait(None);
            syscall::wait(None);
        } else {
            println!("Only single pipe supported");
        }
    }
}

fn run_cmd_strs(args_strs: &Vec<&str>) {
    let mut args: Vec<String> = Vec::new();
    for p in args_strs {
        let mut s = String::from(*p);
        s.push('\0');
        args.push(s);
    }

    // Create argv array of pointers
    let mut argv: Vec<*const u8> = Vec::new();
    for arg in &args {
        argv.push(arg.as_ptr());
    }
    argv.push(core::ptr::null());

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
