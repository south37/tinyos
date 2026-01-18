#![no_std]
#![no_main]

use ulib::{entry, print, println, syscall};

entry!(main);

fn main(_argc: usize, _argv: *const *const u8) {
    let mut buf = [0u8; 100];

    loop {
        print!("$ ");

        // Memset buf to 0
        for b in buf.iter_mut() {
            *b = 0;
        }

        let mut i = 0;
        while i < buf.len() - 1 {
            let mut c = [0u8; 1];
            if syscall::read(0, &mut c) < 1 {
                break;
            }
            if c[0] == b'\n' || c[0] == b'\r' {
                break;
            }
            buf[i] = c[0];
            i += 1;
        }

        if i == 0 {
            continue;
        }

        let line = core::str::from_utf8(&buf[..i]).unwrap_or("");

        // Parse args
        // We need to construct a C-compatible argv array with null pointers
        // The strings themselves must be null-terminated.
        // We can modify buf in place to insert nulls.

        // Let's copy to a new structure or modify buf.
        // Since we are no_std, let's just index into buf.

        let mut argv_ptrs: [*const u8; 11] = [core::ptr::null(); 11]; // MAXARGS+1
        let mut argc = 0;

        let mut ptr = 0;
        while ptr < i && argc < 10 {
            // Skip spaces
            while ptr < i && buf[ptr] == b' ' {
                ptr += 1;
            }
            if ptr == i {
                break;
            }

            argv_ptrs[argc] = &buf[ptr] as *const u8;
            argc += 1;

            // Find end of token
            while ptr < i && buf[ptr] != b' ' {
                ptr += 1;
            }

            // Null terminate
            if ptr < buf.len() {
                // Should always be true as i < buf.len()-1
                buf[ptr] = 0;
                ptr += 1;
            }
        }

        if argc == 0 {
            continue;
        }

        // Check for built-in commands
        let cmd = unsafe {
            let mut len = 0;
            while *argv_ptrs[0].add(len) != 0 {
                len += 1;
            }
            core::str::from_utf8(core::slice::from_raw_parts(argv_ptrs[0], len)).unwrap_or("")
        };

        if cmd == "exit" {
            syscall::exit(0);
        }

        // Fork and Exec
        let pid = syscall::fork();
        if pid < 0 {
            println!("fork failed");
        } else if pid == 0 {
            // Child
            // exec(argv[0], argv)
            let ret = syscall::exec(argv_ptrs[0], &argv_ptrs[0..argc + 1]); // Slice including null
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
