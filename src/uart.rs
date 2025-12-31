use core::fmt;

const COM1: u16 = 0x3F8;

pub struct Uart;

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            unsafe {
                uart_write_byte(b);
            }
        }
        Ok(())
    }
}

unsafe fn uart_write_byte(byte: u8) {
    // Transmit Holding Register (THR)
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") COM1,
            in("al") byte,
        );
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    Uart.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! uart_print {
    ($($arg:tt)*) => ($crate::uart::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! uart_println {
    () => ($crate::uart_print!("\r\n"));
    ($($arg:tt)*) => ($crate::uart_print!("{}\r\n", format_args!($($arg)*)));
}
