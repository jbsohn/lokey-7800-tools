# Atari 7800 Scaling Logo Demo Makefile
# Demonstrates 160A pre-baked sprite scaling via Maria 256-byte page striding
# Assembler / Linker: ca65 / ld65 (cc65 toolchain)

A78GFX       ?= ../../target/debug/a78gfx
A78TOOL      ?= ../../target/debug/a78tool
CA65         := ca65
LD65         := ld65
A78SIGN      ?= ../../target/debug/a78sign

EMU_TEST7800 ?= test7800
EMU_A7800    ?= mame64

.PHONY: all clean run run-a7800 frames FORCE

.DELETE_ON_ERROR:

all: scale_sprite.a78

# Ensure a78gfx binary is up to date
$(A78GFX): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78gfx

# Ensure a78sign binary is up to date
$(A78SIGN): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78sign

# Ensure a78tool binary is up to date
$(A78TOOL): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78tool

# Bake the Fuji into a 256-byte page-strided bank, one frame per scanline of
# height. The master is 128x128, nearly square, pure red on transparent.
# --resize 48x48 --par (the default) produces a 24 data-pixel x 48-line canvas,
# square on a 160A CRT (pixels are 2:1). --scale-lines 8 writes heights
# 48,47,...,12. Below 12 lines the center bar no longer owns a pixel; the two
# sides stay apart. The slots are kept open even when they are narrower than a
# pixel. Lower ALPHA_THRESHOLD fattens the mark.
ALPHA_THRESHOLD ?= 160
BAKE_ARGS       := assets/AtariLogo.png --label logo_bank --palette assets/AtariLogo.pal \
                   --subpalette 0 --angles 0 --resize 48x48 --scale-lines 12 \
                   --scale-filter area --alpha-threshold $(ALPHA_THRESHOLD)

logo_bank.inc: logo_bank.bin
logo_bank.bin: assets/AtariLogo.png assets/AtariLogo.pal Makefile $(A78GFX)
	@echo "  Baking assets/AtariLogo.png into logo_bank.bin & logo_bank.inc (one frame per scanline)..."
	@$(A78GFX) bake $(BAKE_ARGS) -o logo_bank.bin --header logo_bank.inc

# Debug: write every scaled frame as a PNG into frames/ to inspect the bake.
frames: $(A78GFX)
	@$(A78GFX) bake $(BAKE_ARGS) -o frames/logo_bank.bin --header frames/logo_bank.inc --dump-frames frames

# Assemble 6502 main.s
main.o: main.s logo_bank.inc logo_bank.bin include/maria.inc
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I include main.s -o $@

# Link into raw 32KB ROM
scale_sprite.bin: main.o 7800.cfg
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C 7800.cfg $< -o $@

# Sign ROM (standard 7800 encryption signature block). a78sign exits non-zero
# on failure; errors go to stderr.
scale_sprite.rom: scale_sprite.bin $(A78SIGN)
	@cp "$<" "$@"
	@$(A78SIGN) -w "$@" >/dev/null
	@$(A78SIGN) -t "$@"

# Package .a78 emulator ROM header using a78tool
scale_sprite.a78: scale_sprite.rom $(A78TOOL)
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "7800 Maria Scaling Logo Demo" --no-ym2149

run: scale_sprite.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: scale_sprite.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi

clean:
	@rm -rf scale_sprite.bin scale_sprite.rom scale_sprite.a78 main.o logo_bank.bin logo_bank.inc ship_bank.bin ship_bank.inc frames
