# Lokey Bugs

The Silly Bugs demo from [`examples/bugs`](../bugs). A bug is a position, a velocity, and a picture. The pictures are the `a78gfx` files for the beetle, the circle, and the square. [`lokeydll`](../../lib/lokeydll) puts them on the playfield.

`main.s` declares one archetype per picture with `lokeydll_archetype`, and a color list with `lokeydll_color`. The beetle and the circle share color slot 0. The square uses slot 1. Each bug is one object slot: whole-pixel X and Y in the lokeydll object table, and a picture attached once with `lokeydll_set_picture`. The frame loop updates those positions and calls `lokeydll_build`. `main.s` has no MARIA registers, no display-list writes, and no reset/NMI/IRQ code — the library owns all of that.

Sixty bugs are spread across the playfield and bounce off its edges.

## Building

`ca65`, `ld65`, and `ar65` need to be on `PATH`. `a78gfx`, `a78sign`, and `a78tool` are built from this repo. The pictures are `examples/bugs-lokey/assets`. The linker map is `lib/lokeydll/7800.cfg`.

```bash
make -C examples/bugs-lokey
```

Outputs:

- `examples/bugs-lokey/bugs-lokey.bin` — 32KB raw ROM
- `examples/bugs-lokey/bugs-lokey.rom` — signed ROM
- `examples/bugs-lokey/bugs-lokey.a78` — 128-byte header plus the signed ROM
- `examples/bugs-lokey/lokeydll.lib` — the archive linked into the ROM

## Running

```bash
make -C examples/bugs-lokey run-a7800 EMU_A7800=/path/to/mame64
make -C examples/bugs-lokey run EMU_TEST7800=/path/to/test7800
```
