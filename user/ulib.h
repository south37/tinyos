#ifndef ULIB_H
#define ULIB_H

typedef long long int64_t;

#define SYS_READ 0
#define SYS_WRITE 1
#define SYS_FORK 57
#define SYS_EXEC 59
#define SYS_EXIT 60
#define SYS_WAIT 61

static inline int64_t syscall0(int64_t num) {
  int64_t ret;
  __asm__ volatile("syscall" : "=a"(ret) : "a"(num) : "rcx", "r11", "memory");
  return ret;
}

static inline int64_t syscall1(int64_t num, int64_t a1) {
  int64_t ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "a"(num), "D"(a1)
                   : "rcx", "r11", "memory");
  return ret;
}

static inline int64_t syscall2(int64_t num, int64_t a1, int64_t a2) {
  int64_t ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "a"(num), "D"(a1), "S"(a2)
                   : "rcx", "r11", "memory");
  return ret;
}

static inline int64_t syscall3(int64_t num, int64_t a1, int64_t a2,
                               int64_t a3) {
  int64_t ret;
  __asm__ volatile("syscall"
                   : "=a"(ret)
                   : "a"(num), "D"(a1), "S"(a2), "d"(a3)
                   : "rcx", "r11", "memory");
  return ret;
}

static inline int64_t fork() { return syscall0(SYS_FORK); }

static inline int64_t exit(int64_t status) {
  return syscall1(SYS_EXIT, status);
}

static inline int64_t wait(int64_t *status) {
  return syscall1(SYS_WAIT, (int64_t)status);
}

static inline int64_t exec(char *path, char **argv) {
  return syscall2(SYS_EXEC, (int64_t)path, (int64_t)argv);
}

static inline int64_t read(int fd, void *buf, int64_t n) {
  return syscall3(SYS_READ, fd, (int64_t)buf, n);
}

static inline int64_t write(int fd, const void *buf, int64_t n) {
  return syscall3(SYS_WRITE, fd, (int64_t)buf, n);
}

static inline int strlen(const char *s) {
  int n = 0;
  while (s[n])
    n++;
  return n;
}

static inline void puts(const char *s) { write(1, s, strlen(s)); }

static inline void memset(void *dst, int c, int64_t n) {
  char *d = (char *)dst;
  for (int64_t i = 0; i < n; i++)
    d[i] = c;
}

static inline int strcmp(const char *p, const char *q) {
  while (*p && *p == *q)
    p++, q++;
  return (unsigned char)*p - (unsigned char)*q;
}

#endif
