use core::mem::size_of;

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable {
    table: [
        Descriptor(0),
        Descriptor(0),
        Descriptor(0),
        Descriptor(0),
        Descriptor(0),
    ], // Manually initialized because const fn is tricky with array repeat of structs sometimes or just to be safe
};

pub fn init() {
    unsafe {
        // Use addr_of_mut! to avoid creating intermediate references to static mut
        let gdt = core::ptr::addr_of_mut!(GDT);
        (*gdt) = GlobalDescriptorTable::new();

        // Index 1: Kernel Code
        (*gdt).set_entry(KCODE_SELECTOR_INDEX, Descriptor::kernel_code_segment());
        // Index 2: Kernel Data
        (*gdt).set_entry(KDATA_SELECTOR_INDEX, Descriptor::kernel_data_segment());
        // Index 3: User Code
        (*gdt).set_entry(UCODE_SELECTOR_INDEX, Descriptor::user_code_segment());
        // Index 4: User Data
        (*gdt).set_entry(UDATA_SELECTOR_INDEX, Descriptor::user_data_segment());

        (*gdt).load();

        // Reload segment registers
        reload_segments(KCODE_SELECTOR, KDATA_SELECTOR);
    }
}

// Segment Selectors
const KCODE_SELECTOR_INDEX: usize = 1;
const KDATA_SELECTOR_INDEX: usize = 2;
const UCODE_SELECTOR_INDEX: usize = 3;
const UDATA_SELECTOR_INDEX: usize = 4;
pub const KCODE_SELECTOR: u16 = (KCODE_SELECTOR_INDEX << 3) as u16;
pub const KDATA_SELECTOR: u16 = (KDATA_SELECTOR_INDEX << 3) as u16;
pub const UCODE_SELECTOR: u16 = (UCODE_SELECTOR_INDEX << 3 | 3) as u16;
pub const UDATA_SELECTOR: u16 = (UDATA_SELECTOR_INDEX << 3 | 3) as u16;

#[repr(C)]
pub struct GlobalDescriptorTable {
    table: [Descriptor; 5], // Null, Kernel Code, Kernel Data, User Code, User Data
}

impl GlobalDescriptorTable {
    pub fn new() -> Self {
        Self {
            table: [Descriptor::default(); 5],
        }
    }

    pub fn set_entry(&mut self, index: usize, desc: Descriptor) -> u16 {
        if index >= self.table.len() {
            panic!("GDT index out of bounds");
        }
        self.table[index] = desc;
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

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Descriptor(pub u64);

impl Descriptor {
    pub fn default() -> Self {
        Self(0)
    }

    // Helper to create a snippet of a GDT entry.
    // For 64-bit code segment:
    // - Type = Executable
    // - S (Descriptor Type) = 1 (Code/Data)
    // - DPL = 0 (kernel) or 3 (user)
    // - P (Present) = 1
    // - L (Long Mode) = 1

    // For 64-bit data segment:
    // - Type = Read/Write
    // - S (Descriptor Type) = 1 (Code/Data)
    // - DPL = 0 (kernel) or 3 (user)
    // - P (Present) = 1

    pub fn kernel_code_segment() -> Self {
        let flags: u64 = (1 << 43) // Executable
            | (1 << 44) // S (Descriptor Type)
            | (0 << 45) // DPL = 0
            | (1 << 47) // P (Present)
            | (1 << 53); // L (Long Mode)
        Self(flags)
    }

    pub fn kernel_data_segment() -> Self {
        let flags: u64 = (1 << 41) // Read/Write
            | (1 << 44) // S (Descriptor Type)
            | (0 << 45) // DPL = 0
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
