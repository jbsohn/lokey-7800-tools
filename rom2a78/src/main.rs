use clap::{Args, Parser, Subcommand};
use rom2a78::{
    build_a78_header, decode_a78_header, Config, ControllerType, SaveDevice, SlotPassthrough,
    TvType,
};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "rom2a78",
    version,
    about = "Atari 7800 .a78 ROM header utility adhering to the 8BitDev specification",
    long_about = "Created to handle headers for the lokey-2149 cart and fully compatible with \
                  all header items in the 8BitDev specification (https://7800.8bitdev.org/index.php/A78_Header_Specification).\n\n\
                  Generates, inspects, and strips the 128-byte Atari 7800 .a78 header \
                  recognised by emulators (ProSystem, A7800, MAME, JS7800) and flash carts (Concerto 7800)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    generate_args: GenerateArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a 128-byte header and combine with ROM data to produce an .a78 file
    Generate(GenerateArgs),

    /// Inspect and decode the 128-byte .a78 header of an existing ROM file
    Inspect {
        /// Path to the .a78 ROM file
        file: PathBuf,
    },

    /// Strip the 128-byte .a78 header from a file and save raw binary data
    Strip {
        /// Input .a78 ROM file
        #[arg(short, long)]
        input: PathBuf,

        /// Output raw binary file (.bin / .rom)
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Args, Debug, Default)]
struct GenerateArgs {
    /// Raw ROM binary input (.bin or .rom)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output .a78 file path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// JSON config file for header fields
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Header specification version (1, 3, or 4)
    #[arg(long = "spec-version", visible_alias = "header-version")]
    spec_version: Option<u8>,

    /// Cart title (up to 32 ASCII characters)
    #[arg(long)]
    title: Option<String>,

    /// Mapper ID (0=Linear 32K, 1=YM-IOA, 2=SuperGame, 3=Activision, 4=Absolute, 5=RAMBank, 6..255=Custom)
    #[arg(long)]
    mapper: Option<u8>,

    /// Mapper options / flags byte (offset 65)
    #[arg(long, visible_alias = "mapper-flags")]
    mapper_opts: Option<u8>,

    /// Cart type 16-bit word (offsets 53/54)
    #[arg(long)]
    cart_type: Option<u16>,

    /// Audio hardware word (offsets 66/67)
    #[arg(long)]
    audio: Option<u16>,

    /// Interrupt / peripheral 16-bit word (offsets 68/69)
    #[arg(long, visible_alias = "interrupt-flags")]
    interrupt: Option<u16>,

    /// Enable YM2149 sound chip at $0800
    #[arg(long)]
    ym2149: bool,

    /// Disable YM2149 sound chip at $0800
    #[arg(long)]
    no_ym2149: bool,

    /// Enable POKEY sound chip at $4000
    #[arg(long, visible_alias = "pokey4000")]
    pokey_4000: bool,

    /// Disable POKEY sound chip at $4000
    #[arg(long, visible_alias = "no-pokey4000")]
    no_pokey_4000: bool,

    /// Enable POKEY sound chip at $4500
    #[arg(long, visible_alias = "pokey4500")]
    pokey_4500: bool,

    /// Disable POKEY sound chip at $4500
    #[arg(long, visible_alias = "no-pokey4500")]
    no_pokey_4500: bool,

    /// Enable High Score Cartridge (HSC) save device
    #[arg(long)]
    hsc: bool,

    /// Disable High Score Cartridge (HSC) save device
    #[arg(long)]
    no_hsc: bool,

    /// Enable SaveKey / AtariVox EEPROM save device
    #[arg(long)]
    savekey: bool,

    /// Disable SaveKey / AtariVox EEPROM save device
    #[arg(long)]
    no_savekey: bool,

    /// Enable XM Expansion Module passthrough
    #[arg(long)]
    xm: bool,

    /// Disable XM Expansion Module passthrough
    #[arg(long)]
    no_xm: bool,

    /// TV type (ntsc, pal)
    #[arg(long, value_enum)]
    tv_type: Option<TvType>,

    /// Controller 1 type
    #[arg(long, value_enum)]
    controller_1: Option<ControllerType>,

    /// Controller 2 type
    #[arg(long, value_enum)]
    controller_2: Option<ControllerType>,

    /// Save device (none, hsc, savekey/atarivox)
    #[arg(long, value_enum)]
    save_device: Option<SaveDevice>,

    /// Passthrough / expansion slot (none, xm)
    #[arg(long, value_enum)]
    slot_passthrough: Option<SlotPassthrough>,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Inspect { file }) => {
            let data = fs::read(&file)
                .unwrap_or_else(|e| fatal(&format!("Cannot read '{}': {e}", file.display())));
            match decode_a78_header(&data) {
                Ok(info) => print!("{info}"),
                Err(e) => fatal(&e),
            }
        }
        Some(Commands::Strip { input, output }) => {
            let data = fs::read(&input)
                .unwrap_or_else(|e| fatal(&format!("Cannot read '{}': {e}", input.display())));
            if data.len() <= 128 {
                fatal("File is too small to contain a 128-byte header and ROM data.");
            }
            fs::write(&output, &data[128..])
                .unwrap_or_else(|e| fatal(&format!("Cannot write '{}': {e}", output.display())));
            println!(
                "Stripped 128-byte header from {} -> saved {} KB ROM to {}",
                input.display(),
                (data.len() - 128) / 1024,
                output.display()
            );
        }
        Some(Commands::Generate(args)) => {
            run_generate(args);
        }
        None => {
            run_generate(cli.generate_args);
        }
    }
}

fn run_generate(args: GenerateArgs) {
    let mut cfg = if let Some(cfg_path) = &args.config {
        let json = fs::read_to_string(cfg_path).unwrap_or_else(|e| {
            fatal(&format!("Cannot read config '{}': {e}", cfg_path.display()))
        });
        serde_json::from_str::<Config>(&json)
            .unwrap_or_else(|e| fatal(&format!("Invalid config JSON: {e}")))
    } else {
        Config::default()
    };

    let input_path = args
        .input
        .or_else(|| cfg.input.clone())
        .unwrap_or_else(|| {
            fatal("Missing input ROM binary path. Pass --input <PATH> or set \"input\": \"path\" in JSON config.");
        });
    let output_path = args
        .output
        .or_else(|| cfg.output.clone())
        .unwrap_or_else(|| {
            fatal("Missing output .a78 path. Pass --output <PATH> or set \"output\": \"path\" in JSON config.");
        });

    if let Some(v) = args.spec_version {
        cfg.version = v;
    }
    if let Some(t) = args.title {
        cfg.title = Some(t);
    }
    if let Some(m) = args.mapper {
        cfg.mapper = m;
    }
    if let Some(a) = args.audio {
        cfg.audio = a;
    }
    if args.no_ym2149 {
        cfg.audio &= !0x0800;
        cfg.ym2149 = false;
    } else if args.ym2149 || cfg.ym2149 {
        cfg.audio |= 0x0800;
    }

    if args.no_pokey_4000 {
        cfg.audio &= !0x0001;
        cfg.pokey_4000 = false;
    } else if args.pokey_4000 || cfg.pokey_4000 {
        cfg.audio |= 0x0001;
    }

    if args.no_pokey_4500 {
        cfg.audio &= !0x0002;
        cfg.pokey_4500 = false;
    } else if args.pokey_4500 || cfg.pokey_4500 {
        cfg.audio |= 0x0002;
    }

    if args.no_hsc {
        if cfg.save_device == SaveDevice::Hsc {
            cfg.save_device = SaveDevice::None;
        }
        cfg.hsc = false;
    } else if args.hsc || cfg.hsc {
        cfg.save_device = SaveDevice::Hsc;
    }

    if args.no_savekey {
        if cfg.save_device == SaveDevice::Savekey {
            cfg.save_device = SaveDevice::None;
        }
        cfg.savekey = false;
    } else if args.savekey || cfg.savekey {
        cfg.save_device = SaveDevice::Savekey;
    }

    if args.no_xm {
        cfg.slot_passthrough = SlotPassthrough::None;
        cfg.xm = false;
    } else if args.xm || cfg.xm {
        cfg.slot_passthrough = SlotPassthrough::Xm;
    }
    if let Some(ct) = args.cart_type {
        cfg.cart_type = ct;
    }
    if let Some(tv) = args.tv_type {
        cfg.tv_type = tv;
    }
    if let Some(c1) = args.controller_1 {
        cfg.controller_1 = c1;
    }
    if let Some(c2) = args.controller_2 {
        cfg.controller_2 = c2;
    }
    if let Some(sd) = args.save_device {
        cfg.save_device = sd;
    }
    if let Some(sp) = args.slot_passthrough {
        cfg.slot_passthrough = sp;
    }
    if let Some(mo) = args.mapper_opts {
        cfg.mapper_opts = mo;
    }
    if let Some(irq) = args.interrupt {
        cfg.interrupt = irq;
    }

    let raw = fs::read(&input_path)
        .unwrap_or_else(|e| fatal(&format!("Cannot read '{}': {e}", input_path.display())));

    let rom_data: Vec<u8> = match cfg.mapper {
        0 => {
            if raw.len() > 32768 {
                eprintln!(
                    "Warning: input is {} bytes but mapper is 0 (linear/fixed 32K) — \
                     only the top 32 KB will be kept. Use --mapper 1..255 for banked images.",
                    raw.len()
                );
            }
            let mut rom = vec![0xFFu8; 32768];
            let copy_len = raw.len().min(32768);
            let dst_start = 32768 - copy_len;
            let src_start = raw.len() - copy_len;
            rom[dst_start..].copy_from_slice(&raw[src_start..]);
            rom
        }
        1 => {
            if raw.len() != 128 * 1024 && raw.len() != 256 * 1024 {
                eprintln!(
                    "Warning: Mapper 1 (YM-IOA banked) typically uses 128 KB or 256 KB, got {} bytes.",
                    raw.len()
                );
            }
            raw
        }
        2 => {
            if raw.len() != 128 * 1024 && raw.len() != 256 * 1024 && raw.len() != 512 * 1024 {
                eprintln!(
                    "Warning: Mapper 2 (SuperGame banked) typically uses 128 KB, 256 KB, or 512 KB, got {} bytes.",
                    raw.len()
                );
            }
            raw
        }
        3 => {
            if raw.len() != 128 * 1024 {
                eprintln!(
                    "Warning: Mapper 3 (Activision banked) typically uses 128 KB, got {} bytes.",
                    raw.len()
                );
            }
            raw
        }
        4 => {
            if raw.len() != 64 * 1024 {
                eprintln!(
                    "Warning: Mapper 4 (Absolute banked) typically uses 64 KB, got {} bytes.",
                    raw.len()
                );
            }
            raw
        }
        custom_mapper => {
            println!(
                "Note: Using custom/experimental mapper ID {custom_mapper} — passing {} bytes ROM payload.",
                raw.len()
            );
            raw
        }
    };

    let header = build_a78_header(&cfg, rom_data.len() as u32).unwrap_or_else(|e| fatal(&e));

    let mut out: Vec<u8> = Vec::with_capacity(128 + rom_data.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&rom_data);

    fs::write(&output_path, &out)
        .unwrap_or_else(|e| fatal(&format!("Cannot write '{}': {e}", output_path.display())));

    println!(
        "Generated {} (128-byte header + {} KB ROM)",
        output_path.display(),
        rom_data.len() / 1024
    );
}

fn fatal(msg: &str) -> ! {
    eprintln!("Error: {msg}");
    std::process::exit(1);
}
