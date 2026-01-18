#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
}
