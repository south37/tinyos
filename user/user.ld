OUTPUT_FORMAT("elf64-x86-64", "elf64-x86-64", "elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)
ENTRY(start)

SECTIONS
{
	. = 0x1000;
	.text : { *(.text) }
	.data : { *(.data) }
	.bss : { *(.bss) }
}
