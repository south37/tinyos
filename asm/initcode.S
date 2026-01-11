.global start
start:
  int $0x40
loop:
  jmp loop
