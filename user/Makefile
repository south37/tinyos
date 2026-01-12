
CC = gcc
CFLAGS = -std=c99 -O2 -fno-builtin -fno-stack-protector -nostdlib -m64 -mcmodel=small -fno-pic -fno-pie -no-pie
LDFLAGS = -e start -T user.ld

BUILD_DIR = build

UPROGS=\
	$(BUILD_DIR)/init\
	$(BUILD_DIR)/sh\
	$(BUILD_DIR)/echo\

all: $(UPROGS)

$(BUILD_DIR)/init: init.c | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

$(BUILD_DIR)/sh: sh.c | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

$(BUILD_DIR)/echo: echo.c | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

clean:
	rm -rf $(BUILD_DIR)
