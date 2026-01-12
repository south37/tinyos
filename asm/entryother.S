.code16
.global start
start:
    cli

    # Zero data segments
    xorw    %ax, %ax
    movw    %ax, %ds
    movw    %ax, %es
    movw    %ax, %ss

    # Load GDT
    lgdt    gdtdesc

    # Switch to protected mode
    movl    %cr0, %eax
    orl     $1, %eax
    movl    %eax, %cr0

    # Jump to 32-bit code
    ljmp    $8, $start32

.code32
start32:
    # Setup data segments
    movw    $16, %ax
    movw    %ax, %ds
    movw    %ax, %es
    movw    %ax, %ss
    movw    %ax, %fs
    movw    %ax, %gs

    # Enable PAE (Physical Address Extension) - Required for Long Mode
    movl    %cr4, %eax
    orl     $0x20, %eax
    movl    %eax, %cr4

    # Load Page Table (CR3)
    # Argument passed at 0x7000 - 16 (0x6FF0)
    movl    (0x6FF0), %eax
    movl    %eax, %cr3

    # Enable Long Mode in EFER MSR
    movl    $0xC0000080, %ecx
    rdmsr
    orl     $0x100, %eax
    wrmsr

    # Enable Paging (CR0.PG)
    movl    %cr0, %eax
    orl     $0x80000000, %eax
    movl    %eax, %cr0

    # Load 64-bit GDT?
    # We can reuse the same GDT if it has 64-bit descriptors.
    # But checking gdtdesc, I need a 64-bit code segment.
    # Let's add one at offset 24 (0x18).

    ljmp    $24, $start64

.code64
start64:
    # We are in 64-bit mode now.
    # Load Real Stack Pointer
    # Argument passed at 0x7000 - 8 (0x6FF8)
    movq    (0x6FF8), %rsp

    # Jump to mpenter
    # Argument passed at 0x7000 - 24 (0x6FE8)
    movq    (0x6FE8), %rax
    call    *%rax

    # Should not return
spin:
    jmp     spin

.p2align 2
gdt:
    .word 0, 0, 0, 0 # Null

    # Code 32-bit (offset 8)
    .word 0xFFFF, 0x0000, 0x9A00, 0x00CF

    # Data 32-bit (offset 16)
    .word 0xFFFF, 0x0000, 0x9200, 0x00CF

    # Code 64-bit (offset 24)
    # Base=0, Limit=0, Access=0x9A (same as 32), Flags=0x20 (L=1, D=0)
    .word 0x0000, 0x0000, 0x9A00, 0x0020

gdtdesc:
    .word   (gdtdesc - gdt - 1)
    .long   gdt
