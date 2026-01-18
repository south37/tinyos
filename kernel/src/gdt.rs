use core::mem::size_of;

const NCPU: usize = 8;
static mut TSS: [TaskStateSegment; NCPU] = [TaskStateSegment::new(); NCPU];

static mut GDT: [GlobalDescriptorTable; NCPU] = [GlobalDescriptorTable::new(); NCPU];

pub fn init(cpuid: usize) {
    unsafe {
        // Use addr_of_mut! to avoid creating intermediate references to static mut
        let gdt = &mut GDT[cpuid];
        *gdt = GlobalDescriptorTable::new();

        // Index 1: Kernel Code
        gdt.set_entry(KCODE_SELECTOR_INDEX, Descriptor::kernel_code_segment());
        // Index 2: Kernel Data
        gdt.set_entry(KDATA_SELECTOR_INDEX, Descriptor::kernel_data_segment());
        // Index 3: User Code
        gdt.set_entry(UCODE_SELECTOR_INDEX, Descriptor::user_code_segment());
        // Index 4: User Data
        gdt.set_entry(UDATA_SELECTOR_INDEX, Descriptor::user_data_segment());

        // Index 5: TSS (128 bit)
        let tss = &TSS[cpuid];
        let (tss_low, tss_high) = Descriptor::tss_segment(tss);
        gdt.set_entry(TSS_SELECTOR_INDEX, tss_low);
        gdt.set_entry(TSS_SELECTOR_INDEX + 1, tss_high);

        gdt.load();

        // Reload segment registers
        reload_segments(KCODE_SELECTOR, KDATA_SELECTOR);

        // Load Task Register
        load_tr(TSS_SELECTOR);
    }
}

pub fn tss_addr(cpuid: usize) -> u64 {
    unsafe { core::ptr::addr_of!(TSS[cpuid]) as u64 }
}

unsafe fn load_tr(selector: u16) {
    unsafe {
        core::arch::asm!("ltr {0:x}", in(reg) selector, options(nostack, preserves_flags));
    }
}

// Segment Selectors
const KCODE_SELECTOR_INDEX: usize = 1;
const KDATA_SELECTOR_INDEX: usize = 2;
const UCODE_SELECTOR_INDEX: usize = 3;
const UDATA_SELECTOR_INDEX: usize = 4;
const TSS_SELECTOR_INDEX: usize = 5;
pub const KCODE_SELECTOR: u16 = (KCODE_SELECTOR_INDEX << 3) as u16;
pub const KDATA_SELECTOR: u16 = (KDATA_SELECTOR_INDEX << 3) as u16;
pub const UCODE_SELECTOR: u16 = (UCODE_SELECTOR_INDEX << 3 | 3) as u16;
pub const UDATA_SELECTOR: u16 = (UDATA_SELECTOR_INDEX << 3 | 3) as u16;
pub const TSS_SELECTOR: u16 = (TSS_SELECTOR_INDEX << 3) as u16;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct TaskStateSegment {
    reserved1: u32,                      // 4 bytes
    pub privilege_stack_table: [u64; 3], // 24 bytes. RSP0-RSP2.
    reserved2: u64,                      // 8 bytes. Scratch space.
    pub interrupt_stack_table: [u64; 7], // 56 bytes. IST1-7.
    reserved3: u64,                      // 8 bytes
    reserved4: u16,                      // 2 bytes
    pub iomap_base: u16,                 // 2 bytes
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved1: 0,
            privilege_stack_table: [0; 3],
            reserved2: 0,
            interrupt_stack_table: [0; 7],
            reserved3: 0,
            reserved4: 0,
            iomap_base: 104, // End of TSS
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GlobalDescriptorTable {
    table: [Descriptor; 7], // Null, Kernel Code, Kernel Data, User Code, User Data, TSS (2 entries)
}

impl GlobalDescriptorTable {
    pub const fn new() -> Self {
        Self {
            table: [Descriptor(0); 7], // Default is 0
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

    pub fn tss_segment(tss: &'static TaskStateSegment) -> (Self, Self) {
        let ptr = tss as *const _ as u64;
        let size = size_of::<TaskStateSegment>() as u64 - 1; // = 103

        let low = (size & 0xFFFF)
            | ((ptr & 0xFFFFFF) << 16)
            | (0x9 << 40) // Type: 0x9 (TSS Available)
            | (3 << 45)   // DPL = 3
            | (1 << 47)   // Present
            | ((size & 0xF0000) << 32)
            | ((ptr & 0xFF000000) << 32);

        let high = ptr >> 32;

        (Self(low), Self(high))
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

pub unsafe fn set_kernel_stack(stack: u64, cpuid: usize) {
    unsafe {
        TSS[cpuid].privilege_stack_table[0] = stack;
    }
}
