.global start
start:
  mov $64, %rax
  syscall
loop:
  jmp loop
