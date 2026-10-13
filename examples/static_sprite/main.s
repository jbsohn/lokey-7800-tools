; ==============================================================================
; main.s -- Atari 7800 Maria Step 2: Static Spaceship Centered on Screen
;
; Demonstrates:
;   1. 160A 4-color direct graphics mode (4 pixels per byte)
;   2. Multi-scanline 16x16 spaceship sprite loaded from ROM
;   3. MARIA 256-byte page striding: 16 scanlines aligned to consecutive pages
;   4. Static Display List in ROM (no dynamic RAM buffering needed)
;   5. Spaceship centered horizontally at HPOS = 72 in Zone 7 (scanlines 112..127)
; ==============================================================================

.include "maria.inc"

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

        ; Set background color to Deep Space Black ($00)
        lda #$00
        sta BKGRND

        ; Configure Palette 0 for the Spaceship from compiled palette equates:
        ;   P0C1: Electric Cyan Cockpit
        ;   P0C2: Fiery Amber Wingtips & Thrusters
        ;   P0C3: Pure Star White Fuselage
        lda #ship_pal0_c1
        sta P0C1
        lda #ship_pal0_c2
        sta P0C2
        lda #ship_pal0_c3
        sta P0C3

        ; Set Display List List pointer (DPPH / DPPL) to ROM dll_list
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
; Main Idle Loop (Stationary Spaceship Display)
; ------------------------------------------------------------------------------
main_loop:
v1:     bit MSTAT
        bmi v1                  ; Wait while in VBLANK
v2:     bit MSTAT
        bpl v2                  ; Wait for VBLANK to start

        jmp main_loop

; ==============================================================================
; RODATA: Display List List, Display Lists, and Sprite Graphics
; ==============================================================================
.segment "RODATA"

; ------------------------------------------------------------------------------
; Empty Display List (Terminator only)
; ------------------------------------------------------------------------------
dl_empty:
        maria_dl_end

; ------------------------------------------------------------------------------
; Banked Sprite Graphics (Aligned to 256-byte page boundary)
; ------------------------------------------------------------------------------
.include "ship.inc"

; ------------------------------------------------------------------------------
; Spaceship Display List (Active DL for Zone 7: Screen Center)
; Uses TMariaDLHeaderDirect (4 bytes) + TMariaDLTerminator (2 bytes)
;
; Parameters:
;   addr:     ship_pages (start of 256-byte page-strided scanlines in ROM)
;   palette:  0
;   width:    ship_width_bytes (4 bytes = 16 pixels in 160A mode)
;   hpos:     centered: (160 - (ship_width_bytes * 4)) / 2 = 72
; ------------------------------------------------------------------------------
dl_ship:
        maria_dl_direct ship_pages, 0, ship_width_bytes, (160 - (ship_width_bytes * 4)) / 2
        maria_dl_end

; ------------------------------------------------------------------------------
; Display List List (DLL)
; 240 visible scanlines partitioned into 15 zones of 16 lines each:
;   Zones 0..6  (112 scanlines) : dl_empty
;   Zone 7      ( 16 scanlines) : dl_ship (Screen Center: scanlines 112..127)
;   Zones 8..14 (112 scanlines) : dl_empty
; ------------------------------------------------------------------------------
dll_list:
        .repeat 7
            maria_dll_entry 15, dl_empty
        .endrepeat

        maria_dll_entry 15, dl_ship

        .repeat 7
            maria_dll_entry 15, dl_empty
        .endrepeat

; ==============================================================================
; 6502 Vectors & 7800 Cartridge Encryption Header Signature
; ==============================================================================
.segment "VECTORS"
        .byte $FF, $83          ; Atari 7800 Cartridge Flag / Encryption signature
        .word reset             ; NMI vector
        .word reset             ; RESET vector
        .word reset             ; IRQ / BRK vector
