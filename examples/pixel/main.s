; ==============================================================================
; main.s -- Atari 7800 Maria Minimal Demo: One Green Dot in Screen Center
; Toolchain: ca65 / ld65
; ==============================================================================
;
; MARIA DISPLAY ARCHITECTURE OVERVIEW:
; ------------------------------------
; Maria builds each scanline using DMA by reading two hierarchical structures:
;
; 1. Display List List (DLL):
;    Pointed to by registers DPPH ($2C) and DPPL ($30) at the start of the frame.
;    Each DLL entry defines a "zone" (vertical band of 1 to 16 scanlines) and
;    points to a Display List (DL) for that zone.
;
;    Struct definition (in maria.inc):
;      .struct TMariaDLLEntry
;        offset .byte   ; Zone height - 1 (0..15 => 1..16 scanlines)
;        dph    .byte   ; High byte of Display List address
;        dpl    .byte   ; Low byte of Display List address
;      .endstruct
;
;    NTSC visible frame:
;      Total active DMA scanlines: 242 lines (scanlines 16 to 257)
;      We partition the screen into zones:
;        - 7 zones of 16 lines (112 lines) : dl_empty
;        - 1 zone  of  8 lines (  8 lines) : dl_empty
;        - 1 zone  of  1 line  (  1 line ) : dl_dot (Scanline 120 = Screen Center!)
;        - 7 zones of 16 lines (112 lines) : dl_empty
;        - 1 zone  of  9 lines (  9 lines) : dl_empty
;        Total: 112 + 8 + 1 + 112 + 9 = 242 active lines.
;
; 2. Display List (DL):
;    A sequence of graphic headers terminating with a 2-byte terminator ($00, $00).
;
;    Struct definition (in maria.inc):
;      .struct TMariaDLHeaderDirect
;        addr_lo .byte  ; Low byte of graphic data
;        pal_w   .byte  ; Palette (bits 7-5) and 5-bit two's complement width
;        addr_hi .byte  ; High byte of graphic data
;        hpos    .byte  ; Horizontal position (0..159 in 160 modes)
;      .endstruct
;
;    In 160A mode (readMode=0), 1 byte contains 4 pixels (2 bits each):
;      %00 10 00 00 -> Pixel 0: 0, Pixel 1: 2 (Green), Pixel 2: 0, Pixel 3: 0.
;      With HPOS = 79, Pixel 1 lands at X = 80 (exact center of 160-pixel line).
;
; ==============================================================================

.include "maria.inc"

.segment "CODE"

reset:
        sei                     ; Disable interrupts
        cld                     ; Clear decimal mode
        ldx #$FF
        txs                     ; Set stack pointer to $01FF

        ; Disable BIOS and enable Cartridge + Maria.
        ; INPTCTRL ($01):
        ;   bit 0: Lock = 0
        ;   bit 1: Maria enable = 1
        ;   bit 2: Cartridge enable / BIOS disable = 1
        ;   bit 3: TIA mode = 0
        ;   => %00000110 = $06
        lda #$06
        sta PTCTRL
        sta PTCTRL

        ; Turn off Maria DMA during register setup
        lda #$00
        sta CTRL

        ; Set background color to Black ($00)
        lda #$00
        sta BKGRND

        ; Configure Palette 0, Color 2 to bright Atari green ($C8)
        ; (Palette 0, index 2 is addressed by P0C2 = $22)
        lda #$C8
        sta P0C2

        ; Set Display List List pointer (DPPH / DPPL)
        lda #>dll_list
        sta DPPH
        lda #<dll_list
        sta DPPL

        ; Clear character base register
        lda #$00
        sta CHBASE

        ; Wait for VBLANK before enabling Maria DMA
wait_vbi:
        bit MSTAT               ; Bit 7 is high during VBLANK
        bpl wait_vbi

        ; Enable MARIA DMA in 160A mode (readMode = 0):
        ;   Bit 7: Colour kill = 0 (color enabled)
        ;   Bits 6-5: DMA mode = %10 (DMA on)
        ;   Bit 4: Character width = 0
        ;   Bit 3: Border = 0
        ;   Bit 2: Kangaroo mode = 0
        ;   Bits 1-0: Read mode = %00 (160x192/240 direct mode, 160A)
        ;   => %01000000 = $40
        lda #$40
        sta CTRL

main_loop:
        ; Synchronize frame with VBLANK
v1:     bit MSTAT
        bmi v1                  ; Wait while in VBLANK
v2:     bit MSTAT
        bpl v2                  ; Wait for VBLANK to start

        jmp main_loop

; ==============================================================================
; RODATA: Display List List, Display Lists, Graphic Pixel Data
; ==============================================================================
.segment "RODATA"

; Graphic data for the single green dot:
; 160A mode: 1 byte = 4 pixels, 2 bits each.
;   Bits 7-6: Pixel 0 = 0 (Background)
;   Bits 5-4: Pixel 1 = 2 (Palette 0, Color 2 = Green $C8)
;   Bits 3-2: Pixel 2 = 0 (Background)
;   Bits 1-0: Pixel 3 = 0 (Background)
; %00100000 = $20
pixel_data:
        .byte $20               ; 1 byte graphic dot

; ------------------------------------------------------------------------------
; Empty Display List (DL Terminator)
; ------------------------------------------------------------------------------
dl_empty:
        maria_dl_end
        .assert (* - dl_empty) = .sizeof(TMariaDLTerminator), error, "dl_empty size mismatch"

; ------------------------------------------------------------------------------
; Dot Display List (Active DL for the Center Scanline)
; Uses TMariaDLHeaderDirect (4 bytes) + TMariaDLTerminator (2 bytes)
; ------------------------------------------------------------------------------
dl_dot:
        ; Direct header: addr, palette=0, width=1 byte, hpos=79
        maria_dl_direct pixel_data, 0, 1, 79
        .assert (* - dl_dot) = .sizeof(TMariaDLHeaderDirect), error, "dl_dot direct header size mismatch"

        maria_dl_end
        .assert (* - dl_dot) = (.sizeof(TMariaDLHeaderDirect) + .sizeof(TMariaDLTerminator)), error, "dl_dot total size mismatch"

; ------------------------------------------------------------------------------
; Display List List (DLL)
; Uses TMariaDLLEntry struct (3 bytes per zone entry)
;
; Partitioning 242 active lines:
;   Top:    7 zones of 16 lines (112 lines) + 1 zone of 8 lines (8 lines) = 120 lines
;   Center: 1 zone  of  1 line  (offset = 0) at scanline 120 = the green dot!
;   Bottom: 7 zones of 16 lines (112 lines) + 1 zone of 9 lines (9 lines) = 121 lines
;   Total = 120 + 1 + 121 = 242 lines.
; ------------------------------------------------------------------------------
dll_list:
        ; Top 120 blank lines
        .repeat 7
            maria_dll_entry 15, dl_empty    ; 16 scanlines each
        .endrepeat
        maria_dll_entry 7, dl_empty         ; 8 scanlines

        ; Exact center scanline (line 120 of 240 visible lines): the green dot!
        maria_dll_entry 0, dl_dot           ; 1 scanline (offset = 0)

        ; Bottom 121 blank lines
        .repeat 7
            maria_dll_entry 15, dl_empty    ; 16 scanlines each
        .endrepeat
        maria_dll_entry 8, dl_empty         ; 9 scanlines (8 + 1)

; Validate that all DLL entries match TMariaDLLEntry struct size
.assert ((* - dll_list) .mod .sizeof(TMariaDLLEntry)) = 0, error, "DLL list alignment error"

; ==============================================================================
; 6502 Vectors & 7800 Cartridge Encryption Header Signature
; ==============================================================================
.segment "VECTORS"
        .byte $FF, $83          ; Atari 7800 Cartridge Flag / Encryption signature
        .word reset             ; NMI vector
        .word reset             ; RESET vector
        .word reset             ; IRQ / BRK vector
