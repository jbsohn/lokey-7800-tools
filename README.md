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

* **Verify:** `a78sign -t game.bin` reports whether the signature is valid, empty (never signed), or invalid. Exits `0` only if the signature is valid.
* **Sign:** `a78sign -w game.bin` generates the 120-byte signature and writes it back into the image at `$FF80`. Without `-w` the signature is printed and the file is left untouched. If the image is already valid, writing is skipped unless `-f` / `--force` is specified.
* **Inputs:** Raw cartridge ROM binary whose size is a multiple of 4 KB. Sign the ROM *after* it is compiled: the signature covers the code from the hash start page (high nibble of `$FFF9`) to `$FFFF`.
* **Clean Build Pipeline:** Compile assembly to `game.bin` → sign with `a78sign -w game.bin` → package for emulators/flash carts with `a78tool generate -i game.bin -o game.a78`.

> **License:** `a78sign` is a derivative of Bruce Tomlin's LGPL-licensed `sign7800.c` and is licensed under the **LGPL 2.1** (see [`a78sign/LICENSE`](a78sign/LICENSE)). The rest of this repository, including `a78tool`, is MIT.

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
```

---

## Acknowledgements & Credits

- **Bruce Tomlin**: For creating `7800sign` (`sign7800.c`) in the [7800basic](https://github.com/7800-devtools/7800basic) project. His foundational work reverse engineering and implementing the Atari 7800 digital signature algorithm made modern homebrew signing accessible to the entire community.
- **Curt Vendel (The Atari Historical Society)**: For salvaging and preserving the original Atari engineering disks containing the 7800 digital encryption private keys, and sharing them with the community at Classic Gaming Expo 2001.
- **Mike Saarna (RevEng)**: For the indispensable [7800basic](https://github.com/7800-devtools/7800basic) compiler and development suite, and his tireless contributions to modern Atari 7800 homebrew creation.
- **8BitDev.org**: For publishing and maintaining the authoritative [A78 Header Specification](https://7800.8bitdev.org/index.php/A78_Header_Specification) that defines modern emulator and flash cart interoperability.
- **The Atari Community**: We are grateful to the dedicated homebrew developers, tool authors, and enthusiasts on AtariAge and beyond keeping the Atari 7800 platform vibrant.

---

## License

- `a78tool` and documentation are licensed under the **MIT License** (see [LICENSE](LICENSE)).
- `a78sign` is a derivative of Bruce Tomlin's LGPL-licensed `sign7800.c` and is licensed under the **GNU Lesser General Public License v2.1** (see [a78sign/LICENSE](a78sign/LICENSE)).
