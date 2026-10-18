# Atari 7800 Silly Bugs demo
# Port of the 30-Nov-1986 Atari ST dev-kit sample (bugs-st/)
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

all: bugs.a78

# Ensure a78gfx binary is up to date
$(A78GFX): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78gfx

# Ensure a78sign binary is up to date
$(A78SIGN): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78sign

# Ensure a78tool binary is up to date
$(A78TOOL): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78tool

# 160A, PAR-corrected so the square-pixel ST art stays round on 2:1 pixels.
# Beetle and circle share subpalette 0. The tan square uses subpalette 1.
# Each conversion writes the .bin and the .inc together (grouped target).
beetle.bin beetle.inc &: assets/beetle.png assets/bugs.gpl $(A78GFX)
	@echo "  Converting beetle (160A, subpalette 0)..."
	@$(A78GFX) convert assets/beetle.png -o beetle.bin --header beetle.inc --label beetle --palette assets/bugs.gpl --subpalette 0 --par

circle.bin circle.inc &: assets/circle.png assets/bugs.gpl $(A78GFX)
	@echo "  Converting circle (160A, subpalette 0)..."
	@$(A78GFX) convert assets/circle.png -o circle.bin --header circle.inc --label circle --palette assets/bugs.gpl --subpalette 0 --par

square.bin square.inc &: assets/square.png assets/bugs.gpl $(A78GFX)
	@echo "  Converting square (160A, subpalette 1)..."
	@$(A78GFX) convert assets/square.png -o square.bin --header square.inc --label square --palette assets/bugs.gpl --subpalette 1 --par

# Assemble 6502 main.s
main.o: main.s beetle.inc circle.inc square.inc beetle.bin circle.bin square.bin include/maria.inc
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I include main.s -o $@

# Link into raw 32KB ROM
bugs.bin: main.o 7800.cfg
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C 7800.cfg $< -o $@ -m map.txt

# Sign ROM (standard 7800 encryption signature block). a78sign exits non-zero
# on failure; errors go to stderr.
bugs.rom: bugs.bin $(A78SIGN)
	@cp "$<" "$@"
	@$(A78SIGN) -w "$@" >/dev/null
	@$(A78SIGN) -t "$@"

# Package .a78 emulator ROM header using a78tool
bugs.a78: bugs.rom $(A78TOOL)
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "7800 Silly Bugs" --no-ym2149

run: bugs.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: bugs.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi

clean:
	@rm -rf bugs.bin bugs.rom bugs.a78 main.o map.txt beetle.bin beetle.inc circle.bin circle.inc square.bin square.inc
