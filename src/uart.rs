use core::fmt;

const COM1: u16 = 0x3F8;

pub struct Uart;

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            uart_putc(b);
        }
        Ok(())
    }
}

pub fn uart_putc(byte: u8) {
    unsafe {
        // Wait for THR empty
        while (inb(COM1 + 5) & 0x20) == 0 {}
        outb(COM1, byte);
    }
}

pub fn uart_getc() -> Option<u8> {
    unsafe {
        if (inb(COM1 + 5) & 0x01) == 0 {
            None
        } else {
            Some(inb(COM1))
        }
    }
}

// Interrupt handler
pub fn uartintr() {
    crate::console::consoleintr(uart_getc);
}

unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") val,
    );
}

unsafe fn inb(port: u16) -> u8 {
    let ret: u8;
    core::arch::asm!(
        "in al, dx",
        out("al") ret,
        in("dx") port,
    );
    ret
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
