use crate::{MariaSprite, MariaSpriteBank};
use clap::ValueEnum;
use std::fmt::Debug;

/// Target assembler type supported for output code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum AssemblerType {
    /// ca65 assembler from the cc65 suite (standard for Atari 7800 C/assembly development).
    #[default]
    Ca65,
    /// DASM assembler (widely used in Atari 2600 and 7800 homebrew development).
    Dasm,
    /// MADS (Mad Assembler - popular Atari 8-bit and 7800 cross-assembler).
    Mads,
}

impl AssemblerType {
    /// Creates a writer instance corresponding to this assembler type.
    #[must_use]
    pub fn create_writer(&self) -> Box<dyn AssemblerWriter> {
        match self {
            Self::Ca65 => Box::new(Ca65Writer),
            Self::Dasm => Box::new(DasmWriter),
            Self::Mads => Box::new(MadsWriter),
        }
    }
}

/// Abstract base trait for formatting and emitting assembler code for graphics assets.
pub trait AssemblerWriter: Debug + Send + Sync {
    /// Human-readable name of the target assembler.
    fn name(&self) -> &'static str;

    /// File extension typically used for include files (default: "inc").
    fn default_include_extension(&self) -> &'static str {
        "inc"
    }

    /// Format a single comment line.
    fn format_comment(&self, text: &str) -> String;

    /// Format an equate / constant definition.
    fn format_equate(&self, name: &str, value: &str) -> String;

    /// Format a memory alignment directive for the given byte boundary (e.g. 256 bytes for MARIA pages).
    fn format_align(&self, boundary: usize) -> String;

    /// Format a label definition.
    fn format_label(&self, name: &str) -> String;

    /// Format byte data into one or more assembly data lines.
    fn format_bytes(&self, bytes: &[u8]) -> String;

    /// Format an external binary inclusion directive (e.g. `.incbin "file.bin"` or `INCBIN "file.bin"`).
    fn format_incbin(&self, filename: &str) -> String;

    /// Generates an include file (.inc) for a single MARIA sprite.
    ///
    /// Defines equates for width, height, palette mask, and total binary size.
    /// If `bin_filename` is provided, also emits page alignment and an binary inclusion directive.
    fn write_sprite_inc(
        &self,
        sprite: &MariaSprite,
        label: &str,
        bin_filename: Option<&str>,
    ) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(
                "=============================================================================="
            )
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(&format!("MARIA Sprite Include: {label}"))
        );
        let width_pixels = match sprite.mode {
            crate::MariaMode::Mode160A => usize::from(sprite.width_bytes) * 4,
            crate::MariaMode::Mode320 => usize::from(sprite.width_bytes) * 8,
        };
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(&format!(
                "Mode: {:?}, Width: {} pixels ({} bytes), Height: {} scanlines",
                sprite.mode, width_pixels, sprite.width_bytes, sprite.height_lines
            ))
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(
                "=============================================================================="
            )
        );

        let _ = writeln!(
            out,
            "{}",
            self.format_equate(&format!("{label}_width_pixels"), &width_pixels.to_string())
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(
                &format!("{label}_width_bytes"),
                &sprite.width_bytes.to_string()
            )
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(
                &format!("{label}_height_lines"),
                &sprite.height_lines.to_string()
            )
        );

        let pal_w =
            (((-i16::from(sprite.width_bytes)) as u8) & 0x1F) | ((sprite.subpalette & 0x07) << 5);
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(&format!("{label}_pal_w"), &format!("${pal_w:02X}"))
        );

        let total_size = usize::from(sprite.height_lines) * 256;
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(&format!("{label}_size_bytes"), &total_size.to_string())
        );

        let mode_val = match sprite.mode {
            crate::MariaMode::Mode160A => 0,
            crate::MariaMode::Mode320 => 1,
        };
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(&format!("{label}_mode"), &mode_val.to_string())
        );

        if let Some(bin) = bin_filename {
            let _ = writeln!(out);
            let _ = writeln!(out, "{}", self.format_align(256));
            let _ = writeln!(out, "{}", self.format_label(&format!("{label}_pages")));
            let _ = writeln!(out, "{}", self.format_incbin(bin));
        }

        out
    }

    /// Generates an include file (.inc) for an interleaved MARIA sprite bank.
    ///
    /// Defines equates for frame count, height, total size, and lookup tables
    /// (frame offset within page, frame width, frame palette/width word).
    /// If `bin_filename` is provided, also emits page alignment and an binary inclusion directive.
    fn write_sprite_bank_inc(
        &self,
        bank: &MariaSpriteBank,
        label: &str,
        bin_filename: Option<&str>,
    ) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(
                "=============================================================================="
            )
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(&format!("MARIA Sprite Bank Include: {label}"))
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(&format!(
                "Frames: {}, Height: {} scanlines, Mode: {:?}",
                bank.frames.len(),
                bank.height_lines,
                bank.mode
            ))
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_comment(
                "=============================================================================="
            )
        );

        let _ = writeln!(
            out,
            "{}",
            self.format_equate(
                &format!("{label}_frame_count"),
                &bank.frames.len().to_string()
            )
        );
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(
                &format!("{label}_height_lines"),
                &bank.height_lines.to_string()
            )
        );

        let total_size = usize::from(bank.height_lines) * 256;
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(&format!("{label}_size_bytes"), &total_size.to_string())
        );

        let mode_val = match bank.mode {
            crate::MariaMode::Mode160A => 0,
            crate::MariaMode::Mode320 => 1,
        };
        let _ = writeln!(
            out,
            "{}",
            self.format_equate(&format!("{label}_mode"), &mode_val.to_string())
        );

        // Frame lo offset table
        let _ = writeln!(out, "\n{}", self.format_label(&format!("{label}_frame_lo")));
        let offsets: Vec<u8> = bank.frames.iter().map(|f| f.offset_in_page).collect();
        let _ = writeln!(out, "{}", self.format_bytes(&offsets));

        // Frame width table (in bytes)
        let _ = writeln!(
            out,
            "\n{}",
            self.format_label(&format!("{label}_frame_width_bytes"))
        );
        let widths: Vec<u8> = bank.frames.iter().map(|f| f.width_bytes).collect();
        let _ = writeln!(out, "{}", self.format_bytes(&widths));

        // Frame width table (in pixels)
        let _ = writeln!(
            out,
            "\n{}",
            self.format_label(&format!("{label}_frame_width_pixels"))
        );
        let width_multiplier: u8 = match bank.mode {
            crate::MariaMode::Mode160A => 4,
            crate::MariaMode::Mode320 => 8,
        };
        let pixel_widths: Vec<u8> = bank
            .frames
            .iter()
            .map(|f| f.width_bytes.saturating_mul(width_multiplier))
            .collect();
        let _ = writeln!(out, "{}", self.format_bytes(&pixel_widths));

        // Alias for backwards compatibility
        let _ = writeln!(
            out,
            "\n{}",
            self.format_equate(
                &format!("{label}_frame_width"),
                &format!("{label}_frame_width_bytes")
            )
        );

        // Frame pal_w table
        let _ = writeln!(
            out,
            "\n{}",
            self.format_label(&format!("{label}_frame_pal_w"))
        );
        let pals: Vec<u8> = bank.frames.iter().map(|f| f.pal_w).collect();
        let _ = writeln!(out, "{}", self.format_bytes(&pals));

        if let Some(bin) = bin_filename {
            let _ = writeln!(
                out,
                "\n{}",
                self.format_comment("Interleaved 256-byte scanline pages")
            );
            let _ = writeln!(out, "{}", self.format_align(256));
            let _ = writeln!(out, "{}", self.format_label(&format!("{label}_pages")));
            let _ = writeln!(out, "{}", self.format_incbin(bin));
        }

        out
    }

    /// Generates full assembly source (.s) with all scanline data embedded as byte directives.
    fn write_sprite_asm(&self, sprite: &MariaSprite, label: &str) -> String {
        use std::fmt::Write as _;
        let mut out = self.write_sprite_inc(sprite, label, None);

        let _ = writeln!(out, "\n{}", self.format_align(256));
        let _ = writeln!(out, "{}", self.format_label(&format!("{label}_pages")));

        for (line_idx, line) in sprite.scanline_bytes.iter().enumerate() {
            let _ = writeln!(out, "{}", self.format_align(256));
            let _ = writeln!(
                out,
                "{}",
                self.format_label(&format!("{label}_line_{line_idx:02}"))
            );
            let _ = writeln!(out, "{}", self.format_bytes(line));
        }

        out
    }

    /// Generates full assembly source (.s) for a sprite bank with all pages embedded as byte directives.
    fn write_sprite_bank_asm(&self, bank: &MariaSpriteBank, label: &str) -> String {
        use std::fmt::Write as _;
        let mut out = self.write_sprite_bank_inc(bank, label, None);

        let total_width: usize = bank.frames.iter().map(|f| usize::from(f.width_bytes)).sum();
        let occupied_bytes = total_width.min(256);

        let _ = writeln!(
            out,
            "\n{}",
            self.format_comment("Interleaved 256-byte scanline pages")
        );
        let _ = writeln!(out, "{}", self.format_align(256));
        let _ = writeln!(out, "{}", self.format_label(&format!("{label}_pages")));

        for (page_idx, page) in bank.pages.iter().enumerate() {
            let _ = writeln!(out, "{}", self.format_align(256));
            let _ = writeln!(
                out,
                "{}",
                self.format_label(&format!("{label}_page_{page_idx:02}"))
            );
            let active_slice = &page[..occupied_bytes];
            for chunk in active_slice.chunks(16) {
                let _ = writeln!(out, "{}", self.format_bytes(chunk));
            }
        }

        out
    }
}

/// ca65 assembler code generator.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ca65Writer;

impl AssemblerWriter for Ca65Writer {
    fn name(&self) -> &'static str {
        "ca65"
    }

    fn format_comment(&self, text: &str) -> String {
        format!("; {text}")
    }

    fn format_equate(&self, name: &str, value: &str) -> String {
        format!("{name} = {value}")
    }

    fn format_align(&self, boundary: usize) -> String {
        format!(".align ${boundary:X}")
    }

    fn format_label(&self, name: &str) -> String {
        format!("{name}:")
    }

    fn format_bytes(&self, bytes: &[u8]) -> String {
        let hex_vals: Vec<String> = bytes.iter().map(|b| format!("${b:02X}")).collect();
        format!("    .byte {}", hex_vals.join(", "))
    }

    fn format_incbin(&self, filename: &str) -> String {
        format!("    .incbin \"{filename}\"")
    }
}

/// DASM assembler code generator.
#[derive(Debug, Clone, Copy, Default)]
pub struct DasmWriter;

impl AssemblerWriter for DasmWriter {
    fn name(&self) -> &'static str {
        "dasm"
    }

    fn format_comment(&self, text: &str) -> String {
        format!("; {text}")
    }

    fn format_equate(&self, name: &str, value: &str) -> String {
        format!("{name} EQU {value}")
    }

    fn format_align(&self, boundary: usize) -> String {
        format!("    ALIGN {boundary}")
    }

    fn format_label(&self, name: &str) -> String {
        format!("{name}:")
    }

    fn format_bytes(&self, bytes: &[u8]) -> String {
        let hex_vals: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
        format!("    HEX {}", hex_vals.join(" "))
    }

    fn format_incbin(&self, filename: &str) -> String {
        format!("    INCBIN \"{filename}\"")
    }
}

/// MADS (Mad Assembler) code generator.
#[derive(Debug, Clone, Copy, Default)]
pub struct MadsWriter;

impl AssemblerWriter for MadsWriter {
    fn name(&self) -> &'static str {
        "mads"
    }

    fn format_comment(&self, text: &str) -> String {
        format!("; {text}")
    }

    fn format_equate(&self, name: &str, value: &str) -> String {
        format!("{name} = {value}")
    }

    fn format_align(&self, boundary: usize) -> String {
        format!(".align ${boundary:X}")
    }

    fn format_label(&self, name: &str) -> String {
        format!("{name}:")
    }

    fn format_bytes(&self, bytes: &[u8]) -> String {
        let hex_vals: Vec<String> = bytes.iter().map(|b| format!("${b:02X}")).collect();
        format!("    .byte {}", hex_vals.join(", "))
    }

    fn format_incbin(&self, filename: &str) -> String {
        format!("    ins \"{filename}\"")
    }
}
