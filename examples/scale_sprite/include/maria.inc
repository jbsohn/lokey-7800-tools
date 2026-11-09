; ============================================================
; maria.inc -- Atari 7800 Hardware Defines & Data Structures
; Compatible with ca65 / cc65 toolchain
; ============================================================

; --- TIA Registers ---
TIA         = $00
PTCTRL      = $01

INPT0       = $08
INPT1       = $09
INPT2       = $0A
INPT3       = $0B
INPT4       = $0C
INPT5       = $0D

AUDC0       = $15
AUDC1       = $16
AUDF0       = $17
AUDF1       = $18
AUDV0       = $19
AUDV1       = $1A

; --- MARIA Registers ---
MARIA       = $20
BKGRND      = $20
BACKGRND    = $20
P0C1        = $21
P0C2        = $22
P0C3        = $23
WSYNC       = $24
P1C1        = $25
P1C2        = $26
P1C3        = $27
MSTAT       = $28
P2C1        = $29
P2C2        = $2A
P2C3        = $2B
DPPH        = $2C
DPH         = $2C
P3C1        = $2D
P3C2        = $2E
P3C3        = $2F
DPPL        = $30
DPL         = $30
P4C1        = $31
P4C2        = $32
P4C3        = $33
CHBASE      = $34
P5C1        = $35
P5C2        = $36
P5C3        = $37
OFFSET      = $38
P6C1        = $39
P6C2        = $3A
P6C3        = $3B
CTRL        = $3C
P7C1        = $3D
P7C2        = $3E
P7C3        = $3F

; --- PIA and TIMER (6532) ---
SWCHA       = $0280
CTLSWA      = $0281
SWCHB       = $0282
CTLSWB      = $0283
INTIM       = $0284
TIM1T       = $0294
TIM8T       = $0295
TIM64T      = $0296
T1024T      = $0297

; --- Constants ---
RESET       = 1
SELECT      = 2
PAUSE       = 8

RAM1        = $1800
RAM2        = $2100
RAM3        = $2200

; ============================================================
; Maria Data Structure Definitions (.struct)
; ============================================================

; Display List List (DLL) Entry (3 bytes)
; Each entry controls one zone of scanlines:
;   offset: bits 3..0 = zone height - 1 (0..15 => 1..16 lines)
;           bit 5     = H8  (Holey DMA 8)
;           bit 6     = H16 (Holey DMA 16)
;           bit 7     = DLI (Display List Interrupt enable)
;   dph:    High byte of Display List address
;   dpl:    Low byte of Display List address
.struct TMariaDLLEntry
    offset      .byte
    dph         .byte
    dpl         .byte
.endstruct

; Direct Display List Header (4 bytes)
; Used to display graphic objects in direct graphics mode:
;   addr_lo:  Low byte of graphics data address
;   pal_w:    Palette (bits 7..5) and two's complement width (bits 4..0)
;   addr_hi:  High byte of graphics data address
;   hpos:     Horizontal pixel position
.struct TMariaDLHeaderDirect
    addr_lo     .byte
    pal_w       .byte
    addr_hi     .byte
    hpos        .byte
.endstruct

; Extended / Indirect Display List Header (5 bytes)
.struct TMariaDLHeaderExtended
    addr_lo     .byte
    flags       .byte
    addr_hi     .byte
    pal_w       .byte
    hpos        .byte
.endstruct

; Display List Terminator (2 bytes of $00)
.struct TMariaDLTerminator
    term0       .byte
    term1       .byte
.endstruct

; Helper macros to emit struct instances cleanly in ca65:

.macro maria_dll_entry offset_val, dl_addr
    .byte offset_val
    .byte >dl_addr
    .byte <dl_addr
.endmacro

.macro maria_dl_direct addr, palette, width, hpos
    .byte <addr
    .byte ((palette << 5) | ((-width) & $1F))
    .byte >addr
    .byte hpos
.endmacro

.macro maria_dl_end
    .byte $00, $00
.endmacro
