#![no_std]
#![no_main]

use ulib::{alloc, entry, println};

entry!(main);

fn main(_argc: usize, _argv: *const *const u8) {
    println!("malloc_test: starting");

    // Test 1: Small allocation
    let size = 100;
    let ptr = unsafe { alloc::malloc(size) };
    if ptr.is_null() {
        println!("malloc_test: malloc failed");
        return;
    }
    println!("malloc_test: allocated {} bytes at {:p}", size, ptr);

    // Verify write/read
    unsafe {
        for i in 0..size {
            *ptr.add(i) = (i % 256) as u8;
        }
    }

    // Check values
    let mut ok = true;
    unsafe {
        for i in 0..size {
            if *ptr.add(i) != (i % 256) as u8 {
                println!("malloc_test: verify failed at {}", i);
                ok = false;
                break;
            }
        }
    }
    if ok {
        println!("malloc_test: write/read verification passed");
    }

    // Test 2: Free and Reuse
    unsafe { alloc::free(ptr) };
    println!("malloc_test: freed memory");

    let ptr2 = unsafe { alloc::malloc(size) };
    println!("malloc_test: allocated again at {:p}", ptr2);

    // In a simple allocator, ptr2 might be same as ptr if it reused the block
    if ptr == ptr2 {
        println!("malloc_test: reused freed block (expected)");
    } else {
        println!(
            "malloc_test: did not reuse block (might be okay if fragmentation or different logic)"
        );
    }

    // Test 3: Large allocation (trigger sbrk)
    let large_size = 8192;
    let ptr3 = unsafe { alloc::malloc(large_size) };
    if ptr3.is_null() {
        println!("malloc_test: large malloc failed");
    } else {
        println!("malloc_test: allocated {} bytes at {:p}", large_size, ptr3);
        unsafe { alloc::free(ptr3) };
    }

    unsafe { alloc::free(ptr2) };

    println!("malloc_test: finished");
}
