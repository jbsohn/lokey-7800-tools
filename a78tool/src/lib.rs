use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::fmt::Write as _;

/// Originally created to handle headers for the lokey-ym2149 cart, but fully
/// adheres to all header fields in the 8BitDev.org Atari 7800 Header Specification:
/// <https://7800.8bitdev.org/index.php/A78_Header_Specification/>

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TvType {
    Ntsc = 0,
    Pal = 1,
}

impl fmt::Display for TvType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TvType::Ntsc => write!(f, "NTSC (0)"),
            TvType::Pal => write!(f, "PAL (1)"),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerType {
    None = 0,
    Joystick = 1,
    LightGun = 2,
    Paddle = 3,
    TrakBall = 4,
    Keypad = 5,
    Driving = 6,
    AmigaMouse = 7,
    StMouse = 8,
}

impl fmt::Display for ControllerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControllerType::None => write!(f, "None (0)"),
            ControllerType::Joystick => write!(f, "Joystick (1)"),
            ControllerType::LightGun => write!(f, "Light Gun (2)"),
            ControllerType::Paddle => write!(f, "Paddle (3)"),
            ControllerType::TrakBall => write!(f, "Trak Ball (4)"),
            ControllerType::Keypad => write!(f, "Keypad (5)"),
            ControllerType::Driving => write!(f, "Driving Controller (6)"),
            ControllerType::AmigaMouse => write!(f, "Amiga Mouse (7)"),
            ControllerType::StMouse => write!(f, "Atari ST Mouse (8)"),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveDevice {
    None = 0,
    Hsc = 1,
    SaveKey = 2,
}

impl fmt::Display for SaveDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveDevice::None => write!(f, "None (0)"),
            SaveDevice::Hsc => write!(f, "High Score Cartridge (1)"),
            SaveDevice::SaveKey => write!(f, "SaveKey / AtariVox EEPROM (2)"),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlotPassthrough {
    None = 0,
    Xm = 1,
}

impl fmt::Display for SlotPassthrough {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotPassthrough::None => write!(f, "None (0)"),
            SlotPassthrough::Xm => write!(f, "XM Expansion Module (1)"),
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub title: Option<String>,
    #[serde(default = "default_version")]
    pub version: u8,
    pub cart_type: u16,
    pub controller_1: ControllerType,
    pub controller_2: ControllerType,
    pub tv_type: TvType,
    pub save_device: SaveDevice,
    pub slot_passthrough: SlotPassthrough,
    pub mapper: u8,
    pub mapper_opts: u8,
    #[serde(default = "default_audio")]
    pub audio: u16,
    pub interrupt: u16,
    pub ym2149: bool,
    pub pokey_4000: bool,
    pub pokey_4500: bool,
    pub hsc: bool,
    pub savekey: bool,
    pub xm: bool,
}

fn default_version() -> u8 {
    4
}
fn default_audio() -> u16 {
    0x0800
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            title: None,
            version: default_version(),
            cart_type: 0,
            controller_1: ControllerType::Joystick,
            controller_2: ControllerType::Joystick,
            tv_type: TvType::Ntsc,
            save_device: SaveDevice::None,
            slot_passthrough: SlotPassthrough::None,
            mapper: 0,
            mapper_opts: 0,
            audio: default_audio(),
            interrupt: 0,
            ym2149: false,
            pokey_4000: false,
            pokey_4500: false,
            hsc: false,
            savekey: false,
            xm: false,
        }
    }
}

/// Builds a 128-byte `.a78` header from the given config and ROM size.
///
/// # Errors
///
/// This function is currently guaranteed to not return an error, but returns a `Result`
/// for future compatibility or field validation.
#[allow(clippy::cast_possible_truncation)]
pub fn build_a78_header(cfg: &Config, rom_size: u32) -> Result<[u8; 128], String> {
    let mut header = [0u8; 128];

    header[0] = cfg.version;
    header[1..17].copy_from_slice(b"ATARI7800       ");

    let title = cfg.title.as_deref().unwrap_or("YM2149 CART");
    let title_bytes: Vec<u8> = title.bytes().take(32).collect();
    header[17..17 + title_bytes.len()].copy_from_slice(&title_bytes);
    header[17 + title_bytes.len()..49].fill(0x20);

    header[49] = (rom_size >> 24) as u8;
    header[50] = (rom_size >> 16) as u8;
    header[51] = (rom_size >> 8) as u8;
    header[52] = rom_size as u8;

    let mut cart_type = cfg.cart_type;
    if cfg.audio & 0x0800 != 0 {
        cart_type |= 0x0004;
    }
    if cfg.audio & 0x0001 != 0 {
        cart_type |= 0x0008;
    }
    if cfg.mapper == 2 {
        cart_type |= 0x0010;
    }
    if cfg.save_device == SaveDevice::Hsc {
        cart_type |= 0x0080;
    }

    header[53] = (cart_type >> 8) as u8;
    header[54] = cart_type as u8;

    header[55] = cfg.controller_1 as u8;
    header[56] = cfg.controller_2 as u8;
    header[57] = cfg.tv_type as u8;
    header[58] = cfg.save_device as u8;

    header[63] = cfg.slot_passthrough as u8;
    header[64] = cfg.mapper;
    header[65] = cfg.mapper_opts;

    header[66] = (cfg.audio >> 8) as u8;
    header[67] = cfg.audio as u8;

    header[68] = (cfg.interrupt >> 8) as u8;
    header[69] = cfg.interrupt as u8;

    header[100..128].copy_from_slice(b"ACTUAL CART DATA STARTS HERE");

    Ok(header)
}

/// # Errors
pub fn decode_a78_header(header: &[u8]) -> Result<String, String> {
    if header.len() < 128 {
        return Err(format!(
            "File too small for .a78 header (got {} bytes, expected >= 128)",
            header.len()
        ));
    }

    let magic = &header[1..10];
    if magic != b"ATARI7800" {
        return Err(format!(
            "Invalid .a78 magic header: {:?}",
            String::from_utf8_lossy(magic)
        ));
    }

    let version = header[0];
    let title = String::from_utf8_lossy(&header[17..49]).trim().to_string();
    let rom_size = u32::from_be_bytes([header[49], header[50], header[51], header[52]]);
    let cart_type = u16::from_be_bytes([header[53], header[54]]);
    let c1 = match header[55] {
        0 => ControllerType::None,
        1 => ControllerType::Joystick,
        2 => ControllerType::LightGun,
        3 => ControllerType::Paddle,
        4 => ControllerType::TrakBall,
        5 => ControllerType::Keypad,
        6 => ControllerType::Driving,
        7 => ControllerType::AmigaMouse,
        8 => ControllerType::StMouse,
        other => return Err(format!("Unknown Controller 1 value: {other}")),
    };
    let c2 = match header[56] {
        0 => ControllerType::None,
        1 => ControllerType::Joystick,
        2 => ControllerType::LightGun,
        3 => ControllerType::Paddle,
        4 => ControllerType::TrakBall,
        5 => ControllerType::Keypad,
        6 => ControllerType::Driving,
        7 => ControllerType::AmigaMouse,
        8 => ControllerType::StMouse,
        other => return Err(format!("Unknown Controller 2 value: {other}")),
    };
    let tv = if header[57] == 1 {
        TvType::Pal
    } else {
        TvType::Ntsc
    };
    let save = match header[58] {
        1 => SaveDevice::Hsc,
        2 => SaveDevice::SaveKey,
        _ => SaveDevice::None,
    };
    let slot = if header[63] == 1 {
        SlotPassthrough::Xm
    } else {
        SlotPassthrough::None
    };

    let mapper = header[64];
    let mapper_opts = header[65];
    let audio = u16::from_be_bytes([header[66], header[67]]);
    let interrupt = u16::from_be_bytes([header[68], header[69]]);

    let mapper_name = match mapper {
        0 => "0 (Linear / Fixed 32K)",
        1 => "1 (YM-IOA Banked 128K/256K)",
        2 => "2 (SuperGame Banked 128K/256K/512K)",
        3 => "3 (Activision Banked 128K)",
        4 => "4 (Absolute Banked 64K)",
        5 => "5 (CPU RAM Banked)",
        _ => "Unknown Mapper",
    };

    let mut out = String::new();
    let _ = writeln!(out, "====================================================");
    let _ = writeln!(out, "       Atari 7800 .a78 Header Specification        ");
    let _ = writeln!(out, "====================================================");
    let _ = writeln!(out, "Header Version : {version}");
    let _ = writeln!(out, "Title          : {title}");
    let _ = writeln!(out, "ROM Size       : {rom_size} bytes ({} KB)", rom_size / 1024);
    let _ = writeln!(out, "Cart Type Word : 0x{cart_type:04X}");
    let _ = writeln!(out, "Controller 1   : {c1}");
    let _ = writeln!(out, "Controller 2   : {c2}");
    let _ = writeln!(out, "TV Format      : {tv}");
    let _ = writeln!(out, "Save Device    : {save}");
    let _ = writeln!(out, "Expansion Slot : {slot}");
    let _ = writeln!(out, "Mapper         : {mapper_name}");
    let _ = writeln!(out, "Mapper Opts    : 0x{mapper_opts:02X}");
    let _ = writeln!(out, "Audio Word     : 0x{audio:04X} (YM2149: {}, POKEY@4000: {}, POKEY@4500: {})",
        (audio & 0x0800) != 0,
        (audio & 0x0001) != 0,
        (audio & 0x0002) != 0
    );
    let _ = writeln!(out, "Interrupt Word : 0x{interrupt:04X}");
    let _ = writeln!(out, "====================================================");

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_header_signature_and_end_magic() {
        let cfg = Config::default();
        let header = build_a78_header(&cfg, 32768).unwrap();

        assert_eq!(&header[1..17], b"ATARI7800       ");
        assert_eq!(&header[100..128], b"ACTUAL CART DATA STARTS HERE");
    }

    #[test]
    fn test_spec_rom_size_encoding() {
        for (size, expected) in [
            (32768u32, [0x00, 0x00, 0x80, 0x00]),
            (65536u32, [0x00, 0x01, 0x00, 0x00]),
            (131_072_u32, [0x00, 0x02, 0x00, 0x00]),
            (262_144_u32, [0x00, 0x04, 0x00, 0x00]),
            (524_288_u32, [0x00, 0x08, 0x00, 0x00]),
        ] {
            let cfg = Config::default();
            let header = build_a78_header(&cfg, size).unwrap();
            assert_eq!(&header[49..53], &expected);
        }
    }

    #[test]
    fn test_title_formatting() {
        let cfg = Config {
            title: Some("SHORT".to_string()),
            ..Config::default()
        };
        let header = build_a78_header(&cfg, 32768).unwrap();
        assert_eq!(&header[17..22], b"SHORT");
        assert_eq!(&header[22..49], &[0x20; 27]);

        let cfg = Config {
            title: Some("A VERY LONG TITLE THAT EXCEEDS THIRTY-TWO CHARACTERS".to_string()),
            ..Config::default()
        };
        let header = build_a78_header(&cfg, 32768).unwrap();
        assert_eq!(&header[17..49], b"A VERY LONG TITLE THAT EXCEEDS T");

        let cfg = Config {
            title: None,
            ..Config::default()
        };
        let header = build_a78_header(&cfg, 32768).unwrap();
        assert_eq!(&header[17..28], b"YM2149 CART");
    }

    #[test]
    fn test_all_controller_enum_variants() {
        let cases = [
            (ControllerType::None, 0u8, "none"),
            (ControllerType::Joystick, 1u8, "joystick"),
            (ControllerType::LightGun, 2u8, "lightgun"),
            (ControllerType::Paddle, 3u8, "paddle"),
            (ControllerType::TrakBall, 4u8, "trakball"),
            (ControllerType::Keypad, 5u8, "keypad"),
            (ControllerType::Driving, 6u8, "driving"),
            (ControllerType::AmigaMouse, 7u8, "amiga-mouse"),
            (ControllerType::StMouse, 8u8, "st-mouse"),
        ];

        for (ct, expected_val, json_str) in cases {
            assert_eq!(ct as u8, expected_val);

            let json = format!("\"{json_str}\"");
            let deserialized: ControllerType = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, ct);

            let cfg = Config {
                controller_1: ct,
                controller_2: ct,
                ..Config::default()
            };
            let header = build_a78_header(&cfg, 32768).unwrap();
            assert_eq!(header[55], expected_val);
            assert_eq!(header[56], expected_val);
        }
    }

    #[test]
    fn test_all_tv_type_enum_variants() {
        let cases = [
            (TvType::Ntsc, 0u8, "ntsc"),
            (TvType::Pal, 1u8, "pal"),
        ];

        for (tv, expected_val, json_str) in cases {
            assert_eq!(tv as u8, expected_val);

            let json = format!("\"{json_str}\"");
            let deserialized: TvType = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, tv);

            let cfg = Config {
                tv_type: tv,
                ..Config::default()
            };
            let header = build_a78_header(&cfg, 32768).unwrap();
            assert_eq!(header[57], expected_val);
        }
    }

    #[test]
    fn test_all_save_device_enum_variants() {
        let cases = [
            (SaveDevice::None, 0u8, "none"),
            (SaveDevice::Hsc, 1u8, "hsc"),
            (SaveDevice::SaveKey, 2u8, "savekey"),
        ];

        for (sd, expected_val, json_str) in cases {
            assert_eq!(sd as u8, expected_val);

            let json = format!("\"{json_str}\"");
            let deserialized: SaveDevice = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, sd);

            let cfg = Config {
                save_device: sd,
                ..Config::default()
            };
            let header = build_a78_header(&cfg, 32768).unwrap();
            assert_eq!(header[58], expected_val);
        }
    }

    #[test]
    fn test_all_slot_passthrough_enum_variants() {
        let cases = [
            (SlotPassthrough::None, 0u8, "none"),
            (SlotPassthrough::Xm, 1u8, "xm"),
        ];

        for (sp, expected_val, json_str) in cases {
            assert_eq!(sp as u8, expected_val);

            let json = format!("\"{json_str}\"");
            let deserialized: SlotPassthrough = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, sp);

            let cfg = Config {
                slot_passthrough: sp,
                ..Config::default()
            };
            let header = build_a78_header(&cfg, 32768).unwrap();
            assert_eq!(header[63], expected_val);
        }
    }

    #[test]
    fn test_audio_flags_and_v3_synthesis() {
        let cfg = Config {
            audio: 0x0800,
            ..Config::default()
        };
        let header = build_a78_header(&cfg, 32768).unwrap();
        assert_eq!(u16::from_be_bytes([header[66], header[67]]), 0x0800);
        assert_ne!(header[54] & 0x04, 0);

        let cfg = Config {
            audio: 0x0001,
            ..Config::default()
        };
        let header = build_a78_header(&cfg, 32768).unwrap();
        assert_eq!(u16::from_be_bytes([header[66], header[67]]), 0x0001);
        assert_ne!(header[54] & 0x08, 0);

        let cfg = Config {
            audio: 0x0803,
            ..Config::default()
        };
        let header = build_a78_header(&cfg, 32768).unwrap();
        assert_eq!(u16::from_be_bytes([header[66], header[67]]), 0x0803);
        assert_ne!(header[54] & 0x04, 0);
        assert_ne!(header[54] & 0x08, 0);
    }

    #[test]
    fn test_all_mapper_ids_and_custom_options() {
        for mapper_id in [0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 42u8, 128u8, 255u8] {
            let cfg = Config {
                mapper: mapper_id,
                mapper_opts: 0x7E,
                interrupt: 0xCAFE,
                ..Config::default()
            };

            let header = build_a78_header(&cfg, 32768).unwrap();
            assert_eq!(header[64], mapper_id);
            assert_eq!(header[65], 0x7E);
            assert_eq!(u16::from_be_bytes([header[68], header[69]]), 0xCAFE);
        }
    }

    #[test]
    fn test_comprehensive_json_deserialization() {
        let json = r#"{
            "input": "roms/input.bin",
            "output": "roms/output.a78",
            "title": "FULL OPTION TEST",
            "version": 4,
            "cart_type": 12,
            "controller_1": "lightgun",
            "controller_2": "amiga-mouse",
            "tv_type": "pal",
            "save_device": "savekey",
            "slot_passthrough": "xm",
            "mapper": 42,
            "mapper_opts": 255,
            "audio": 2048,
            "interrupt": 4660,
            "ym2149": true,
            "hsc": false
        }"#;

        let cfg: Config = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.input, Some(PathBuf::from("roms/input.bin")));
        assert_eq!(cfg.output, Some(PathBuf::from("roms/output.a78")));
        assert_eq!(cfg.title.as_deref(), Some("FULL OPTION TEST"));
        assert_eq!(cfg.version, 4);
        assert_eq!(cfg.cart_type, 12);
        assert_eq!(cfg.controller_1, ControllerType::LightGun);
        assert_eq!(cfg.controller_2, ControllerType::AmigaMouse);
        assert_eq!(cfg.tv_type, TvType::Pal);
        assert_eq!(cfg.save_device, SaveDevice::SaveKey);
        assert_eq!(cfg.slot_passthrough, SlotPassthrough::Xm);
        assert_eq!(cfg.mapper, 42);
        assert_eq!(cfg.mapper_opts, 255);
        assert_eq!(cfg.audio, 2048);
        assert_eq!(cfg.interrupt, 4660);
        assert!(cfg.ym2149);
        assert!(!cfg.hsc);
    }

    #[test]
    fn test_header_decode_and_inspect_formatting() {
        let cfg = Config {
            title: Some("DECODE TEST".to_string()),
            controller_1: ControllerType::Driving,
            controller_2: ControllerType::StMouse,
            tv_type: TvType::Pal,
            save_device: SaveDevice::Hsc,
            slot_passthrough: SlotPassthrough::Xm,
            mapper: 1,
            audio: 0x0800,
            ..Config::default()
        };

        let header = build_a78_header(&cfg, 131_072).unwrap();
        let decoded = decode_a78_header(&header).unwrap();

        assert!(decoded.contains("Header Version : 4"));
        assert!(decoded.contains("Title          : DECODE TEST"));
        assert!(decoded.contains("ROM Size       : 131072 bytes (128 KB)"));
        assert!(decoded.contains("Controller 1   : Driving Controller (6)"));
        assert!(decoded.contains("Controller 2   : Atari ST Mouse (8)"));
        assert!(decoded.contains("TV Format      : PAL (1)"));
        assert!(decoded.contains("Save Device    : High Score Cartridge (1)"));
        assert!(decoded.contains("Expansion Slot : XM Expansion Module (1)"));
        assert!(decoded.contains("Mapper         : 1 (YM-IOA Banked 128K/256K)"));
        assert!(decoded.contains("Audio Word     : 0x0800 (YM2149: true, POKEY@4000: false, POKEY@4500: false)"));
    }

    #[test]
    fn test_header_decode_invalid_magic_error() {
        let invalid_header = [0u8; 128];
        let err = decode_a78_header(&invalid_header).unwrap_err();
        assert!(err.contains("Invalid .a78 magic header"));

        let short_buffer = [0u8; 64];
        let err_short = decode_a78_header(&short_buffer).unwrap_err();
        assert!(err_short.contains("File too small"));
    }
}
