# Atari 7800 Single Green Pixel Demo Makefile
# Assembler / Linker: ca65 / ld65 (cc65 toolchain)

A78TOOL      ?= ../../target/debug/a78tool
CA65         := ca65
LD65         := ld65
A78SIGN      ?= ../../target/debug/a78sign

EMU_TEST7800 ?= test7800
EMU_A7800    ?= mame64

.PHONY: all clean run run-a7800 FORCE

.DELETE_ON_ERROR:

all: pixel.a78

# Ensure a78sign binary is up to date
$(A78SIGN): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78sign

# Ensure a78tool binary is up to date
$(A78TOOL): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78tool

# Assemble 6502 main.s
main.o: main.s include/maria.inc
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I include main.s -o $@

# Link into raw 32KB ROM
pixel.bin: main.o 7800.cfg
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C 7800.cfg $< -o $@

# Sign ROM (standard 7800 encryption signature block). a78sign exits non-zero
# on failure; errors go to stderr.
pixel.rom: pixel.bin $(A78SIGN)
	@cp "$<" "$@"
	@$(A78SIGN) -w "$@" >/dev/null
	@$(A78SIGN) -t "$@"

# Package .a78 emulator ROM header using a78tool
pixel.a78: pixel.rom $(A78TOOL)
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "7800 Maria Pixel Demo" --no-ym2149

run: pixel.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: pixel.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi


clean:
	@rm -rf pixel.bin pixel.rom pixel.a78 main.o
