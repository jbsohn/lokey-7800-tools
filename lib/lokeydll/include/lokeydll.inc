; lokeydll.inc -- display list for a game that has objects and pictures.
;
; A picture is an a78gfx include: pages, size, pal_w, and palette bytes.
; The game declares one archetype per picture with lokeydll_archetype.
; One picture occupies one of LOKEYDLL_GFX0, LOKEYDLL_GFX1, LOKEYDLL_GFX2.
; The linker map that places those slots is 7800.cfg in this directory.
;
; The library owns the cartridge bootstrap: the reset/NMI/IRQ vectors, CPU
; init, and every MARIA register and display-list write. A game links against
; lokeydll.lib and exports `lokeydll_game`. The reset vector jumps there
; after setup. Game code never touches the hardware.
;
; An object is one slot, indexed by Y, in the parallel tables below. The
; game stores whole-pixel X and Y there and calls lokeydll_set_picture when
; the picture changes. lokeydll_build reads those slots and writes the
; display list. Fractional position and velocity stay with the game; Maria
; only sees the whole pixels.
;
;   ldy slot
;   sta lokeydll_obj_x,y
;   sta lokeydll_obj_y,y
;   lda #<arch
;   sta lokeydll_param_arch_lo
;   lda #>arch
;   sta lokeydll_param_arch_hi
;   jsr lokeydll_set_picture
;
; Each frame, after the positions are current:
;   lda #count
;   sta lokeydll_obj_count
;   jsr lokeydll_build
;   jsr lokeydll_swap_buffers
;
; X and Y are pixels in a LOKEYDLL_VIEW_W by LOKEYDLL_VIEW_H playfield.
; Slots are 0 .. lokeydll_obj_count-1, and the count is at most
; LOKEYDLL_MAX_SPRITES. Once, at the top of lokeydll_game, point
; lokeydll_param_ptr at a color list and call lokeydll_load_colors:
;
;   colors:
;       lokeydll_color 0, beetle_pal0_c1, beetle_pal0_c2, beetle_pal0_c3
;       .byte $FF
;
; `lokeydll_reset` already calls `lokeydll_init`. `lokeydll_init` is
; exported for a game that wants the display back to a blank state.

LOKEYDLL_VIEW_W       = 160
LOKEYDLL_VIEW_H       = 200
; The display-list pool is 512 bytes (LOKEYDLL_RAM_A/B in 7800.cfg). A
; fully-spilled frame writes two headers (8 bytes) per sprite plus a 2-byte
; terminator per playfield zone: 60*8 + 13*2 = 506 bytes. 60 is the largest
; count the pool can hold; raise it only after growing both pools in 7800.cfg.
LOKEYDLL_MAX_SPRITES  = 60

.struct TMariaSpriteArchetype
    pal_w       .byte
    height      .byte
    gfx_lo      .byte
    gfx_hi      .byte
.endstruct

; Declare a sprite archetype from an a78gfx picture. name is the label;
; pal_w, height, and pages are the picture's own equates. The byte order
; matches TMariaSpriteArchetype, which the game never builds by hand.
.macro lokeydll_archetype name, pal_w, height, pages
        name:
            .byte pal_w
            .byte height
            .byte <(pages)
            .byte >(pages)
.endmacro

; One entry in the color list passed to lokeydll_load_colors: a palette slot
; and its three color bytes. A slot of $FF ends the list.
.macro lokeydll_color slot, c1, c2, c3
        .byte slot, c1, c2, c3
.endmacro

; Whole-pixel position of slot Y. The game writes these. Indexed by Y, and
; each table stays inside one page, so the indexed stores in the physics
; loop do not pay a page-cross penalty.
.global lokeydll_obj_x, lokeydll_obj_y
.globalzp lokeydll_obj_count

; lokeydll_set_picture reads the archetype through this pointer. Y is the
; slot and is preserved. A and X are not.
.globalzp lokeydll_param_arch_lo, lokeydll_param_arch_hi
.globalzp lokeydll_param_ptr

.global lokeydll_reset
.global lokeydll_init
.global lokeydll_load_colors
.global lokeydll_set_picture
.global lokeydll_build
.global lokeydll_swap_buffers
.global lokeydll_overflow
