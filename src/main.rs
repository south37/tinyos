#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(link_section = ".multiboot")]
#[unsafe(no_mangle)]
pub static MULTIBOOT_HEADER: [u32; 3] = [
    0x1BADB002, // magic
    0x00000003, // flags
    0xE4524FFB, // checksum = -(magic + flags)
];

#[unsafe(no_mangle)]
pub extern "C" fn _start(_multiboot_info: u32) -> ! {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
