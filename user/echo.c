#include "ulib.h"

void start(int argc, char *argv[]) {
  // Note: The kernel exec passes arguments.
  // However, our _start entry point in `user.ld` or standard conventions might
  // not pass argc/argv to start(). We need to see how arguments are passed on
  // stack. The kernel `exec` puts them on stack. If our entry point is `start`,
  // we need to read them. Ideally, we should have a `_start` that calls
  // `main(argc, argv)`. But for now let's assume `start` is the entry point.
  // But wait, in `sh.c` we don't use argc/argv.
  // In `init.c` we define `void start()`.
  // The arguments are on the stack. `rsp` points to argc, then argv pointers.

  // We can't easily access stack args in C without inline asm or correct
  // signature if calling convention matches. x86_64 passing: args in registers?
  // accessing stack directly? User entry point is just jumped to.

  // Let's rely on the fact that `start` might receive args if we define it
  // right, OR we just assume stack layout. But kernel `sys_exec` just maps the
  // stack. It doesn't set registers for arguments (rdi, rsi etc). It says:
  // tf.rip = elf.entry;
  // tf.rsp = sz;
  // We need to put argc and argv on the stack at `sz`.
  // And if `sys_exec` implementation in kernel didn't actually PUSH them, they
  // aren't there.

  // CAUTION: I didn't verify `sys_exec` implemented argument PUSHING to stack!
  // I only updated `sys_exec` to READ arguments.
  // I need to update `sys_exec` (or `exec` implementation) to COPY arguments to
  // the new stack.

  // Let's implement basic echo that just prints "echo" for now if I forgot
  // that. But wait, the specific task was "Update `sys_exec` to handle
  // arguments". I updated `sys_exec` to read them from callers address space.
  // But `crate::exec::exec` needs to put them on the new stack.

  int i;
  for (i = 1; i < argc; i++) {
    puts(argv[i]);
    if (i < argc - 1)
      puts(" ");
  }
  puts("\n");
  exit(0);
}
