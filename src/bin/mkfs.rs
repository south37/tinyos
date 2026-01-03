use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::mem::size_of;

// Constants mapping fs.rs
const BSIZE: usize = 1024;
const ROOTINO: u32 = 1;
const FSMAGIC: u32 = 0x10203040;
const NDIRECT: usize = 12;
// const NINDIRECT: usize = BSIZE / size_of::<u32>();
const MAXFILE: usize = NDIRECT + 100; // Simplified

// Inode types
const T_DIR: u16 = 1;
const T_FILE: u16 = 2;
const T_DEV: u16 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct SuperBlock {
    magic: u32,
    size: u32,
    nblocks: u32,
    ninodes: u32,
    nlog: u32,
    logstart: u32,
    inodestart: u32,
    bmapstart: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DiskInode {
    type_: u16,
    major: u16,
    minor: u16,
    nlink: u16,
    size: u32,
    addrs: [u32; NDIRECT + 1],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Dirent {
    inum: u16,
    name: [u8; 14],
}

impl Default for Dirent {
    fn default() -> Self {
        Self {
            inum: 0,
            name: [0; 14],
        }
    }
}

const NINODES: u32 = 200;
const NLOG: u32 = 30; // Not used yet
const FSSIZE: u32 = 1000; // Total blocks

fn main() -> std::io::Result<()> {
    let mut file = File::create("disk.img")?;

    // 128MB image size (to match qemu.sh)
    // qemu.sh expects 128M? Actually we can just write 128MB of zeros or seek.
    file.set_len(128 * 1024 * 1024)?;

    // Calculate layout
    let nbitmap = FSSIZE / (BSIZE as u32 * 8) + 1;
    let ninodeblocks = NINODES / (BSIZE as u32 / size_of::<DiskInode>() as u32) + 1;
    let nmeta = 2 + NLOG + ninodeblocks + nbitmap;
    let ndata = FSSIZE - nmeta;

    let sb = SuperBlock {
        magic: FSMAGIC,
        size: FSSIZE,
        nblocks: ndata,
        ninodes: NINODES,
        nlog: NLOG,
        logstart: 2,
        inodestart: 2 + NLOG,
        bmapstart: 2 + NLOG + ninodeblocks,
    };

    println!("SuperBlock: {:?}", sb);

    // Write Superblock at block 1
    // Block 0 is padding (bootblock).
    // BSIZE=1024. So offset 1024.

    let mut buf = [0u8; BSIZE];

    // Serialize SB
    let sb_ptr = &sb as *const SuperBlock as *const u8;
    let sb_slice = unsafe { std::slice::from_raw_parts(sb_ptr, size_of::<SuperBlock>()) };
    buf[0..sb_slice.len()].copy_from_slice(sb_slice);

    file.seek(SeekFrom::Start(BSIZE as u64))?;
    file.write_all(&buf)?;

    // Initialize Root Inode (inum 1)
    // Inodes start at sb.inodestart * BSIZE.
    // Inode 0 is unused. Inode 1 is root.
    // We need to write DiskInode at correct offset.

    let ipb = BSIZE / size_of::<DiskInode>();
    let root_block = sb.inodestart + (ROOTINO as u32 / ipb as u32);
    let root_offset = (ROOTINO as u32 % ipb as u32) * size_of::<DiskInode>() as u32;

    // Alloc data block for root directory
    let root_data_block = sb.bmapstart + nbitmap + 1; // First free data block?
    // Actually free map starts at bmapstart.
    // We need to mark used blocks in bitmap.
    // Used:
    // 0..nmeta (boot, sb, log, inodes, bitmap) are META.
    // nmeta.. are DATA.
    // We allocated root_data_block.

    let mut root_dinode = DiskInode::default();
    root_dinode.type_ = T_DIR;
    root_dinode.nlink = 1;
    root_dinode.size = size_of::<Dirent>() as u32 * 2; // . and ..
    root_dinode.addrs[0] = root_data_block;

    // Write Root Inode
    // Read inode block first? It's zeroed by set_len? Yes.
    // So just write partial?
    // seek to block * BSIZE + offset
    file.seek(SeekFrom::Start(
        root_block as u64 * BSIZE as u64 + root_offset as u64,
    ))?;

    let dinode_ptr = &root_dinode as *const DiskInode as *const u8;
    let dinode_slice = unsafe { std::slice::from_raw_parts(dinode_ptr, size_of::<DiskInode>()) };
    file.write_all(dinode_slice)?;

    // Write Root Directory Entries (. and ..)
    let mut dirents = [Dirent::default(); 2];
    dirents[0].inum = ROOTINO as u16;
    dirents[0].name[0] = b'.';

    dirents[1].inum = ROOTINO as u16; // Parent is self for root
    dirents[1].name[0] = b'.';
    dirents[1].name[1] = b'.';

    // Serialize dirents
    let mut dir_buf = [0u8; BSIZE];
    let de_size = size_of::<Dirent>();
    for (i, de) in dirents.iter().enumerate() {
        let ptr = de as *const Dirent as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(ptr, de_size) };
        dir_buf[i * de_size..(i + 1) * de_size].copy_from_slice(slice);
    }

    // Write dirents to root_data_block
    file.seek(SeekFrom::Start(root_data_block as u64 * BSIZE as u64))?;
    file.write_all(&dir_buf)?;

    // Update Bitmap
    // We need to mark blocks 0..root_data_block as used.
    // Bitmap block is at sb.bmapstart.
    // We used up to root_data_block (inclusive).
    // Bitmap bits: 1 = free? Or 1 = used?
    // Usually 0=free, 1=used.
    // Blocks 0..root_data_block are used.
    // We need to fill bitmap.

    let mut bitmap = [0u8; BSIZE];
    for i in 0..=root_data_block {
        let byte = i / 8;
        let bit = i % 8;
        bitmap[byte as usize] |= 1 << bit;
    }

    file.seek(SeekFrom::Start(sb.bmapstart as u64 * BSIZE as u64))?;
    file.write_all(&bitmap)?;

    println!(
        "mkfs: created disk.img with root inode at block {}",
        root_data_block
    );

    Ok(())
}
