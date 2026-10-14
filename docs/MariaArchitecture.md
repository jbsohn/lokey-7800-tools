# Atari 7800 MARIA Architecture & `libmaria` Specification

This document captures the architectural insights, hardware mechanics, memory budgets, and design specification for **`libmaria`**, an open-source, modular 6502 assembly sprite and display engine for the Atari 7800 using the **ca65 / ld65** toolchain.

>
> **Author's Note / Living Document**: Everything documented here was discovered through reading vintage hardware manuals, inspecting community research, and actively building and testing working code samples on emulators and assembler toolchains with AI assistance. I might not have every nuance 100% right yet, but this is an evolving, living document that will continue to be refined and updated as I learn more through hands-on development.

---

## 1. Executive Summary & Core Philosophy

The Atari 7800 graphics chip (**MARIA**) is not a frame buffer device, nor is it a traditional tilemap VDP with fixed sprite slots (like the NES or Master System). It is a **cycle-budgeted DMA coprocessor** that constructs the display scanline-by-scanline on the fly into an internal alternating **Line RAM** buffer.

### Key Architectural Truths

1. **Vertical Division Only**: The screen is sliced vertically into horizontal bands called **Zones** ($1 \dots 16$ scanlines tall). There are **zero horizontal zones**—every zone spans the full $160$-pixel (or $320$-pixel) width of the television line.
2. **Horizontal Placement via Coordinates**: Left-to-right placement and horizontal layering are handled strictly within each zone's **Display List (DL)** using `HPOS` ($0 \dots 159$) and `Width`.
3. **Data Lives in ROM**: Bitmap artwork and graphics data never touch RAM unnecessarily. MARIA streams graphics directly from cartridge ROM via DMA; RAM is reserved strictly for dynamic Display Lists and active game objects.
4. **The Only Constraint is the Scanline DMA Budget**: There are no arbitrary sprite limits (e.g. "8 sprites per line"). Developers can stack multiple layers and palettes on the same scanline as long as total DMA time stays under $\approx 450$ MARIA color cycles per scanline.

### 1.1 The Engineering Thesis: Pre-Process Offline, Stream from Flash ROM

The graphics engine is built on a clear architectural blueprint: **trade inexpensive Flash ROM to eliminate runtime CPU math, minimize RAM footprint, and maximize frame rate**.

| Architectural Dimension | Traditional 7800 Approach | `libmaria` + `a78gfx` Pipeline |
| :--- | :--- | :--- |
| **Heavy Math & Pre-Processing** | Runtime 6502 math or hand-drawn frames | Offline host compilation (`a78gfx` discrete angle rotations with PAR, scaling, flips; RotSprite & layer flattening planned) |
| **ROM Storage** | Uncompressed monolithic tiles | Pre-baked page-strided graphics & DL templates in banked ROM |
| **RAM Footprint** | Ad-hoc buffers scattered across RAM | Standardized $< 1\text{ KB}$ footprint for Display Lists and active game entities |
| **Runtime 6502 CPU Work** | Complex zone clipping and manual table math | Fast blast-copies of 4-byte pre-baked headers into RAM Display Lists ($\approx 80\text{ cycles/sprite}$) |
| **Visual Result** | Static or jittery sprites with manual clipping | 16-bit arcade-quality rotation, scaling, and multi-layer visuals with **$< 20\%$ CPU** |
| **Free CPU Remaining** | CPU bottlenecked by graphics setup | **$> 75\%$ of 6502 CPU remains 100% free** for game AI, collisions, physics, and gameplay at 60 FPS! |

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

#### A. Display List List Entry (`TMariaDLLEntry` - 3 bytes)

```ca65
.struct TMariaDLLEntry
    offset      .byte   ; Bits 3..0: Height-1 (0..15 => 1..16 lines)
                        ; Bit 5: H8  (Holey DMA 8)
                        ; Bit 6: H16 (Holey DMA 16)
                        ; Bit 7: DLI (Display List Interrupt enable)
    dph         .byte   ; High byte of Display List address
    dpl         .byte   ; Low byte of Display List address
.endstruct
```

#### B. Direct Display List Header (`TMariaDLHeaderDirect` - 4 bytes)

```ca65
.struct TMariaDLHeaderDirect
    addr_lo     .byte   ; Low byte of graphic data address
    pal_w       .byte   ; Bits 7..5: Palette (0..7), Bits 4..0: -Width ($1F = 1 byte)
    addr_hi     .byte   ; High byte of graphic data address
    hpos        .byte   ; Horizontal pixel position (0..159)
.endstruct
```

#### C. Extended Display List Header (`TMariaDLHeaderExtended` - 5 bytes)

```ca65
.struct TMariaDLHeaderExtended
    addr_lo     .byte   ; Low byte of graphic data address
    flags       .byte   ; Bit 7: Kangaroo, Bit 6: WriteMode, Bits 5..0: Indirect offset
    addr_hi     .byte   ; High byte of graphic data address
    pal_w       .byte   ; Bits 7..5: Palette (0..7), Bits 4..0: -Width
    hpos        .byte   ; Horizontal pixel position (0..159)
.endstruct
```

#### D. Display List Terminator (`TMariaDLTerminator` - 2 bytes)

```ca65
.struct TMariaDLTerminator
    term0       .byte   ; $00
    term1       .byte   ; $00 (second byte has bits 0..4, 6 clear)
.endstruct
```

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

### 3.4 Combining Objects in 320 Modes & DMA Overhead Analysis

#### A. Multi-Layer / Multi-Palette Overlays (Color Combination)

In 320A mode, each byte contains 8 single-wide pixels, but each pixel is only 1 bit ($0$ = background/transparent, $1$ = foreground color from the selected palette). Consequently, a single 320A object can only display **one foreground color**.

By combining multiple 320A objects at the **exact same $(X, Y)$ position**:

- **Layer 1**: White highlights / eyes (Palette 0)
- **Layer 2**: Red armor / body (Palette 1)
- **Layer 3**: Dark shadow / outline (Palette 2)

Because transparent bits allow the underlying Line RAM pixels to show through, MARIA’s painter's algorithm stacks them into a single, razor-sharp **multi-color 320 sprite**.

#### B. Meta-Sprites / Multi-Part Assemblies (Spatial Combination)

Objects can also be combined spatially to create large characters or articulated models:

- **Horizontal tiling**: Left segment at $(X, Y)$ and right segment at $(X + \text{width}, Y)$.
- **Articulated modular sprites**: Head, torso, and weapons managed as individual sub-sprites that move and animate relative to a single root game object.

#### C. Exact Hardware DMA Cycle Costs & The "DMA Hole"

On an NTSC 7800, each scanline provides $\approx 454$ MARIA color cycles ($\approx 227$ CPU cycles):

- **Scanline Setup Overhead**: $\approx 16$ cycles (fetching zone DLL entry)
- **DL End Terminator (`$00, $00`)**: $6$ cycles
- **Available DMA Budget for Sprites & Tiles**: **$\approx 430$ cycles per scanline**

MARIA fetch costs:

- **4-byte Header fetch**: **8 cycles** ($2$ cycles per byte)
- **5-byte Extended Header fetch**: **10 cycles** ($2$ cycles per byte)
- **Pixel Data fetch**: **3 cycles per byte of graphics ROM**

Because 320 mode packs **8 pixels into a single byte** (compared to 4 pixels per byte in 160A), graphics data in 320 mode is **half as many bytes wide** for an equivalent pixel width.

**Overhead for a standard 16-pixel wide sprite on a scanline:**

| Sprite Format | Width in Bytes | Header Fetch | Data Fetch | Total DMA per Scanline | % of Scanline Budget |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Standard 160A** (1 layer, 3 colors) | 4 bytes | 8 cycles | $4 \text{ B} \times 3 = 12$ cycles | **20 cycles** | ~4.6% |
| **Single 320A** (1 layer, 1 color) | 2 bytes | 8 cycles | $2 \text{ B} \times 3 = 6$ cycles | **14 cycles** | ~3.2% |
| **2-Layer 320A** (2 layers, 2 colors) | $2 \times 2$ bytes | $2 \times 8 = 16$ | $2 \times 6 = 12$ cycles | **28 cycles** | ~6.5% |
| **3-Layer 320A** (3 layers, 3+ colors) | $3 \times 2$ bytes | $3 \times 8 = 24$ | $3 \times 6 = 18$ cycles | **42 cycles** | ~9.7% |

#### D. Best Practices to Prevent Scanline Overload

Layering objects is cheap and safe for **player characters, bosses, cursors, and HUD elements** ($< 10\%$ of scanline DMA). However, if 8 swarm enemies all share the same scanline, layering them would require 16 headers and exceed available DMA time, causing a **MARIA DMA hole** (screen tearing / missing graphics).

- **Rule of Thumb**: Reserve multi-layer 320 combining for heroes and bosses; keep high-density swarm enemies and bullets single-layer (160A or single 320A).

---

## 4. `libmaria` Software Architecture: Game Objects vs. Graphics Separation

A core design tenet of `libmaria` is the **strict separation between Game Objects and the MARIA Display Pipeline**. Game logic never directly manipulates Display Lists, zone pointers, or MARIA registers.

```
┌────────────────────────────────────────────────────────┐
│  STAGE 1: Game Logic & Object Simulation               │
│  - Physics, movement, player input, enemy AI           │
│  - Operates purely on high-level Game Object state     │
│  - Zero knowledge of MARIA zones, DLL, or DL headers   │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│  STAGE 2: `libmaria` Render Pipeline (DL Assembler)    │
│  - Iterates active Game Objects                        │
│  - Expands Composite / Multi-Layer Meta-Sprites        │
│  - Performs automatic vertical Zone Boundary Splitting │
│  - Emits 4/5-byte headers into active RAM DL pool      │
│  - Writes $00, $00 terminators per zone                │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│  STAGE 3: VBLANK Swap (Hardware Display)               │
│  - Swaps DPPL to point MARIA to newly assembled buffer │
│  - CPU begins assembling next frame in opposite buffer │
└────────────────────────────────────────────────────────┘
```

### 4.1 Strict Separation: Static Display Lists (ROM) vs. Dynamic Objects (RAM)

To protect the 7800's precious 4 KB of RAM and minimize CPU cycles, screen graphics are divided into two completely separate tiers:

1. **Static Display Lists (100% ROM Resident)**:
   - **Use cases**: HUD score display, life counters, status bars, static playfield backdrops, screen borders.
   - **Mechanic**: `a78gfx` compiles these Display Lists directly into cartridge ROM.
   - **Wiring**: In the Display List List (DLL), the zone entries for these static bands point **directly to ROM addresses** (`dph:dpl = >hud_rom : <hud_rom`).
   - **Cost**: **0 bytes of RAM** and **0 CPU cycles per frame** (the 6502 never touches or loops over them during gameplay).

2. **Dynamic Objects (RAM Scratchpad)**:
   - **Use cases**: Player ships, enemies, projectiles, pickups, moving hazards.
   - **Mechanic**: Managed in the dynamic `GameObject` pool. Only these active moving entities are processed by the 6502 each frame and emitted into the double-buffered RAM Display List.

---

### 4.2 The High-Level `GameObject` Abstraction

Game logic operates on high-level entities that store coordinates and a direct 16-bit pointer into ROM:

```ca65
.struct GameObject
    x_pos       .byte   ; 0..159 (Horizontal pixel position)
    y_pos       .byte   ; 0..239 (Vertical scanline position)
    rom_ptr     .word   ; 16-bit direct pointer to pre-baked ROM archetype
    flags       .byte   ; Active, Visible, Flip X, Flip Y
.endstruct
```

#### Structure of Arrays (SoA) Layout in RAM

To avoid costly 16-bit pointer arithmetic on the 6502, active objects are stored in parallel arrays indexed by `X`:

```ca65
MAX_OBJECTS = 48

obj_x:          .res MAX_OBJECTS    ; 48 bytes
obj_y:          .res MAX_OBJECTS    ; 48 bytes
obj_rom_ptr_lo: .res MAX_OBJECTS    ; 48 bytes (Low byte of ROM archetype)
obj_rom_ptr_hi: .res MAX_OBJECTS    ; 48 bytes (High byte of ROM archetype)
obj_flags:      .res MAX_OBJECTS    ; 48 bytes
```

*Total RAM for 48 active dynamic entities: **240 bytes**.*

---

### 4.3 Pre-Baked ROM Archetypes & Meta-Sprites

The `rom_ptr` points directly into ROM to an archetype block generated at build time by `a78gfx`:

```ca65
; In ROM: Pre-baked by a78gfx
player_ship_angle_0:
    .byte 2     ; num_parts = 2 (e.g. 2-layer 320 mode composite)

    ; Part 0 (Hull - Palette 1)
    .byte <ship_hull_gfx,   (PAL_1 << 5) | (-2 & $1F), >ship_hull_gfx
    
    ; Part 1 (Canopy - Palette 2)
    .byte <ship_canopy_gfx, (PAL_2 << 5) | (-2 & $1F), >ship_canopy_gfx
```

#### How Game Logic Changes Shapes or Animations

Game logic never touches palettes or widths. To rotate or animate, it simply updates `rom_ptr`:

```ca65
lda #<player_ship_angle_1
sta obj_rom_ptr_lo,x
lda #>player_ship_angle_1
sta obj_rom_ptr_hi,x
```

#### Fast Traversal & Display List Emission

During the render pass, the engine performs a fast blast-copy directly from ROM into the active zone's RAM Display List:

1. Loads `obj_rom_ptr` into a Zero Page pointer (`ptr_lo, ptr_hi`).
2. Reads `num_parts`.
3. For each part, copies the 3 pre-baked bytes (`addr_lo`, `pal_w`, `addr_hi`) directly from ROM into RAM.
4. Writes `obj_x` into `hpos`.
Zero runtime math, zero palette decoding, zero temporary RAM.

---

### 4.4 Automatic Zone Boundary Splitting

When an entity's vertical position straddles a zone boundary ($Y + \text{height} > \text{ZoneEnd}$), `libmaria` automatically:

1. Calculates lines remaining in `Zone N`: $\text{top\_h} = \text{ZoneEnd} - Y$.
2. Emits Header 1 into `Zone N` pointing to `Graphic_Base` with height = $\text{top\_h}$.
3. Emits Header 2 into `Zone N+1` pointing to `Graphic_Base + top_h` with height = $\text{height} - \text{top\_h}$.
4. The game developer never performs manual boundary clipping!

---

### 4.5 Dynamic Single-Pool Display List Allocation

Instead of partitioning RAM into 15 rigid chunks, all zone lists share a single continuous pool in RAM ($1900 \dots $1BFF):

- `dl_write_ptr`: Current insertion point in RAM.
- Empty zones cost only **2 bytes** (`$00, $00`).
- Busy zones can expand dynamically up to the hardware DMA limit.
- **Double buffering**: Two identical pools (`Buffer_A` and `Buffer_B`). While MARIA displays Buffer A, the 6502 builds Buffer B. At VBLANK, swapping `DPPL` flips the screen tear-free.

---

### 4.6 Architectural Planning: Viewport Scrolling via Render Pipeline Decoupling

> [!NOTE]
> **Planning & Design Preview**: This section outlines the theoretical architectural roadmap for scrolling support in `libmaria`. Because MARIA lacks hardware scroll registers (`SCROLL_X` / `SCROLL_Y`), scrolling must be achieved in software. The strict separation between high-level **Game Objects** and low-level **MARIA Display Lists** allows this to be handled entirely inside the render pipeline—in theory—without polluting game simulation logic.

```
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 1: Game Simulation (6502)                      │
│ - Entities move in pure World Space $(X_{w}, Y_{w})$                   │
│ - Zero knowledge of camera offsets, screen clipping, or MARIA zones    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│            STAGE 2: `libmaria` Render Pipeline (Screen Mapper)         │
│                                                                        │
│ 1. Viewport Translation:                                               │
│    $X_{screen} = X_{world} - Camera_{X}$                               │
│    $Y_{screen} = Y_{world} - Camera_{Y}$                               │
│                                                                        │
│ 2. Frustum / Viewport Culling:                                         │
│    Is $(X_{screen}, Y_{screen})$ completely off-screen?                │
│    - YES ➔ Skip object immediately (0 DMA cycle cost, 0 DL bytes).     │
│                                                                        │
│ 3. Automated Horizontal Edge Clipping:                                 │
│    Does object straddle the left ($X < 0$) or right ($X > 160$) edge?  │
│    - Trim `width` and advance `addr_lo` automatically.                 │
│                                                                        │
│ 4. Automated Vertical Zone Slicing:                                    │
│    Does object straddle a 16-scanline Zone boundary?                   │
│    - Split into Top Slice and Bottom Slice across Zone $Z$ & $Z+1$.    │
│                                                                        │
│ 5. Template Blast Copy:                                                │
│    - Blast-copy 4-byte header directly from pre-baked ROM archetype.   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   STAGE 3: VBLANK Display Swap                         │
│ - Tear-free 60 Hz frame presented to MARIA.                            │
└────────────────────────────────────────────────────────────────────────┘
```

#### How Horizontal Scrolling Will Be Attempted

1. **Background Ribbons**: `a78gfx` bakes wide horizontal terrain ribbons into ROM (e.g., 320 pixels wide).
2. **Zone Header Clipping**: In each zone, only 1 or 2 background headers are needed. `libmaria` clips the ribbon at screen edges:
   - When scrolling off the left edge ($X < 0$), `libmaria` calculates $\text{clipped\_bytes} = |X| / 4$, advances `addr_lo += clipped_bytes`, decreases `width -= clipped_bytes`, and pins `hpos = 0`.
   - When entering from the right, `width` is capped to fit within the 160-pixel boundary.
3. **Parallax for Free**: Different zones (e.g., distant mountain zone vs. foreground track zone) can simply sample different camera multipliers ($0.5\times \text{ vs. } 1.0\times$), achieving hardware-free parallax scrolling with minimal CPU cost.

#### How Vertical Scrolling Will Be Attempted

1. **Zone Truncation Trick (Fine Vertical Scroll)**:
   - For fine vertical scrolling ($F = Camera_Y \pmod{16}$):
     - At VBLANK, `libmaria` adjusts Zone 0's height in the DLL to $(16 - F)$ scanlines.
     - Adjusts the bottom zone's height to $(F)$ scanlines.
     - Adjusts Zone 0's graphics base pointer `addr_hi` by $+F$ scanline pages.
     - The entire display shifts vertically by $F$ fine scanlines with zero tile redrawing.
2. **Coarse Zone Rollover ($\lfloor Camera_Y / 16 \rfloor$)**:
   - When $F$ rolls over 16 scanlines, rotate the zone pointers in the master DLL array and stream in the next zone entry from the level table.

---

## 5. Total Frame & Memory Budgets

### 5.1 RAM Budget (Total: ~1.5 KB out of 4 KB available)

```text
Address Range       Size      Allocation
-------------       ----      ----------
$1800 - $18EF       240 B     Active Dynamic Objects Table (48 objects * 5 bytes)
$18F0 - $18FF        16 B     Engine State, Zone Indices, ZP shadow vars
$1900 - $1AFF       512 B     Display List Dynamic Pool Buffer A
$1B00 - $1CFF       512 B     Display List Dynamic Pool Buffer B
$1D00 - $1D7F        50 B     Display List List (DLL) Buffer A (16 zones)
$1D80 - $1DFF        50 B     Display List List (DLL) Buffer B (16 zones)
$1E00 - $27FF     2,560 B     100% FREE FOR GAME LOGIC, AUDIO, RAM TILES & SCORES!
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
- **Free CPU remaining for Game AI, Collisions, Physics, and Audio**: **$> 14,000\text{ cycles}$ ($77\%$)**.

---

## 6. Companion Tooling: `a78gfx` (PNG Asset Pipeline)

To bridge the gap between modern pixel art and MARIA's bare-metal memory architecture, a dedicated Rust CLI tool called **`a78gfx`** handles automated compilation of Indexed PNGs into ready-to-assemble 7800 ROM assets.

### 6.1 The 512 KB Flash ROM Super-Cart Advantage ($2 ROM vs. Precious RAM)

While 1980s commercial cartridges were strictly constrained by chip costs to 32 KB or 48 KB, modern hardware production changes the economics completely: **a 512 KB flash ROM chip costs $\approx \$2$**.

- **The Core Trade-off**: We intentionally trade massive, inexpensive ROM capacity to achieve **maximum 6502 execution speed**, **minimum RAM consumption (< 600 bytes)**, and **minimum MARIA DMA pressure**.
- **Zero Runtime Math**: Any graphic transformation the 7800 lacks in hardware (Rotation, Scaling, Horizontal Flip, Vertical Flip, Palette Slicing) is **pre-baked into ROM at compile time**.
- This gives the Atari 7800 the visual capabilities of a 16-bit Super Scaler / Neo Geo arcade board with **zero runtime CPU cycle penalty** and **zero RAM allocation**!

### 6.2 `a78gfx` Core Capabilities

1. **Format-Agnostic Graphic Input (`GfxFile` / `GraphicDecoder`)**:
   - Decoupled from format specifics via an extensible `GraphicDecoder` trait and `GfxFile` container.
   - Automatically detects format via file extension or magic bytes (PNG, BMP, GIF, Atari ST Degas `.pi1`/`.pc1`, Amiga IFF/ILBM, and raw binary).
   - Converts any supported source format into our internal format-agnostic `IndexedGraphic` raster before running transformations.
2. **Compile-Time Image Combination & Layer Flattening (`--combine <files...>` / `--flatten`) [Planned Task]**:
   - Allows artists to draw modular art across multiple source PNG files (e.g. `hull.png`, `cockpit.png`, `weapons.png`).
   - `a78gfx` composites and flattens them into a **single unified sprite** at build time before generating rotations and scale steps.
   - **Massive DMA Savings**: MARIA fetches **1 single header and 1 stream of pixels** instead of 3 separate headers, cutting scanline DMA consumption by 50–70% and preventing scanline overruns.
3. **Automated Multi-Layer Palette Splitting [Planned Task]**: When an image exceeds single-palette color limits (e.g. 6 colors in 160A or 3+ colors in 320A), `a78gfx` will automatically separate the image into synchronized layer bitplanes, rotating and scaling each layer in lockstep and generating the composite ROM archetype.
4. **256-Byte Page Transposition**: Automatically arranges 16-line graphics across consecutive 256-byte pages in memory so MARIA's `addr + offset` DMA addressing hits the correct scanlines.
5. **Abstract Assembler Target Support (`ca65`, `dasm`, `mads`) & Binary-First Architecture**:
   - Strictly enforces a **binary-first output model**: emits raw `.bin` graphic binary slices alongside companion `.inc` metadata headers.
   - Completely eliminates assembly `.byte` dumps, preventing build-time compiler memory blowups and keeping source trees clean and fast.
   - Companion `.inc` headers define image attributes (width in pixels and bytes, height in scanlines, palette masks, binary byte size, mode) and assembler-specific binary inclusion directives:
     - `ca65`: `.incbin "file.bin"`, `.align $100`
     - `dasm`: `INCBIN "file.bin"`, `ALIGN 256`
     - `mads`: `ins "file.bin"`, `.align $100`
6. **Abstract Palette Import System & Perceptual Color Quantization (`PaletteDecoder`, `PaletteFormat`, `PaletteFile`)**:
   - **Extensible Importer Trait:** Trait-based palette decoder decoupled from file formats. Supports GIMP Palette (`.gpl`) format (exported by Aseprite, GIMP, LibreSprite), extensible for TOML, JSON, and JASC-PAL.
   - **Unified Project Palette Enforcement (`--palette <file.gpl>`):** Guarantees that all sprites and tiles across a game project are quantized against the exact same master palette.
   - **Perceptual Redmean Distance Matching:** Maps arbitrary RGB/RGBA pixel colors to the closest palette entry using human-eye weighted Euclidean distance ($2\Delta R^2 + 4\Delta G^2 + 3\Delta B^2$ with average red level sensitivity scaling).
   - **Smart Alpha vs. Foreground Preservation:**
     - Pixels with $\text{alpha} < 128$ map directly to index 0 (MARIA transparent).
     - For alpha-masked artwork, opaque pixels are restricted to matching visible sprite colors ($1 \dots 3$) to prevent black/dark foreground elements from accidentally becoming transparent holes.
     - For solid opaque images without an alpha channel, pixels match across all 4 colors ($0 \dots 3$), cleanly converting solid backgrounds to index 0.
   - **Hardware Palette Register Equates (`MariaColor` / `ATARI_7800_NTSC_PALETTE`):** Matches palette RGB entries to the canonical 256-entry Atari 7800 NTSC hardware color table and automatically emits hardware equates in the `.inc` header (`HERO_pal0_c1 = $36`, `HERO_pal0_c2 = $48`, `HERO_pal0_c3 = $0E`).
   - **Subpalette Slot Selection (`--subpalette <0..7>`):** Targets any of MARIA's 8 subpalettes and embeds the slot index into bits 7..5 of `pal_w`.
7. **Asset-Time Rotation Generation (`--angles <N>` or `--step-degrees <deg>`)**:
   - Instead of generating 360 separate images, the tool lets you specify exact discrete angular steps matching classic retro joystick or gameplay physics:
     - `--angles 8` (or `--step-degrees 45`): 8-way directional games (e.g. standard 8-way joystick aiming).
     - `--angles 16` (or `--step-degrees 22.5`): Silky-smooth *Asteroids* / *Robotron* / overhead racer steering.
     - `--angles 32` (or `--step-degrees 11.25`): Analog paddle / flight combat resolution.
   - **Single Source Canonical Image**: The artist draws only **one single canonical PNG** (e.g. facing North / $0^\circ$).
   - **Inverse-Trigonometric Sampling & PAR Correction**: Implements inverse-mapping rotation with Pixel Aspect Ratio (PAR) correction, automatically compensating for 160A mode's 2:1 non-square rectangular pixels so rotated shapes stay geometrically true on real CRT displays. *(Note: True Scale2x/EPX RotSprite contour preservation is tracked as a roadmap task).*
   - **Automatic ca65 Angle Lookup Tables**: Generates ready-to-use lookup tables:

     ```ca65
     ship_angles_lo: .byte <ship_rot_0, <ship_rot_1, <ship_rot_2 ...
     ship_angles_hi: .byte >ship_rot_0, >ship_rot_1, >ship_rot_2 ...
     ```

8. **Pre-Baked Sprite Scaling (`--scales <N>`)**:
   - Generates $N$ pre-scaled sprite variations (e.g., $100\%, 80\%, 60\%, 40\%, 20\%$) from a single source image for pseudo-3D road, flight, and scaling arcade games (*OutRun*, *Space Harrier*, *After Burner* style).
   - Generates matching scale pointer tables and pre-computed header widths.
9. **Hardware Flip Emulation (`--flip-x`, `--flip-y`)**:
   - MARIA lacks hardware sprite mirroring bits. `a78gfx` automatically generates horizontal and vertical flipped sprite variants in ROM, so character turnaround animations cost zero manual artwork time.
10. **Multi-Frame Animation Sequences (PNG Series / Spritesheets)**:
   - Supports animation at $0^\circ$ (and optionally per-angle animations) by accepting either:
     - A sequence of numbered PNG files (e.g., `walk_00.png`, `walk_01.png`, `walk_02.png` or `--frames <glob/list>`), or
     - A horizontal/vertical sprite sheet with `--frame-width` and `--frame-height`.
   - Generates pre-strided ROM tables for each frame step and emits ca65 frame lookup tables:

     ```ca65
     sprite_anim_frame_0: .byte ...
     sprite_anim_frame_1: .byte ...
     sprite_anim_table_lo: .byte <sprite_anim_frame_0, <sprite_anim_frame_1 ...
     sprite_anim_table_hi: .byte >sprite_anim_frame_0, >sprite_anim_frame_1 ...
     ```

   - In 6502 code, indexing an animation frame at $0^\circ$ is identical in CPU cost to indexing an angle or scale (single `LDA table,X` in 4 cycles).

### 6.3 Flag-Driven Shape & Orientation Management in `libmaria`

In the game entity model, rotation, scale, and frame selection are controlled by `SpriteObject::flags`, `SpriteObject::scale`, and `SpriteObject::angle`:

```ca65
.struct SpriteObject
    x_pos       .byte   ; Horizontal coordinate (0..159)
    y_pos       .byte   ; Vertical scanline coordinate (0..239)
    type_id     .byte   ; Entity archetype (player, alien, bullet)
    angle       .byte   ; Direction index (e.g. 0..7 or 0..15)
    scale       .byte   ; Scale step index (e.g. 0..4)
    frame       .byte   ; Current animation frame step
    flags       .byte   ; Bit 7: Active, Bit 6: Visible, Bit 1: Flip X, Bit 0: Flip Y
.endstruct
```

When `DrawObject` runs:

1. It combines `type_id`, `scale`, and `angle` to resolve the exact ROM graphic pointer in 8 CPU cycles:

   ```ca65
   ldx obj_angle,y
   lda (angle_table_lo),x
   sta dl_header + MariaDLHeader::addr_lo
   lda (angle_table_hi),x
   sta dl_header + MariaDLHeader::addr_hi
   ```

2. The game developer only ever modifies `angle`, `scale`, or `flags`. The library seamlessly switches the underlying pre-baked shape in ROM, keeping game code ultra-clean with zero runtime math!

---

## 7. Architectural Comparison

| **Dimension** | 1980s Monolithic DASM Games | Modern `libmaria` + ca65 |
| :--- | :--- | :--- |
| **Toolchain** | Single monolithic `.asm` file, hardcoded `ORG` | Modular `.s` modules, relocatable `ld65` segments, clean Makefiles |
| **Data Layout** | Magic hex numbers, manual offsets | Hardware-accurate `.struct` definitions (`MariaDLLEntry`, `MariaDLHeader`) |
| **Sprite Positioning** | Manual zone math hardcoded in enemy AI | Clean $(X, Y)$ entity abstraction with automatic zone splitting |
| **Audio Separation** | Intertwined custom raster hacks | Decoupled from graphics engine; managed by dedicated external sound drivers |
| **Asset Pipeline** | Hand-typed hex matrices | Modern Indexed PNG converter via Rust CLI |
| **RAM Footprint** | Custom per-game hacks | Standardized $< 1\text{ KB}$ footprint, $> 3\text{ KB}$ free for games |

---

## 8. `libmaria` Development Roadmap: Multi-Sprite Transformation & Scaling Demos

> [!NOTE]
> The combined audio and video demo featuring the YM2149 chiptune sound engine belongs in the **`ym2149-tools`** repository. This repository focuses strictly on the MARIA graphics coprocessor, toolchain utilities (`a78tool`, `a78gfx`), and the `libmaria` display engine.

### Visual Objectives

- Cosmic space background with starfield.
- Player ship in the center (multi-color 160A sprite with pre-baked 16-angle rotation).
- Objects / asteroids scaling smoothly toward the camera across pre-baked scale steps.
- Smooth zone transitions and boundary splitting with zero tearing or sprite flicker.
- Razor-sharp 320-mode HUD status line at the top.

### Development Steps

1. **Step 1: `a78gfx` (Rust Asset Pipeline)**:
   - Indexed PNG loader and GIMP palette (`.gpl`) importer with perceptual quantization.
   - Pre-baked scaling (`--scales <N>`), discrete rotations (`--angles <N>`), and flips (`--flip-x`).
   - 256-byte page-strided transposition and ca65 assembly header emitter.
2. **Step 2: Core `libmaria` 6502 Engine**:
   - Double-buffered zone allocator in RAM ($< 1\text{ KB}$ footprint).
   - `DrawSprite(x, y, scale, angle, type_id)` with automated zone boundary splitting.
3. **Step 3: Verification & Packaging**:
   - Assemble with `ca65`/`ld65`, sign with `7800sign`, and package with `a78tool`.
   - Boot and verify in both `test7800` and `a7800` (MAME).

---

## 9. Master Implementation Checklist & Milestones

### Phase 0: Foundations & Minimal POC (`examples/pixel`)

- [x] Document MARIA hardware mechanics, DMA limits, and `libmaria` architecture.
- [x] Configure workspace `Cargo.toml` for `a78tool` and `a78gfx`.
- [x] Implement minimal ca65 proof-of-concept (`examples/pixel`):
  - [x] Direct DL / DLL structures (`include/maria.inc`).
  - [x] Single 1-scanline zone render (center green dot at scanline 120, HPOS 79).
  - [x] Linker config (`7800.cfg`), signature, and `.a78` ROM generation via `a78tool`.

---

### Phase 0.5: Repository Architecture, Tooling, & Code Hygiene

- [ ] **Task 0.5.1: Centralize `maria.inc` into shared root directory**:
  - *Context*: `maria.inc` is currently triplicated byte-for-byte across `examples/pixel/include/maria.inc`, `examples/static_sprite/include/maria.inc`, and `examples/transform_sprite/include/maria.inc`.
  - *Action*: Create `include/maria.inc` at the repository root; update example Makefiles (`-I ../../include`) to include the shared header; remove duplicate subdirectories.
- [x] **Task 0.5.2: Remove obsolete duplicate documentation**:
  - *Context*: `examples/pixel/MARIA_ARCHITECTURE.md` was an outdated, truncated duplicate of `MariaArchitecture.md`.
  - *Action*: Verified all content was represented in `docs/` and deleted the duplicate to prevent doc drift.
- [ ] **Task 0.5.3: Harmonize struct definitions and add high-level object models to `maria.inc`**:
  - *Context*: Document specifies `TMariaDLLEntry`, `TMariaDLHeaderDirect`, `TMariaDLHeaderExtended`, `TMariaDLTerminator`.
  - *Action*: Add alias equates if needed, and define high-level `GameObject` and `SpriteObject` structs in `maria.inc` so game and engine code have standard structures.
- [ ] **Task 0.5.4: Standardize visible scanline count conventions**:
  - *Context*: `examples/pixel/main.s` partitions 242 lines across 17 zone entries, while `static_sprite` and `transform_sprite` partition 240 lines across 15 zones (16 lines each).
  - *Action*: Standardize on canonical 240-line (15-zone $\times$ 16 lines) or 242-line NTSC display lists across all sample code, documenting the trade-off.

---

### Phase 1: `a78gfx` Asset Pipeline (Rust CLI)

#### Completed Features:
- [x] Indexed PNG parsing (160A 4-color palettes, 320 modes).
- [x] 256-byte page-strided scanline transposition for MARIA hardware DMA (`addr + offset`).
- [x] Discrete inverse-trigonometric rotation generator (`--angles <N>`, 2:1 pixel aspect ratio correction).
- [x] Pre-baked discrete scaling generator (`--scales <N>`, `scale_centered`).
- [x] Horizontal and vertical sprite flipping (`--flip-x`, `--flip-y`).
- [x] Binary-first output pipeline: emits `.bin` slices alongside companion `.inc` metadata headers.
- [x] Abstract assembler target backend (`AssemblerWriter` trait supporting `ca65`, `dasm`, and `mads`).
- [x] Format-agnostic image input abstraction (`GfxFile` / `GraphicDecoder` / `GfxFormat`).
- [x] Abstract palette import system (`PaletteFile` / `PaletteDecoder` / `PaletteFormat`).
- [x] GIMP Palette (`.gpl`) format decoder for Aseprite, GIMP, and LibreSprite workflows.
- [x] Perceptual redmean color quantization and nearest color matching.
- [x] Hardware color table and register equates generation (`MariaColor`, `ATARI_7800_NTSC_PALETTE`).
- [x] Subpalette slot selection (`--subpalette <0..7>`) and DL header `pal_w` bit embedding.

#### Pending Tasks:
- [ ] **Task 1.1: Multi-Page Set Banking in `MariaSpriteBank` ($N \times 256$-byte pages)**:
  - *Context*: In `MariaSpriteBank::add_frame`, total frame width across all banked frames is capped at 256 bytes (`next_end > 256`). For a 16x16 sprite (4 bytes/line in 160A), this limits the bank to 64 frames. Combining 16 rotations $\times$ 5 scales (80 frames = 320 bytes) or larger sprites causes a fatal conversion error.
  - *Action*: Extend `MariaSpriteBank` to allocate multiple 256-byte page blocks (`page_sets: Vec<Vec<Vec<u8>>>`), track which page-set base address (`addr_hi`) each frame belongs to, and emit corresponding `_bank_pages_0`, `_bank_pages_1` symbols and table arrays (`bank_frame_hi: .byte >page_0, >page_1...`).
- [ ] **Task 1.2: 2D Transformation Lookup Matrix Generation ($Angle \times Scale$)**:
  - *Context*: When both `--angles` and `--scales` are passed to `a78gfx bake`, scale frames are currently concatenated flat at the end of `bank.frames`.
  - *Action*: Generate a two-dimensional matrix of frames ($N_{angles} \times M_{scales}$), emitting structured 2D ca65 lookup tables indexed by `(angle * num_scales) + scale_idx` or parallel 2D pointer arrays.
- [ ] **Task 1.3: Compile-Time Image Combining & Layer Flattening (`--combine <files...>` / `--flatten`)**:
  - *Context*: Artists drawing multi-layer modular sprites (hull, cockpit, weapons) currently produce multiple headers that multiply scanline DMA overhead.
  - *Action*: Implement composite layering in `a78gfx` that blends multiple PNGs into a single unified `IndexedGraphic` raster prior to rotation/scale baking, eliminating extra headers.
- [ ] **Task 1.4: Automated Multi-Layer 320-Mode Palette Decomposition**:
  - *Context*: Single 320A objects are 1-bit (transparent + 1 foreground color). Rich multi-color 320 sprites require multiple overlaid headers.
  - *Action*: Automatically decompose RGBA source artwork into synchronized layer bitplanes (Layer 0, Layer 1, Layer 2), baking them into multi-layer ROM archetypes.
- [ ] **Task 1.5: True RotSprite Algorithm Implementation**:
  - *Context*: Current rotation uses nearest-neighbor sampling with PAR correction. True RotSprite applies Scale2x / EPX pattern matching to preserve sharp pixel art contours and prevent stepping artifacts at non-orthogonal angles.
  - *Action*: Implement an optional `--rotsprite` algorithm flag in `IndexedGraphic::rotate`.

---

### Phase 1.5: Static Multi-Line Sprite Demo (`examples/static_sprite`)

- [x] Single 16x16 4-color spaceship centered in Zone 7 (`assets/ship.png`, `ship.gpl`).
- [x] 256-byte page-strided scanline alignment (`ship_pages`).
- [x] 100% ROM-resident Display List (`dl_ship` with direct header + terminator) costing 0 bytes RAM.
- [x] Build automated via Makefile, signed via `7800sign`, packaged via `a78tool`.

---

### Phase 2: Single-Sprite Transformation Demo (`examples/transform_sprite`)

*Objective: Load a PNG, center it on screen, and continuously cycle through discrete pre-baked rotations and scale steps in a loop without full game logic.*

#### Completed Work:
- [x] Canonical source asset (`assets/ship.png` - 16x16 4-color indexed sprite).
- [x] Process asset through `a78gfx bake` with 16 discrete rotations and 2:1 PAR compensation (`ship_bank.bin`, `ship_bank.inc`).
- [x] Dynamic RAM Display List at `$1800` updated during VBLANK (16 CPU cycles per update).
- [x] 60 Hz frame synchronization via `MSTAT` VBLANK polling.
- [x] Build automated via Makefile and verified in `test7800` and `a7800`.

#### Pending Tasks:
- [ ] **Task 2.1: True Double-Buffered DL Zone Allocation (`dl_buffer_0` / `dl_buffer_1`)**:
  - *Context*: Currently uses a single 6-byte buffer `dl_ship` at `$1800` updated directly during VBLANK.
  - *Action*: Allocate two separate RAM buffers (`dl_buffer_0` at `$1900`, `dl_buffer_1` at `$1A00`); swap the zone's DL pointer or DLL base during VBLANK; construct the next frame in the inactive buffer.
- [ ] **Task 2.2: Discrete Scale Steps Frame Loop Integration**:
  - *Context*: Makefile bakes only 16 rotations (`--angles 16`). Scale steps are not baked or animated.
  - *Action*: Bake scale variations using `a78gfx bake` (e.g. `--scales 0.5,0.75,1.0,1.25` or combined matrix); add `scale_index` and `scale_timer` in 6502 Zero Page; animate ship pulsing / scaling while rotating.
- [ ] **Task 2.3: Assembly Code Quality & Struct Offset Cleanup**:
  - *Context*: [examples/transform_sprite/main.s](file:///home/john/Projects/lokey-7800-tools/examples/transform_sprite/main.s#L86-L88) hardcodes magic offsets `dl_ship + 4` and `dl_ship + 5` for terminator bytes.
  - *Action*: Replace hardcoded byte offsets with `.sizeof(TMariaDLHeaderDirect)` and `TMariaDLTerminator::term0/term1` symbolic struct members.

---

### Phase 3: Classic "Atari ST Dev Kit Bug" Demo (`examples/bug_ram`)

*Objective: Recreate the iconic bouncing bug sample from the Atari ST development kit, demonstrating dynamic positioning / frame buffer / multi-zone rendering on MARIA.*

- [x] **Task 3.1: Asset Preparation**:
  - Convert classic Atari ST beetle/bug artwork for 160A 4-color mode and 320A high-res overlay mode using `a78gfx`.
  - *Status*: 160A done (indexed PNGs decoded from `bugs-st/IMAGES.S`). 320A overlay not done.
- [x] **Task 3.2: 6502 Bouncing Physics & Coordinate Simulation**:
  - Implement 8.8 fixed-point $(X, Y)$ position and $(V_x, V_y)$ velocity logic.
  - Implement screen edge collision detection and bounce deflection ($160 \times 240$ boundaries).
  - *Status*: Ported from `upd_gravity` using a 160×200 field to match the ST screen.
- [x] **Task 3.3: Multi-Zone Display List Generation & Dynamic Traversal**:
  - Dynamically compute which 16-scanline zone(s) the bug occupies based on `y_pos`.
  - Handle smooth vertical traversal across zone boundaries.
- [ ] **Task 3.4: Build, Sign, and Emulator Verification**:
  - Package with `a78tool` and verify tear-free animation in `test7800` and `a7800`.
  - *Status*: Builds and signs. Emulator verification pending.

---

### Phase 4: Core `libmaria` 6502 Engine Modularization

> [!NOTE]
> Detailed design specifications, dynamic pool memory layout, and implementation tasks for extracting `libmaria` using `examples/bug_ram` as the driving testbed are detailed in [docs/LIBMARIA_PLAN.md](libMariaPlan.md).

- [ ] **Task 4.1: Strict Static (ROM) vs. Dynamic (RAM) Separation**:
  - Wire static HUD elements and borders directly from ROM via DLL zone pointers (0 bytes RAM overhead).
- [ ] **Task 4.2: Structure of Arrays (SoA) Object Management**:
  - Implement parallel arrays indexed by entity slot: `obj_x`, `obj_y`, `obj_rom_ptr_lo`, `obj_rom_ptr_hi`, `obj_flags` (240 bytes RAM for 48 active entities).
- [ ] **Task 4.3: Fast ROM-to-RAM Blast Copy Loop for DL Emission**:
  - Dereference entity `rom_ptr` through Zero Page, blast-copy pre-baked 4-byte headers directly into RAM Display List, and write `hpos = obj_x`.
- [ ] **Task 4.4: Composite Archetype Expansion**:
  - Support multi-part / multi-layer entities where 1 Game Object expands into $N$ MARIA DL headers (e.g. 2-layer 320 mode composite).
- [ ] **Task 4.5: Dynamic Single Continuous RAM Display List Pool ($1900 \dots $1B7F)**:
  - Implement bump allocator pointer `dl_write_ptr` across double-buffered pools (`Buffer_A` and `Buffer_B`).
  - Auto-emit `$00, $00` terminators per active zone.
- [ ] **Task 4.6: Automated Vertical Zone Boundary Splitting Routine**:
  - Calculate remaining lines in Zone $N$ ($\text{top\_h} = \text{ZoneEnd} - Y$).
  - Emit Header 1 into Zone $N$ (pointing to `Graphic_Base` with height = `top_h`).
  - Emit Header 2 into Zone $N+1$ (pointing to `Graphic_Base + top_h` with height = `height - top_h`).
- [ ] **Task 4.7: Modular ca65 Library Interface**:
  - Package engine as a clean reusable library module (`libmaria.s` and `maria.inc`).

---

### Phase 5: Multi-Sprite Scaling & Starfield Demo

- [ ] **Task 5.1: Multi-Object Starfield Backdrop**:
  - Implement pseudo-3D starfield with depth-based scrolling speed.
- [ ] **Task 5.2: Depth-Sorted Multi-Sprite Scaling**:
  - Manage multiple simultaneous entities scaling toward/away from the camera using pre-baked ROM scale tables.
- [ ] **Task 5.3: Scanline DMA Budget Verification**:
  - Profile worst-case scanline density to verify DMA margin remains within $\approx 430$ color cycles.

---

### Future Phase / TODO: Multi-Frame Animation & Viewport Scrolling

- [ ] **Task F.1: `a78gfx` Multi-Frame Animation Support**:
  - Support numbered PNG series (`--frames <glob/list>`) and spritesheets (`--frame-width`, `--frame-height`).
  - Generate sequential page-strided ROM tables and frame pointer arrays (`sprite_anim_table_lo/hi`).
- [ ] **Task F.2: Animation Integration in `libmaria`**:
  - Integrate `SpriteObject::frame` into the render pipeline.
- [ ] **Task F.3: Viewport Camera & Scrolling Exploration**:
  - Test horizontal ribbon clipping ($X < 0$ and $X > 160$).
  - Test vertical fine scrolling via DLL zone truncation ($(16 - F)$ scanline trick).
