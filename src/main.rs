#![no_std]
#![no_main]

mod uart;

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    uart_println!("Hello, world!");

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_println!("panicked: {}", info.message());
    loop {}
}
