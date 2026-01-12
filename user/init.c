
#include <stddef.h>

typedef long long int64_t;

// Syscall numbers
#define SYS_EXEC 59
#define SYS_WRITE 1

void syscall0(int64_t num) {
  __asm__ volatile("syscall" : : "a"(num) : "rcx", "r11", "memory");
}

void syscall1(int64_t num, int64_t a1) {
  __asm__ volatile("syscall" : : "a"(num), "D"(a1) : "rcx", "r11", "memory");
}

void syscall2(int64_t num, int64_t a1, int64_t a2) {
  __asm__ volatile("syscall"
                   :
                   : "a"(num), "D"(a1), "S"(a2)
                   : "rcx", "r11", "memory");
}

void syscall3(int64_t num, int64_t a1, int64_t a2, int64_t a3) {
  __asm__ volatile("syscall"
                   :
                   : "a"(num), "D"(a1), "S"(a2), "d"(a3)
                   : "rcx", "r11", "memory");
}

void exec(char *path, char **argv) {
  syscall2(SYS_EXEC, (int64_t)path, (int64_t)argv);
}

// Simple print via imaginary write syscall or just use whatever we have.
// Since we don't have write syscall implemented fully (only exec),
// we presumably can't verify output unless we implement SYS_WRITE.
// But calling an unknown syscall will print an error in kernel.

void start() {
  char *msg = "init: starting\n";
  syscall3(SYS_WRITE, 1, (long)msg, 15);

  for (;;) {
    // Yield?
  }
}
