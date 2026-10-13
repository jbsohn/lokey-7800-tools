use a78tool::{A78Header, Config, ControllerType, RomImage, SaveDevice, SlotPassthrough, TvType};
use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "a78tool",
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

#[allow(clippy::struct_excessive_bools)]
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

    /// Enable `SaveKey` / `AtariVox` EEPROM save device
    #[arg(long)]
    savekey: bool,

    /// Disable `SaveKey` / `AtariVox` EEPROM save device
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

fn main() -> std::process::ExitCode {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Inspect { file }) => run_inspect(&file),
        Some(Commands::Strip { input, output }) => run_strip(&input, &output),
        Some(Commands::Generate(args)) => run_generate(args),
        None => run_generate(cli.generate_args),
    }
}

fn run_inspect(file: &std::path::Path) -> Result<(), String> {
    let data = fs::read(file).map_err(|e| format!("Cannot read '{}': {e}", file.display()))?;
    let header = A78Header::from_bytes(&data).map_err(|e| e.to_string())?;
    print!("{header}");
    Ok(())
}

fn run_strip(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    let data = fs::read(input).map_err(|e| format!("Cannot read '{}': {e}", input.display()))?;
    let rom = RomImage::from_bytes(&data).map_err(|e| e.to_string())?;
    if !rom.has_header() {
        return Err("File does not contain a valid 128-byte .a78 header.".to_string());
    }
    fs::write(output, rom.payload())
        .map_err(|e| format!("Cannot write '{}': {e}", output.display()))?;
    println!(
        "Stripped 128-byte header from {} -> saved {} KB ROM to {}",
        input.display(),
        rom.payload().len() / 1024,
        output.display()
    );
    Ok(())
}

fn load_config(path: Option<&std::path::Path>) -> Result<Config, String> {
    if let Some(cfg_path) = path {
        let json = fs::read_to_string(cfg_path)
            .map_err(|e| format!("Cannot read config '{}': {e}", cfg_path.display()))?;
        serde_json::from_str::<Config>(&json).map_err(|e| format!("Invalid config JSON: {e}"))
    } else {
        Ok(Config::default())
    }
}

impl GenerateArgs {
    fn apply_to_config(&self, cfg: &mut Config) {
        cfg.version = self.spec_version.unwrap_or(cfg.version);
        cfg.title = self.title.clone().or_else(|| cfg.title.clone());
        cfg.mapper = self.mapper.unwrap_or(cfg.mapper);
        cfg.audio = self.audio.unwrap_or(cfg.audio);
        if self.no_ym2149 {
            cfg.audio &= !0x0800;
            cfg.ym2149 = false;
        } else if self.ym2149 || cfg.ym2149 {
            cfg.audio |= 0x0800;
        }

        if self.no_pokey_4000 {
            cfg.audio &= !0x0001;
            cfg.pokey_4000 = false;
        } else if self.pokey_4000 || cfg.pokey_4000 {
            cfg.audio |= 0x0001;
        }

        if self.no_pokey_4500 {
            cfg.audio &= !0x0002;
            cfg.pokey_4500 = false;
        } else if self.pokey_4500 || cfg.pokey_4500 {
            cfg.audio |= 0x0002;
        }

        if self.no_hsc {
            if cfg.save_device == SaveDevice::Hsc {
                cfg.save_device = SaveDevice::None;
            }
            cfg.hsc = false;
        } else if self.hsc || cfg.hsc {
            cfg.save_device = SaveDevice::Hsc;
        }

        if self.no_savekey {
            if cfg.save_device == SaveDevice::SaveKey {
                cfg.save_device = SaveDevice::None;
            }
            cfg.savekey = false;
        } else if self.savekey || cfg.savekey {
            cfg.save_device = SaveDevice::SaveKey;
        }

        if self.no_xm {
            cfg.slot_passthrough = SlotPassthrough::None;
            cfg.xm = false;
        } else if self.xm || cfg.xm {
            cfg.slot_passthrough = SlotPassthrough::Xm;
        }
        cfg.cart_type = self.cart_type.unwrap_or(cfg.cart_type);
        cfg.tv_type = self.tv_type.unwrap_or(cfg.tv_type);
        cfg.controller_1 = self.controller_1.unwrap_or(cfg.controller_1);
        cfg.controller_2 = self.controller_2.unwrap_or(cfg.controller_2);
        cfg.save_device = self.save_device.unwrap_or(cfg.save_device);
        cfg.slot_passthrough = self.slot_passthrough.unwrap_or(cfg.slot_passthrough);
        cfg.mapper_opts = self.mapper_opts.unwrap_or(cfg.mapper_opts);
        cfg.interrupt = self.interrupt.unwrap_or(cfg.interrupt);
    }
}

fn run_generate(args: GenerateArgs) -> Result<(), String> {
    let mut cfg = load_config(args.config.as_deref())?;
    args.apply_to_config(&mut cfg);

    let input_path = args
        .input
        .or_else(|| cfg.input.clone())
        .ok_or_else(|| {
            "Missing input ROM binary path. Pass --input <PATH> or set \"input\": \"path\" in JSON config.".to_string()
        })?;
    let output_path = args
        .output
        .or_else(|| cfg.output.clone())
        .ok_or_else(|| {
            "Missing output .a78 path. Pass --output <PATH> or set \"output\": \"path\" in JSON config.".to_string()
        })?;

    let raw = fs::read(&input_path)
        .map_err(|e| format!("Cannot read '{}': {e}", input_path.display()))?;

    let (rom_data, notes) = RomImage::prepare_mapper_payload(cfg.mapper, &raw);
    for note in notes {
        eprintln!("{note}");
    }

    let rom_data_len = u32::try_from(rom_data.len())
        .map_err(|_| "ROM data size is too large to fit in 32-bit integer".to_string())?;
    let header = A78Header::from_config(&cfg, rom_data_len);
    let rom = RomImage {
        header: Some(header),
        payload: rom_data,
    };

    fs::write(&output_path, rom.to_bytes())
        .map_err(|e| format!("Cannot write '{}': {e}", output_path.display()))?;

    println!(
        "Generated {} (128-byte header + {} KB ROM)",
        output_path.display(),
        rom.payload().len() / 1024
    );
    Ok(())
}
