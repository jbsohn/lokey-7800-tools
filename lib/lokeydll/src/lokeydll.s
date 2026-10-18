; lokeydll.s -- build a double-buffered Display List List from objects.
;
; Maria has no framebuffer. Each frame lokeydll_build writes direct headers
; into the pool Maria is not reading, terminates every zone that got a
; header, and points that buffer's DLL at those lists. lokeydll_swap_buffers
; publishes the DLL at vblank.
;
; An object is slot Y of the parallel tables in LOKEYDLL_STATE. The game
; writes lokeydll_obj_x and lokeydll_obj_y. lokeydll_set_picture copies the
; archetype into that slot once: graphics low, palette/width, the adjusted
; graphics high byte, and the spill limit. The frame builder never reads
; the archetype again.
;
; The header graphics address is the page Maria treats as the bottom of the
; zone. Maria adds the zone countdown (15, 14, ... 0) to the high byte.
; For a page-strided sprite whose top is k lines down the zone:
;
;   addr_hi = gfx_hi + height + k - 16
;
; set_picture stores (gfx_hi + height - 16). build adds k. That reaches the
; top sprite page on scanline k and falls off the bottom of the bitmap into
; the holey region. When k + height > 16 the next zone gets the same
; address minus 16, which is the rest of the bitmap.
;
; This file also owns the cartridge bootstrap: the reset/NMI/IRQ vectors,
; CPU init, and every MARIA register and display-list write. A game links
; against this file and exports `lokeydll_game`; the reset vector lands
; here and jumps to it after setup.

.include "lokeydll.inc"

.import lokeydll_game

; One blank zone, the playfield, five blank zones: 304 lines, past PAL's 293.
LOKEYDLL_TOP_ZONES    = 1
LOKEYDLL_PF_ZONES     = 13
LOKEYDLL_BOTTOM_ZONES = 5
LOKEYDLL_ZONES        = LOKEYDLL_TOP_ZONES + LOKEYDLL_PF_ZONES + LOKEYDLL_BOTTOM_ZONES
LOKEYDLL_DLL_ZONE16   = $0F
LOKEYDLL_DLL_HOLEY16  = $4F
LOKEYDLL_HEADER_BYTES = 4
LOKEYDLL_CTRL_160A    = $40

PTCTRL          = $01
P0C1            = $21
P0C2            = $22
P0C3            = $23
BKGRND          = $20
MSTAT           = $28
DPPH            = $2C
DPPL            = $30
CHBASE          = $34
CTRL            = $3C

PTCTRL_LOCK     = $07
CTRL_DMA_OFF    = $7F

.assert LOKEYDLL_ZONES * 3 <= 64, error, "DLL buffer is 64 bytes"
.assert LOKEYDLL_ZONES * 16 >= 293, error, "DLL ends before PAL line 293"
.assert LOKEYDLL_VIEW_H <= LOKEYDLL_PF_ZONES * 16, error, "view is taller than the playfield"
.assert LOKEYDLL_MAX_SPRITES <= 64, error, "object columns are 64-aligned; a longer column crosses a page"
.assert LOKEYDLL_MAX_SPRITES < 256, error, "object index is a single byte"
.assert LOKEYDLL_HEADER_BYTES = 4, error, "header store is unrolled for 4 bytes"
.assert LOKEYDLL_MAX_SPRITES * 2 * LOKEYDLL_HEADER_BYTES + LOKEYDLL_PF_ZONES * 2 <= 512, error, "pool smaller than the worst frame"

; ------------------------------------------------------------------------------
; Zero page. $0080, sixteen bytes. obj_count and the param_ labels are the
; call interface. The header bytes and ptr are scratch for the builder.
; ------------------------------------------------------------------------------
.segment "LOKEYDLL_ZP": zeropage

lokeydll_obj_count:     .res 1
lokeydll_param_arch_lo: .res 1
lokeydll_param_arch_hi: .res 1
lokeydll_param_ptr:     .res 2
lokeydll_ptr:           .res 2
lokeydll_hlo:           .res 1
lokeydll_hpal:          .res 1
lokeydll_hhi:           .res 1
lokeydll_hx:            .res 1
lokeydll_fine:          .res 1
lokeydll_i:             .res 1

; ------------------------------------------------------------------------------
; Objects and the zone map for the buffer being written.
;
; x and y are the whole pixels the game stores. lo, pal, base, and lim are
; filled by lokeydll_set_picture and left alone while the object moves.
; base is gfx_hi + height - 16. lim is 17 - height: the sprite spills into
; the next zone when (y & 15) >= lim.
;
; Each 60-byte column is aligned to 64 so lda col,y never crosses a page.
; A 64-aligned address is at most $C0 into its page, and $C0 + 59 stays
; inside that page. zn/zlo/zhi sit after the columns and stay inside one
; page for the same reason: inc zn,x and sta zlo,x must stay 6 and 4 cycles.
; ------------------------------------------------------------------------------
.segment "LOKEYDLL_STATE"

.align 64
lokeydll_obj_x:    .res LOKEYDLL_MAX_SPRITES
.align 64
lokeydll_obj_y:    .res LOKEYDLL_MAX_SPRITES
.align 64
lokeydll_obj_lo:   .res LOKEYDLL_MAX_SPRITES
.align 64
lokeydll_obj_pal:  .res LOKEYDLL_MAX_SPRITES
.align 64
lokeydll_obj_base: .res LOKEYDLL_MAX_SPRITES
.align 64
lokeydll_obj_lim:  .res LOKEYDLL_MAX_SPRITES
lokeydll_zn:       .res LOKEYDLL_PF_ZONES
lokeydll_zlo:      .res LOKEYDLL_PF_ZONES
lokeydll_zhi:      .res LOKEYDLL_PF_ZONES
lokeydll_buf:     .res 1
lokeydll_overflow: .res 1

.export lokeydll_overflow

.segment "LOKEYDLL_RAM_A"
lokeydll_pool0: .res 512

.segment "LOKEYDLL_RAM_B"
lokeydll_pool1: .res 512

.segment "LOKEYDLL_DLL"
lokeydll_dll0:  .res 64
lokeydll_dll1:  .res 64

.segment "RODATA"

lokeydll_dl_empty:
        .byte $00, $00

; Y >> 4 for every byte. Page aligned by the LOKEYDLL_ZONE segment so the
; indexed load is 4 cycles at every Y, including $FF.
.segment "LOKEYDLL_ZONE"
.align 256
lokeydll_zone_tab:
        .repeat 256, I
                .byte I >> 4
        .endrepeat

.segment "CODE"

; ------------------------------------------------------------------------------
; lokeydll_reset. The cartridge reset vector. Boots the CPU, initializes
; MARIA, then hands control to the game's `lokeydll_game` entry point.
; ------------------------------------------------------------------------------
.proc lokeydll_reset
        sei
        cld
        ldx #$FF
        txs
        jsr lokeydll_init
        jmp lokeydll_game
.endproc

; No DLL entry sets the interrupt bit, so neither vector should fire. If one
; does, return and leave the frame running.
.proc lokeydll_nmi
        rti
.endproc

.proc lokeydll_irq
        rti
.endproc

; ------------------------------------------------------------------------------
; lokeydll_init
; ------------------------------------------------------------------------------
.proc lokeydll_init
        lda #PTCTRL_LOCK
        sta PTCTRL
        lda #CTRL_DMA_OFF
        sta CTRL
        lda #$00
        sta BKGRND
        sta CHBASE
        sta lokeydll_buf
        sta lokeydll_obj_count
        sta lokeydll_overflow

        ; Pre-populate both DLL buffers with blank padding zones pointing
        ; at lokeydll_dl_empty so display memory is valid before the first
        ; built frame is published.
        lda #<lokeydll_dll0
        sta lokeydll_ptr
        lda #>lokeydll_dll0
        sta lokeydll_ptr+1
        jsr ld_fill_blank_dll

        lda #<lokeydll_dll1
        sta lokeydll_ptr
        lda #>lokeydll_dll1
        sta lokeydll_ptr+1
        jsr ld_fill_blank_dll

        lda #>lokeydll_dll0
        sta DPPH
        lda #<lokeydll_dll0
        sta DPPL
        rts
.endproc

.proc ld_fill_blank_dll
        ldx #LOKEYDLL_ZONES
@loop:  jsr ld_put_blank
        dex
        bne @loop
        rts
.endproc

; ------------------------------------------------------------------------------
; lokeydll_load_colors. lokeydll_param_ptr points at slot, c1, c2, c3
; records. A slot with bit 7 set ends the list. Slot 0 is P0, slot 1 is P1,
; and so on. The three color bytes are stored in that slot.
; ------------------------------------------------------------------------------
.proc lokeydll_load_colors
        lda lokeydll_param_ptr
        sta lokeydll_ptr
        lda lokeydll_param_ptr+1
        sta lokeydll_ptr+1
@rec:   ldy #$00
        lda (lokeydll_ptr),y
        bmi @done
        asl a
        asl a
        tax
        iny
        lda (lokeydll_ptr),y
        sta P0C1,x
        iny
        lda (lokeydll_ptr),y
        sta P0C2,x
        iny
        lda (lokeydll_ptr),y
        sta P0C3,x
        clc
        lda lokeydll_ptr
        adc #4
        sta lokeydll_ptr
        bcc @rec
        inc lokeydll_ptr+1
        bne @rec
@done:  rts
.endproc

; ------------------------------------------------------------------------------
; lokeydll_set_picture. Y is the object slot and is preserved. The archetype
; pointer is lokeydll_param_arch_lo/hi. Copies pal_w, gfx_lo, base, and lim
; into the slot. Height is 1..16; a taller picture does not fit one spill.
; A slot past the table sets the overflow flag and writes nothing.
; ------------------------------------------------------------------------------
.proc lokeydll_set_picture
        cpy #LOKEYDLL_MAX_SPRITES
        bcc @ok
        inc lokeydll_overflow
        rts
@ok:    tya
        tax
        ldy #TMariaSpriteArchetype::pal_w
        lda (lokeydll_param_arch_lo),y
        sta lokeydll_obj_pal,x
        iny                             ; TMariaSpriteArchetype::height
        lda (lokeydll_param_arch_lo),y
        sta lokeydll_fine
        lda #17
        sec
        sbc lokeydll_fine
        sta lokeydll_obj_lim,x
        iny                             ; TMariaSpriteArchetype::gfx_lo
        lda (lokeydll_param_arch_lo),y
        sta lokeydll_obj_lo,x
        iny                             ; TMariaSpriteArchetype::gfx_hi
        lda (lokeydll_param_arch_lo),y
        clc
        adc lokeydll_fine
        sec
        sbc #16
        sta lokeydll_obj_base,x
        txa
        tay
        rts
.endproc

; ------------------------------------------------------------------------------
; lokeydll_build. Writes the inactive pool and its DLL from the object
; table. Objects are slots 0 .. lokeydll_obj_count-1. A count above the
; table size is clamped and counted in lokeydll_overflow.
; ------------------------------------------------------------------------------
.proc lokeydll_build
        lda #$00
        sta lokeydll_overflow
        lda lokeydll_obj_count
        cmp #LOKEYDLL_MAX_SPRITES + 1
        bcc @counted
        lda #LOKEYDLL_MAX_SPRITES
        sta lokeydll_obj_count
        inc lokeydll_overflow
@counted:
        jsr ld_clear
        jsr ld_tally
        jsr ld_assign
        jsr ld_write_dll
        jsr ld_emit
        jmp ld_terminate
.endproc

; ------------------------------------------------------------------------------
; lokeydll_swap_buffers. Synchronizes with the leading edge of VBLANK,
; points Maria at the newly assembled DLL, and turns 160A DMA on.
; ------------------------------------------------------------------------------
.proc lokeydll_swap_buffers
@wait_exit:
        bit MSTAT
        bmi @wait_exit
@wait_enter:
        bit MSTAT
        bpl @wait_enter
        lda lokeydll_buf
        bne @one
        lda #>lokeydll_dll0
        sta DPPH
        lda #<lokeydll_dll0
        sta DPPL
        lda #$01
        bne @store
@one:   lda #>lokeydll_dll1
        sta DPPH
        lda #<lokeydll_dll1
        sta DPPL
        lda #$00
@store: sta lokeydll_buf
        lda #LOKEYDLL_CTRL_160A
        sta CTRL
        rts
.endproc

; ------------------------------------------------------------------------------
; Clear playfield counts. Assign and the DLL treat a zero count as "point
; this zone at the empty list".
; ------------------------------------------------------------------------------
.proc ld_clear
        ldx #LOKEYDLL_PF_ZONES - 1
        lda #$00
@clr:   sta lokeydll_zn,x
        dex
        bpl @clr
        rts
.endproc

; ------------------------------------------------------------------------------
; Count headers per playfield zone. A sprite past the last playfield zone
; is dropped. The second header is dropped when it would land in the blank
; zones under the playfield. Spill is (y & 15) >= lim, and lim was stored
; as 17 - height, which is the same test as k + height > 16.
; ------------------------------------------------------------------------------
.proc ld_tally
        ldy lokeydll_obj_count
        beq @done
@each:  dey
        lda lokeydll_obj_y,y
        tax
        and #$0F
        sta lokeydll_fine
        lda lokeydll_zone_tab,x
        tax
        cpx #LOKEYDLL_PF_ZONES
        bcs @next
        inc lokeydll_zn,x
        lda lokeydll_fine
        cmp lokeydll_obj_lim,y
        bcc @next
        inx
        cpx #LOKEYDLL_PF_ZONES
        bcs @next
        inc lokeydll_zn,x
@next:  cpy #$00
        bne @each
@done:  rts
.endproc

; ------------------------------------------------------------------------------
; Lay the non-empty zones into the inactive pool, in zone order. Each used
; zone is count*4 header bytes plus a two-byte terminator. zlo/zhi hold the
; start address; emit advances them to the terminator.
; ------------------------------------------------------------------------------
.proc ld_assign
        jsr ld_pool_base
        ldx #$00
@zone:  lda lokeydll_zn,x
        beq @next
        lda lokeydll_ptr
        sta lokeydll_zlo,x
        lda lokeydll_ptr+1
        sta lokeydll_zhi,x
        lda lokeydll_zn,x
        asl a
        asl a
        clc
        adc #2
        adc lokeydll_ptr
        tay
        lda lokeydll_ptr+1
        adc #$00
        sta lokeydll_ptr+1
        tya
        sta lokeydll_ptr
@next:  inx
        cpx #LOKEYDLL_PF_ZONES
        bne @zone
        rts
.endproc

; ------------------------------------------------------------------------------
; DLL: the playfield entries. The top and bottom blank zones were written by
; lokeydll_init and never change, so only the middle is rewritten each frame.
; The playfield entries of one DLL stay inside a single page, so Y can walk
; them without advancing the pointer.
; ------------------------------------------------------------------------------
.proc ld_write_dll
        jsr ld_dll_base
        lda lokeydll_ptr
        clc
        adc #LOKEYDLL_TOP_ZONES * 3
        sta lokeydll_ptr
        bcc @play
        inc lokeydll_ptr+1
@play:  ldx #$00
        ldy #$00
@pf:    lda lokeydll_zn,x
        beq @empty
        lda #LOKEYDLL_DLL_HOLEY16
        sta (lokeydll_ptr),y
        iny
        lda lokeydll_zhi,x
        sta (lokeydll_ptr),y
        iny
        lda lokeydll_zlo,x
        jmp @wrote
@empty: lda #LOKEYDLL_DLL_HOLEY16
        sta (lokeydll_ptr),y
        iny
        lda #>lokeydll_dl_empty
        sta (lokeydll_ptr),y
        iny
        lda #<lokeydll_dl_empty
@wrote: sta (lokeydll_ptr),y
        iny
        inx
        cpx #LOKEYDLL_PF_ZONES
        bne @pf
        rts
.endproc

.proc ld_put_blank
        lda #LOKEYDLL_DLL_ZONE16
        jsr ld_put
        lda #>lokeydll_dl_empty
        jsr ld_put
        lda #<lokeydll_dl_empty
        jmp ld_put
.endproc

; Store A and advance the DLL cursor. Clobbers Y.
.proc ld_put
        ldy #$00
        sta (lokeydll_ptr),y
        inc lokeydll_ptr
        bne @ok
        inc lokeydll_ptr+1
@ok:    rts
.endproc

; Write the four staged header bytes at zone X and advance that zone's
; cursor by 4. Clobbers A and Y. Preserves X. A header may cross a page;
; (zp),y applies the carry, and the cursor add below keeps zhi in step.
.macro ld_store_hdr
        lda lokeydll_zlo,x
        sta lokeydll_ptr
        lda lokeydll_zhi,x
        sta lokeydll_ptr+1
        ldy #$00
        lda lokeydll_hlo
        sta (lokeydll_ptr),y
        iny
        lda lokeydll_hpal
        sta (lokeydll_ptr),y
        iny
        lda lokeydll_hhi
        sta (lokeydll_ptr),y
        iny
        lda lokeydll_hx
        sta (lokeydll_ptr),y
        clc
        lda lokeydll_zlo,x
        adc #LOKEYDLL_HEADER_BYTES
        sta lokeydll_zlo,x
        bcc :+
        inc lokeydll_zhi,x
:
.endmacro

; ------------------------------------------------------------------------------
; Write one header per queued zone, straight from the object. The picture
; bytes were resolved by lokeydll_set_picture, so this loop only adds the
; fine Y offset and copies four bytes into the pool.
; ------------------------------------------------------------------------------
.proc ld_emit
        ldy #$00
@each:  cpy lokeydll_obj_count
        bne @body
        rts
@body:  lda lokeydll_obj_y,y
        tax
        and #$0F
        sta lokeydll_fine
        lda lokeydll_zone_tab,x
        tax
        cpx #LOKEYDLL_PF_ZONES
        bcc @place
        jmp @next
@place: lda lokeydll_obj_lo,y
        sta lokeydll_hlo
        lda lokeydll_obj_pal,y
        sta lokeydll_hpal
        lda lokeydll_obj_base,y
        clc
        adc lokeydll_fine
        sta lokeydll_hhi
        lda lokeydll_obj_x,y
        sta lokeydll_hx
        lda lokeydll_fine
        cmp lokeydll_obj_lim,y
        bcs @spill
        sty lokeydll_i
        ld_store_hdr
        ldy lokeydll_i
        jmp @next
@spill: sty lokeydll_i
        ld_store_hdr
        lda lokeydll_hhi
        sec
        sbc #16
        sta lokeydll_hhi
        inx
        cpx #LOKEYDLL_PF_ZONES
        bcs @restore
        ld_store_hdr
@restore:
        ldy lokeydll_i
@next:  iny
        jmp @each
.endproc

.proc ld_terminate
        ldx #$00
@zone:  lda lokeydll_zn,x
        beq @next
        lda lokeydll_zlo,x
        sta lokeydll_ptr
        lda lokeydll_zhi,x
        sta lokeydll_ptr+1
        lda #$00
        tay
        sta (lokeydll_ptr),y
        iny
        sta (lokeydll_ptr),y
@next:  inx
        cpx #LOKEYDLL_PF_ZONES
        bne @zone
        rts
.endproc

.proc ld_pool_base
        lda lokeydll_buf
        bne @one
        lda #<lokeydll_pool0
        sta lokeydll_ptr
        lda #>lokeydll_pool0
        sta lokeydll_ptr+1
        rts
@one:   lda #<lokeydll_pool1
        sta lokeydll_ptr
        lda #>lokeydll_pool1
        sta lokeydll_ptr+1
        rts
.endproc

.proc ld_dll_base
        lda lokeydll_buf
        bne @one
        lda #<lokeydll_dll0
        sta lokeydll_ptr
        lda #>lokeydll_dll0
        sta lokeydll_ptr+1
        rts
@one:   lda #<lokeydll_dll1
        sta lokeydll_ptr
        lda #>lokeydll_dll1
        sta lokeydll_ptr+1
        rts
.endproc

.segment "VECTORS"
        .byte $FF, $83
        .word lokeydll_nmi
        .word lokeydll_reset
        .word lokeydll_irq
