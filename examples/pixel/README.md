# Atari 7800 Maria Minimal Example: Single Green Pixel

This example is a minimal Atari 7800 Maria display program written for **ca65** / **ld65** (cc65 toolchain), packaged with `a78tool`. It renders **one green dot in the center of the screen**.

## How Maria Works in This Example

The Atari 7800 graphics chip (MARIA) does not use a frame buffer. Instead, it reads a 2-level display list hierarchy via DMA on each scanline:

## Maria Data Structures (.struct)

The structs and emission macros are defined in [include/maria.inc](file:///home/john/Projects/lokey-7800-tools/examples/pixel/include/maria.inc):

### 1. Display List List Entry (`TMariaDLLEntry`)
Each 3-byte entry controls one vertical zone:
```ca65
.struct TMariaDLLEntry
    offset      .byte   ; bits 3..0 = zone height - 1 (0..15 => 1..16 lines), plus DLI/Holey bits
    dph         .byte   ; High byte of Display List address
    dpl         .byte   ; Low byte of Display List address
.endstruct
```

In NTSC, Maria active DMA covers 242 scanlines (lines 16 to 257). We partition the screen into:
- 7 zones × 16 lines (112 blank lines) : `dl_empty`
- 1 zone  ×  8 lines (  8 blank lines) : `dl_empty`
- **1 zone  ×  1 line  (  1 line, offset = 0) : `dl_dot` (Scanline 120 = Screen Center!)**
- 7 zones × 16 lines (112 blank lines) : `dl_empty`
- 1 zone  ×  9 lines (  9 blank lines) : `dl_empty`
- Total: 112 + 8 + 1 + 112 + 9 = 242 lines.

Because the center zone is exactly 1 scanline high (`offset = 0`), Maria's graphics fetch calculation (`addr + offset`) always fetches from the exact byte address without needing multi-page sprite tables!

### 2. Display List Headers (`TMariaDLHeaderDirect`, `TMariaDLTerminator`)
```ca65
.struct TMariaDLHeaderDirect
    addr_lo     .byte   ; Low byte of graphic data
    pal_w       .byte   ; Palette (bits 7..5) and two's complement width (bits 4..0)
    addr_hi     .byte   ; High byte of graphic data
    hpos        .byte   ; Horizontal pixel position (0..159)
.endstruct

.struct TMariaDLTerminator
    term0       .byte   ; $00
    term1       .byte   ; $00
.endstruct
```

In 160A mode (160×240, 2 bits per pixel), 1 byte contains 4 pixels:
- Bits 7–6: Pixel 0 (color 0 = background black)
- Bits 5–4: Pixel 1 (color 2 = bright green `$C8`)
- Bits 3–2: Pixel 2 (color 0)
- Bits 1–0: Pixel 3 (color 0)

With `HPOS = 79`, Pixel 1 lands at `X = 80`, which is the exact center of the 160-pixel line (`0..159`).

3. **Palette & Control**:
   - Palette 0, Color 2 (`P0C2`, `$22`) is set to `$C8` (bright green).
   - Background (`BKGRND`, `$20`) is set to `$00` (black).
   - MARIA DMA is enabled by writing `$40` to `CTRL` (`$3C`) during VBLANK (DMA enabled, 160A mode).

## Building

Make sure `ca65`, `ld65`, `7800sign`, and `a78tool` are in your PATH (or built in this repository).

```bash
make
```

Outputs generated in `build/`:
- `build/pixel.bin`: 32 KB raw ROM
- `build/pixel.rom`: Signed 32 KB ROM
- `build/pixel.a78`: 128-byte `.a78` header + signed ROM

## Running in Emulators

### Using `test7800`
```bash
make run
# Or override emulator executable path:
make run EMU_TEST7800=/path/to/test7800
```

### Using `A7800` (MAME)
```bash
make run-a7800
# Or override MAME executable:
make run-a7800 EMU_A7800=/path/to/mame64
```
