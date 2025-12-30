ENTRY(mboot_entry)

mboot_load_addr = 0x00100000;

SECTIONS
{
	/* Link the kernel at this address: "." means the current address */
	/* Must be equal to KERNLINK */
	. = 0xFFFFFFFF80100000; 

	PROVIDE(begin = .);

    .multiboot : AT(mboot_load_addr)
    {
        KEEP(*(.text.entry_header))
    }

    . = ALIGN(0x1000);

    .text :
    {
        KEEP(*(.text.entry))
        *(.text*)
    }

    . = ALIGN(0x1000);
    .rodata :
    {
        *(.rodata*)
    }

    . = ALIGN(0x1000);

    .data :
    {
        *(.data*)
    }

    . = ALIGN(0x1000);
    PROVIDE(edata = .);

    .bss :
    {
        *(.bss*)
    }

    . = ALIGN(0x1000);
    PROVIDE(end = .);
}

/* Multiboot header information */
/* Magic number */
mboot_magic = 0x1BADB002;
/* Flags: bit 16 = address fields present */
mboot_flags = 0x00010000;
mboot_checksum = 0xE4514FFE; /* -mboot_magic -mboot_flags */
mboot_load_end = mboot_load_addr + (edata - begin);
mboot_bss_end = mboot_load_addr + (end - begin);
mboot_entry_addr = mboot_load_addr + (mboot_entry - begin);