; ==============================================================================
; main.s -- Atari 7800 Maria Step 3: Rotating Spaceship Demo
;
; Demonstrates:
;   1. 160A 4-color mode with 2:1 pixel aspect ratio (PAR) correction
;   2. 256-byte page-strided Maria sprite bank with 16 pre-baked rotation frames:
;        - Frames 00..15: 16 discrete rotation angles (0 to 337.5 deg, 22.5 deg steps)
;   3. Dynamic RAM Display List at $1800 updated during VBLANK
;   4. Glitch-free, tear-free animation synchronized to MARIA VBLANK
;   5. Spaceship centered on screen at HPOS = 72 in Zone 7 (scanlines 112..127)
; ==============================================================================

.include "maria.inc"

ANIM_DELAY = 4                  ; Number of 60Hz frames between rotation steps (15 fps rotation)

; ==============================================================================
; ZEROPAGE Variables
; ==============================================================================
.segment "ZEROPAGE"

frame_timer:    .res 1          ; Delay counter for animation timing
anim_index:     .res 1          ; Current rotation frame (0..ship_bank_frame_count-1)

; ==============================================================================
; RAM Display List (Dynamic Object in RAM at $1800)
; ==============================================================================
.segment "BSS"

; Spaceship Display List (6 bytes: Direct header + Terminator)
dl_ship:        .res 6

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
        ; PTCTRL ($01): bit 1 = Maria enable, bit 2 = Cartridge enable
        lda #$06
        sta PTCTRL
        sta PTCTRL

        ; Turn off Maria DMA during register setup
        lda #$00
        sta CTRL

        ; Set background color to Space Black ($00)
        lda #$00
        sta BKGRND

        ; Configure Palette 0 for the Spaceship from compiled palette equates:
        ;   P0C1: Electric Cyan Cockpit
        ;   P0C2: Fiery Amber Wingtips & Thrusters
        ;   P0C3: Pure Star White Fuselage
        lda #ship_bank_pal0_c1
        sta P0C1
        lda #ship_bank_pal0_c2
        sta P0C2
        lda #ship_bank_pal0_c3
        sta P0C3

        ; Clear state variables
        lda #$00
        sta frame_timer
        sta anim_index

        ; Initialize RAM Display List for spaceship with Frame 0:
        ;   Direct header: addr_lo, pal_w, addr_hi, hpos
        ;   Terminator:    $00, $00
        ldx #0
        lda ship_bank_frame_lo,x
        sta dl_ship + TMariaDLHeaderDirect::addr_lo
        lda ship_bank_frame_pal_w,x
        sta dl_ship + TMariaDLHeaderDirect::pal_w
        lda #>ship_bank_pages
        sta dl_ship + TMariaDLHeaderDirect::addr_hi
        lda #72                 ; Centered: (160 - 16) / 2 = 72
        sta dl_ship + TMariaDLHeaderDirect::hpos
        lda #$00
        sta dl_ship + 4         ; Terminator byte 0
        sta dl_ship + 5         ; Terminator byte 1

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

        ; Enable MARIA DMA in 160A mode (readMode = 0):
        ;   DMA mode = %10 (on), Read mode = %00 (160 direct) -> $40
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

        ; 2. Animation Timing: Advance rotation angle every ANIM_DELAY ticks
        inc frame_timer
        lda frame_timer
        cmp #ANIM_DELAY
        bcc @done_frame

        lda #$00
        sta frame_timer

        ; Advance to next rotation frame
        inc anim_index
        lda anim_index
        cmp #ship_bank_frame_count
        bcc @apply_frame

        ; Loop back to frame 0
        lda #$00
        sta anim_index

@apply_frame:
        ; 3. Update RAM Display List directly during VBLANK (16 CPU cycles!)
        ; Maria DMA is idle during VBLANK, guaranteeing 100% glitch-free, tear-free updates.
        ldx anim_index
        lda ship_bank_frame_lo,x
        sta dl_ship + TMariaDLHeaderDirect::addr_lo
        lda ship_bank_frame_pal_w,x
        sta dl_ship + TMariaDLHeaderDirect::pal_w

@done_frame:
        jmp main_loop

; ==============================================================================
; RODATA Segment: Display Lists, Display List List, and Banked Sprite Graphics
; ==============================================================================
.segment "RODATA"

; ------------------------------------------------------------------------------
; Empty Display List (Terminator only)
; ------------------------------------------------------------------------------
dl_empty:
        maria_dl_end

; ------------------------------------------------------------------------------
; Display List List (Points Zone 7 to dl_ship in RAM)
; 240 visible scanlines = 15 zones of 16 lines each:
;   Zones 0..6  (112 lines) : dl_empty
;   Zone 7      ( 16 lines) : dl_ship (Screen center: scanlines 112..127)
;   Zones 8..14 (112 lines) : dl_empty
; ------------------------------------------------------------------------------
dll_list:
        .repeat 7
            maria_dll_entry 15, dl_empty
        .endrepeat
        maria_dll_entry 15, dl_ship
        .repeat 7
            maria_dll_entry 15, dl_empty
        .endrepeat

; ------------------------------------------------------------------------------
; Banked Sprite Graphics (16 rotation frames aligned to 256-byte page boundary)
; ------------------------------------------------------------------------------
.include "ship_bank.inc"

; ==============================================================================
; 6502 Vectors & 7800 Cartridge Encryption Header Signature
; ==============================================================================
.segment "VECTORS"
        .byte $FF, $83          ; Atari 7800 Cartridge Flag / Encryption signature
        .word reset             ; NMI vector
        .word reset             ; RESET vector
        .word reset             ; IRQ / BRK vector
