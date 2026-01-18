#![no_std]
#![no_main]

extern crate alloc;
use ulib::{entry, print, println, syscall};

entry!(main);

fn main(argc: usize, argv: *const *const u8) {
    let args = unsafe { ulib::env::args(argc, argv) };

    if args.len() <= 1 {
        wc(0, "");
    } else {
        for i in 1..args.len() {
            let path_str = match args[i].to_str() {
                Ok(s) => s,
                Err(_) => {
                    println!("wc: invalid utf8 name");
                    continue;
                }
            };

            let fd = syscall::open(path_str, 0);
            if fd < 0 {
                // println!("wc: cannot open {:?}", args[i]); // CStr debug might work or not in no_std core?
                println!("wc: cannot open {}", path_str);
                continue;
            }
            wc(fd, path_str);
            syscall::close(fd);
        }
    }
}

fn wc(fd: i32, name: &str) {
    let mut line_count = 0;
    let mut word_count = 0;
    let mut char_count = 0;
    let mut in_word = false;
    let mut buf = [0u8; 512];

    loop {
        let n = syscall::read(fd, &mut buf);
        if n < 0 {
            println!("wc: read error");
            break;
        }
        if n == 0 {
            break;
        }

        for i in 0..n {
            let c = buf[i as usize];
            char_count += 1;
            if c == b'\n' {
                line_count += 1;
            }
            // Check whitespace
            let is_whitespace = c == b' ' || c == b'\t' || c == b'\n' || c == b'\r';
            if is_whitespace {
                in_word = false;
            } else if !in_word {
                in_word = true;
                word_count += 1;
            }
        }
    }

    println!("{} {} {} {}", line_count, word_count, char_count, name);
}
