.altmacro
.macro HANDLER num
.global vector\num
vector\num:
    push $0     # error code
    push $\num  # trap number
    jmp alltraps
.endm

.macro HANDLER_ERR num
.global vector\num
vector\num:
    push $\num  # trap number
    jmp alltraps
.endm

.section .text
.global alltraps
alltraps:
    # Save registers
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
    mov %rsp, %rdi  # Pass TrapFrame
    call trap_handler
.global trapret
trapret:
    # Restore registers
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
    add $16, %rsp  # trap_num, error_code
    iretq

# Generate handler vectors
.set i, 0
.rept 256
    # Exceptions with error codes: 8, 10-14, 17
    .if i == 8 || (i >= 10 && i <= 14) || i == 17
        HANDLER_ERR %i
    .else
        HANDLER %i
    .endif
    .set i, i+1
.endr

.macro HANDLER_ADDR num
.quad vector\num
.endm

.section .data
.global vectors
vectors:
.set i, 0
.rept 256
    HANDLER_ADDR %i
    .set i, i+1
.endr