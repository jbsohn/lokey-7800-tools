# Silly Bugs, rendered through lokeydll.
# Assembler / Linker: ca65 / ld65 (cc65 toolchain)

A78GFX       ?= ../../target/debug/a78gfx
A78TOOL      ?= ../../target/debug/a78tool
CA65         := ca65
LD65         := ld65
AR65         := ar65
A78SIGN      ?= ../../target/debug/a78sign

EMU_TEST7800 ?= test7800
EMU_A7800    ?= mame64

LOKEYDLL_S   := ../../lib/lokeydll/src/lokeydll.s
LOKEYDLL_INC := ../../lib/lokeydll/include
LOKEYDLL_CFG := ../../lib/lokeydll/7800.cfg

.PHONY: all clean run run-a7800 FORCE

.DELETE_ON_ERROR:

all: bugs-lokey.a78

$(A78GFX): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78gfx

$(A78SIGN): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78sign

$(A78TOOL): FORCE
	@cargo build --manifest-path ../../Cargo.toml --bin a78tool

beetle.bin beetle.inc &: assets/beetle.png assets/bugs.gpl $(A78GFX)
	@echo "  Converting beetle (160A, subpalette 0)..."
	@$(A78GFX) convert assets/beetle.png -o beetle.bin --header beetle.inc --label beetle --palette assets/bugs.gpl --subpalette 0 --par

circle.bin circle.inc &: assets/circle.png assets/bugs.gpl $(A78GFX)
	@echo "  Converting circle (160A, subpalette 0)..."
	@$(A78GFX) convert assets/circle.png -o circle.bin --header circle.inc --label circle --palette assets/bugs.gpl --subpalette 0 --par

square.bin square.inc &: assets/square.png assets/bugs.gpl $(A78GFX)
	@echo "  Converting square (160A, subpalette 1)..."
	@$(A78GFX) convert assets/square.png -o square.bin --header square.inc --label square --palette assets/bugs.gpl --subpalette 1 --par

lokeydll.o: $(LOKEYDLL_S) $(LOKEYDLL_INC)/lokeydll.inc
	@echo "  Assembling lokeydll.s..."
	@$(CA65) -I $(LOKEYDLL_INC) $(LOKEYDLL_S) -o $@

lokeydll.lib: lokeydll.o
	@echo "  Archiving lokeydll.lib..."
	@$(AR65) r $@ $<

main.o: main.s beetle.inc circle.inc square.inc beetle.bin circle.bin square.bin $(LOKEYDLL_INC)/lokeydll.inc
	@echo "  Assembling main.s..."
	@$(CA65) -I . -I $(LOKEYDLL_INC) main.s -o $@

bugs-lokey.bin: main.o lokeydll.lib $(LOKEYDLL_CFG)
	@echo "  Linking 7800 ROM..."
	@$(LD65) -C $(LOKEYDLL_CFG) main.o lokeydll.lib -o $@ -m map.txt

bugs-lokey.rom: bugs-lokey.bin $(A78SIGN)
	@cp "$<" "$@"
	@$(A78SIGN) -w "$@" >/dev/null
	@$(A78SIGN) -t "$@"

bugs-lokey.a78: bugs-lokey.rom $(A78TOOL)
	@echo "  Packaging A78 header with a78tool..."
	@$(A78TOOL) --input "$<" --output "$@" --title "Lokey Bugs" --no-ym2149

run: bugs-lokey.a78
	@if command -v $(EMU_TEST7800) >/dev/null 2>&1; then \
		$(EMU_TEST7800) -run "$<"; \
	else \
		echo "test7800 emulator not found (set EMU_TEST7800 or add to PATH)"; \
	fi

run-a7800: bugs-lokey.a78
	@if command -v $(EMU_A7800) >/dev/null 2>&1; then \
		$(EMU_A7800) a7800 -cart "$<"; \
	else \
		echo "a7800 / MAME emulator not found (set EMU_A7800 or add to PATH)"; \
	fi

clean:
	@rm -rf bugs-lokey.bin bugs-lokey.rom bugs-lokey.a78 lokeydll.o lokeydll.lib main.o map.txt beetle.bin beetle.inc circle.bin circle.inc square.bin square.inc
