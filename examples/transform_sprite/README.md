# Atari 7800 Maria Single-Sprite Rotation Demo

A complete demonstration program showcasing **pre-baked 2D sprite rotation** rendered through the MARIA graphics coprocessor using **256-byte page-strided scanline DMA** and **double-buffered Display Lists**.

---

## 1. What This Demo Showcases

1. **Pre-Baked Rotation Pipeline (`a78gfx bake`)**:
   - Compiles a canonical 16x16 4-color indexed PNG (`assets/ship.png`) into a 16-frame hardware-ready sprite bank.
   - **Rotation**: 16 discrete angles ($0^\circ$ to $337.5^\circ$ in $22.5^\circ$ increments) with **2:1 Pixel Aspect Ratio (PAR) correction** for 160A rectangular pixels.

2. **MARIA 256-Byte Page Striding**:
   - In MARIA hardware, scanline $Y$ of a graphic is fetched from:
     $$\text{Address} = (\text{addr\_hi} + Y) \times 256 + \text{addr\_lo}$$
   - All 20 frames share the **same 16 pages (4 KB total)**.
   - Frame $i$ is selected simply by setting `addr_lo = ship_bank_frame_lo[i]`.
   - Result: 20 transformation frames stored in just 4 KB of ROM with **zero runtime CPU transformation math**!

3. **Dynamic RAM Display List (Glitch-Free & Tear-Free VBLANK Updates)**:
   - A single 6-byte Display List buffer in RAM at `$1800` (`dl_ship`):
     - 4-byte Direct Header (`addr_lo`, `pal_w`, `addr_hi`, `hpos`)
     - 2-byte DL Terminator (`$00, $00`)
   - At VBLANK (synchronized via `MSTAT`), MARIA DMA is inactive, allowing the 6502 CPU to update `addr_lo` and `pal_w` directly in just 16 CPU cycles.
   - When scanline 112 (Zone 7) arrives, MARIA reads the updated frame from RAM with zero tearing, jitter, or emulator compatibility hazards.

4. **Centered Screen Coordinates**:
   - In 160A mode, the screen is 160 pixels wide. The 16-pixel sprite is centered horizontally at $\text{HPOS} = (160 - 16) / 2 = 72$.
   - The visible screen is 240 scanlines divided into 15 zones of 16 lines each. The sprite is centered vertically in **Zone 7** (scanlines 112 to 127).

---

## 2. Memory Layout & Budgets

| Memory Region | Address Range | Purpose |
| :--- | :--- | :--- |
| **Zero Page** | `$0040 - $0041` | Animation timer, frame index |
| **RAM Buffers** | `$1800 - $1805` | Dynamic Display List in RAM (6 bytes: direct header + terminator) |
| **ROM Code** | `$8000 - $8089` | 6502 initialization, VBLANK synchronization, animation loop |
| **ROM RODATA** | `$8100 - $91FF` | Display List List (`dll_list`), `dl_empty`, 16-frame sprite bank + lookup tables |
| **Vectors** | `$FFF8 - $FFFF` | Encryption signature (`$FF, $83`), NMI, RESET, IRQ vectors |

Total RAM used: **8 bytes** (out of 4,096 bytes available).  
Total ROM used: **~4.5 KB** (within 32 KB cartridge space).

---

## 3. Building the Demo

### Prerequisites
Ensure the following tools are installed on your system:
- `ca65` and `ld65` (cc65 toolchain)
- `a78tool` (header packaging)
- `7800sign` (cartridge signature block)
- `cargo` (Rust toolchain for `a78gfx`)

### Build Commands
From the `examples/transform_sprite/` directory:

```bash
# Build the complete .a78 ROM
make all
```

This will automatically:
1. Compile `a78gfx` (if not already built).
2. Bake `assets/ship.png` into `ship_bank.bin` & `ship_bank.inc` (16 rotations with 2:1 PAR).
3. Assemble `main.s` with `ca65`.
4. Link `transform_sprite.bin` (32 KB raw ROM) with `ld65`.
5. Sign the ROM with `7800sign`.
6. Package the standard Atari 7800 emulator header with `a78tool` into `build/transform_sprite.a78`.

---

## 4. Running the Demo

### Run with `test7800`
```bash
make run
# Or override emulator executable path:
make run EMU_TEST7800=/path/to/test7800
```

### Run with `a7800` (MAME)
```bash
make run-a7800
# Or override MAME executable:
make run-a7800 EMU_A7800=/path/to/mame64
```

---

## 5. What to Verify

1. **Center Position**: A spaceship sprite appears dead center on the black screen.
2. **Smooth Rotation**: The spaceship smoothly rotates clockwise continuously through 16 angles with correct aspect ratio (circular, not squished).
3. **Visual Quality**:
   - Cockpit canopy & reactor core appear in bright Electric Cyan (`$98`).
   - Wing tips and thruster cowls appear in Fiery Amber (`$38`).
   - Fuselage armor plating appears in Star White (`$0E`).
   - Background is deep Space Black (`$00`).
5. **Stability**: Zero tearing, flickering, or DMA glitches.
