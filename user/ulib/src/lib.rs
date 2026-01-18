#![no_std]

use core::panic::PanicInfo;

pub mod alloc;
pub mod fs;
pub mod io;
pub mod syscall;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    syscall::exit(1);
}

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[no_mangle]
        pub extern "C" fn start(argc: usize, argv: *const *const u8) -> ! {
            unsafe { $path(argc, argv) }
            $crate::syscall::exit(0);
        }
    };
}
