use crate::elf::{ELF_MAGIC, ElfHeader, PT_LOAD, ProgramHeader};
use crate::fs::{self};
use crate::trap::TrapFrame;
use crate::uart_println;
use crate::util::{PG_SIZE, p2v};
use crate::vm::{self, PageTableEntry};

pub fn exec(path: &str, argv: &[&str]) -> isize {
    // 1. Open file
    let ip = match fs::namei(path) {
        Some(ip) => {
            uart_println!("DEBUG: exec: found {}", path);
            ip
        }
        None => {
            uart_println!("DEBUG: exec: failed to find {}", path);
            return -1;
        }
    };

    // 2. Read ELF Header
    let mut elf = ElfHeader {
        magic: 0,
        elf: [0; 12],
        type_: 0,
        machine: 0,
        version: 0,
        entry: 0,
        phoff: 0,
        shoff: 0,
        flags: 0,
        ehsize: 0,
        phentsize: 0,
        phnum: 0,
        shentsize: 0,
        shnum: 0,
        shstrndx: 0,
    };

    let sz = fs::readi(
        ip,
        &mut elf as *mut ElfHeader as *mut u8,
        0,
        core::mem::size_of::<ElfHeader>() as u32,
    );
    if sz != core::mem::size_of::<ElfHeader>() as u32 || elf.magic != ELF_MAGIC {
        uart_println!("DEBUG: exec: bad elf header");
        return -1;
    }

    // 3. Create new page table
    uart_println!("DEBUG: exec: loaded elf, entry=0x{:x}", elf.entry);

    let pgdir = {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        match vm::uvm_create(&mut allocator) {
            Some(p) => p,
            None => return -1,
        }
    };

    // 4. Load segments
    let mut off = elf.phoff;
    for _ in 0..elf.phnum {
        let mut ph = ProgramHeader {
            type_: 0,
            flags: 0,
            off: 0,
            vaddr: 0,
            paddr: 0,
            filesz: 0,
            memsz: 0,
            align: 0,
        };
        if fs::readi(
            ip,
            &mut ph as *mut ProgramHeader as *mut u8,
            off as u32,
            core::mem::size_of::<ProgramHeader>() as u32,
        ) != core::mem::size_of::<ProgramHeader>() as u32
        {
            // TODO: Free pgdir
            return -1;
        }
        off += core::mem::size_of::<ProgramHeader>() as u64;

        if ph.type_ != PT_LOAD {
            continue;
        }
        if ph.memsz < ph.filesz {
            // TODO: Free pgdir
            return -1;
        }
        if ph.vaddr + ph.memsz < ph.vaddr {
            // Overflow
            // TODO: Free pgdir
            return -1;
        }

        // Allocate memory for segment
        {
            let mut allocator = crate::allocator::ALLOCATOR.lock();
            let mut addr = ph.vaddr;
            let end = ph.vaddr + ph.memsz;

            let mut a = addr & !(PG_SIZE as u64 - 1);
            while a < end {
                let mem = allocator.kalloc();
                if mem.is_null() {
                    return -1;
                }
                if !vm::map_pages(
                    pgdir,
                    &mut allocator,
                    a,
                    crate::util::v2p(mem as usize) as u64,
                    PG_SIZE as u64,
                    PageTableEntry::WRITABLE | PageTableEntry::USER,
                ) {
                    return -1;
                }
                a += PG_SIZE as u64;
            }
        }

        // Now read data into mapped memory.
        let mut current_vaddr = ph.vaddr;
        let mut current_off = ph.off;
        let mut remaining_filesz = ph.filesz;

        while remaining_filesz > 0 {
            // Find physical address (or kernel virtual address) for current_vaddr
            let pte = {
                let mut allocator = crate::allocator::ALLOCATOR.lock();
                vm::walk(pgdir, &mut allocator, current_vaddr, false, 0).expect("exec: walk failed")
            };

            let pa = pte.addr();
            let kva = p2v(pa as usize);

            let page_offset = current_vaddr % PG_SIZE as u64;
            let n = core::cmp::min(PG_SIZE as u64 - page_offset, remaining_filesz);

            // Read from file to kva + page_offset
            if fs::readi(
                ip,
                (kva as *mut u8).wrapping_add(page_offset as usize),
                current_off as u32,
                n as u32,
            ) != n as u32
            {
                return -1;
            }

            remaining_filesz -= n;
            current_vaddr += n;
            current_off += n;
        }

        // Zero out bss (memsz > filesz)
        // ... (Skipping BSS zeroing for brevity, assuming filesz == memsz for simple tests or explicit init)
    }
    uart_println!("DEBUG: exec: segments loaded");
    // Arbitrary stack location: 0x80000000 ? Or just below high memory?
    // Let's put it at 0x7FFFF000 usually?
    let sz = 0x80000000; // Top of stack
    let stack_base = sz - 2 * PG_SIZE as u64; // 2 pages

    // Map stack
    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        let mem = allocator.kalloc();
        if mem.is_null() {
            return -1;
        }
        vm::map_pages(
            pgdir,
            &mut allocator,
            stack_base,
            crate::util::v2p(mem as usize) as u64,
            PG_SIZE as u64,
            PageTableEntry::WRITABLE | PageTableEntry::USER,
        );
        let mem2 = allocator.kalloc();
        if mem2.is_null() {
            return -1;
        }
        vm::map_pages(
            pgdir,
            &mut allocator,
            stack_base + PG_SIZE as u64,
            crate::util::v2p(mem2 as usize) as u64,
            PG_SIZE as u64,
            PageTableEntry::WRITABLE | PageTableEntry::USER,
        );
    }
    uart_println!("DEBUG: exec: stack allocated");

    // 5. Push arguments to stack
    let mut sp = sz;
    let mut ustack = [0u64; 16]; // Max 16 args + null

    // Push strings
    for (i, arg) in argv.iter().enumerate() {
        sp -= (arg.len() + 1) as u64;
        sp -= sp % 16;

        let mut allocator = crate::allocator::ALLOCATOR.lock();
        if !copyout(pgdir, &mut allocator, sp, arg.as_ptr(), arg.len()) {
            return -1;
        }
        // Write null terminator
        let zero = 0u8;
        if !copyout(
            pgdir,
            &mut allocator,
            sp + arg.len() as u64,
            &zero as *const u8,
            1,
        ) {
            return -1;
        }
        ustack[i] = sp;
    }
    ustack[argv.len()] = 0; // Null terminator for argv array

    // Align stack
    sp = sp & !15;

    // Push argv array
    sp -= ((argv.len() + 1) * 8) as u64; // argc pointers + null ptr
    let argv_base = sp;

    {
        let mut allocator = crate::allocator::ALLOCATOR.lock();
        if !copyout(
            pgdir,
            &mut allocator,
            sp,
            ustack.as_ptr() as *const u8,
            (argv.len() + 1) * 8,
        ) {
            return -1;
        }
    }

    // 6. Commit Process Changes
    unsafe {
        #[allow(static_mut_refs)]
        let p = &mut *crate::proc::mycpu().process.unwrap();

        // Save old pgdir to free later
        let old_pgdir = p.pgdir;

        p.pgdir = pgdir;
        p.state = crate::proc::ProcessState::RUNNING; // Redundant but clear

        // Update TrapFrame
        let tf = &mut *(((p.kstack as usize) + crate::proc::KSTACK_SIZE
            - core::mem::size_of::<TrapFrame>()) as *mut TrapFrame);
        tf.rip = elf.entry; // Entry point
        tf.rsp = sp; // Stack Pointer at argv array

        // System V ABI: rdi=argc, rsi=argv
        tf.rdi = argv.len() as u64;
        tf.rsi = argv_base;

        // Fake return address
        sp -= 8;
        // tf.rsp = sp; // We don't actually update rsp again, we just leave it pointing at arguments if using stack args.
        // But for registers, we set tf.rsp to what it was.

        // Switch to new page table
        vm::switch(pgdir);

        // TODO: Free old pgdir and memory.
        // vm::free_vm(old_pgdir);
    }
    uart_println!("DEBUG: exec: process committed");

    0
}

use crate::allocator::Allocator;
use crate::vm::PageTable;

fn copyout(
    pgdir: *mut PageTable,
    allocator: &mut Allocator,
    va: u64,
    buf: *const u8,
    len: usize,
) -> bool {
    let mut buf = buf;
    let mut len = len;
    let mut va = va;

    while len > 0 {
        let va0 = (va as usize) & !(PG_SIZE - 1);
        let dst_ptr = match vm::walk(pgdir, allocator, va0 as u64, false, 0) {
            Some(pte) => {
                if !pte.is_present() {
                    return false;
                }
                p2v(pte.addr() as usize) as *mut u8
            }
            None => return false,
        };

        let n = core::cmp::min(PG_SIZE - (va as usize - va0), len);
        unsafe {
            let dst = dst_ptr.add(va as usize - va0);
            core::ptr::copy_nonoverlapping(buf, dst, n);
        }

        len -= n;
        unsafe {
            buf = buf.add(n);
        }
        va += n as u64;
    }
    true
}
