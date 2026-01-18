#![no_std]
#![no_main]

use ulib::{entry, fs::DirEntry, println, syscall};

entry!(main);

fn main(argc: usize, argv: *const *const u8) {
    let path = if argc > 1 {
        // Parse argv[1]
        // TODO: helper to convert argv[i] to &str
        "." // Placeholder, default to CWD (which is root for now)
    } else {
        "."
    };

    let fd = syscall::open(path, 0);
    if fd < 0 {
        println!("ls: cannot open {}", path);
        return;
    }

    let mut buf = [0u8; 1024]; // Buffer for DirEntry
    let de_size = core::mem::size_of::<DirEntry>();

    loop {
        let n = syscall::read(fd, &mut buf);
        if n < 0 {
            println!("ls: read error");
            break;
        }
        if n == 0 {
            break;
        }

        // We read raw bytes, need to cast to DirEntry
        // Assume we read exactly one DirEntry or multiple?
        // kernel readi reads bytes. `sys_read` calls `fileread`.
        // `fileread` for Inode calls `fs::readi`.
        // `readi` reads bytes from current offset.
        // So we get a stream of DirEntries.

        // Iterate over buffer
        let mut offset = 0;
        while offset < n as usize {
            if offset + de_size > n as usize {
                break; // Should not happen if read aligns
            }

            let de = unsafe { &*(buf.as_ptr().add(offset) as *const DirEntry) };

            if de.inode != 0 {
                let name_len = de.name_len as usize;
                // variable length record?
                // DirEntry struct in fs.rs has fixed fields but checking `rec_len`.
                // kernel `fs.rs`:
                // pub struct DirEntry { pub inode: u32, pub rec_len: u16, pub name_len: u8, pub file_type: u8 }
                // Name follows the struct.
                // So size_of<DirEntry> is header size (8 bytes).

                // Wait, in `kernel/src/fs.rs`:
                // while ptr < limit { let de = ...; let name_ptr = ptr.add(size_of::<DirEntry>()); ... ptr = ptr.add(de.rec_len) }

                // The `read` syscall reads raw bytes from file. Directory file contains these structures.
                // But `read` might return partial entry if buffer end.
                // We should handle that, but for now assuming small chunks.

                let name_ptr =
                    unsafe { buf.as_ptr().add(offset + core::mem::size_of::<DirEntry>()) };
                let name_slice = unsafe { core::slice::from_raw_parts(name_ptr, name_len) };
                let name = core::str::from_utf8(name_slice).unwrap_or("???");

                println!("{}", name);
            }

            if de.rec_len == 0 {
                break;
            }
            offset += de.rec_len as usize;
        }
    }

    syscall::close(fd);
}
