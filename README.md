# lokey-7800-tools

A suite of modern command-line utilities for Atari 7800 software and homebrew development.

## Included Tools

### `a78tool`
Atari 7800 `.a78` ROM header utility adhering to the [8BitDev.org A78 Header Specification](https://7800.8bitdev.org/index.php/A78_Header_Specification).

> **Background:** `a78tool` was originally created to handle header generation for the **lokey-ym2149** cart, but it fully supports every header item and field defined in the [8BitDev.org A78 Header Specification](https://7800.8bitdev.org/index.php/A78_Header_Specification).

* **Header Generation:** Combines raw ROM binaries with 128-byte `.a78` emulator headers (`a78tool generate -i game.bin -o game.a78 -c a78header.json`).
* **Full Spec Support:** Complete v1, v3, and v4 header fields including YM2149 sound flags (`--ym2149`), POKEY sound flags, controllers, TV format, save devices, and passthrough slots.
* **Custom Mappers:** Full support for standard (0–5) and custom/experimental mapper IDs (6–255), custom `mapper_opts`, and `interrupt` flags.
* **Header Inspection:** Decodes and prints header fields in a clean summary (`a78tool inspect game.a78`).
* **Header Stripping:** Extracts raw binary ROM payload from `.a78` files (`a78tool strip -i game.a78 -o game.bin`).
* **Build Pipeline Integration:** Supports standalone JSON configuration files (`a78tool -c a78header.json`).

### `a78gfx`
Atari 7800 MARIA graphics asset compiler and code generator. Compiles images into hardware-ready 256-byte page-strided binaries and companion assembler include headers.

* **Binary-First Architecture:** Always emits raw `.bin` graphic binary slices alongside lightweight companion `.inc` headers with attributes and binary inclusion directives (`.incbin`, `INCBIN`, `ins`). Strictly avoids assembly `.byte` dumps to prevent source bloat and minimize assemble times.
* **Abstract Assembler Targets (`--assembler ca65|dasm|mads`):**
  * `ca65` (cc65 toolchain): `.incbin "file.bin"`, `.align $100`, `;` comments.
  * `dasm`: `INCBIN "file.bin"`, `ALIGN 256`, `;` comments.
  * `mads`: `ins "file.bin"`, `.align $100`, `;` comments.
* **Format-Agnostic Image Input (`GfxFile` / `GraphicDecoder`):**
  * Automatic format detection via file extension or magic bytes.
  * PNG supported (Indexed, Grayscale, RGB, RGBA), extensible for BMP, GIF, Atari ST Degas (`.pi1`/`.pc1`), Amiga IFF/ILBM, and raw binary.
* **Abstract Palette System & Color Quantization (`--palette <file.gpl>`):**
  * **Format Support:** Full parser for GIMP Palette (`.gpl`) format (exported by Aseprite, GIMP, LibreSprite), extensible for future formats (TOML, JSON, JASC-PAL).
  * **Unified Project Palette:** Enforces the same palette across all imported project sprites and tiles.
  * **Perceptual Redmean Quantization:** Maps arbitrary RGB pixel colors to the nearest color in the active palette using weighted Euclidean redmean distance.
  * **Smart Transparency Handling:** Pixels with $\text{alpha} < 128$ map directly to index 0 (MARIA transparent). For alpha-masked artwork, opaque pixels match visible colors ($1 \dots 3$) to prevent foreground holes. Solid opaque images match against all colors ($0 \dots 3$).
  * **Hardware Equates & Display List Integration:** Maps colors to the 256-entry Atari 7800 NTSC palette table (`MariaColor`) and automatically emits hardware equates in the `.inc` header (e.g. `HERO_pal0_c1 = $36`).
  * **Subpalette Selection (`--subpalette <0..7>`):** Selects active subpalette slot and embeds palette index into bits 7..5 of `pal_w`.
* **Compile-Time Geometric Transformations:**
  * **Rotations:** Discrete rotation generation (`--rotate <deg>`, `--angles <N>`) with RotSprite-style nearest sampling.
  * **Pixel Aspect Ratio (PAR) Correction (`--par`):** Compensates for 160A mode's 2:1 rectangular pixels so rotated artwork stays circular on real CRT screens.
  * **Scaling (`--scale <val>`, `--scales <list>`):** Scaling around sprite center (`scale_centered`) or canvas resizing (`--resize WxH`).
  * **Mirroring:** Horizontal (`--flip-x`) and vertical (`--flip-y`) mirroring.
* **Sprite Banks & Page-Strided DMA Packing (`bake`):**
  * Interleaves multiple transformation frames into consecutive 256-byte pages in memory.
  * Emits frame tables (`frame_lo`, `frame_width_bytes`, `frame_width_pixels`, `frame_pal_w`) allowing the 6502 CPU to switch frames simply by setting `addr_lo = frame_lo[frame]`.

#### CLI Quick Reference

```bash
# Convert a single sprite with shared palette and ca65 header
a78gfx convert assets/ship.png -o build/ship.bin --palette assets/game.gpl --subpalette 0 -a ca65

# Convert with PAR correction and vertical flip
a78gfx convert assets/hero.png -o build/hero.bin --par --flip-y

# Bake 16 rotation angles into an interleaved 256-byte page-strided bank
a78gfx bake assets/ship.png -o build/ship_bank.bin --palette assets/game.gpl --subpalette 1 --angles 16 --par

# Bake with MADS assembler include output
a78gfx bake assets/ship.png -o build/ship_bank.bin -a mads --angles 8
```

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
```
