# Atari 7800 Maria Step 2: Static Sprite Demo

A minimal Atari 7800 demonstration program showcasing **a single 16x16 160A sprite centered on screen** using the MARIA graphics coprocessor.

---

## 1. What This Demo Showcases

1. **MARIA 160A Direct Graphics Mode**:
   - 4 pixels per byte (2 bits each: 3 colors + transparency).
   - Direct DMA rendering using `TMariaDLHeaderDirect` (4 bytes) + `TMariaDLTerminator` (2 bytes).

2. **256-Byte Page-Strided Sprite DMA**:
   - MARIA fetches line $Y$ of a graphic object by adding $Y$ to the high byte:
     $$\text{Address} = (\text{addr\_hi} + Y) \times 256 + \text{addr\_lo}$$
   - The 16 scanlines of `ship.png` are compiled by `a78gfx` with `.align $100` before each line.
   - On scanline $Y$, MARIA automatically reads the correct scanline data from consecutive pages in ROM.

3. **Pure ROM Architecture**:
   - Zero RAM buffers required! The Display List (`dl_ship`) and Display List List (`dll_list`) reside entirely in ROM.

4. **Centered Screen Coordinates**:
   - Visible screen: 240 scanlines partitioned into 15 zones of 16 lines each ($15 \times 16 = 240$).
   - Horizontal centering: $\text{HPOS} = (160 - 16) / 2 = 72$.
   - Vertical centering: Zone 7 (scanlines 112 to 127).

---

## 2. Building the Demo

```bash
make all
```

This will:
1. Convert `assets/ship.png` to `ship.bin` & `ship.inc` using `a78gfx convert`.
2. Assemble `main.s` with `ca65`.
3. Link `build/static_sprite.bin` (32 KB ROM) with `ld65`.
4. Sign the ROM with `7800sign`.
5. Package `build/static_sprite.a78` with `a78tool`.

---

## 3. Running the Demo

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

## 4. What to Verify

1. **Center Position**: The spaceship sprite sits stationary dead center on the black screen.
2. **Palette**:
   - Cockpit canopy & reactor core: Electric Cyan (`$98`)
   - Wing tips & thrusters: Fiery Amber (`$38`)
   - Fuselage armor plating: Star White (`$0E`)
   - Background: Space Black (`$00`)
3. **Clarity**: Crisp pixels without distortion or DMA jitter.
