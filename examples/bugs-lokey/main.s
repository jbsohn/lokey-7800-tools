; ==============================================================================
; main.s -- Silly Bugs
;
; Same motion as examples/bugs. A bug is a lokeydll object: whole-pixel X
; and Y in the object table, plus a picture attached with
; lokeydll_set_picture. Velocity and the fractional byte stay here. The
; pictures are the a78gfx files in LOKEYDLL_GFX0..2. lokeydll puts the
; objects on the LOKEYDLL_VIEW_W by LOKEYDLL_VIEW_H playfield.
;
; Positions and velocities are 8.8 fixed point. The ST sample was 16.16 on a
; 320-wide screen, so X and the X speeds are half of the ST values. Y is one
; unit per scanline. Gravity is a quarter pixel per frame ($40).
; ==============================================================================

.include "lokeydll.inc"

.segment "LOKEYDLL_GFX0"
.include "beetle.inc"

.segment "LOKEYDLL_GFX1"
.include "circle.inc"

.segment "LOKEYDLL_GFX2"
.include "square.inc"

NBUGS     = 32
SPRITE_W  = 8
SPRITE_H  = 16
MAX_X     = LOKEYDLL_VIEW_W - SPRITE_W + 1
MAX_Y     = LOKEYDLL_VIEW_H - SPRITE_H + 1
GRAVITY   = $40

.assert beetle_width_pixels = SPRITE_W && beetle_height_lines = SPRITE_H, error, "beetle is not 8x16"
.assert circle_width_pixels = SPRITE_W && circle_height_lines = SPRITE_H, error, "circle is not 8x16"
.assert square_width_pixels = SPRITE_W && square_height_lines = SPRITE_H, error, "square is not 8x16"
.assert LOKEYDLL_MAX_SPRITES >= NBUGS, error, "lokeydll object table is smaller than the bug table"

.segment "ZEROPAGE"
old_lo: .res 1
old_hi: .res 1

.segment "BSS"
bug_x_lo: .res NBUGS
bug_y_lo: .res NBUGS
bug_vx_lo: .res NBUGS
bug_vx_hi: .res NBUGS
bug_vy_lo: .res NBUGS
bug_vy_hi: .res NBUGS

.segment "CODE"

.export lokeydll_game
lokeydll_game:
        lda #<colors
        sta lokeydll_param_ptr
        lda #>colors
        sta lokeydll_param_ptr+1
        jsr lokeydll_load_colors

        jsr init_bugs
        jsr lokeydll_build
        jsr lokeydll_swap_buffers

main_loop:
        jsr physics
        jsr lokeydll_build
        jsr lokeydll_swap_buffers
        jmp main_loop

; ------------------------------------------------------------------------------
; One object per bug. The whole pixels live in the lokeydll object table.
; The picture is copied into the slot once; physics only moves X and Y.
; ------------------------------------------------------------------------------
init_bugs:
        ldy #$00
@init:  lda init_x,y
        sta lokeydll_obj_x,y
        lda init_y,y
        sta lokeydll_obj_y,y
        ldx init_img,y
        lda arch_lo,x
        sta lokeydll_param_arch_lo
        lda arch_hi,x
        sta lokeydll_param_arch_hi
        jsr lokeydll_set_picture
        lda init_vx,y
        sta bug_vx_lo,y
        lda #$00
        sta bug_x_lo,y
        sta bug_y_lo,y
        sta bug_vx_hi,y
        sta bug_vy_lo,y
        sta bug_vy_hi,y
        iny
        cpy #NBUGS
        bne @init
        lda #NBUGS
        sta lokeydll_obj_count
        rts

; Deterministic stress-test spread: every zone and palette slot gets bugs,
; and velocities vary so the swarm de-synchronizes instead of bouncing in
; lockstep. Generated with .repeat so it scales with NBUGS.
init_x:
        .repeat NBUGS, I
            .byte (I * 5 + 2) .MOD MAX_X
        .endrepeat
init_x_end:

init_y:
        .repeat NBUGS, I
            .byte (I * 7 + 5) .MOD MAX_Y
        .endrepeat
init_y_end:

init_img:
        .repeat NBUGS, I
            .byte I .MOD 3
        .endrepeat
init_img_end:

init_vx:
        .repeat NBUGS, I
            .byte $40 + ((I * 3) .MOD $40)
        .endrepeat
init_vx_end:

.assert (init_x_end - init_x) = NBUGS, error, "init_x table size does not match NBUGS"
.assert (init_y_end - init_y) = NBUGS, error, "init_y table size does not match NBUGS"
.assert (init_img_end - init_img) = NBUGS, error, "init_img table size does not match NBUGS"
.assert (init_vx_end - init_vx) = NBUGS, error, "init_vx table size does not match NBUGS"

; ------------------------------------------------------------------------------
; Physics. Mirrors upd_gravity in BUGS.S. Called once per frame.
;
; Units are 8.8. The high byte is whole pixels (X) or scanlines (Y). Velocity
; is signed. Gravity is added before the move. A bounce negates that axis and
; puts the position back where it was, which matches the ST retry without
; spinning on a zero velocity. The edge test is unsigned: a step below zero
; wraps to $F5..$FF, still at or above MAX_Y, because |vy| stays under 11.
; ------------------------------------------------------------------------------
physics:
        ldy #$00
@each:  jsr update_bug
        iny
        cpy #NBUGS
        bne @each
        rts

update_bug:
        lda bug_y_lo,y
        sta old_lo
        lda lokeydll_obj_y,y
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
        lda lokeydll_obj_y,y
        adc bug_vy_hi,y
        sta lokeydll_obj_y,y

        cmp #MAX_Y
        bcc @yok
        jsr neg_vy
        lda old_lo
        sta bug_y_lo,y
        lda old_hi
        sta lokeydll_obj_y,y
@yok:   lda bug_x_lo,y
        sta old_lo
        lda lokeydll_obj_x,y
        sta old_hi

        clc
        lda bug_x_lo,y
        adc bug_vx_lo,y
        sta bug_x_lo,y
        lda lokeydll_obj_x,y
        adc bug_vx_hi,y
        sta lokeydll_obj_x,y

        cmp #MAX_X
        bcc @xok
        jsr neg_vx
        lda old_lo
        sta bug_x_lo,y
        lda old_hi
        sta lokeydll_obj_x,y
@xok:   rts

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

.segment "RODATA"

lokeydll_archetype beetle_arch, beetle_pal_w, beetle_height_lines, beetle_pages
lokeydll_archetype circle_arch, circle_pal_w, circle_height_lines, circle_pages
lokeydll_archetype square_arch, square_pal_w, square_height_lines, square_pages

arch_lo:
        .byte <beetle_arch, <circle_arch, <square_arch
arch_hi:
        .byte >beetle_arch, >circle_arch, >square_arch

; Beetle and circle share slot 0. The square uses slot 1. The bytes are the
; ones a78gfx wrote into the picture includes.
colors:
        lokeydll_color 0, beetle_pal0_c1, beetle_pal0_c2, beetle_pal0_c3
        lokeydll_color 1, square_pal1_c1, square_pal1_c2, square_pal1_c3
        .byte $FF

