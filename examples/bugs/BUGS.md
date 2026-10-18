# Silly Bugs Demo (Atari ST) — Physics & Motion Specification

> **SOURCE:** This is a sample from the original **Atari ST development kit** — `bugs-st/BUGS.S`, dated 30-Nov-1986, by `lmd`. This repository ports it to the Atari 7800 (see [`examples/bugs`](.) for the hand-built display-list port, and [`examples/bugs-lokey`](../bugs-lokey) for the `lokeydll` library version).

This document provides a technical explanation of the physics simulation, coordinate arithmetic, and trajectory calculations implemented in `BUGS.S`.

---

## 1. Overview of the Simulation

The demo simulates **16 concurrent autonomous sprites ("bugs")** bouncing within the screen boundaries under constant downward gravity.

- **Timing Model:** Frame-locked discrete simulation. The main loop synchronizes to the display refresh via TOS `Vsync()` (GEMDOS/XBIOS `trap #14`, call `$25`), advancing the physics by one discrete time step $\Delta t = 1$ per vertical blank (~50 Hz PAL / ~60 Hz NTSC).
- **Kinematics:** 2D Newtonian particle kinematics with semi-implicit Euler integration, uniform vertical gravity ($g$), zero horizontal acceleration ($a_x = 0$), and perfectly elastic specular boundary reflections ($e = 1.0$).

---

## 2. Fixed-Point Arithmetic (16.16 Format)

Because the Motorola 68000 lacks hardware floating-point capabilities and integer coordinates alone would cause severe discretization errors in acceleration, all positions and velocities are stored as **signed 32-bit fixed-point numbers** in **16.16 format**:

```
 31                          16 15                           0
+------------------------------+------------------------------+
|   Signed Integer (16 bits)   |  Fractional Part (16 bits)   |
+------------------------------+------------------------------+
```

- **Integer Part (bits 16–31):** The integral pixel coordinate on screen.
- **Fractional Part (bits 0–15):** The sub-pixel accumulator in units of $\frac{1}{65536} \approx 1.5259 \times 10^{-5}$ pixels.

### Big-Endian Representation in 68000 Memory

On the big-endian MC68000, the high word resides at the lower byte address:

- Reading or writing `BGxpos(a5)` as a word (`.w`) accesses the **integer pixel coordinate**.
- Reading or writing `BGxpos(a5)` as a longword (`.l`) accesses the **full 16.16 fixed-point value**.

This design enables zero-cycle truncation: routines requiring screen pixel positions (such as raster address calculation) read `BGxpos` and `BGypos` directly using `.w` instructions without needing bit-shifts.

---

## 3. Physical Constants & Parameters

The demo configures the following physical values (`BUGS.S`):

| Variable / Parameter | Assembly Hex Value | Decimal Equivalent | Interpretation |
| :--- | :--- | :--- | :--- |
| `gravity` | `$00004000` | $0.25$ | Downward vertical acceleration ($g = 16384 / 65536\text{ px/frame}^2$) |
| Initial $v_x$ (`i_xvel`) | `$00008000`<br>`$0000c000`<br>`$00010000`<br>`$00014000` | $0.50\text{ px/frame}$<br>$0.75\text{ px/frame}$<br>$1.00\text{ px/frame}$<br>$1.25\text{ px/frame}$ | Horizontal velocity assigned per bug across 4 speed variations |
| Initial $v_y$ (`BGyvel`) | `$00000000` | $0.00\text{ px/frame}$ | Initial vertical velocity (cleared BSS memory) |

---

## 4. Discrete Integration Step (`upd_gravity`)

Every vertical blank, `upd_gravity` executes for each bug (pointer in `a5`). The update follows a modified semi-implicit (symplectic) Euler scheme:

### A. Vertical Trajectory (Y Axis)

1. **Velocity Integration (Gravity):**
   $$v_y(t+1) = v_y(t) + g$$

   ```assembly
   move.l  BGyvel(a5),d0
   add.l   gravity,d0
   ```

2. **Position Integration:**
   $$y(t+1) = y(t) + v_y(t+1)$$

   ```assembly
   move.l  d0,BGyvel(a5)
   add.l   d0,BGypos(a5)
   ```

3. **Boundary Check:**
   The integer portion of the new position ($y_{\text{int}}$) is checked against screen bounds:

   ```assembly
   move.w  BGypos(a5),d0
   bpl.s   .1               ; if y < 0 -> bounce
   bra.s   .2
   .1: cmp.w   maxy,d0      ; if y >= maxy -> bounce
   blt.s   .3
   ```

   - Top bound: $y < 0$
   - Bottom bound: $y \ge \text{maxy}$ (where $\text{maxy} = 183$ in low resolution, reserving 16 pixels for the sprite height within the 200-line display).

4. **Elastic Collision & Position Restoration:**
   When a collision occurs, velocity is reversed ($v_y \leftarrow -v_y$):

   ```assembly
   .2: move.l  BGyvel(a5),d0
       neg.l   d0
       bra     .y           ; re-enter displacement loop
   ```

   **The Displacement Cancellation Mechanism:**
   Instead of clamping or projecting the position using geometric intersection math, the routine negates the velocity and branches back to `.y`.
   At label `.y`, it adds the negated velocity to `BGypos`:
   $$y_{\text{corrected}} = y(t+1) + (-v_y(t+1)) = \big(y(t) + v_y(t+1)\big) - v_y(t+1) = y(t)$$
   This immediately restores the bug to its pre-collision integer coordinate $y(t)$ inside bounds, while reversing the velocity vector for subsequent frames.

---

### B. Horizontal Trajectory (X Axis)

Because horizontal acceleration is zero, horizontal velocity remains constant except upon wall collisions:

1. **Position Integration:**
   $$x(t+1) = x(t) + v_x(t)$$

   ```assembly
   move.l  BGxvel(a5),d0
   .7: add.l   BGxpos(a5),d0
       move.l  d0,BGxpos(a5)
   ```

2. **Boundary Check:**

   ```assembly
   bmi.s   .5               ; if x < 0 -> bounce
   swap    d0               ; extract integer word to low 16 bits
   cmp.w   maxx,d0          ; if x >= maxx -> bounce
   blt.s   .6
   ```

   - Left bound: $x < 0$ (detected via sign bit with `bmi`)
   - Right bound: $x \ge \text{maxx}$ (where $\text{maxx} = 303$ in low resolution, reserving 16 pixels for the sprite width within the 320-pixel display).

3. **Elastic Rebound:**
   If out of bounds, the routine negates $v_x$ and jumps back to `.7`:

   ```assembly
   .5: move.l  BGxvel(a5),d0
       neg.l   d0
       move.l  d0,BGxvel(a5)
       bra.s   .7
   ```

   Just as with the vertical axis, adding the negated velocity $-v_x$ to $x_{\text{new}} = x(t) + v_x$ restores the position to $x(t)$ in a single step without requiring floating-point collision solvers.

---

## 5. Movement Detection & Redraw Threshold

Sub-pixel movements do not change the rasterized appearance on screen until the integer word boundary changes. To avoid redundant redraws:

```assembly
    cmp.w   BGxpos(a5),d3    ; compare new integer X with old X
    bne.s   .8               ; if changed, redraw
    tst.b   d2               ; did integer Y change? (MovedFlag)
    beq.s   .r               ; if neither changed, skip redraw
.8: bsr     undraw           ; erase sprite at old screen location
    bsr     draw             ; render sprite at new screen location
```

If neither $x_{\text{int}}$ nor $y_{\text{int}}$ shifted by at least 1 pixel, the frame update terminates immediately, preserving CPU cycles and preventing display flicker.

---

## 6. Raster Address Calculation from Coordinates

When a redraw occurs, `draw` translates the continuous integer coordinates $(x, y)$ into Atari ST video RAM byte addresses.

### Atari ST Interleaved Bitplane Layout (Low Resolution: 320×200, 4 Planes)

- Each 16 horizontal pixels are represented by 4 consecutive 16-bit words (one word per bitplane), totaling 8 bytes per 16-pixel block.
- Total bytes per scanline: $\frac{320}{16} \times 8 = 160$ bytes.

The byte offset for coordinate $(x, y)$ is calculated as:
$$\text{Offset} = (y \times \text{width}) + \left(\left\lfloor\frac{x}{16}\right\rfloor \times 8\right)$$

In 68000 assembly, the horizontal offset $\left\lfloor\frac{x}{16}\right\rfloor \times 8 = (x \gg 4) \ll 3$ is computed by shifting right by 1 and masking:

```assembly
    move.w  BGypos(a5),d0
    mulu    width,d0          ; d0 = y * 160
    move.w  BGxpos(a5),d1
    lsr.w   #1,d1             ; x >> 1
    and.w   #$fff8,d1         ; clear bottom 3 bits -> equivalent to (x >> 4) << 3
    add.w   d1,d0             ; total byte offset
    add.l   screen,d0         ; pointer to screen VRAM
```

### Sub-Word Shift Alignment

Sprites are 16×16 pixels. If $x \pmod{16} \ne 0$:

- The sprite spans across two 16-pixel screen words (spanning 32 contiguous pixel bits across 4 bitplanes).
- The fine shift index $x \pmod{16} \in [0, 15]$ indexes into pre-shifted image and mask lookup tables constructed in memory by `mksh` (`SHIFT.S`).
- If $x \pmod{16} == 0$, the fast 1-word drawer `ld1x16` is used.
- If $x \pmod{16} \ne 0$, the 2-word spanning drawer `ld2x16` is used.

---

## 7. Mathematical Summary

The continuous trajectory of each bug between bounces satisfies standard projectile equations:
$$x(t) = x_0 + v_x t$$
$$y(t) = y_0 + v_{y0} t + \frac{1}{2} g t^2$$

Subject to reflective boundary conditions on the bounding box $[0, \text{maxx}] \times [0, \text{maxy}]$:
$$v_x \leftarrow -v_x \quad \text{when } x \le 0 \text{ or } x \ge \text{maxx}$$
$$v_y \leftarrow -v_y \quad \text{when } y \le 0 \text{ or } y \ge \text{maxy}$$

Because each bug in `i_xpos`, `i_ypos`, and `i_xvel` has different initial coordinates and speeds, each follows an out-of-phase parabolic bouncing arc across the display.
