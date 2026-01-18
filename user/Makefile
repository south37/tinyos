BUILD_DIR = build
CARGO = cargo
TARGET = x86_64-unknown-none
PROFILE ?= release

ifneq ($(PROFILE),release)
	CARGO_FLAGS = --target $(TARGET)
	TARGET_DIR = ../target/$(TARGET)/debug
else
	CARGO_FLAGS = --release --target $(TARGET)
	TARGET_DIR = ../target/$(TARGET)/release
endif

# Override RUSTFLAGS to ignore root .cargo/config.toml headers and set user-specific flags
export RUSTFLAGS := -C link-arg=-T$(CURDIR)/user.ld -C link-arg=-estart -C code-model=small -C relocation-model=static

UPROGS=\
	$(BUILD_DIR)/init\
	$(BUILD_DIR)/sh\
	$(BUILD_DIR)/echo\

all: $(UPROGS)

$(BUILD_DIR)/init: init/src/main.rs | $(BUILD_DIR)
	$(CARGO) build -p init $(CARGO_FLAGS)
	cp $(TARGET_DIR)/init $@

$(BUILD_DIR)/sh: sh/src/main.rs | $(BUILD_DIR)
	$(CARGO) build -p sh $(CARGO_FLAGS)
	cp $(TARGET_DIR)/sh $@

$(BUILD_DIR)/echo: echo/src/main.rs | $(BUILD_DIR)
	$(CARGO) build -p echo $(CARGO_FLAGS)
	cp $(TARGET_DIR)/echo $@

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

clean:
	$(CARGO) clean
	rm -rf $(BUILD_DIR)
