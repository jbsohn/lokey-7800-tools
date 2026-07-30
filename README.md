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

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
```
