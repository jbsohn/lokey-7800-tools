; ==============================================================================
; main.s -- Silly Bugs
;
; Port of the Atari ST dev-kit demo (bugs-st/BUGS.S, 30-Nov-1986, lmd).
; Sixteen sprites fall under gravity and bounce off the screen edges.
;
; The ST sample draws masked sprites into a 320x200 framebuffer in software.
; Maria has no framebuffer, so each frame rebuilds a display list per
; 16-line zone:
;
;   * Y is in scanlines. Zone = Y / 16, fine offset k = Y & 15.
;   * The header graphics address points at the bottom scanline of the zone.
;     Maria adds the DLL offset (15, 14, ... 0) to the high byte.
;   * On a zone boundary (k = 0) one header covers the sprite.
;   * Otherwise the top zone's high byte is (base + k) and the next zone's
;     high byte is (base + k - 16). Reads that fall outside the sprite's 4KB
;     block land in an odd 4KB region, which 16-line holey DMA returns as $00.
;
; Positions and velocities are 8.8 fixed point. The ST used 16.16 on a
; 320-wide screen; HPOS is 0..159, so the X coordinates and X speeds are half
; of the ST values. Y is still one unit per scanline, so gravity is the same
; 0.25 px/frame^2. The playfield is the ST's 200 lines, 160 by 200, below one
; blank 16-line zone so it stays out of the top overscan.
;
; Sprites are 160A. The beetle keeps the ST red / gray / magenta in palette 0.
; Its two dark-gray antenna tips do not survive: each sits in a 2-pixel pair
; with a red pixel, and --par breaks the tie toward red. The circle is the same
; gray. The square is the ST tan, in palette 1. a78gfx --par halves the width
; so the square-pixel art is round on 160A's 2:1 pixels.
; ==============================================================================

.include "maria.inc"

; --- Graphics. Included first so the generated equates exist for the code. ---
.segment "GFX0"
.include "beetle.inc"

.segment "GFX1"
.include "circle.inc"

.segment "GFX2"
.include "square.inc"

NBUGS        = 16
; NTSC Maria draws rasters 16..258 (243 lines); PAL draws 293. The DLL is one
; blank zone, 13 playfield zones (208 lines, the bugs use 200), and five blank
; zones: 304 lines, so vblank arrives before Maria reads past the list on
; either standard. NTSC never reads the last three zones.
TOP_ZONES    = 1
PF_ZONES     = 13
BOTTOM_ZONES = 5
DL_SLOTS     = NBUGS            ; one header per bug in a single zone
DL_BYTES     = (DL_SLOTS * 4) + 2

; ST gravity $00004000 (16.16) = 1/4 scanline per frame.
GRAVITY      = $40

; Bounce when the coordinate reaches this value, so the last legal top-left
; is one less. That top-left plus the sprite size lands on the last playfield
; pixel (159) and the last playfield line (199).
VISIBLE_X    = 160
VISIBLE_Y    = 200
MAX_X        = VISIBLE_X - beetle_width_pixels + 1
MAX_Y        = VISIBLE_Y - beetle_height_lines + 1

.assert circle_width_pixels = beetle_width_pixels && square_width_pixels = beetle_width_pixels, error, "all sprites must share the beetle's width"
.assert circle_height_lines = beetle_height_lines && square_height_lines = beetle_height_lines, error, "all sprites must share the beetle's height"

.assert (TOP_ZONES + PF_ZONES + BOTTOM_ZONES) * 16 >= 293, error, "DLL must cover all 293 PAL lines"
.assert (MAX_Y - 1) / 16 + 1 < PF_ZONES, error, "lowest sprite must end inside the playfield zones"

; INPTCTRL: lock (bit 0), Maria on (bit 1), BIOS off (bit 2).
PTCTRL_LOCK  = $07

; CTRL: DMA bits 6-5 = 11 is DMA off. 00 and 01 are test modes, not off.
CTRL_DMA_OFF = $7F
; 160A, DMA on, transparency enabled (kangaroo off, so color 0 is not written).
CTRL_160A    = $40

; DLL bytes: a plain 16-line zone, and one with 16-line holey DMA (bit 6).
DLL_ZONE16   = $0F
DLL_HOLEY16  = $4F

; ==============================================================================
; Zero page
; ==============================================================================
.segment "ZEROPAGE"

dl_ptr:         .res 2          ; display-list write cursor
buf_base:       .res 2          ; dl_buf0 or dl_buf1
fine_y:         .res 1          ; Y & 15
spr_lo:         .res 1
spr_hi:         .res 1          ; page of the sprite's bottom line
spr_pal:        .res 1
spr_x:          .res 1
adj_hi:         .res 1          ; graphics high byte stored in the header
slot4:          .res 1          ; header byte offset within the zone
old_lo:         .res 1          ; position before this frame's velocity add
old_hi:         .res 1
build_which:    .res 1          ; 0 = build dl_buf0, 1 = build dl_buf1
zone_n:         .res PF_ZONES   ; headers queued in each zone this frame

; ==============================================================================
; RAM
; ==============================================================================
.segment "BSS"

; Buffer 0. Each zone is DL_SLOTS headers plus a 2-byte terminator.
dl_buf0:        .res PF_ZONES * DL_BYTES

bug_x_lo:       .res NBUGS
bug_x_hi:       .res NBUGS
bug_y_lo:       .res NBUGS
bug_y_hi:       .res NBUGS
bug_vx_lo:      .res NBUGS
bug_vx_hi:      .res NBUGS
bug_vy_lo:      .res NBUGS
bug_vy_hi:      .res NBUGS
bug_img:        .res NBUGS      ; 0 beetle, 1 circle, 2 square

.segment "DLBUF1"

dl_buf1:        .res PF_ZONES * DL_BYTES

; ==============================================================================
; Code
; ==============================================================================
.segment "CODE"

reset:
        sei
        cld
        ldx #$FF
        txs

        ; Disable the BIOS, enable the cartridge and Maria, and lock.
        lda #PTCTRL_LOCK
        sta PTCTRL

        lda #CTRL_DMA_OFF
        sta CTRL
        lda #$00
        sta BKGRND

        ; Palette 0: beetle and circle. Palette 1: tan square, which only
        ; uses color 1, so P1C2 and P1C3 are left alone.
        lda #beetle_pal0_c1
        sta P0C1
        lda #beetle_pal0_c2
        sta P0C2
        lda #beetle_pal0_c3
        sta P0C3
        lda #square_pal1_c1
        sta P1C1

        jsr init_bugs

        ; Draw the opening frame into buffer 0, then show it.
        lda #$00
        sta build_which
        jsr build_frame

        lda #>dll_0
        sta DPPH
        lda #<dll_0
        sta DPPL
        lda #$01
        sta build_which

        ; Turn DMA on at the start of vblank, so Maria's first frame reads dll_0.
@wait_vbi:
        bit MSTAT
        bpl @wait_vbi
        lda #CTRL_160A
        sta CTRL

; ------------------------------------------------------------------------------
; One physics step and one display-list build per frame. The list is written
; into the buffer Maria is not reading, and published at vblank.
; ------------------------------------------------------------------------------
main_loop:
@wait_visible:
        bit MSTAT
        bmi @wait_visible

        jsr physics
        jsr build_frame

@wait_vblank:
        bit MSTAT
        bpl @wait_vblank

        lda build_which
        bne @show1
        lda #>dll_0
        sta DPPH
        lda #<dll_0
        sta DPPL
        lda #$01
        sta build_which
        jmp main_loop
@show1:
        lda #>dll_1
        sta DPPH
        lda #<dll_1
        sta DPPL
        lda #$00
        sta build_which
        jmp main_loop

; ------------------------------------------------------------------------------
; Initial state. X is half of the ST table (320 -> 160). Y and the image
; numbers are the ST values. X velocity is half of the ST 16.16 speeds,
; stored as 8.8. Y velocity starts at 0; gravity creates it.
; ------------------------------------------------------------------------------
init_bugs:
        ldy #$00
@init:
        lda init_x,y
        sta bug_x_hi,y
        lda init_y,y
        sta bug_y_hi,y
        lda init_vx,y
        sta bug_vx_lo,y
        lda init_img,y
        sta bug_img,y
        lda #$00
        sta bug_x_lo,y
        sta bug_y_lo,y
        sta bug_vx_hi,y
        sta bug_vy_lo,y
        sta bug_vy_hi,y
        iny
        cpy #NBUGS
        bne @init
        rts

init_x:
        .byte 0, 8, 16, 24, 32, 40, 48, 56
        .byte 64, 72, 80, 88, 96, 104, 112, 120

init_y:
        .byte $00, $08, $10, $18, $20, $28, $30, $38
        .byte $40, $48, $30, $38, $20, $28, $10, $18

init_img:
        .byte 0, 1, 2, 0, 0, 1, 2, 0
        .byte 0, 1, 2, 0, 0, 1, 2, 0

; 0.25, 0.375, 0.5, 0.625 in 8.8. See the header for the ST values.
init_vx:
        .byte $40, $60, $80, $A0, $40, $60, $80, $40
        .byte $A0, $60, $80, $40, $60, $A0, $80, $A0

; ------------------------------------------------------------------------------
; Physics. Mirrors upd_gravity in BUGS.S. Called once per frame (60 Hz NTSC).
;
; Units: position and velocity are 8.8 fixed point, high byte = whole pixels
; (X, 0..159) or scanlines (Y, 0..199), low byte = 1/256ths. Velocity is in
; units per frame. Positions are unsigned; velocities are signed two's
; complement (-1.5 = $FE80), so neg_vx/neg_vy negate with EOR #$FF, +1, and
; one 16-bit ADC moves a bug either way.
;
; Each bug, each frame:
;   Y: vy += GRAVITY (1/4 line per frame^2), then y += vy. Gravity is added
;      before the move (semi-implicit Euler), as on the ST.
;   X: x += vx. No forces on X, so vx only ever changes sign.
;   Bounce: if the new integer coordinate is outside [0, MAX), negate that
;      axis's velocity and restore the position from before the move. The
;      ST instead added the negated velocity and retried in a loop; restoring
;      gives the same result without spinning forever on a zero velocity.
;
; The bounce is elastic: nothing damps the velocity, and the fall and the rise
; replay the same steps in reverse, so a bug that starts at rest returns to
; exactly its starting line every bounce. X speed never changes. The floor is
; MAX_Y (sprite bottom on line 199). The ceiling (0) is never reached from the
; init table, since no bug rises above where it started. The peak speed, for
; a bug dropped from line 0, is 9.5 lines per frame.
; ------------------------------------------------------------------------------
physics:
        ldy #$00
@each:
        jsr update_bug
        iny
        cpy #NBUGS
        bne @each
        rts

update_bug:
        lda bug_y_lo,y
        sta old_lo
        lda bug_y_hi,y
        sta old_hi

        clc
        lda bug_vy_lo,y
        adc #GRAVITY
        sta bug_vy_lo,y
        lda bug_vy_hi,y
        adc #$00
        sta bug_vy_hi,y

        clc
        lda bug_y_lo,y
        adc bug_vy_lo,y
        sta bug_y_lo,y
        lda bug_y_hi,y
        adc bug_vy_hi,y
        sta bug_y_hi,y

        ; Unsigned compare. |vy| stays under 11 lines per frame, so a step
        ; below zero wraps to $F5..$FF, which is still at or above MAX_Y.
        cmp #MAX_Y
        bcc @yok
        jsr neg_vy
        lda old_lo
        sta bug_y_lo,y
        lda old_hi
        sta bug_y_hi,y
@yok:
        lda bug_x_lo,y
        sta old_lo
        lda bug_x_hi,y
        sta old_hi

        clc
        lda bug_x_lo,y
        adc bug_vx_lo,y
        sta bug_x_lo,y
        lda bug_x_hi,y
        adc bug_vx_hi,y
        sta bug_x_hi,y

        cmp #MAX_X
        bcc @xok
        jsr neg_vx
        lda old_lo
        sta bug_x_lo,y
        lda old_hi
        sta bug_x_hi,y
@xok:
        rts

neg_vy:
        lda bug_vy_lo,y
        eor #$FF
        clc
        adc #$01
        sta bug_vy_lo,y
        lda bug_vy_hi,y
        eor #$FF
        adc #$00
        sta bug_vy_hi,y
        rts

neg_vx:
        lda bug_vx_lo,y
        eor #$FF
        clc
        adc #$01
        sta bug_vx_lo,y
        lda bug_vx_hi,y
        eor #$FF
        adc #$00
        sta bug_vx_hi,y
        rts

; ------------------------------------------------------------------------------
; Build the hidden display-list buffer from the current bug positions.
; ------------------------------------------------------------------------------
build_frame:
        jsr set_buf_base
        jsr clear_counts
        ldy #$00
@bug:
        jsr emit_bug
        iny
        cpy #NBUGS
        bne @bug
        jmp write_terminators

set_buf_base:
        lda build_which
        bne @buf1
        lda #<dl_buf0
        sta buf_base
        lda #>dl_buf0
        sta buf_base+1
        rts
@buf1:
        lda #<dl_buf1
        sta buf_base
        lda #>dl_buf1
        sta buf_base+1
        rts

clear_counts:
        ldx #PF_ZONES-1
        lda #$00
@clr:
        sta zone_n,x
        dex
        bpl @clr
        rts

; Y = bug index, preserved. Emits one header, or two when the sprite crosses
; a zone boundary.
emit_bug:
        lda bug_x_hi,y
        sta spr_x

        ldx bug_img,y
        lda img_lo,x
        sta spr_lo
        lda img_hi,x
        sta spr_hi
        lda img_pal,x
        sta spr_pal

        lda bug_y_hi,y
        and #$0F
        sta fine_y

        ; Top zone. Y < MAX_Y, so it and the zone below are both playfield
        ; zones (see the .assert at the top).
        lda bug_y_hi,y
        lsr a
        lsr a
        lsr a
        lsr a
        tax

        ; Top (or only) piece. addr_hi = base + k, so the zone's countdown
        ; reaches the top sprite line on scanline k.
        lda spr_hi
        clc
        adc fine_y
        sta adj_hi
        jsr emit_header

        lda fine_y
        beq @done

        ; Bottom piece, in the next zone. addr_hi = base + k - 16.
        inx
        lda adj_hi
        sec
        sbc #16
        sta adj_hi
        jsr emit_header
@done:
        rts

; X = zone. Writes one 4-byte direct header and reserves the slot. A zone
; never overflows: each bug adds at most one header to it, and DL_SLOTS = NBUGS.
; Preserves X and Y.
emit_header:
        lda zone_n,x
        asl a
        asl a
        sta slot4
        inc zone_n,x
        jsr zone_addr

        tya
        pha
        ldy #$00
        lda spr_lo
        sta (dl_ptr),y
        iny
        lda spr_pal
        sta (dl_ptr),y
        iny
        lda adj_hi
        sta (dl_ptr),y
        iny
        lda spr_x
        sta (dl_ptr),y
        pla
        tay
        rts

; dl_ptr = buf_base + zone_off[X] + slot4. Preserves X and Y.
zone_addr:
        lda zone_off_lo,x
        clc
        adc slot4
        sta dl_ptr
        lda zone_off_hi,x
        adc #$00
        sta dl_ptr+1

        lda dl_ptr
        clc
        adc buf_base
        sta dl_ptr
        lda dl_ptr+1
        adc buf_base+1
        sta dl_ptr+1
        rts

write_terminators:
        ldx #$00
@zone:
        lda zone_n,x
        asl a
        asl a
        sta slot4
        jsr zone_addr
        lda #$00
        tay
        sta (dl_ptr),y
        iny
        sta (dl_ptr),y
        inx
        cpx #PF_ZONES
        bne @zone
        rts

; No DLL entry sets the interrupt bit, so neither vector should fire. If one
; does, return and leave the frame running.
nmi:
irq:
        rti

; ==============================================================================
; ROM tables and the two display-list lists
; ==============================================================================
.segment "RODATA"

img_lo:
        .byte <beetle_pages, <circle_pages, <square_pages
img_hi:
        .byte >beetle_pages, >circle_pages, >square_pages
img_pal:
        .byte beetle_pal_w, circle_pal_w, square_pal_w

zone_off_lo:
        .repeat PF_ZONES, I
            .byte <(I * DL_BYTES)
        .endrepeat

zone_off_hi:
        .repeat PF_ZONES, I
            .byte >(I * DL_BYTES)
        .endrepeat

; Blank zones point here.
dl_empty:
        .byte $00, $00

; Sixteen 16-line zones: blank, 13 playfield, blank, blank. Maria stops
; during the last zone (raster 258), so nothing past this list is read.
.macro dll_for buf
        .repeat TOP_ZONES
            .byte DLL_ZONE16, >dl_empty, <dl_empty
        .endrepeat
        .repeat PF_ZONES, I
            .byte DLL_HOLEY16
            .byte >(buf + (I * DL_BYTES))
            .byte <(buf + (I * DL_BYTES))
        .endrepeat
        .repeat BOTTOM_ZONES
            .byte DLL_ZONE16, >dl_empty, <dl_empty
        .endrepeat
.endmacro

dll_0:
        dll_for dl_buf0

dll_1:
        dll_for dl_buf1

; ==============================================================================
; Vectors. $FFF9 = $83 selects a hash range that starts at $8000.
; ==============================================================================
.segment "VECTORS"
        .byte $FF, $83
        .word nmi
        .word reset
        .word irq
