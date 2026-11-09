use a78gfx::{
    AssemblerType, GfxFile, IndexedGraphic, MariaMode, MariaSpriteBank, PaletteFile, RgbaGraphic,
};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
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

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ScaleFilter {
    /// Copy the nearest already-quantized pixel (fast; thin strokes can break up)
    Nearest,
    /// Box-filter the master and keep the palette color covering each pixel
    /// (needs --palette and a PNG). Thin strokes keep partial coverage, and
    /// neighbouring colors are not blended into a third index.
    Area,
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

        /// Output 256-byte page-strided binary layout (default). `--page-strided=false` writes contiguous scanlines (for .bin outputs)
        #[arg(
            long,
            default_value_t = true,
            action = ArgAction::Set,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "true"
        )]
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

        /// One scale frame per scanline, from the full canvas height down to this many
        /// lines (inclusive). The sprite shrinks by one pixel per frame.
        /// Mutually exclusive with --scales.
        #[arg(long, value_name = "MIN_LINES")]
        scale_lines: Option<usize>,

        /// Apply pixel aspect ratio (PAR) correction for 160A rectangular pixels (2:1)
        #[arg(long, default_value_t = true)]
        par: bool,

        /// Directory to dump each baked frame as a PNG for visual inspection (debug)
        #[arg(long, value_name = "DIR")]
        dump_frames: Option<PathBuf>,

        /// How --scales frames are resampled
        #[arg(long, value_enum, default_value_t = ScaleFilter::Nearest)]
        scale_filter: ScaleFilter,

        /// With --scale-filter area: an output pixel whose ink coverage rounds to less than
        /// this (0..255) becomes transparent. Lower keeps thin or partly covered strokes;
        /// higher drops them.
        #[arg(long, default_value_t = 128, value_name = "0..255")]
        alpha_threshold: u8,
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

fn dump_frame(dir: &Path, name: &str, graphic: &IndexedGraphic) {
    let path = dir.join(format!("{name}.png"));
    graphic
        .save_debug_png(&path)
        .unwrap_or_else(|e| fatal(format!("failed to write '{}': {e}", path.display())));
    println!("Dumped frame -> '{}'", path.display());
}

/// One scale factor per scanline, largest first: `height/height`, `(height-1)/height`, ...
fn scale_line_steps(height: usize, min_lines: usize) -> Vec<(String, f32)> {
    (min_lines..=height)
        .rev()
        .map(|lines| {
            let factor = lines as f32 / height as f32;
            (lines.to_string(), factor)
        })
        .collect()
}

fn parse_scale_list(scales_str: &str) -> Vec<(String, f32)> {
    let mut out = Vec::new();
    for s_part in scales_str.split(',') {
        let s_str = s_part.trim();
        if s_str.is_empty() {
            continue;
        }
        let s_val: f32 = s_str
            .parse()
            .unwrap_or_else(|_| fatal(format!("invalid scale value '{s_str}' (expected float)")));
        out.push((s_str.to_string(), s_val));
    }
    out
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
            scale_lines,
            par,
            dump_frames,
            scale_filter,
            alpha_threshold,
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

            // Compensate for 160A's 2:1 (wide) pixels before deriving any frame, so the
            // sprite renders square on hardware, matching the `convert --par` path. 320 mode
            // is unchanged. The area scaler below works in source pixels, so it must also see
            // the corrected dimensions rather than the raw PNG.
            if par {
                graphic = graphic.apply_par_correction(maria_mode);
            }

            let subpalette_u8 = u8::try_from(subpalette & 0x07).unwrap_or(0);
            let base_sprite = graphic
                .to_maria_sprite(maria_mode)
                .unwrap_or_else(|e| fatal(format!("initial conversion error: {e}")));

            let mut bank = MariaSpriteBank::new(maria_mode, base_sprite.height_lines);

            if let Some(ref dir) = dump_frames {
                std::fs::create_dir_all(dir).unwrap_or_else(|e| {
                    fatal(format!("failed to create '{}': {e}", dir.display()))
                });
                dump_frame(dir, &format!("{label}_source"), &graphic);
            }

            // 3. Add rotation frames
            if angles > 0 {
                let step_deg = 360.0 / (angles as f32);
                for i in 0..angles {
                    let deg = (i as f32) * step_deg;
                    let rot = graphic.rotate(deg, par);
                    if let Some(ref dir) = dump_frames {
                        dump_frame(dir, &format!("{label}_rot_{i:02}"), &rot);
                    }
                    let mut rot_sprite = rot
                        .to_maria_sprite(maria_mode)
                        .unwrap_or_else(|e| fatal(format!("rotation error at {deg} deg: {e}")));
                    rot_sprite.subpalette = subpalette_u8;
                    bank.add_frame(&rot_sprite, format!("{label}_rot_{i:02}"))
                        .unwrap_or_else(|e| fatal(format!("failed to add rotation frame: {e}")));
                }
            }

            // 4. Add scale frames if provided. Area filtering samples the original PNG
            // once, straight onto the (resized, PAR-corrected) canvas. A second resample
            // of an already-shrunk image is what breaks thin strokes.
            let scale_specs = if let Some(min_lines) = scale_lines {
                if scales.is_some() {
                    fatal("--scale-lines and --scales cannot be used together");
                }
                if graphic.height == 0 || min_lines == 0 || min_lines > graphic.height {
                    fatal(format!(
                        "--scale-lines {min_lines} is outside 1..={}",
                        graphic.height
                    ));
                }
                scale_line_steps(graphic.height, min_lines)
            } else if let Some(ref scales_str) = scales {
                parse_scale_list(scales_str)
            } else {
                Vec::new()
            };

            if !scale_specs.is_empty() {
                let area_master = if scale_filter == ScaleFilter::Area {
                    let pal = loaded_palette
                        .as_ref()
                        .unwrap_or_else(|| fatal("--scale-filter area requires --palette"));
                    let rgba = RgbaGraphic::load_png(&input).unwrap_or_else(|e| {
                        fatal(format!(
                            "--scale-filter area needs a PNG input, failed on '{}': {e}",
                            input.display()
                        ))
                    });
                    Some((rgba, pal))
                } else {
                    None
                };

                for (s_str, s_val) in scale_specs {
                    let scaled = if let Some((ref rgba, pal)) = area_master {
                        let canvas_w = graphic.width;
                        let canvas_h = graphic.height;
                        let fx = f64::from(s_val) * canvas_w as f64 / rgba.width as f64;
                        let fy = f64::from(s_val) * canvas_h as f64 / rgba.height as f64;
                        rgba.scale_area_indexed(
                            canvas_w,
                            canvas_h,
                            fx,
                            fy,
                            pal,
                            subpalette,
                            maria_mode,
                            alpha_threshold,
                        )
                    } else {
                        graphic.scale_centered(s_val, s_val)
                    };
                    if let Some(ref dir) = dump_frames {
                        dump_frame(dir, &format!("{label}_scale_{s_str}"), &scaled);
                    }
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

    fn parse_page_strided(extra: &[&str]) -> Option<bool> {
        let mut args = vec!["a78gfx", "convert", "ship.png", "-o", "ship.bin"];
        args.extend_from_slice(extra);
        match Cli::try_parse_from(args) {
            Ok(Cli {
                command: Commands::Convert { page_strided, .. },
            }) => Some(page_strided),
            _ => None,
        }
    }

    #[test]
    fn test_cli_convert_page_strided_flag() {
        assert_eq!(parse_page_strided(&[]), Some(true));
        assert_eq!(parse_page_strided(&["--page-strided"]), Some(true));
        assert_eq!(parse_page_strided(&["--page-strided=true"]), Some(true));
        assert_eq!(parse_page_strided(&["--page-strided=false"]), Some(false));
        // require_equals: a bare value must not be swallowed as the flag's argument.
        assert_eq!(parse_page_strided(&["--page-strided", "false"]), None);
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
    fn test_scale_line_steps_are_one_pixel_apart() {
        let steps = scale_line_steps(48, 8);
        assert_eq!(steps.len(), 41);
        assert_eq!(steps.first().map(|(_, f)| *f), Some(1.0));
        assert_eq!(steps.last().map(|(label, _)| label.as_str()), Some("8"));
        let heights: Vec<f32> = steps.iter().map(|(_, f)| f * 48.0).collect();
        for pair in heights.windows(2) {
            let delta = pair[0] - pair[1];
            assert!(
                (delta - 1.0).abs() < 1e-4,
                "height step {delta} is not one scanline"
            );
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
