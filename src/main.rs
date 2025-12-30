#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    uart_print("Hello, world!\r\n");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

const COM1: u16 = 0x3F8;

unsafe fn uart_write_byte(byte: u8) {
    // Transmit Holding Register (THR)
    core::arch::asm!(
        "out dx, al",
        in("dx") COM1,
        in("al") byte,
    );
}

fn uart_print(s: &str) {
    for b in s.bytes() {
        unsafe {
            uart_write_byte(b);
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
