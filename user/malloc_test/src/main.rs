#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use ulib::{entry, println};

entry!(main);

fn main(_argc: usize, _argv: *const *const u8) {
    println!("malloc_test: starting");

    // Test 1: Vec
    let mut v = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    println!(
        "malloc_test: vec pushed 100 items. len={} cap={}",
        v.len(),
        v.capacity()
    );

    let mut ok = true;
    for (i, x) in v.iter().enumerate() {
        if *x != i {
            println!("malloc_test: vec verify failed at {}", i);
            ok = false;
            break;
        }
    }
    if ok {
        println!("malloc_test: vec verification passed");
    }

    // Test 2: Box
    let b = Box::new(12345);
    println!("malloc_test: box value = {}", *b);
    if *b == 12345 {
        println!("malloc_test: box verification passed");
    } else {
        println!("malloc_test: box verification failed");
    }

    // Test 3: Large separate allocations
    let v2: Vec<u8> = alloc::vec![0u8; 8192];
    println!("malloc_test: large vec allocated. len={}", v2.len());

    println!("malloc_test: finished");
}
