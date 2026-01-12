#include "ulib.h"

void start() {
  puts("init: starting\n");

  for (;;) {
    int64_t pid = fork();
    if (pid < 0) {
      puts("init: fork failed\n");
      continue;
    }
    if (pid == 0) {
      char *argv[] = {"sh", 0};
      exec("sh", argv);
      puts("init: exec sh failed\n");
      exit(1);
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
