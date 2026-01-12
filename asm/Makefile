BUILD_DIR = build

all: $(BUILD_DIR)/entry.o $(BUILD_DIR)/vectors.o $(BUILD_DIR)/syscall.o $(BUILD_DIR)/initcode $(BUILD_DIR)/entryother

$(BUILD_DIR)/entry.o: entry.S | $(BUILD_DIR)
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c entry.S -o $@

$(BUILD_DIR)/syscall.o: syscall.S | $(BUILD_DIR)
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c syscall.S -o $@

$(BUILD_DIR)/vectors.o: vectors.S | $(BUILD_DIR)
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c vectors.S -o $@

$(BUILD_DIR)/initcode: initcode.S | $(BUILD_DIR)
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c initcode.S -o $(BUILD_DIR)/initcode.o
	ld -N -e _user_start -Ttext 0 -o $(BUILD_DIR)/initcode.out $(BUILD_DIR)/initcode.o
	objcopy -S -O binary $(BUILD_DIR)/initcode.out $@

$(BUILD_DIR)/entryother: entryother.S | $(BUILD_DIR)
	cc -fno-pic -nostdinc -I. -c entryother.S -o $(BUILD_DIR)/entryother.o
	ld -N -e start -Ttext 0x7000 -o $(BUILD_DIR)/entryother.out $(BUILD_DIR)/entryother.o
	objcopy -S -O binary $(BUILD_DIR)/entryother.out $@

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

clean: 
	rm -rf $(BUILD_DIR)