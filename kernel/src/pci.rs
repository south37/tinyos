use crate::util::{inl, outl};

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub base_addr: u32, // Base Address from BAR0 (assumed to be IO base for legacy virtio)
    pub irq_line: u8,
}

unsafe fn pci_read(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let address = (1u32 << 31)
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | (offset as u32 & 0xFC);

    unsafe {
        outl(CONFIG_ADDRESS, address);
        inl(CONFIG_DATA)
    }
}

pub unsafe fn check_device(bus: u8, slot: u8) -> Option<PciDevice> {
    let vendor_id = unsafe { pci_read(bus, slot, 0, 0) } & 0xFFFF;
    if vendor_id == 0xFFFF {
        return None;
    }

    let device_id = (unsafe { pci_read(bus, slot, 0, 0) } >> 16) & 0xFFFF;

    // Check for Virtio Vendor ID (0x1AF4)
    if vendor_id == 0x1AF4 {
        // Read BAR0
        let bar0 = unsafe { pci_read(bus, slot, 0, 0x10) };
        // Read Interrupt Line
        let irq_line = (unsafe { pci_read(bus, slot, 0, 0x3C) } & 0xFF) as u8;

        // If it's an IO BAR, the lowest bit is 1. We mask it out to get the address.
        // For Legacy virtio, BAR0 is typically the IO base.
        let base_addr = bar0 & !0x3;

        // Enable Bus Master (Bit 2) and IO Space (Bit 0)
        let command = unsafe { pci_read(bus, slot, 0, 0x04) };
        unsafe {
            outl(
                CONFIG_ADDRESS,
                (1u32 << 31) | ((bus as u32) << 16) | ((slot as u32) << 11) | (0x04),
            );
            outl(CONFIG_DATA, command | 0x4 | 0x1);
        }

        return Some(PciDevice {
            bus,
            slot,
            func: 0,
            vendor_id: vendor_id as u16,
            device_id: device_id as u16,
            base_addr,
            irq_line,
        });
    }

    None
}

pub fn scan_pci(device_id: u16) -> Option<PciDevice> {
    for bus in 0..256 {
        for slot in 0..32 {
            // Only checking function 0 for simplicity.
            // In a real OS we should check header type for multifunction.
            unsafe {
                if let Some(dev) = check_device(bus as u8, slot as u8) {
                    crate::info!(
                        "PCI: {:02x}:{:02x}.0 Vendor={:04x} Device={:04x} BAR0={:x} IRQ={}",
                        dev.bus,
                        dev.slot,
                        dev.vendor_id,
                        dev.device_id,
                        dev.base_addr,
                        dev.irq_line
                    );

                    // Look for Virtio Block Device
                    if dev.device_id == device_id {
                        return Some(dev);
                    }
                }
            }
        }
    }
    None
}
