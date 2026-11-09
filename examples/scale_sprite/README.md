# Atari 7800 Maria Scaling Sprite Demo

A demonstration of **pre-baked 2D sprite scaling** and **multi-zone rendering** on the MARIA graphics coprocessor, using **256-byte page-strided scanline DMA** and RAM display lists updated during VBLANK.

---

## 1. What This Demo Showcases

1. **Pre-Baked Scaling Pipeline (`a78gfx bake`)**:
   - Compiles `assets/AtariLogo.png` (128×128, the Fuji) onto a 24×48 canvas (`--resize 48x48` plus the default `--par`, which halves the width so the square art stays square on 160A's 2:1 pixels).
   - **One frame per scanline** (`--scale-lines 12`): heights 48, 47, … 12. The outline moves by one pixel per step. Below 12 lines the center bar no longer owns a pixel; the two side bars still have a slot between them.
   - **Area filter** (`--scale-filter area`): each frame is a single box-filter of the master PNG. Ink is counted per connected bar, and if two bars would sit on neighbouring pixels the bake shifts them apart so the slot stays open. `--alpha-threshold` defaults to 160.

2. **Multi-Zone Rendering (3 Display Lists)**:
   - A 48-line sprite is taller than MARIA's 16-line zone, so it is **three headers** (zones 6-8, lines 96-143).
   - The headers share `addr_lo`, `pal_w`, and `hpos`. Only `addr_hi` differs, stepping down 16 pages per zone (the 48 scanlines, stored bottom-up).
   - Every scale frame uses the same canvas, so the mark shrinks toward the middle and `hpos` stays put.

3. **MARIA 256-Byte Page Striding**:
   - Each scanline lives on its own 256-byte page, so the 48-line mark occupies 48 pages (12 KB). A 24-pixel 160A row is 6 bytes, and 37 frames fit in the 256-byte page (37 × 6 = 222).
   - Frame $i$ is selected by setting `addr_lo = logo_bank_frame_lo[i]`.

4. **RAM Display Lists**:
   - Three 6-byte display lists in RAM (header + terminator each).
   - At VBLANK the 6502 writes `addr_lo` and `pal_w` in all three before MARIA draws the next frame.

5. **Scale-Up / Scale-Down Animation (Ping-Pong)**:
   - `main.s` steps the frame index to the smallest size, then back to full size, holding each size for `ANIM_DELAY` frames.

---

## 2. Memory Layout & Budgets

| Memory Region | Address Range | Purpose |
| :--- | :--- | :--- |
| **Zero Page** | `$0040 - $0042` | Animation timer, frame index, scale direction |
| **RAM Buffers** | `$1800 - $1811` | Three 6-byte Display Lists (3 headers + terminators) |
| **ROM Code** | `$8000 - $80xx` | 6502 initialization, VBLANK synchronization, animation loop |
| **ROM RODATA** | `$82xx - $B2xx` | Display List List (`dll_list`), `dl_empty`, 48-page sprite bank (12 KB) + lookup tables |
| **Vectors** | `$FFF8 - $FFFF` | Encryption signature (`$FF, $83`), NMI, RESET, IRQ vectors |

Total RAM used: **21 bytes** (out of 4,096 bytes available).  
Total ROM used: **~12.5 KB** (within 32 KB cartridge space).

---

## 3. Assets

- `assets/AtariLogo.png` — the 128×128 bake input. Pure red `(255, 0, 0)` on transparent, hard alpha, nearly filling the canvas.
- `assets/AtariLogo.pal` — transparent plus Atari red `(216, 40, 0)`. The bake snaps that red to the nearest 7800 NTSC color for `P0C1`.

---

## 4. Building the Demo

### Prerequisites
- `ca65` and `ld65` (cc65 toolchain)
- `cargo` (Rust toolchain for `a78gfx`, `a78sign`, and `a78tool`)

### Build Commands
From the `examples/scale_sprite/` directory:

```bash
make all
```

This will:
1. Compile `a78gfx` (if not already built).
2. Bake `assets/AtariLogo.png` into `logo_bank.bin` & `logo_bank.inc` (one frame per scanline, area filter).
3. Assemble `main.s` with `ca65`.
4. Link `scale_sprite.bin` (32 KB raw ROM) with `ld65`.
5. Sign the ROM with `a78sign`.
6. Package the `.a78` emulator header with `a78tool`.

---

## 5. Running the Demo

```bash
make run                 # test7800
make run-a7800           # MAME a7800
```

To look at the scaled frames without running the ROM, dump them as PNGs (index 1 = red, 2 = green, 3 = blue in the debug palette; transparent = 0) and tune the cutoff:

```bash
make frames                          # writes frames/*.png
make frames ALPHA_THRESHOLD=96
```

---

## 6. What to Verify

1. **Position**: The Fuji is centered (HPOS 68, zones 6-8, lines 96-143).
2. **Scaling**: It shrinks by about one scanline every other frame down to 12 lines, then grows back. The slots between the three bars stay open. At the small end the center bar drops out and the two sides remain separated.
3. **No zone seams**: The three 16-line bands join — no horizontal gap or doubled row at lines 112 and 128.
4. **Color**: Atari red on black.
5. **Stability**: No tearing when the frame index changes.
