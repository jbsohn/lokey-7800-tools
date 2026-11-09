; ==============================================================================
; main.s -- Atari 7800 Maria: Scaling sprite (scale_sprite)
;
; The Fuji is baked as one 160A frame per scanline of height, from the full
; canvas down to 12 lines. Each step is a single area-filter of AtariLogo.png.
; The three bars are separate shapes, and a slot narrower than a pixel is kept
; open by shifting the bars apart instead of letting them touch.
;
; 160A pixels are 2:1 (twice as wide as a scanline is tall). The bake resizes
; the square master to 48x48 and --par halves the width, leaving a 24 data-pixel
; by 48-line canvas. On the CRT that is 48 clocks wide by 48 lines: square.
; Every scale frame keeps that canvas, so the three headers only change
; addr_lo. addr_hi steps down 16 pages per zone (the 48 scanlines, stored
; bottom-up). The mark shrinks toward the middle of the canvas.
; ==============================================================================

.include "maria.inc"

ANIM_DELAY = 2                  ; 60Hz frames held on each size (~30 sizes/sec)
SPRITE_PX  = 24                 ; Data pixels. Matches --resize 48x48 --par.
HPOS       = (160 - SPRITE_PX) / 2

; Fill one 6-byte RAM Display List (direct header + terminator) for one zone.
; page_off selects which 16-line band of the 48-line bank this zone shows:
;   32 = top band, 16 = middle, 0 = bottom band.
.macro init_zone_dl label, page_off
        lda logo_bank_frame_lo          ; frame 0 (full size) addr_lo
        sta label + TMariaDLHeaderDirect::addr_lo
        lda logo_bank_frame_pal_w       ; frame 0 pal_w
        sta label + TMariaDLHeaderDirect::pal_w
        lda #>(logo_bank_pages + page_off*256)
        sta label + TMariaDLHeaderDirect::addr_hi
        lda #HPOS
        sta label + TMariaDLHeaderDirect::hpos
        lda #$00
        sta label + 4                   ; terminator byte 0
        sta label + 5                   ; terminator byte 1
.endmacro

; ==============================================================================
; ZEROPAGE Variables
; ==============================================================================
.segment "ZEROPAGE"

frame_timer:    .res 1          ; Delay counter for animation timing
anim_index:     .res 1          ; Current scale step (0 = full size)
scale_dir:      .res 1          ; 0 = shrinking (index up), 1 = growing (index down)

; ==============================================================================
; RAM Display Lists: one per zone, 6 bytes each (direct header + terminator)
; ==============================================================================
.segment "BSS"

dl_z0:          .res 6          ; zone 6 (top band,    lines  96-111)
dl_z1:          .res 6          ; zone 7 (             lines 112-127)
dl_z2:          .res 6          ; zone 8 (bottom band, lines 128-143)

; ==============================================================================
; CODE Segment: Initialization and Main Animation Loop
; ==============================================================================
.segment "CODE"

reset:
        sei                     ; Disable interrupts
        cld                     ; Clear decimal mode
        ldx #$FF
        txs                     ; Set stack pointer to $01FF

        ; Disable BIOS and enable Cartridge + Maria.
        lda #$06
        sta PTCTRL
        sta PTCTRL

        ; Turn off Maria DMA during register setup
        lda #$00
        sta CTRL

        ; Set background color to Space Black ($00)
        lda #$00
        sta BKGRND

        ; Palette 0: index 1 is the Fuji red. c2/c3 are unused copies of c1.
        lda #logo_bank_pal0_c1
        sta P0C1
        lda #logo_bank_pal0_c2
        sta P0C2
        lda #logo_bank_pal0_c3
        sta P0C3

        ; Clear state variables
        lda #$00
        sta frame_timer
        sta anim_index
        sta scale_dir

        ; Build the three zone headers (frame 0 = full size)
        init_zone_dl dl_z0, 32
        init_zone_dl dl_z1, 16
        init_zone_dl dl_z2, 0

        ; Point Maria DPPH/DPPL to dll_list in ROM
        lda #>dll_list
        sta DPPH
        lda #<dll_list
        sta DPPL

        ; Clear character base register
        lda #$00
        sta CHBASE

        ; Wait for VBLANK before enabling Maria DMA
wait_vbi:
        bit MSTAT
        bpl wait_vbi

        ; Enable MARIA DMA in 160A mode (readMode = 0)
        lda #$40
        sta CTRL

; ------------------------------------------------------------------------------
; Main Animation Loop (Runs once per 60Hz frame)
; ------------------------------------------------------------------------------
main_loop:
        ; 1. Synchronize frame with VBLANK
v_wait_off:
        bit MSTAT
        bmi v_wait_off          ; Wait while in VBLANK
v_wait_on:
        bit MSTAT
        bpl v_wait_on           ; Wait until VBLANK starts

        ; 2. Animation Timing: Advance the scale step every ANIM_DELAY ticks
        inc frame_timer
        lda frame_timer
        cmp #ANIM_DELAY
        bcc @done_frame

        lda #$00
        sta frame_timer

        ; Advance one scanline of size. Frame 0 is full size; the last frame is
        ; the smallest. Ping-pong: shrink to the end, then grow back, and repeat.
        lda scale_dir
        bne @grow
        inc anim_index
        lda anim_index
        cmp #logo_bank_frame_count - 1
        bcc @apply_frame
        lda #$01                ; reached the smallest: reverse to grow
        sta scale_dir
        jmp @apply_frame
@grow:
        dec anim_index
        lda anim_index
        bne @apply_frame        ; not back to full yet, keep growing
        lda #$00                ; reached full: reverse to shrink
        sta scale_dir

@apply_frame:
        ; 3. Update all three headers' addr_lo (and pal_w) during VBLANK. The
        ; zones share the same frame; addr_hi is fixed per zone.
        ldx anim_index
        lda logo_bank_frame_lo,x
        sta dl_z0 + TMariaDLHeaderDirect::addr_lo
        sta dl_z1 + TMariaDLHeaderDirect::addr_lo
        sta dl_z2 + TMariaDLHeaderDirect::addr_lo
        lda logo_bank_frame_pal_w,x
        sta dl_z0 + TMariaDLHeaderDirect::pal_w
        sta dl_z1 + TMariaDLHeaderDirect::pal_w
        sta dl_z2 + TMariaDLHeaderDirect::pal_w

@done_frame:
        jmp main_loop

; ==============================================================================
; RODATA Segment: Display Lists, Display List List, and Banked Sprite Graphics
; ==============================================================================
.segment "RODATA"

; Empty Display List (Terminator only)
dl_empty:
        maria_dl_end

; Display List List: 15 zones of 16 lines (240 lines). Zones 6-8 hold the
; three 16-line bands of the 48-line mark; the rest point at dl_empty.
dll_list:
        .repeat 6
            maria_dll_entry 15, dl_empty
        .endrepeat
        maria_dll_entry 15, dl_z0
        maria_dll_entry 15, dl_z1
        maria_dll_entry 15, dl_z2
        .repeat 6
            maria_dll_entry 15, dl_empty
        .endrepeat

; Banked sprite graphics (one frame per scanline, 48 pages, aligned to $100)
.include "logo_bank.inc"

; ==============================================================================
; 6502 Vectors & 7800 Cartridge Encryption Header Signature
; ==============================================================================
.segment "VECTORS"
        .byte $FF, $83          ; Atari 7800 Cartridge Flag / Encryption signature
        .word reset             ; NMI vector
        .word reset             ; RESET vector
        .word reset             ; IRQ / BRK vector
