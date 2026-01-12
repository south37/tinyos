
#include <stddef.h>

typedef long long int64_t;

// Syscall numbers
#define SYS_EXEC 59
#define SYS_WRITE 1

int64_t syscall0(int64_t num) {
  int64_t ret;
  __asm__ volatile("syscall" : "=a"(ret) : "a"(num) : "rcx", "r11", "memory");
  return ret;
}

int64_t syscall1(int64_t num, int64_t a1) {
  int64_t ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "a"(num), "D"(a1)
                   : "rcx", "r11", "memory");
  return ret;
}

int64_t syscall2(int64_t num, int64_t a1, int64_t a2) {
  int64_t ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "a"(num), "D"(a1), "S"(a2)
                   : "rcx", "r11", "memory");
  return ret;
}

int64_t syscall3(int64_t num, int64_t a1, int64_t a2, int64_t a3) {
  int64_t ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "a"(num), "D"(a1), "S"(a2), "d"(a3)
                   : "rcx", "r11", "memory");
  return ret;
}

void exec(char *path, char **argv) {
  syscall2(SYS_EXEC, (int64_t)path, (int64_t)argv);
}

// Simple print via imaginary write syscall or just use whatever we have.
// Since we don't have write syscall implemented fully (only exec),
// we presumably can't verify output unless we implement SYS_WRITE.
// But calling an unknown syscall will print an error in kernel.

#define SYS_FORK 57
#define SYS_WAIT 61

int64_t fork() { return syscall0(SYS_FORK); }

int64_t wait(int64_t *status) { return syscall1(SYS_WAIT, (int64_t)status); }

void start() {
  char *msg = "init: starting\n";
  syscall3(SYS_WRITE, 1, (long)msg, 15);

  for (;;) {
    int64_t pid = fork();
    if (pid < 0) {
      char *err = "init: fork failed\n";
      syscall3(SYS_WRITE, 1, (long)err, 18);
      continue;
    }
    if (pid == 0) {
      char *sh_path = "sh";
      char *sh_argv[] = {"sh", 0};
      exec(sh_path, sh_argv);
      char *exec_err = "init: exec sh failed\n";
      syscall3(SYS_WRITE, 1, (long)exec_err, 21);
      // exit(1);
      for (;;)
        ;
    } else {
      for (;;) {
        // Wait for shell to exit
        int64_t wpid = wait(0);
        if (wpid == pid) {
          // Shell exited, restart it
          break;
        } else if (wpid < 0) {
          // Wait failed?
          // dummy loop
        }
      }
    }
  }
}
