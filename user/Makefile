
CC = gcc
CFLAGS = -std=c99 -O2 -fno-builtin -fno-stack-protector -nostdlib -m64 -mcmodel=small -fno-pic -fno-pie -no-pie
LDFLAGS = -e start -T user.ld

UPROGS=\
	init\
	sh\
	echo\

all: $(UPROGS)

init: init.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o init init.c

sh: sh.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o sh sh.c

echo: echo.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o echo echo.c

clean:
	rm -f $(UPROGS)
