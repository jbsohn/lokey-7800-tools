# Atari 7800 Single-Sprite Transformation Demo Makefile
# Demonstrates 160A 2:1 PAR rotation and scaling via Maria 256-byte page striding
# Assembler / Linker: ca65 / ld65 (cc65 toolchain)

A78GFX       ?= ../../target/debug/a78gfx
A78TOOL      ?= ../../target/debug/a78tool
CA65         := ca65
LD65         := ld65
A78SIGN      ?= ../../target/debug/a78sign

EMU_TEST7800 ?= test7800
EMU_A7800    ?= mame64

.PHONY: all clean run run-a7800 FORCE

.DELETE_ON_ERROR:

all: transform_sprite.a78

# Ensure a78gfx binary is up to date
$(A78GFX): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78gfx

# Ensure a78sign binary is up to date
$(A78SIGN): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78sign

# Ensure a78tool binary is up to date
$(A78TOOL): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78tool

# Bake canonical PNG into 16-frame 256-byte page-strided bank (16 rotations)
ship_bank.inc: ship_bank.bin
ship_bank.bin: assets/ship.png assets/ship.gpl $(A78GFX)
	@echo "  Baking assets/ship.png into ship_bank.bin & ship_bank.inc with palette (16 rotations)..."
	@$(A78GFX) bake assets/ship.png -o ship_bank.bin --header ship_bank.inc --label ship_bank --palette assets/ship.gpl --subpalette 0 --angles 16 --par

# Assemble 6502 main.s
main.o: main.s ship_bank.inc ship_bank.bin include/maria.inc
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I include main.s -o $@

# Link into raw 32KB ROM
transform_sprite.bin: main.o 7800.cfg
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C 7800.cfg $< -o $@

# Sign ROM (standard 7800 encryption signature block). a78sign exits non-zero
# on failure; errors go to stderr.
transform_sprite.rom: transform_sprite.bin $(A78SIGN)
	@cp "$<" "$@"
	@$(A78SIGN) -w "$@" >/dev/null
	@$(A78SIGN) -t "$@"

# Package .a78 emulator ROM header using a78tool
transform_sprite.a78: transform_sprite.rom $(A78TOOL)
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "7800 Maria Rotating Sprite Demo" --no-ym2149

run: transform_sprite.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: transform_sprite.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi

clean:
	@rm -rf transform_sprite.bin transform_sprite.rom transform_sprite.a78 main.o ship_bank.bin ship_bank.inc atari_bank.bin atari_bank.inc ship_bank.s atari_bank.s
