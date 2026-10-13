# Atari 7800 Static Sprite Demo Makefile
# Demonstrates displaying a single 16x16 160A sprite centered on screen
# Assembler / Linker: ca65 / ld65 (cc65 toolchain)

BUILD_DIR    := build
A78GFX       ?= ../../target/debug/a78gfx
A78TOOL      ?= a78tool
CA65         := ca65
LD65         := ld65
SIGN         := 7800sign

EMU_TEST7800 ?= test7800
EMU_A7800    ?= mame64

.PHONY: all clean run run-a7800 FORCE

all: $(BUILD_DIR)/static_sprite.a78

$(BUILD_DIR):
	@mkdir -p $(BUILD_DIR)

# Ensure a78gfx binary is up to date
$(A78GFX): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78gfx

# Convert canonical PNG into 256-byte page-strided binary and ca65 include
ship.inc: ship.bin
ship.bin: assets/ship.png assets/ship.gpl $(A78GFX)
	@echo "  Converting assets/ship.png -> ship.bin & ship.inc with palette..."
	@$(A78GFX) convert assets/ship.png -o ship.bin --header ship.inc --label ship --palette assets/ship.gpl --subpalette 0

# Assemble 6502 main.s
$(BUILD_DIR)/main.o: main.s ship.inc ship.bin include/maria.inc | $(BUILD_DIR)
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I include main.s -o $@

# Link into raw 32KB ROM
$(BUILD_DIR)/static_sprite.bin: $(BUILD_DIR)/main.o 7800.cfg | $(BUILD_DIR)
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C 7800.cfg $< -o $@

# Sign ROM (standard 7800 encryption signature block)
$(BUILD_DIR)/static_sprite.rom: $(BUILD_DIR)/static_sprite.bin
	@cp "$<" "$@"
	@$(SIGN) -w "$@" >/dev/null 2>&1 && $(SIGN) -t "$@" 2>/dev/null || true

# Package .a78 emulator ROM header using a78tool
$(BUILD_DIR)/static_sprite.a78: $(BUILD_DIR)/static_sprite.rom
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "7800 Maria Static Sprite Demo" --no-ym2149

run: $(BUILD_DIR)/static_sprite.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: $(BUILD_DIR)/static_sprite.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi

clean:
	@rm -rf $(BUILD_DIR) ship.bin ship.inc atari.bin atari.inc ship.s atari.s
