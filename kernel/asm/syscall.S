# Syscall entry point

.section .text
.extern trap_handler

# Constants (Must match gdt.rs)
.set KCODE_SELECTOR, 0x08  # 1 << 3
.set KDATA_SELECTOR, 0x10  # 2 << 3
.set UCODE_SELECTOR, 0x1B  # (3 << 3) | 3
.set UDATA_SELECTOR, 0x23  # (4 << 3) | 3
.set TSS_RSP0_OFFSET, 4       # RSP0
.set TSS_RSP_SAVE_OFFSET, 28  # reserved2

# fn syscall_entry();
.global syscall_entry
syscall_entry:
    # 1. Switch to kernel GS (for TSS access)
    swapgs

    # 2. Save User RSP to TSS scratch space (reserved2)
    movq %rsp, %gs:TSS_RSP_SAVE_OFFSET

    # 3. Load Kernel RSP from TSS (RSP0)
    movq %gs:TSS_RSP0_OFFSET, %rsp

    # 4. Construct TrapFrame for iretq
    # Stack layout for iretq: SS, RSP, RFLAGS, CS, RIP
    pushq $UDATA_SELECTOR         # SS
    pushq %gs:TSS_RSP_SAVE_OFFSET # RSP (User Stack)
    pushq %r11                    # RFLAGS (Saved by syscall in R11)
    pushq $UCODE_SELECTOR         # CS
    pushq %rcx                    # RIP (Saved by syscall in RCX)

    # 5. Push other TrapFrame fields
    # Layout:
    # Error Code = 0 (dummy)
    # Trap Num (T_SYSCALL = 64)
    # Registers...

    pushq $0   # Error Code
    pushq $64  # Trap Num (T_SYSCALL)

    # Save General Purpose Registers
    push %r15
    push %r14
    push %r13
    push %r12
    push %r11
    push %r10
    push %r9
    push %r8
    push %rdi
    push %rsi
    push %rbp
    push %rdx
    push %rcx
    push %rbx
    push %rax

    # 6. Call trap handler
    mov %rsp, %rdi  # Argument: &TrapFrame
    call trap_handler

    # 7. Restore Registers
    pop %rax
    pop %rbx
    pop %rcx
    pop %rdx
    pop %rbp
    pop %rsi
    pop %rdi
    pop %r8
    pop %r9
    pop %r10
    pop %r11
    pop %r12
    pop %r13
    pop %r14
    pop %r15

    add $16, %rsp             # Skip trap_num, error_code

    # 8. Return to user
    # When we entered, we did swapgs. So GS.base is now KernelGSBase (pointing to TSS).
    # We need to restore user GS base before returning.
    swapgs

    # iretq pops RIP, CS, RFLAGS, RSP, SS
    iretq
