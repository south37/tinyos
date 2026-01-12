use crate::util::{inb, outb};
use core::fmt;

const COM1: u16 = 0x3F8;

pub struct Uart;

pub fn init() {
    unsafe {
        outb(COM1 + 1, 0x00); // Disable all interrupts
        outb(COM1 + 3, 0x80); // Enable DLAB (set baud rate divisor)
        outb(COM1 + 0, 0x03); // Set divisor to 3 (lo byte) 38400 baud
        outb(COM1 + 1, 0x00); //                  (hi byte)
        outb(COM1 + 3, 0x03); // 8 bits, no parity, one stop bit
        outb(COM1 + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
        outb(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
        outb(COM1 + 1, 0x01); // Enable interrupts
    }
}

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

use crate::spinlock::Spinlock;

pub static UART_TX: Spinlock<Uart> = Spinlock::new(Uart, "UART_TX");

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    UART_TX.lock().write_fmt(args).unwrap();
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
