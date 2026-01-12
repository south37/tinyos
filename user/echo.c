#include "ulib.h"

void start(int argc, char *argv[]) {
  int i;
  for (i = 1; i < argc; i++) {
    puts(argv[i]);
    if (i < argc - 1)
      puts(" ");
  }
  puts("\n");
  exit(0);
}
