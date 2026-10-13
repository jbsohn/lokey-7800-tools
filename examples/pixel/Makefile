# Atari 7800 Single Green Pixel Demo Makefile
# Assembler / Linker: ca65 / ld65 (cc65 toolchain)

BUILD_DIR    := build
A78TOOL      ?= a78tool
CA65         := ca65
LD65         := ld65
SIGN         := 7800sign

EMU_TEST7800 ?= test7800
EMU_A7800    ?= mame64

.PHONY: all clean run run-a7800

all: $(BUILD_DIR)/pixel.a78

$(BUILD_DIR):
	@mkdir -p $(BUILD_DIR)

# Assemble 6502 main.s
$(BUILD_DIR)/main.o: main.s include/maria.inc | $(BUILD_DIR)
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I include main.s -o $@

# Link into raw 32KB ROM
$(BUILD_DIR)/pixel.bin: $(BUILD_DIR)/main.o 7800.cfg | $(BUILD_DIR)
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C 7800.cfg $< -o $@

# Sign ROM (standard 7800 encryption signature block)
$(BUILD_DIR)/pixel.rom: $(BUILD_DIR)/pixel.bin
	@cp "$<" "$@"
	@$(SIGN) -w "$@" >/dev/null 2>&1 && $(SIGN) -t "$@" 2>/dev/null || true

# Package .a78 emulator ROM header using a78tool
$(BUILD_DIR)/pixel.a78: $(BUILD_DIR)/pixel.rom
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "7800 Maria Pixel Demo" --no-ym2149

run: $(BUILD_DIR)/pixel.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: $(BUILD_DIR)/pixel.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi


clean:
	@rm -rf $(BUILD_DIR)
