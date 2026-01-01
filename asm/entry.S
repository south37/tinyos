# Entry point of kernel. Start as a 32-bit protected mode, and 
# switches to 64-bit long mode, then jump to kmain.

.section .multiboot
.global mboot_header
mboot_header:
    .long mboot_magic
    .long mboot_flags
    .long mboot_checksum
    .long mboot_load_addr   # header_addr
    .long mboot_load_addr
    .long mboot_load_end
    .long mboot_bss_end
    .long mboot_entry_addr

# Null segment descriptor
#define SEG_NULLASM                                                            \
  .word 0, 0;                                                                  \
  .byte 0, 0, 0, 0

# Segment descriptor for long mode (64-bit).
# The 0x90 means the presence of P and S. If l=1, the descriptor
# defines a 64-bit code segment.
#define SEG64_ASM(type, l)                                                     \
  .word 0, 0;                                                                  \
  .byte 0, (0x90 | (type)), (l << 5), 0

#define SEG_KCODE 1   // kernel code
#define SEG_KDATA 2   // kernel data+stack

#define STA_X 0x8     // Executable segment

# EFER(Extended Feature Enable Register)
#define EFER_MSR 0xC0000080

# First kernel virtual address
#define KERNBASE 0xFFFFFFFF80000000

# Address of the initial page table. Linear map [0-1GiB).
#define PAGETABLE 0x1000
# Page table/directory entry flags.
#define PTE_P  0x001   // Present
#define PTE_W  0x002   // Writeable
#define PTE_PS 0x080   // Page Size

# Entry point of kernel
.code32
.section .text.entry
.global entry
entry:
    # zero 4 pages for our bootstrap page tables
    xor    %eax, %eax
    mov    $PAGETABLE, %edi
    mov    $0x5000, %ecx
    rep    stosb

    # Map the first 1GiB to low and high memory.

    # PML4[0] -> 0x2000 (PDPT-A)
    mov    $(0x2000 | PTE_P | PTE_W), %eax
    mov    %eax, PAGETABLE

    # PML4[511] -> 0x3000 (PDPT-B)
    mov    $(0x3000 | PTE_P | PTE_W), %eax
    mov    %eax, 0x1FF8

    # PDPT-A[0] -> 0x4000 (PD)
    mov    $(0x4000 | PTE_P | PTE_W), %eax
    mov    %eax, 0x2000

    # PDPT-B[510] -> 0x4000 (PD)
    mov    $(0x4000 | PTE_P | PTE_W), %eax
    mov    %eax, 0x3FF0

    # PD[0..511] -> 0..1GiB
    mov    $(PTE_P | PTE_W | PTE_PS), %eax
    mov    $0x4000, %ebx
    mov    $512, %ecx
    # Loop to map each 2MiB entry
ptbl_loop:
    mov    %eax, (%ebx)
    add    $0x200000, %eax
    add    $0x8, %ebx
    dec    %ecx
    jnz    ptbl_loop

    # CR3 -> 0x1000 (P4ML)
    mov    $PAGETABLE, %eax
    mov    %eax, %cr3

    # Load GDT
    lgdt   (gdt64desc - KERNBASE)

    # Enable PAE (Physical Address Extension) - CR4.PAE=1
    mov    %cr4, %eax
    bts    $5, %eax
    mov    %eax, %cr4

    # LME (Long Mode Enable) - EFER.LME=1
    mov    $EFER_MSR, %ecx
    rdmsr
    bts    $8, %eax
    wrmsr

    # Enable paging
    mov    %cr0, %eax
    bts    $31, %eax
    mov    %eax, %cr0

    # shift to 64bit segment
    ljmp   $(SEG_KCODE<<3), $(entry64low - KERNBASE)

# GDT for 64-bit code. Only code seg (no data seg).
.p2align 4               # force 16 byte alignment
gdt64:
    SEG_NULLASM          # 0: null seg
    SEG64_ASM(STA_X, 1)  # SEG_KCODE=1: code seg. R/X, Nonconforming

gdt64desc:
    .word  (gdt64desc - gdt64 - 1)  # sizeof(gdt64) - 1
    .quad  gdt64 - KERNBASE

# Enter 64-bit code
.p2align 4
.code64
entry64low:
    movq   $entry64high, %rax
    jmp    *%rax

entry64high:
    # Ensure data segment registers are sane
    xor    %rax, %rax
    mov    %ax, %ds
    mov    %ax, %es
    mov    %ax, %ss
    mov    %ax, %fs
    mov    %ax, %gs

    # Setup initial stack (64kiB)
    mov    $(KERNBASE + 0x10000), %rax
    mov    %rax, %rsp

    # Enter kmain()
    jmp    kmain

__deadloop:
    # we should never return here...
    jmp    __deadloop
