use core::mem::size_of;

// Segment Selectors
pub const KCODE_SELECTOR: u16 = 0x08;
pub const KDATA_SELECTOR: u16 = 0x10;
pub const UCODE_SELECTOR: u16 = 0x18 | 3;
pub const UDATA_SELECTOR: u16 = 0x20 | 3;

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Descriptor(pub u64);

impl Descriptor {
    pub fn default() -> Self {
        Self(0)
    }

    pub fn kernel_code_segment() -> Self {
        let flags: u64 = (1 << 43) // Executable
            | (1 << 44) // S (Descriptor Type)
            | (1 << 47) // P (Present)
            | (1 << 53); // L (Long Mode)
        Self(flags)
    }

    pub fn kernel_data_segment() -> Self {
        let flags: u64 = (1 << 41) // Read/Write
            | (1 << 44) // S (Descriptor Type)
            | (1 << 47); // P (Present)
        Self(flags)
    }

    pub fn user_code_segment() -> Self {
        let flags: u64 = (1 << 43) // Executable
            | (1 << 44) // S (Descriptor Type)
            | (3 << 45) // DPL = 3
            | (1 << 47) // P (Present)
            | (1 << 53); // L (Long Mode)
        Self(flags)
    }

    pub fn user_data_segment() -> Self {
        let flags: u64 = (1 << 41) // Read/Write
            | (1 << 44) // S (Descriptor Type)
            | (3 << 45) // DPL = 3
            | (1 << 47); // P (Present)
        Self(flags)
    }
}

#[repr(C)]
pub struct GlobalDescriptorTable {
    table: [Descriptor; 5], // Null, Kernel Code, Kernel Data, User Code, User Data
    next_free: usize,
}

impl GlobalDescriptorTable {
    pub fn new() -> Self {
        Self {
            table: [Descriptor::default(); 5],
            next_free: 1, // Skip null descriptor
        }
    }

    pub fn add_entry(&mut self, desc: Descriptor) -> u16 {
        let index = self.next_free;
        if index >= self.table.len() {
            panic!("GDT full");
        }
        self.table[index] = desc;
        self.next_free += 1;
        (index * 8) as u16
    }

    pub fn load(&'static self) {
        let descriptor = GdtDescriptor {
            limit: (self.table.len() * size_of::<u64>() - 1) as u16,
            base: self.table.as_ptr() as u64,
        };

        unsafe {
            core::arch::asm!("lgdt [{}]", in(reg) &descriptor, options(nostack));
        }
    }
}

#[repr(C, packed)]
struct GdtDescriptor {
    limit: u16,
    base: u64,
}

// Helper to create a snippet of a GDT entry.
// For 64-bit code segment:
// - L (Long Mode) = 1
// - D (Size) = 0 (Must be 0 for 64-bit code)
// - P (Present) = 1
// - S (Descriptor Type) = 1 (Code/Data)
// - Type = Executable | Read
// - DPL = 0

// For 64-bit data segment:
// - P (Present) = 1
// - S (Descriptor Type) = 1 (Code/Data)
// - Type = Read/Write
// - DPL = 0

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable {
    table: [
        Descriptor(0),
        Descriptor(0),
        Descriptor(0),
        Descriptor(0),
        Descriptor(0),
    ], // Manually initialized because const fn is tricky with array repeat of structs sometimes or just to be safe
    next_free: 1,
};

pub fn init() {
    unsafe {
        // Use addr_of_mut! to avoid creating intermediate references to static mut
        let gdt = core::ptr::addr_of_mut!(GDT);
        (*gdt) = GlobalDescriptorTable::new();

        // Index 1: Kernel Code
        let code_selector = (*gdt).add_entry(Descriptor::kernel_code_segment());
        assert_eq!(code_selector, KCODE_SELECTOR);
        // Index 2: Kernel Data
        let data_selector = (*gdt).add_entry(Descriptor::kernel_data_segment());
        assert_eq!(data_selector, KDATA_SELECTOR);
        // Index 3: User Code
        let ucode_selector = (*gdt).add_entry(Descriptor::user_code_segment()) | 3;
        assert_eq!(ucode_selector, UCODE_SELECTOR);
        // Index 4: User Data
        let udata_selector = (*gdt).add_entry(Descriptor::user_data_segment()) | 3;
        assert_eq!(udata_selector, UDATA_SELECTOR);

        (*gdt).load();

        // Reload segment registers
        reload_segments(KCODE_SELECTOR, KDATA_SELECTOR);
    }
}

unsafe fn reload_segments(code_selector: u16, data_selector: u16) {
    unsafe {
        core::arch::asm!(
            "push {0}",
            "lea {1}, [rip + 2f]",
            "push {1}",
            "retfq",
            "2:",
            "mov ds, {2:e}",
            "mov es, {2:e}",
            "mov fs, {2:e}",
            "mov gs, {2:e}",
            "mov ss, {2:e}",
            in(reg) code_selector as u64,
            out(reg) _,
            in(reg) data_selector,
        );
    }
}
