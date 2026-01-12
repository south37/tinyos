#![allow(static_mut_refs)]
use crate::spinlock::Spinlock;
use crate::uart::uart_putc;

pub const INPUT_BUF_SIZE: usize = 128;

pub struct Console {
    pub buf: [u8; INPUT_BUF_SIZE],
    pub r: usize, // Read index
    pub w: usize, // Write index
    pub e: usize, // Edit index
}

pub static CONSOLE: Spinlock<Console> = Spinlock::new(Console {
    buf: [0; INPUT_BUF_SIZE],
    r: 0,
    w: 0,
    e: 0,
});

// Write to console (wraps uart_putc)
pub fn consolewrite(src: u64, n: usize) -> usize {
    let buf = unsafe { core::slice::from_raw_parts(src as *const u8, n) };
    for &b in buf {
        uart_putc(b);
    }
    n
}

// Read from console
pub fn consoleread(dst: u64, n: usize) -> usize {
    let mut guard = CONSOLE.lock();
    let mut target = dst as *mut u8;
    let mut c: u8;
    let mut count = 0;

    while count < n {
        // Wait for input
        while guard.r == guard.w {
            if unsafe { crate::proc::killed(crate::proc::CURRENT_PROCESS.as_deref().unwrap()) } {
                return 0; // -1?
            }
            crate::proc::sleep(
                unsafe { core::ptr::addr_of!(guard.r) as usize },
                Some(guard),
            );
            guard = CONSOLE.lock();
        }

        c = guard.buf[guard.r % INPUT_BUF_SIZE];
        guard.r = guard.r.wrapping_add(1);

        if c == 4 {
            // Ctrl-D (EOF)
            if count > 0 {
                // Save it for next time? typical Unix: return what we have.
                // But here we consumed it.
                guard.r -= 1; // Put back? No.
            }
            // EOF
            return count;
        }

        unsafe {
            *target = c;
            target = target.add(1);
        }
        count += 1;

        if c == b'\n' {
            break;
        }
    }
    count
}

// Called by UART trap handler on character input
pub fn consoleintr(c: fn() -> Option<u8>) {
    let mut guard = CONSOLE.lock();
    loop {
        let c_in = c();
        if c_in.is_none() {
            break;
        }
        let c = c_in.unwrap();

        match c {
            // C-U
            21 => {
                while guard.e != guard.w
                    && guard.buf[guard.e.wrapping_sub(1) % INPUT_BUF_SIZE] != b'\n'
                {
                    guard.e = guard.e.wrapping_sub(1);
                    backspace();
                }
            }
            // C-H or Backspace
            8 | 127 => {
                if guard.e != guard.w {
                    guard.e = guard.e.wrapping_sub(1);
                    backspace();
                }
            }
            _ => {
                if c != 0 && (guard.e.wrapping_sub(guard.r) < INPUT_BUF_SIZE) {
                    let val = if c == b'\r' { b'\n' } else { c };
                    let idx = guard.e % INPUT_BUF_SIZE;
                    guard.buf[idx] = val;
                    guard.e = guard.e.wrapping_add(1);
                    uart_putc(val);
                    if val == b'\n' || val == 4 || guard.e == guard.r.wrapping_add(INPUT_BUF_SIZE) {
                        guard.w = guard.e;
                        crate::proc::wakeup(unsafe { core::ptr::addr_of!(guard.r) as usize });
                    }
                }
            }
        }
    }
}

const ASCII_BS: u8 = 8;

fn backspace() {
    uart_putc(ASCII_BS);
    uart_putc(b' ');
    uart_putc(ASCII_BS);
}
