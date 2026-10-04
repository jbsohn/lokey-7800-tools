# Atari 7800 A78 Header Specification & `a78tool` Manual

This document details the **A78 Header Specification** (v1, v3, and v4) for Atari 7800 emulators and flash cartridges, and documents the usage of **`a78tool`**.

---

## 1. Overview of `a78tool`

`a78tool` is a modern, high-performance CLI utility written in Rust for generating, inspecting, and manipulating Atari 7800 `.a78` ROM headers.

It adheres to the [8BitDev.org A78 Header Specification](https://7800.8bitdev.org/index.php/A78_Header_Specification).

### Features
* **Header Generation**: Wraps raw binary ROM images with a standard 128-byte `.a78` header.
* **Full Header Version Support**: Supports v1, v3, and v4 header extensions, including YM2149 sound flags (`--ym2149`), POKEY sound flags (`--pokey`), controller types, TV standard (NTSC/PAL), and save peripherals.
* **Custom Mapper Mapping**: Native support for standard mappers (0–5) and custom/experimental mapper IDs (6–255), custom `mapper_opts`, and interrupt flags.
* **Header Inspection**: Decodes and displays header properties in a human-readable summary.
* **Header Stripping**: Extracts the raw ROM binary payload from any `.a78` image.
* **JSON Configuration**: Configurable via CLI flags or JSON project files.

---

## 2. CLI Usage & Commands

### Generate an `.a78` File from JSON Config
```bash
a78tool generate -i build/game.bin -o build/game.a78 -c examples/a78header.json
```

### Generate with CLI Flags
```bash
a78tool generate -i build/game.bin -o build/game.a78 --title "My YM Game" --mapper 1 --ym2149 --tv-type ntsc
```

### Inspect an Existing `.a78` File
```bash
a78tool inspect build/game.a78
```

### Strip Header from an `.a78` File
```bash
a78tool strip -i build/game.a78 -o build/game.bin
```

### Sign the Resulting Cartridge (`a78sign`)
```bash
a78sign -w build/game.a78
```

---

## 3. A78 Header Structure (128 Bytes)

The `.a78` header consists of a fixed 128-byte binary structure prepended to the 7800 ROM binary:

| Offset (Dec) | Size (Bytes) | Field Name | Description |
|:---|:---|:---|:---|
| **0** | 1 | `HeaderVersion` | Header version (typically `1`, `3`, or `4`). |
| **1–16** | 16 | `Magic` | `"ATARI7800       "` ASCII magic string. |
| **17–48** | 32 | `CartName` | Null-padded game title string. |
| **49–52** | 4 | `RomSize` | 32-bit unsigned integer (big-endian), size of ROM binary in bytes. |
| **53** | 1 | `CartTypeHi` | Cartridge type high byte (mapper ID high / legacy flags). |
| **54** | 1 | `CartTypeLo` | Cartridge type low byte (Bit 0: POKEY at $4000, Bit 1: SuperGame RAM, Bit 2: YM2149). |
| **55** | 1 | `Controller1` | Primary controller type (Joystick, Lightgun, Paddle, etc.). |
| **56** | 1 | `Controller2` | Secondary controller type. |
| **57** | 1 | `TvType` | TV standard (`0` = NTSC, `1` = PAL). |
| **58** | 1 | `SaveDevice` | Save device flag (Savekey, AtariVox, HighScore Cart). |
| **64** | 1 | `Mapper` | Mapper ID (0 = Linear, 1 = YM-IOA Banked, 2 = SuperGame, etc.). |
| **66–67** | 2 | `AudioAddress` | 16-bit address for expansion sound chip (e.g., `$0800` for YM2149). |

---

## 4. Integration in 7800 Hardware & Toolchains

When developing for expansion hardware like the **Lokey 7800 YM** cartridge:
- Set **Mapper** to `0` for linear 32KB ROMs.
- Set **Mapper** to `1` for 32-pin banked ROMs (switched via YM2149 IOA port).
- Set **YM2149 Present** flag (`--ym2149` or `--audio 0x0800`) to signal hardware detection to emulators such as `a7800` and `js7800`.
