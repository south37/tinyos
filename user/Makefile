
CC = gcc
CFLAGS = -std=c99 -O2 -fno-builtin -fno-stack-protector -nostdlib -m64 -mcmodel=small -fno-pic -fno-pie
LDFLAGS = -e start -T user.ld

UPROGS=\
	init\

all: $(UPROGS)

init: init.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o init init.c

clean:
	rm -f $(UPROGS)
