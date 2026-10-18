# lokeydll

Display for a 7800 game that has objects and pictures. The game loads `a78gfx` pictures, declares one archetype per picture, and stores whole-pixel X and Y on an object. Attaching a picture copies the archetype into that object once. This library owns everything else: the cartridge bootstrap, the CPU and MARIA init, the display lists, and the double-buffered DLL swap.

## The game's contract

The game links against `lokeydll.lib` and exports a single entry point, `lokeydll_game`. The library's reset vector boots the CPU, initializes MARIA, then jumps to `lokeydll_game`. The game never writes a MARIA register or builds a display-list structure.

Inside `lokeydll_game`, the game:

1. Loads its pictures (`.include` the `a78gfx` `.inc` files) and declares one archetype per picture with the `lokeydll_archetype` macro.
2. Points `lokeydll_param_ptr` at a color list and calls `lokeydll_load_colors` once.
3. For each object slot `Y`, stores `lokeydll_obj_x,y` and `lokeydll_obj_y,y`, points `lokeydll_param_arch_lo/hi` at the archetype, and calls `lokeydll_set_picture`.
4. Stores how many leading slots are live in `lokeydll_obj_count`.
5. Runs the frame loop:

```asm
update positions in lokeydll_obj_x / lokeydll_obj_y
jsr lokeydll_build
jsr lokeydll_swap_buffers
```

`lokeydll_build` reads the object table and writes the hidden display list. `lokeydll_swap_buffers` waits for the leading edge of VBLANK and publishes that DLL. Fractional position and velocity stay in the game; Maria only sees the whole pixels.

## Pictures and colors

A picture goes in `LOKEYDLL_GFX0`, `LOKEYDLL_GFX1`, or `LOKEYDLL_GFX2`. The 32KB map is `7800.cfg` in this directory. The playfield is `LOKEYDLL_VIEW_W` by `LOKEYDLL_VIEW_H`.

Declare an archetype with the picture's own equates; the library owns the byte layout:

```asm
lokeydll_archetype beetle_arch, beetle_pal_w, beetle_height_lines, beetle_pages
```

Colors are `slot, c1, c2, c3` records from the picture includes, ended by `$FF`:

```asm
colors:
        lokeydll_color 0, beetle_pal0_c1, beetle_pal0_c2, beetle_pal0_c3
        .byte $FF
```

The calling demo is [`examples/bugs-lokey`](../../examples/bugs-lokey).
