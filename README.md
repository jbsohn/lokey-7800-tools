# lokey-7800-tools

A suite of modern command-line utilities for Atari 7800 software and homebrew development.

## Included Tools

### `a78tool`
Atari 7800 `.a78` ROM header utility adhering to the [8BitDev.org A78 Header Specification](https://7800.8bitdev.org/index.php/A78_Header_Specification).

> **Background:** `a78tool` was originally created to handle header generation for the **lokey-ym2149** cart, but it fully supports every header item and field defined in the [8BitDev.org A78 Header Specification](https://7800.8bitdev.org/index.php/A78_Header_Specification).

* **Header Generation:** Combines raw ROM binaries with 128-byte `.a78` emulator headers (`a78tool generate -i game.bin -o game.a78 -c examples/a78header.json`).
* **Full Spec Support:** Complete v1, v3, and v4 header fields including YM2149 sound flags (`--ym2149`), POKEY sound flags, controllers, TV format, save devices, and passthrough slots.
* **Custom Mappers:** Full support for standard (0–5) and custom/experimental mapper IDs (6–255), custom `mapper_opts`, and `interrupt` flags.
* **Header Inspection:** Decodes and prints header fields in a clean summary (`a78tool inspect game.a78`).
* **Header Stripping:** Extracts raw binary ROM payload from `.a78` files (`a78tool strip -i game.a78 -o game.bin`).
* **Build Pipeline Integration:** Supports standalone JSON configuration files (`a78tool -c examples/a78header.json`).

### `a78sign`
Atari 7800 cartridge digital signature tool: a Rust port of `7800sign` (`sign7800.c`) by Bruce Tomlin, from the [7800basic](https://github.com/7800-devtools/7800basic) project. Produces byte-for-byte the same signatures as the original.

* **Verify:** `a78sign -t game.a78` reports whether the signature is valid, empty (never signed), or invalid. Exits `0` only if the signature is valid.
* **Sign:** `a78sign -w game.a78` generates the 120-byte signature and writes it back into the image at `$FF80`. Without `-w` the signature is printed and the file is left untouched. If the image is already valid, writing is skipped unless `-f` / `--force` is specified.
* **Inputs:** a multiple of 4 KB, optionally with a 128-byte `.a78` header in front. Sign the ROM *after* it is final: the signature covers the code from the hash start page (high nibble of `$FFF9`) to `$FFFF`.
* Works on raw `.bin` and `.a78` files alike, so it can follow `a78tool generate` in a build pipeline.

> **License:** `a78sign` is a derivative of Bruce Tomlin's LGPL-licensed `sign7800.c` and is licensed under the **LGPL 2.1** (see [`a78sign/LICENSE`](a78sign/LICENSE)). The rest of this repository, including `a78tool`, is MIT.

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
```
