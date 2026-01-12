# Entry point of init process. Used by proc::init_process.
.global start
start:
    # exec("/init", 0)
    lea init(%rip), %rdi
    xor %rsi, %rsi
    mov $59, %rax # SYS_EXEC
    syscall

__deadloop:
    # we should never return here...
    jmp    __deadloop

init:
    .asciz "/init"
