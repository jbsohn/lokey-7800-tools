# Atari 7800 MARIA Architecture & `libmaria` Specification

This document captures the architectural insights, hardware mechanics, memory budgets, and design specification for **`libmaria`**, an open-source, modular 6502 assembly sprite and display engine for the Atari 7800 using the **ca65 / ld65** toolchain.

---

## 1. Executive Summary & Core Philosophy

The Atari 7800 graphics chip (**MARIA**) is not a frame buffer device, nor is it a traditional tilemap VDP with fixed sprite slots (like the NES or Master System). It is a **cycle-budgeted DMA coprocessor** that constructs the display scanline-by-scanline on the fly into an internal alternating **Line RAM** buffer.

### Key Architectural Truths:
1. **Vertical Division Only**: The screen is sliced vertically into horizontal bands called **Zones** ($1 \dots 16$ scanlines tall). There are **zero horizontal zones**—every zone spans the full $160$-pixel (or $320$-pixel) width of the television line.
2. **Horizontal Placement via Coordinates**: Left-to-right placement and horizontal layering are handled strictly within each zone's **Display List (DL)** using `HPOS` ($0 \dots 159$) and `Width`.
3. **Data Lives in ROM**: Bitmap artwork and graphics data never touch RAM unnecessarily. MARIA streams graphics directly from cartridge ROM via DMA; RAM is reserved strictly for active Display Lists and dynamic game objects.
4. **The Only Constraint is the Scanline DMA Budget**: There are no arbitrary sprite limits (e.g. "8 sprites per line"). Developers can stack multiple layers and palettes on the same scanline as long as total DMA time stays under $\approx 450$ MARIA color cycles per scanline.

---

## 2. MARIA Hardware Mechanics

### 2.1 The Scanline Pipeline (Ping-Pong Line RAM)
MARIA contains two internal horizontal buffers, **Line RAM A** and **Line RAM B** ($160$ or $320$ clocks wide):
- While the CRT electron beam sweeps horizontally across **Scanline $N$** reading out **Line RAM A**, MARIA's DMA pauses the 6502 CPU during HBLANK to assemble **Scanline $N+1$** inside **Line RAM B**.
- At the start of the next HBLANK, the buffers flip.

### 2.2 The Two-Level DMA Table Hierarchy
```
DPPH / DPPL ($2C / $30)
         │
         ▼
┌──────────────────────────────────────────────┐
│  Display List List (DLL)                     │
│  Sequential table of 3-byte zone entries     │
└───────┬──────────────────────────────┬───────┘
        │ Zone 0 Entry                 │ Zone 1 Entry
        ▼                              ▼
┌─────────────────────────┐    ┌─────────────────────────┐
│ Zone 0 Display List     │    │ Zone 1 Display List     │
│ [Header 1 (4 bytes)]    │    │ [Header 1 (4 bytes)]    │
│ [Header 2 (4 bytes)]    │    │ [Header 2 (4 bytes)]    │
│ [Terminator: $00, $00]  │    │ [Terminator: $00, $00]  │
└───────┬─────────────────┘    └─────────────────────────┘
        │
        ▼ Points directly to:
┌─────────────────────────┐
│ Graphic Bitmap in ROM   │  (Zero RAM overhead)
└─────────────────────────┘
```

#### A. Display List List Entry (`MariaDLLEntry` - 3 bytes)
```ca65
.struct MariaDLLEntry
    offset      .byte   ; Bits 3..0: Height-1 (0..15 => 1..16 lines)
                        ; Bit 5: H8  (Holey DMA 8)
                        ; Bit 6: H16 (Holey DMA 16)
                        ; Bit 7: DLI (Display List Interrupt enable)
    dph         .byte   ; High byte of Display List address
    dpl         .byte   ; Low byte of Display List address
.endstruct
```

#### B. Direct Display List Header (`MariaDLHeader` - 4 bytes)
```ca65
.struct MariaDLHeader
    addr_lo     .byte   ; Low byte of graphic data address
    pal_w       .byte   ; Bits 7..5: Palette (0..7), Bits 4..0: -Width ($1F = 1 byte)
    addr_hi     .byte   ; High byte of graphic data address
    hpos        .byte   ; Horizontal pixel position (0..159)
.endstruct
```

#### C. Display List Terminator (`MariaDLTerminator` - 2 bytes)
Indicated by `$00, $00` (second byte has bits 0..4, 6 clear).

---

## 3. Colors, Resolutions, and Multi-Palette Layering

### 3.1 Color Registers & Palettes
MARIA provides **25 colors simultaneously on screen**:
- `BKGRND` ($20): Universal background color (Color 0 for all palettes, transparent for sprites).
- 8 Palettes (Palette 0 to Palette 7): Each has 3 independent colors (`P<n>C1`, `P<n>C2`, `P<n>C3`).

### 3.2 Mixing 160A and 320 Modes on the Same Scanline
- **160A Mode**: 4 pixels per byte (2 bits each: 3 colors + transparency). Ideal for colorful action sprites.
- **320 Modes**: 8 pixels per byte (single-wide pixels). Ideal for crisp text, scoreboards, and health bars.
- In 5-byte extended headers, the `WriteMode` bit selects 160 vs. 320 mode on an **item-by-item basis**. Crisp HUD text and rich 16-color sprites can coexist on the exact same scanline!

### 3.3 Multi-Palette Layering ("Painter's Algorithm")
- The first item in a zone's Display List is drawn at the **bottom** (background).
- Later items in the Display List overwrite earlier pixels (foreground). Transparent pixels (`%00`) allow lower layers to show through.
- **Multi-color sprites**: A single game character can be composed of two consecutive headers sharing the same `HPOS`:
  - Header A: Skin tone & eyes using Palette 5 (3 colors)
  - Header B: Uniform / armor using Palette 1 (3 colors)
  - Result: A seamless **6-color character** on screen with zero flicker!

---

## 4. `libmaria` Software Architecture

### 4.1 The High-Level Abstraction
Game logic only ever manages high-level entities:
```ca65
.struct SpriteObject
    x_pos       .byte   ; 0..159 (Horizontal position)
    y_pos       .byte   ; 0..239 (Vertical scanline position)
    type_id     .byte   ; References ROM metadata table (width, height, gfx pointer, palette)
    flags       .byte   ; Active, Visible, Flip X, Flip Y
.endstruct
```

### 4.2 Structure of Arrays (SoA) for Entity Management
To avoid 16-bit pointer math on the 6502, active game objects are stored in parallel arrays indexed by `X`:
```ca65
MAX_OBJECTS = 48

obj_x:      .res MAX_OBJECTS    ; 48 bytes
obj_y:      .res MAX_OBJECTS    ; 48 bytes
obj_type:   .res MAX_OBJECTS    ; 48 bytes
```
*Total RAM for 48 entities: **144 bytes**.*

All static attributes (Width, Height, Palette, Graphics Pointer) are indexed out of **ROM tables** using `obj_type`.

### 4.3 Automatic Zone Boundary Splitting
When an entity's vertical position straddles a zone boundary ($Y + \text{height} > \text{ZoneEnd}$), `libmaria` automatically:
1. Calculates lines remaining in `Zone N`: $\text{top\_h} = \text{ZoneEnd} - Y$.
2. Emits Header 1 into `Zone N` pointing to `Graphic_Base` with height = $\text{top\_h}$.
3. Emits Header 2 into `Zone N+1` pointing to `Graphic_Base + top_h` with height = $\text{height} - \text{top\_h}$.
4. The game developer never performs manual boundary clipping!

### 4.4 Dynamic Single-Pool Display List Allocation
Instead of partitioning RAM into 15 rigid chunks, all zone lists share a single continuous pool in RAM ($1900 \dots $1BFF):
- `dl_write_ptr`: Current insertion point in RAM.
- Empty zones cost only **2 bytes** (`$00, $00`).
- Busy zones can expand dynamically up to the hardware DMA limit.
- **Double buffering**: Two identical pools (`Buffer_A` and `Buffer_B`). While MARIA displays Buffer A, the 6502 builds Buffer B. At VBLANK, swapping `DPPL` flips the screen tear-free.

---

## 5. Total Frame & Memory Budgets

### 5.1 RAM Budget (Total: < 1 KB out of 4 KB available)
```text
Address Range       Size      Allocation
-------------       ----      ----------
$1800 - $18BF       192 B     Active Objects Table (48 objects * 4 bytes)
$18C0 - $18FF        64 B     Engine State, Zone Indices, ZP shadow vars
$1900 - $1B7F       640 B     Display List Pools (Buffer A & Buffer B double buffer)
$1B80 - $27FF     3,200 B     100% FREE FOR GAME LOGIC, RAM TILES & SCORES!
```

### 5.2 CPU Frame Budget (Robotron Scenario: 50 Moving Sprites)
- **Total CPU cycles per frame (NTSC 60 Hz)**: $29,833$ cycles.
- **MARIA DMA pauses (worst-case heavy display)**: $\approx 11,000$ cycles.
- **Net CPU cycles available for 6502**: $\approx 18,800$ cycles.
- **Engine Processing Cost**:
  - Entity loop + zone lookup + header emission: $\approx 80\text{ cycles/sprite}$.
  - $50 \text{ sprites} \times 80 = \mathbf{4,000\text{ cycles}}$ ($\approx 21\%$ of available CPU).
  - Zone boundary splits (~15% of sprites): $\approx 300\text{ cycles}$.
  - Double buffer pointer flip & terminator write: $\approx 150\text{ cycles}$.
  - **Total Engine Overhead: $\approx 4,450\text{ cycles}$ ($\approx 23\%$)**.
- **Free CPU remaining for Game AI, Collisions, Physics, and Game Logic**: **$> 14,000\text{ cycles}$ ($77\%$)**.

---

## 6. Companion Tooling: `mariatool` (PNG Asset Pipeline)

To eliminate manual sprite byte construction, a dedicated CLI tool (or `a78tool` subcommand) written in Rust processes standard Indexed PNGs:

### Capabilities:
1. **Indexed PNG Input**: Reads 4-color indexed artwork from Aseprite/GIMP (Color 0 = transparent, Colors 1..3 = sprite palette).
2. **256-Byte Page Transposition**: Automatically arranges 16-line graphics across consecutive 256-byte pages in memory so MARIA's `addr + offset` DMA addressing hits the correct scanlines.
3. **ca65 Assembly & Binary Export**:
   - Generates `.s` assembly files containing `.byte` tables and `MariaDLHeader` struct definitions.
   - Exports raw `.bin` files ready for `.incbin`.
4. **Multi-Layer Palette Splitting**: Optional flag to take an 8-color or 16-color PNG and automatically decompose it into two or three 4-color layer masks with matched `HPOS` coordinates.

---

## 7. Architectural Comparison

| Dimension | 1980s Monolithic DASM Games | Modern `libmaria` + ca65 |
| :--- | :--- | :--- |
| **Toolchain** | Single monolithic `.asm` file, hardcoded `ORG` | Modular `.s` modules, relocatable `ld65` segments, clean Makefiles |
| **Data Layout** | Magic hex numbers, manual offsets | Hardware-accurate `.struct` definitions (`MariaDLLEntry`, `MariaDLHeader`) |
| **Sprite Positioning** | Manual zone math hardcoded in enemy AI | Clean $(X, Y)$ entity abstraction with automatic zone splitting |
| **Audio Separation** | Intertwined custom raster hacks | Decoupled from graphics engine; managed by dedicated external sound drivers |
| **Asset Pipeline** | Hand-typed hex matrices | Modern Indexed PNG converter via Rust CLI |
| **RAM Footprint** | Custom per-game hacks | Standardized $< 1\text{ KB}$ footprint, $> 3\text{ KB}$ free for games |
