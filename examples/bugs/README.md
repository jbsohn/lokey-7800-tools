# Silly Bugs

Port of the 30-Nov-1986 Atari ST dev-kit demo (lmd). Sixteen sprites fall under gravity and bounce off the edges of the screen.

The pictures are the ST beetle, textured circle, and solid square, converted with `a78gfx` to 160A. `--par` halves the 16×16 square-pixel art to 8×16, so it stays round on the 7800's 2:1 pixels. A 320A overlay is not used: one frame is one read mode, and the beetle's three colors already fit a single 160A header. The beetle's two dark-gray antenna tips are lost: each shares a 2-pixel pair with a red pixel, and the `--par` merge breaks the tie toward red.

## What the frame does

Maria has no framebuffer. Each frame rebuilds a display list for every 16-line zone and publishes it at vblank. The list Maria is reading is not the one being written.

Y is a scanline. Zone = Y / 16, and k = Y & 15. On a zone boundary one header covers the sprite. Otherwise the top zone's graphics high byte is `base + k` and the next zone's is `base + k - 16`. Pages that fall outside the sprite's 4KB block land in an odd 4KB region. DLL bit 6 (the `$4F` zone byte) makes those reads return zero.

Positions and velocities are 8.8 fixed point. The ST sample was 16.16 on a 320-wide screen, so X and the X speeds are half of the ST values. Y is still one unit per scanline, and gravity is the same quarter-pixel per frame (`$40`). The playfield is the ST's 200 lines, 160×200. A sprite's top-left stops at X = 152 and Y = 184, so the far pixel is column 159 and the far line is playfield line 199.

NTSC Maria walks rasters 16 through 258 (243 lines); PAL walks 293. The display-list list has nineteen 16-line zones: one blank zone, so the playfield clears the top overscan, then 13 playfield zones (208 lines, the bugs use 200), then five blank zones. That is 304 lines, so vblank arrives before Maria can read past the list on either standard. NTSC never reaches the last three blank zones. The `.a78` header still declares NTSC, and the speeds and timing are NTSC-tuned. The blank zones are plain (not holey) and share one empty display list in ROM.

## Palette

Subpalette 0 is the beetle and the circle. Subpalette 1 is the square.

| Register | Value | Use |
| --- | --- | --- |
| `P0C1` | `$44` | red |
| `P0C2` | `$0A` | gray |
| `P0C3` | `$76` | magenta |
| `P1C1` | `$2C` | tan square |
| `BKGRND` | `$00` | black |

`$2C` is the nearest automatic NTSC match to the ST tan. It reads as a light peach.

## Memory

| Region | Address | Contents |
| --- | --- | --- |
| Display list 0 | `$1800`–`$1B59` | 13 playfield zones × 66 bytes |
| Bug state | `$1B5A`–`$1BE9` | X/Y/velocity/image, 16 each |
| Display list 1 | `$2200`–`$2559` | second buffer |
| Code and DLL | `$8000` | both display-list lists stay in ROM |
| Beetle | `$A000` | one even 4KB block |
| Circle | `$C000` | one even 4KB block |
| Square | `$E000` | one even 4KB block |

`$9xxx`, `$Bxxx`, `$Dxxx`, and `$Fxxx` are the holey regions. Maria reads them as zero. The CPU still sees the fill.

## Building

`ca65` and `ld65` need to be on `PATH`. `a78gfx`, `a78sign`, and `a78tool` are built from this repo.

```bash
make -C examples/bugs
```

Outputs:

- `examples/bugs/bugs.bin` — 32KB raw ROM
- `examples/bugs/bugs.rom` — signed ROM
- `examples/bugs/bugs.a78` — 128-byte header plus the signed ROM

`assets/beetle.png`, `circle.png`, `square.png`, and `bugs.gpl` are the source art. They were decoded once from the ST bitplanes in `IMAGES.S` (16×16 RGBA, transparent background, ST colors; the palette holds subpalette 0 then subpalette 1). Edit them directly.

## Running

```bash
make -C examples/bugs run-a7800 EMU_A7800=/path/to/mame64
make -C examples/bugs run EMU_TEST7800=/path/to/test7800
```

Bugs should fall from a diagonal at the top left, bounce off the floor about 27 lines above the bottom of the window (the playfield ends at line 216 of 243), and bounce off both side walls. There should be a black band one zone (16 lines) tall above the playfield. Sprites crossing a zone boundary every 16 lines should not vanish, tear, or show garbage rows.
