#include "ulib.h"

#define MAXARGS 10

void start() {
  char buf[100];

  while (1) {
    puts("$ ");
    memset(buf, 0, sizeof(buf));

    // Read line
    int i = 0;
    while (i < sizeof(buf) - 1) {
      char c = 0;
      if (read(0, &c, 1) < 1)
        break;
      if (c == '\n' || c == '\r')
        break;
      buf[i++] = c;
    }
    buf[i] = 0;

    if (i == 0)
      continue;

    // Parse args
    char *argv[MAXARGS];
    int argc = 0;
    char *p = buf;
    while (*p && argc < MAXARGS - 1) {
      while (*p == ' ')
        p++;
      if (*p == 0)
        break;
      argv[argc++] = p;
      while (*p && *p != ' ')
        p++;
      if (*p)
        *p++ = 0;
    }
    argv[argc] = 0;

    if (argc == 0)
      continue;

    if (strcmp(argv[0], "exit") == 0) {
      exit(0);
    }

    int64_t pid = fork();
    if (pid < 0) {
      puts("fork failed\n");
    } else if (pid == 0) {
      exec(argv[0], argv);
      puts("exec failed\n");
      exit(1);
    } else {
      wait(0);
    }
  }
}
