use a78gfx::{AssemblerType, GfxFile, MariaMode, MariaSpriteBank, PaletteFile};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser, Debug)]
#[command(
    name = "a78gfx",
    about = "Atari 7800 MARIA graphics asset compiler and ca65/dasm code generator",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ModeArg {
    #[value(name = "160A", alias = "160a")]
    Mode160A,
    #[value(name = "320", alias = "320a", alias = "320A")]
    Mode320,
}

impl From<ModeArg> for MariaMode {
    fn from(arg: ModeArg) -> Self {
        match arg {
            ModeArg::Mode160A => MariaMode::Mode160A,
            ModeArg::Mode320 => MariaMode::Mode320,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Convert an input graphic into a MARIA sprite binary (.bin) and include header (.inc)
    Convert {
        /// Path to input graphic file (PNG, BMP, etc.)
        #[arg(value_name = "INPUT_FILE")]
        input: PathBuf,

        /// Output file path (.bin for binary, .inc for include header, or .s for assembly)
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output: Option<PathBuf>,

        /// Explicit output path for binary file (.bin)
        #[arg(long, value_name = "BIN_FILE")]
        bin: Option<PathBuf>,

        /// Explicit output path for assembler include header (.inc)
        #[arg(long, visible_alias = "inc", value_name = "HEADER_FILE")]
        header: Option<PathBuf>,

        /// Target assembler syntax for generated include/assembly file
        #[arg(short = 'a', long, value_enum, default_value_t = AssemblerType::Ca65)]
        assembler: AssemblerType,

        /// Custom filename or path string to embed in the .incbin directive
        #[arg(long, value_name = "INCBIN_PATH")]
        incbin_path: Option<String>,

        /// Suppress generating include header (.inc)
        #[arg(long)]
        no_header: bool,

        /// Suppress generating binary file (.bin)
        #[arg(long)]
        no_bin: bool,

        /// Assembly label prefix (defaults to input file stem)
        #[arg(short, long, value_name = "LABEL")]
        label: Option<String>,

        /// Graphics mode
        #[arg(short, long, value_enum, default_value_t = ModeArg::Mode160A)]
        mode: ModeArg,

        /// Path to palette file (.gpl) for color matching and hardware register generation
        #[arg(short = 'p', long, value_name = "PALETTE_FILE")]
        palette: Option<PathBuf>,

        /// Subpalette slot index (0..7) to target for 160A sprites (sets DL header bits 7..5)
        #[arg(long, default_value_t = 0)]
        subpalette: usize,

        /// Clockwise rotation in degrees (e.g. 45.0, 90.0)
        #[arg(long, default_value_t = 0.0)]
        rotate: f32,

        /// Scale multiplier (e.g. 0.5 for 50%, 2.0 for 200%)
        #[arg(long, default_value_t = 1.0)]
        scale: f32,

        /// Resize graphic to WIDTHxHEIGHT (e.g. 16x16)
        #[arg(long, value_name = "WIDTHxHEIGHT")]
        resize: Option<String>,

        /// Mirror sprite horizontally
        #[arg(long)]
        flip_x: bool,

        /// Mirror sprite vertically
        #[arg(long)]
        flip_y: bool,

        /// Apply pixel aspect ratio (PAR) correction for non-square pixel modes (compresses 1:1 square source images by 50% for 160A)
        #[arg(long, visible_alias = "square-source")]
        par: bool,

        /// Output 256-byte page-strided binary layout instead of contiguous (for .bin outputs)
        #[arg(long, default_value_t = true)]
        page_strided: bool,
    },

    /// Bake discrete transformations (rotations/scales) into an interleaved MARIA sprite bank
    Bake {
        /// Path to input graphic file (PNG, BMP, etc.)
        #[arg(value_name = "INPUT_FILE")]
        input: PathBuf,

        /// Output file path (.bin for binary, .inc for include header, or .s for assembly)
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output: Option<PathBuf>,

        /// Explicit output path for binary file (.bin)
        #[arg(long, value_name = "BIN_FILE")]
        bin: Option<PathBuf>,

        /// Output path for assembler include header (.inc)
        #[arg(long, visible_alias = "inc", value_name = "HEADER_FILE")]
        header: Option<PathBuf>,

        /// Target assembler syntax for generated include/assembly file
        #[arg(short = 'a', long, value_enum, default_value_t = AssemblerType::Ca65)]
        assembler: AssemblerType,

        /// Custom filename or path string to embed in the .incbin directive
        #[arg(long, value_name = "INCBIN_PATH")]
        incbin_path: Option<String>,

        /// Suppress generating include header (.inc)
        #[arg(long)]
        no_header: bool,

        /// Suppress generating binary file (.bin)
        #[arg(long)]
        no_bin: bool,

        /// Assembly label prefix (defaults to input file stem)
        #[arg(short, long, value_name = "LABEL")]
        label: Option<String>,

        /// Graphics mode
        #[arg(short, long, value_enum, default_value_t = ModeArg::Mode160A)]
        mode: ModeArg,

        /// Path to palette file (.gpl) for color matching and hardware register generation
        #[arg(short = 'p', long, value_name = "PALETTE_FILE")]
        palette: Option<PathBuf>,

        /// Subpalette slot index (0..7) to target for 160A sprites (sets DL header bits 7..5)
        #[arg(long, default_value_t = 0)]
        subpalette: usize,

        /// Number of discrete rotation angles (e.g. 16 for 22.5-degree steps)
        #[arg(long, default_value_t = 16)]
        angles: usize,

        /// Resize graphic to WIDTHxHEIGHT (e.g. 16x16) before baking
        #[arg(long, value_name = "WIDTHxHEIGHT")]
        resize: Option<String>,

        /// Optional comma-separated list of scale factors (e.g. "0.5,0.75,1.0,1.25")
        #[arg(long)]
        scales: Option<String>,

        /// Apply pixel aspect ratio (PAR) correction for 160A rectangular pixels (2:1)
        #[arg(long, default_value_t = true)]
        par: bool,
    },
}

fn fatal(msg: impl std::fmt::Display) -> ! {
    eprintln!("error: {msg}");
    process::exit(1);
}

fn derive_label(input_path: &Path, user_label: Option<String>) -> String {
    if let Some(l) = user_label {
        l
    } else {
        input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("sprite")
            .to_string()
    }
}

fn parse_dimensions(s: &str) -> Option<(usize, usize)> {
    let mut parts = s.split(['x', 'X']);
    let w: usize = parts.next()?.trim().parse().ok()?;
    let h: usize = parts.next()?.trim().parse().ok()?;
    if parts.next().is_none() && w > 0 && h > 0 {
        Some((w, h))
    } else {
        None
    }
}

#[derive(Debug)]
struct OutputTargets {
    bin: Option<PathBuf>,
    header: Option<PathBuf>,
}

fn resolve_output_targets(
    output: Option<PathBuf>,
    bin: Option<PathBuf>,
    header: Option<PathBuf>,
    no_bin: bool,
    no_header: bool,
    inc_ext: &str,
) -> OutputTargets {
    let mut resolved_bin = bin;
    let mut resolved_header = header;

    if resolved_bin.is_none() && resolved_header.is_none() {
        if let Some(out) = output {
            let is_header = out
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("inc") || e.eq_ignore_ascii_case("h"));
            if is_header {
                resolved_header = Some(out.clone());
                if !no_bin {
                    resolved_bin = Some(out.with_extension("bin"));
                }
            } else {
                resolved_bin = Some(out.with_extension("bin"));
                if !no_header {
                    resolved_header = Some(out.with_extension(inc_ext));
                }
            }
        }
    } else if let Some(out) = output {
        let is_header = out
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("inc") || e.eq_ignore_ascii_case("h"));
        if is_header && resolved_header.is_none() {
            resolved_header = Some(out);
        } else if resolved_bin.is_none() {
            resolved_bin = Some(out.with_extension("bin"));
        }
    }

    if no_bin {
        resolved_bin = None;
    }
    if no_header {
        resolved_header = None;
    }

    OutputTargets {
        bin: resolved_bin,
        header: resolved_header,
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert {
            input,
            output,
            bin,
            header,
            assembler,
            incbin_path,
            no_header,
            no_bin,
            label,
            mode,
            palette,
            subpalette,
            rotate,
            scale,
            resize,
            flip_x,
            flip_y,
            par,
            page_strided,
        } => {
            let label = derive_label(&input, label);
            let writer = assembler.create_writer();
            let targets = resolve_output_targets(
                output,
                bin,
                header,
                no_bin,
                no_header,
                writer.default_include_extension(),
            );

            if targets.bin.is_none() && targets.header.is_none() {
                fatal("no output file specified: provide -o, --bin, or --header");
            }

            // 1. Load palette if provided
            let loaded_palette = if let Some(ref pal_path) = palette {
                let pal_file = PaletteFile::load(pal_path).unwrap_or_else(|e| {
                    fatal(format!(
                        "failed to load palette '{}': {e}",
                        pal_path.display()
                    ))
                });
                Some(pal_file.into_palette())
            } else {
                None
            };

            let maria_mode: MariaMode = mode.into();

            // 2. Load graphic file (quantized against palette if provided)
            let gfx_file = if let Some(ref pal) = loaded_palette {
                GfxFile::load_with_palette(&input, pal, subpalette, maria_mode)
                    .unwrap_or_else(|e| fatal(format!("failed to load '{}': {e}", input.display())))
            } else {
                GfxFile::load(&input)
                    .unwrap_or_else(|e| fatal(format!("failed to load '{}': {e}", input.display())))
            };
            let mut graphic = gfx_file.into_graphic();

            // 3. Apply transformations
            if let Some(ref res) = resize {
                let (w, h) = parse_dimensions(res).unwrap_or_else(|| {
                    fatal(format!(
                        "invalid resize '{res}': expected WIDTHxHEIGHT, e.g. 16x16"
                    ))
                });
                graphic = graphic.resize(w, h);
            }
            if flip_x {
                graphic = graphic.flip_h();
            }
            if flip_y {
                graphic = graphic.flip_v();
            }
            if (scale - 1.0).abs() > f32::EPSILON {
                graphic = graphic.scale(scale, scale);
            }
            if rotate.abs() > f32::EPSILON {
                graphic = graphic.rotate(rotate, par);
            } else if par {
                graphic = graphic.apply_par_correction(maria_mode);
            }

            // 4. Convert to MARIA sprite
            let subpalette_u8 = u8::try_from(subpalette & 0x07).unwrap_or(0);
            let mut sprite = graphic
                .to_maria_sprite(maria_mode)
                .unwrap_or_else(|e| fatal(format!("conversion error: {e}")));
            sprite.subpalette = subpalette_u8;

            // 5. Output binary file if requested
            if let Some(ref bin_p) = targets.bin {
                sprite.save_binary(bin_p, page_strided).unwrap_or_else(|e| {
                    fatal(format!("failed to write binary '{}': {e}", bin_p.display()))
                });
                println!(
                    "Compiled '{}' -> '{}' (binary: {} bytes wide, {} lines, strided: {page_strided})",
                    input.display(),
                    bin_p.display(),
                    sprite.width_bytes,
                    sprite.height_lines
                );
            }

            // 6. Output include header (.inc) if requested
            if let Some(ref header_p) = targets.header {
                let incbin_str = incbin_path.as_deref().or_else(|| {
                    targets
                        .bin
                        .as_ref()
                        .and_then(|bp| bp.file_name().and_then(|s| s.to_str()))
                });
                let mut header_text = sprite.write_inc(writer.as_ref(), &label, incbin_str);

                if let Some(ref pal) = loaded_palette {
                    header_text.push('\n');
                    header_text.push_str(&pal.format_subpalette_equates(
                        writer.as_ref(),
                        &label,
                        subpalette,
                    ));
                }

                std::fs::write(header_p, header_text.as_bytes()).unwrap_or_else(|e| {
                    fatal(format!(
                        "failed to write header '{}': {e}",
                        header_p.display()
                    ))
                });
                println!(
                    "Wrote {} include header -> '{}'",
                    writer.name(),
                    header_p.display()
                );
            }
        }
        Commands::Bake {
            input,
            output,
            bin,
            header,
            assembler,
            incbin_path,
            no_header,
            no_bin,
            label,
            mode,
            palette,
            subpalette,
            angles,
            resize,
            scales,
            par,
        } => {
            let label = derive_label(&input, label);
            let writer = assembler.create_writer();
            let targets = resolve_output_targets(
                output,
                bin,
                header,
                no_bin,
                no_header,
                writer.default_include_extension(),
            );

            if targets.bin.is_none() && targets.header.is_none() {
                fatal("no output file specified: provide -o, --bin, or --header");
            }

            // 1. Load palette if provided
            let loaded_palette = if let Some(ref pal_path) = palette {
                let pal_file = PaletteFile::load(pal_path).unwrap_or_else(|e| {
                    fatal(format!(
                        "failed to load palette '{}': {e}",
                        pal_path.display()
                    ))
                });
                Some(pal_file.into_palette())
            } else {
                None
            };

            let maria_mode: MariaMode = mode.into();

            // 2. Load graphic file (quantized against palette if provided)
            let gfx_file = if let Some(ref pal) = loaded_palette {
                GfxFile::load_with_palette(&input, pal, subpalette, maria_mode)
                    .unwrap_or_else(|e| fatal(format!("failed to load '{}': {e}", input.display())))
            } else {
                GfxFile::load(&input)
                    .unwrap_or_else(|e| fatal(format!("failed to load '{}': {e}", input.display())))
            };
            let mut graphic = gfx_file.into_graphic();

            if let Some(ref res) = resize {
                let (w, h) = parse_dimensions(res).unwrap_or_else(|| {
                    fatal(format!(
                        "invalid resize '{res}': expected WIDTHxHEIGHT, e.g. 16x16"
                    ))
                });
                graphic = graphic.resize(w, h);
            }

            let subpalette_u8 = u8::try_from(subpalette & 0x07).unwrap_or(0);
            let base_sprite = graphic
                .to_maria_sprite(maria_mode)
                .unwrap_or_else(|e| fatal(format!("initial conversion error: {e}")));

            let mut bank = MariaSpriteBank::new(maria_mode, base_sprite.height_lines);

            // 3. Add rotation frames
            if angles > 0 {
                let step_deg = 360.0 / (angles as f32);
                for i in 0..angles {
                    let deg = (i as f32) * step_deg;
                    let rot = graphic.rotate(deg, par);
                    let mut rot_sprite = rot
                        .to_maria_sprite(maria_mode)
                        .unwrap_or_else(|e| fatal(format!("rotation error at {deg} deg: {e}")));
                    rot_sprite.subpalette = subpalette_u8;
                    bank.add_frame(&rot_sprite, format!("{label}_rot_{i:02}"))
                        .unwrap_or_else(|e| fatal(format!("failed to add rotation frame: {e}")));
                }
            }

            // 4. Add scale frames if provided
            if let Some(scales_str) = scales {
                for s_part in scales_str.split(',') {
                    let s_str = s_part.trim();
                    if s_str.is_empty() {
                        continue;
                    }
                    let s_val: f32 = s_str.parse().unwrap_or_else(|_| {
                        fatal(format!("invalid scale value '{s_str}' (expected float)"))
                    });
                    let scaled = graphic.scale_centered(s_val, s_val);
                    let mut scaled_sprite = scaled
                        .to_maria_sprite(maria_mode)
                        .unwrap_or_else(|e| fatal(format!("scale error at {s_val}: {e}")));
                    scaled_sprite.subpalette = subpalette_u8;
                    bank.add_frame(&scaled_sprite, format!("{label}_scale_{s_str}"))
                        .unwrap_or_else(|e| fatal(format!("failed to add scale frame: {e}")));
                }
            }

            // 5. Output binary file if requested
            if let Some(ref bin_p) = targets.bin {
                bank.save_binary(bin_p).unwrap_or_else(|e| {
                    fatal(format!("failed to write binary '{}': {e}", bin_p.display()))
                });
                println!(
                    "Baked bank '{}' -> '{}' (binary: {} frames, {} lines, mode: {mode:?})",
                    input.display(),
                    bin_p.display(),
                    bank.frames.len(),
                    bank.height_lines
                );
            }

            // 6. Output include header (.inc) if requested
            if let Some(ref header_p) = targets.header {
                let incbin_str = incbin_path.as_deref().or_else(|| {
                    targets
                        .bin
                        .as_ref()
                        .and_then(|bp| bp.file_name().and_then(|s| s.to_str()))
                });
                let mut header_text = bank.write_inc(writer.as_ref(), &label, incbin_str);

                if let Some(ref pal) = loaded_palette {
                    header_text.push('\n');
                    header_text.push_str(&pal.format_subpalette_equates(
                        writer.as_ref(),
                        &label,
                        subpalette,
                    ));
                }

                std::fs::write(header_p, header_text.as_bytes()).unwrap_or_else(|e| {
                    fatal(format!(
                        "failed to write header '{}': {e}",
                        header_p.display()
                    ))
                });
                println!(
                    "Wrote {} include header -> '{}'",
                    writer.name(),
                    header_p.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_output_targets_bin_output() {
        let targets = resolve_output_targets(
            Some(PathBuf::from("ship.bin")),
            None,
            None,
            false,
            false,
            "inc",
        );
        assert_eq!(targets.bin, Some(PathBuf::from("ship.bin")));
        assert_eq!(targets.header, Some(PathBuf::from("ship.inc")));
    }

    #[test]
    fn test_resolve_output_targets_inc_output() {
        let targets = resolve_output_targets(
            Some(PathBuf::from("ship.inc")),
            None,
            None,
            false,
            false,
            "inc",
        );
        assert_eq!(targets.bin, Some(PathBuf::from("ship.bin")));
        assert_eq!(targets.header, Some(PathBuf::from("ship.inc")));
    }

    #[test]
    fn test_resolve_output_targets_base_output() {
        let targets =
            resolve_output_targets(Some(PathBuf::from("ship")), None, None, false, false, "inc");
        assert_eq!(targets.bin, Some(PathBuf::from("ship.bin")));
        assert_eq!(targets.header, Some(PathBuf::from("ship.inc")));
    }

    #[test]
    fn test_resolve_output_targets_explicit_and_flags() {
        let targets = resolve_output_targets(
            Some(PathBuf::from("ship.bin")),
            None,
            None,
            false,
            true,
            "inc",
        );
        assert_eq!(targets.bin, Some(PathBuf::from("ship.bin")));
        assert_eq!(targets.header, None);

        let targets2 = resolve_output_targets(
            None,
            Some(PathBuf::from("data.bin")),
            Some(PathBuf::from("defs.inc")),
            false,
            false,
            "inc",
        );
        assert_eq!(targets2.bin, Some(PathBuf::from("data.bin")));
        assert_eq!(targets2.header, Some(PathBuf::from("defs.inc")));
    }

    #[test]
    fn test_cli_convert_with_palette() {
        let args = [
            "a78gfx",
            "convert",
            "ship.png",
            "-o",
            "ship.bin",
            "-p",
            "game.gpl",
            "--subpalette",
            "3",
        ];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());
        if let Ok(Cli {
            command:
                Commands::Convert {
                    palette,
                    subpalette,
                    ..
                },
        }) = cli
        {
            assert_eq!(palette, Some(PathBuf::from("game.gpl")));
            assert_eq!(subpalette, 3);
        } else {
            assert!(matches!(
                cli,
                Ok(Cli {
                    command: Commands::Convert { .. }
                })
            ));
        }
    }

    #[test]
    fn test_cli_bake_with_palette() {
        let args = [
            "a78gfx",
            "bake",
            "ship.png",
            "-o",
            "bank.bin",
            "--palette",
            "game.gpl",
            "--subpalette",
            "1",
        ];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());
        if let Ok(Cli {
            command:
                Commands::Bake {
                    palette,
                    subpalette,
                    ..
                },
        }) = cli
        {
            assert_eq!(palette, Some(PathBuf::from("game.gpl")));
            assert_eq!(subpalette, 1);
        } else {
            assert!(matches!(
                cli,
                Ok(Cli {
                    command: Commands::Bake { .. }
                })
            ));
        }
    }
}
