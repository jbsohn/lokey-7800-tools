//! Originally created to handle headers for the lokey-ym2149 cart, but fully
//! adheres to all header fields in the 8BitDev.org Atari 7800 Header Specification:
//! <https://7800.8bitdev.org/index.php/A78_Header_Specification/>

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Header constants per the 8BitDev.org Atari 7800 Header Specification.
pub const HEADER_LEN: usize = 128;
pub const HEADER_MAGIC: &[u8; 9] = b"ATARI7800";
pub const HEADER_MAGIC_EXTENDED: &[u8; 16] = b"ATARI7800       ";
pub const HEADER_END_MAGIC: &[u8; 28] = b"ACTUAL CART DATA STARTS HERE";

/// Errors encountered when parsing or decoding an `.a78` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderError {
    FileTooSmall { actual: usize, expected: usize },
    InvalidMagic(String),
    UnknownController { port: u8, value: u8 },
    UnknownTvType(u8),
    UnknownSaveDevice(u8),
    UnknownSlotPassthrough(u8),
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileTooSmall { actual, expected } => write!(
                f,
                "File too small for .a78 header (got {actual} bytes, expected >= {expected})"
            ),
            Self::InvalidMagic(m) => write!(f, "Invalid .a78 magic header: {m:?}"),
            Self::UnknownController { port, value } => {
                write!(f, "Unknown Controller {port} value: {value}")
            }
            Self::UnknownTvType(v) => write!(f, "Unknown TV format value: {v}"),
            Self::UnknownSaveDevice(v) => write!(f, "Unknown save device value: {v}"),
            Self::UnknownSlotPassthrough(v) => write!(f, "Unknown expansion slot value: {v}"),
        }
    }
}

impl std::error::Error for HeaderError {}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TvType {
    Ntsc = 0,
    Pal = 1,
}

impl TryFrom<u8> for TvType {
    type Error = HeaderError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Ntsc),
            1 => Ok(Self::Pal),
            other => Err(HeaderError::UnknownTvType(other)),
        }
    }
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
    #[serde(alias = "lightgun")]
    #[clap(alias = "lightgun")]
    LightGun = 2,
    Paddle = 3,
    #[serde(alias = "trakball")]
    #[clap(alias = "trakball")]
    TrakBall = 4,
    Keypad = 5,
    Driving = 6,
    AmigaMouse = 7,
    StMouse = 8,
}

impl ControllerType {
    /// Parse a controller byte for a specific port number.
    ///
    /// # Errors
    /// Returns [`HeaderError::UnknownController`] if `val` is not a recognized controller id.
    pub fn try_from_port(port: u8, val: u8) -> Result<Self, HeaderError> {
        match val {
            0 => Ok(Self::None),
            1 => Ok(Self::Joystick),
            2 => Ok(Self::LightGun),
            3 => Ok(Self::Paddle),
            4 => Ok(Self::TrakBall),
            5 => Ok(Self::Keypad),
            6 => Ok(Self::Driving),
            7 => Ok(Self::AmigaMouse),
            8 => Ok(Self::StMouse),
            other => Err(HeaderError::UnknownController { port, value: other }),
        }
    }
}

impl TryFrom<u8> for ControllerType {
    type Error = HeaderError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        Self::try_from_port(0, val)
    }
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

impl TryFrom<u8> for SaveDevice {
    type Error = HeaderError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::None),
            1 => Ok(Self::Hsc),
            2 => Ok(Self::SaveKey),
            other => Err(HeaderError::UnknownSaveDevice(other)),
        }
    }
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

impl TryFrom<u8> for SlotPassthrough {
    type Error = HeaderError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::None),
            1 => Ok(Self::Xm),
            other => Err(HeaderError::UnknownSlotPassthrough(other)),
        }
    }
}

impl fmt::Display for SlotPassthrough {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotPassthrough::None => write!(f, "None (0)"),
            SlotPassthrough::Xm => write!(f, "XM Expansion Module (1)"),
        }
    }
}

/// Bitfield flags for Atari 7800 expansion sound hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AudioFlags(pub u16);

impl AudioFlags {
    pub const POKEY_4000: u16 = 0x0001;
    pub const POKEY_4500: u16 = 0x0002;
    pub const YM2149: u16 = 0x0800;

    #[must_use]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn has_pokey_4000(self) -> bool {
        self.0 & Self::POKEY_4000 != 0
    }

    #[must_use]
    pub const fn has_pokey_4500(self) -> bool {
        self.0 & Self::POKEY_4500 != 0
    }

    #[must_use]
    pub const fn has_ym2149(self) -> bool {
        self.0 & Self::YM2149 != 0
    }

    pub fn set_pokey_4000(&mut self, enabled: bool) {
        if enabled {
            self.0 |= Self::POKEY_4000;
        } else {
            self.0 &= !Self::POKEY_4000;
        }
    }

    pub fn set_pokey_4500(&mut self, enabled: bool) {
        if enabled {
            self.0 |= Self::POKEY_4500;
        } else {
            self.0 &= !Self::POKEY_4500;
        }
    }

    pub fn set_ym2149(&mut self, enabled: bool) {
        if enabled {
            self.0 |= Self::YM2149;
        } else {
            self.0 &= !Self::YM2149;
        }
    }
}

impl fmt::Display for AudioFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "0x{:04X} (YM2149: {}, POKEY@4000: {}, POKEY@4500: {})",
            self.0,
            self.has_ym2149(),
            self.has_pokey_4000(),
            self.has_pokey_4500()
        )
    }
}

impl From<u16> for AudioFlags {
    fn from(val: u16) -> Self {
        Self(val)
    }
}

impl From<AudioFlags> for u16 {
    fn from(flags: AudioFlags) -> Self {
        flags.0
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub title: Option<String>,
    #[serde(default = "Config::default_version")]
    pub version: u8,
    pub cart_type: u16,
    pub controller_1: ControllerType,
    pub controller_2: ControllerType,
    pub tv_type: TvType,
    pub save_device: SaveDevice,
    pub slot_passthrough: SlotPassthrough,
    pub mapper: u8,
    pub mapper_opts: u8,
    #[serde(default = "Config::default_audio")]
    pub audio: u16,
    pub interrupt: u16,
    pub ym2149: bool,
    pub pokey_4000: bool,
    pub pokey_4500: bool,
    pub hsc: bool,
    pub savekey: bool,
    pub xm: bool,
}

impl Config {
    pub const DEFAULT_VERSION: u8 = 4;
    pub const DEFAULT_AUDIO: u16 = AudioFlags::YM2149;

    #[must_use]
    pub const fn default_version() -> u8 {
        Self::DEFAULT_VERSION
    }

    #[must_use]
    pub const fn default_audio() -> u16 {
        Self::DEFAULT_AUDIO
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            title: None,
            version: Self::DEFAULT_VERSION,
            cart_type: 0,
            controller_1: ControllerType::Joystick,
            controller_2: ControllerType::Joystick,
            tv_type: TvType::Ntsc,
            save_device: SaveDevice::None,
            slot_passthrough: SlotPassthrough::None,
            mapper: 0,
            mapper_opts: 0,
            audio: Self::DEFAULT_AUDIO,
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

/// Strongly-typed Atari 7800 `.a78` 128-byte ROM header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A78Header {
    pub version: u8,
    pub title: String,
    pub rom_size: u32,
    pub cart_type: u16,
    pub controller_1: ControllerType,
    pub controller_2: ControllerType,
    pub tv_type: TvType,
    pub save_device: SaveDevice,
    pub slot_passthrough: SlotPassthrough,
    pub mapper: u8,
    pub mapper_opts: u8,
    pub audio: AudioFlags,
    pub interrupt: u16,
}

impl A78Header {
    /// Parse an A78 header from a byte slice of at least 128 bytes.
    ///
    /// # Errors
    /// Returns [`HeaderError`] if the slice is too short, magic bytes don't match,
    /// or field values cannot be decoded.
    pub fn from_bytes(header: &[u8]) -> Result<Self, HeaderError> {
        if header.len() < HEADER_LEN {
            return Err(HeaderError::FileTooSmall {
                actual: header.len(),
                expected: HEADER_LEN,
            });
        }

        let magic = &header[1..10];
        if magic != HEADER_MAGIC {
            return Err(HeaderError::InvalidMagic(
                String::from_utf8_lossy(magic).into_owned(),
            ));
        }

        let version = header[0];
        let title = String::from_utf8_lossy(&header[17..49]).trim().to_string();
        let rom_size = u32::from_be_bytes([header[49], header[50], header[51], header[52]]);
        let cart_type = u16::from_be_bytes([header[53], header[54]]);
        let controller_1 = ControllerType::try_from_port(1, header[55])?;
        let controller_2 = ControllerType::try_from_port(2, header[56])?;
        let tv_type = if header[57] == 1 {
            TvType::Pal
        } else {
            TvType::Ntsc
        };
        let save_device = match header[58] {
            1 => SaveDevice::Hsc,
            2 => SaveDevice::SaveKey,
            _ => SaveDevice::None,
        };
        let slot_passthrough = if header[63] == 1 {
            SlotPassthrough::Xm
        } else {
            SlotPassthrough::None
        };
        let mapper = header[64];
        let mapper_opts = header[65];
        let audio = AudioFlags::from_raw(u16::from_be_bytes([header[66], header[67]]));
        let interrupt = u16::from_be_bytes([header[68], header[69]]);

        Ok(Self {
            version,
            title,
            rom_size,
            cart_type,
            controller_1,
            controller_2,
            tv_type,
            save_device,
            slot_passthrough,
            mapper,
            mapper_opts,
            audio,
            interrupt,
        })
    }

    /// Construct an `A78Header` from a [`Config`] and ROM size.
    #[must_use]
    pub fn from_config(cfg: &Config, rom_size: u32) -> Self {
        let title = cfg
            .title
            .clone()
            .unwrap_or_else(|| "YM2149 CART".to_string());
        let mut cart_type = cfg.cart_type;
        if cfg.audio & AudioFlags::YM2149 != 0 {
            cart_type |= 0x0004;
        }
        if cfg.audio & AudioFlags::POKEY_4000 != 0 {
            cart_type |= 0x0008;
        }
        if cfg.mapper == 2 {
            cart_type |= 0x0010;
        }
        if cfg.save_device == SaveDevice::Hsc {
            cart_type |= 0x0080;
        }

        Self {
            version: cfg.version,
            title,
            rom_size,
            cart_type,
            controller_1: cfg.controller_1,
            controller_2: cfg.controller_2,
            tv_type: cfg.tv_type,
            save_device: cfg.save_device,
            slot_passthrough: cfg.slot_passthrough,
            mapper: cfg.mapper,
            mapper_opts: cfg.mapper_opts,
            audio: AudioFlags::from_raw(cfg.audio),
            interrupt: cfg.interrupt,
        }
    }

    /// Serialize the header into the standard 128-byte array.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut header = [0u8; HEADER_LEN];

        header[0] = self.version;
        header[1..17].copy_from_slice(HEADER_MAGIC_EXTENDED);

        let title_bytes: Vec<u8> = self.title.bytes().take(32).collect();
        header[17..17 + title_bytes.len()].copy_from_slice(&title_bytes);
        header[17 + title_bytes.len()..49].fill(0x20);

        header[49..53].copy_from_slice(&self.rom_size.to_be_bytes());
        header[53..55].copy_from_slice(&self.cart_type.to_be_bytes());

        header[55] = self.controller_1 as u8;
        header[56] = self.controller_2 as u8;
        header[57] = self.tv_type as u8;
        header[58] = self.save_device as u8;

        header[63] = self.slot_passthrough as u8;
        header[64] = self.mapper;
        header[65] = self.mapper_opts;

        header[66..68].copy_from_slice(&self.audio.raw().to_be_bytes());
        header[68..70].copy_from_slice(&self.interrupt.to_be_bytes());

        header[100..128].copy_from_slice(HEADER_END_MAGIC);

        header
    }

    /// Return a human-readable mapper description.
    #[must_use]
    pub fn mapper_name(&self) -> &'static str {
        match self.mapper {
            0 => "0 (Linear / Fixed 32K)",
            1 => "1 (YM-IOA Banked 128K/256K)",
            2 => "2 (SuperGame Banked 128K/256K/512K)",
            3 => "3 (Activision Banked 128K)",
            4 => "4 (Absolute Banked 64K)",
            5 => "5 (CPU RAM Banked)",
            _ => "Unknown Mapper",
        }
    }
}

impl fmt::Display for A78Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "====================================================")?;
        writeln!(f, "       Atari 7800 .a78 Header Specification        ")?;
        writeln!(f, "====================================================")?;
        writeln!(f, "Header Version : {}", self.version)?;
        writeln!(f, "Title          : {}", self.title)?;
        writeln!(
            f,
            "ROM Size       : {} bytes ({} KB)",
            self.rom_size,
            self.rom_size / 1024
        )?;
        writeln!(f, "Cart Type Word : 0x{:04X}", self.cart_type)?;
        writeln!(f, "Controller 1   : {}", self.controller_1)?;
        writeln!(f, "Controller 2   : {}", self.controller_2)?;
        writeln!(f, "TV Format      : {}", self.tv_type)?;
        writeln!(f, "Save Device    : {}", self.save_device)?;
        writeln!(f, "Expansion Slot : {}", self.slot_passthrough)?;
        writeln!(f, "Mapper         : {}", self.mapper_name())?;
        writeln!(f, "Mapper Opts    : 0x{:02X}", self.mapper_opts)?;
        writeln!(f, "Audio Word     : {}", self.audio)?;
        writeln!(f, "Interrupt Word : 0x{:04X}", self.interrupt)?;
        writeln!(f, "====================================================")
    }
}

/// A complete ROM image, holding an optional 128-byte `.a78` header and raw payload data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomImage {
    pub header: Option<A78Header>,
    pub payload: Vec<u8>,
}

impl RomImage {
    /// Load a ROM image from raw file bytes. Automatically detects whether a 128-byte
    /// `.a78` header is present.
    ///
    /// # Errors
    /// Returns [`HeaderError`] if a header magic is present but malformed.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() >= HEADER_LEN && &bytes[1..10] == HEADER_MAGIC {
            let header = A78Header::from_bytes(&bytes[..HEADER_LEN])?;
            let payload = bytes[HEADER_LEN..].to_vec();
            Ok(Self {
                header: Some(header),
                payload,
            })
        } else {
            Ok(Self {
                header: None,
                payload: bytes.to_vec(),
            })
        }
    }

    /// Returns true if this image contains a valid 128-byte `.a78` header.
    #[must_use]
    pub fn has_header(&self) -> bool {
        self.header.is_some()
    }

    /// Strips the 128-byte header, returning the raw ROM payload.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Assembles the complete file representation (header + payload, or raw payload if no header).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        if let Some(header) = &self.header {
            let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
            out.extend_from_slice(&header.to_bytes());
            out.extend_from_slice(&self.payload);
            out
        } else {
            self.payload.clone()
        }
    }

    /// Prepares raw ROM data for cartridge output according to Atari 7800 mapper rules.
    ///
    /// - **Mapper 0 (Linear 32K)**: Padded or truncated to exactly 32 KB ($8000–$FFFF).
    ///   Images smaller than 32 KB are placed at the top of the 32 KB buffer with `$FF` filler,
    ///   ensuring 6502 vectors at `$FFFA..$FFFF` remain aligned.
    /// - **Banked mappers (1–4)**: Validated against standard bank sizes, returning diagnostic notes
    ///   if unexpected sizes are encountered.
    /// - **Custom mappers (>4)**: Passed through untouched with diagnostic notation.
    ///
    /// Returns the prepared payload bytes along with any diagnostic messages.
    #[must_use]
    pub fn prepare_mapper_payload(mapper: u8, raw: &[u8]) -> (Vec<u8>, Vec<String>) {
        let mut notes = Vec::new();
        let payload = match mapper {
            0 => {
                if raw.len() > 32768 {
                    notes.push(format!(
                        "Warning: input is {} bytes but mapper is 0 (linear/fixed 32K) — \
                         only the top 32 KB will be kept. Use --mapper 1..255 for banked images.",
                        raw.len()
                    ));
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
                    notes.push(format!(
                        "Warning: Mapper 1 (YM-IOA banked) typically uses 128 KB or 256 KB, got {} bytes.",
                        raw.len()
                    ));
                }
                raw.to_vec()
            }
            2 => {
                if raw.len() != 128 * 1024 && raw.len() != 256 * 1024 && raw.len() != 512 * 1024 {
                    notes.push(format!(
                        "Warning: Mapper 2 (SuperGame banked) typically uses 128 KB, 256 KB, or 512 KB, got {} bytes.",
                        raw.len()
                    ));
                }
                raw.to_vec()
            }
            3 => {
                if raw.len() != 128 * 1024 {
                    notes.push(format!(
                        "Warning: Mapper 3 (Activision banked) typically uses 128 KB, got {} bytes.",
                        raw.len()
                    ));
                }
                raw.to_vec()
            }
            4 => {
                if raw.len() != 64 * 1024 {
                    notes.push(format!(
                        "Warning: Mapper 4 (Absolute banked) typically uses 64 KB, got {} bytes.",
                        raw.len()
                    ));
                }
                raw.to_vec()
            }
            custom_mapper => {
                notes.push(format!(
                    "Note: Using custom/experimental mapper ID {custom_mapper} — passing {} bytes ROM payload.",
                    raw.len()
                ));
                raw.to_vec()
            }
        };

        (payload, notes)
    }
}

/// Builds a 128-byte `.a78` header from the given config and ROM size.
///
/// # Errors
/// Returns `Ok([u8; 128])` for backwards compatibility.
pub fn build_a78_header(cfg: &Config, rom_size: u32) -> Result<[u8; 128], String> {
    Ok(A78Header::from_config(cfg, rom_size).to_bytes())
}

/// Decodes an `.a78` header and formats an ASCII summary report.
///
/// # Errors
/// Returns an error string if header parsing fails.
pub fn decode_a78_header(header: &[u8]) -> Result<String, String> {
    A78Header::from_bytes(header)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
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
            (ControllerType::LightGun, 2u8, "light-gun"),
            (ControllerType::Paddle, 3u8, "paddle"),
            (ControllerType::TrakBall, 4u8, "trak-ball"),
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

        // Test non-hyphenated alias compatibility
        assert_eq!(
            serde_json::from_str::<ControllerType>("\"lightgun\"").unwrap(),
            ControllerType::LightGun
        );
        assert_eq!(
            serde_json::from_str::<ControllerType>("\"trakball\"").unwrap(),
            ControllerType::TrakBall
        );
    }

    #[test]
    fn test_all_tv_type_enum_variants() {
        let cases = [(TvType::Ntsc, 0u8, "ntsc"), (TvType::Pal, 1u8, "pal")];

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
        assert!(decoded.contains(
            "Audio Word     : 0x0800 (YM2149: true, POKEY@4000: false, POKEY@4500: false)"
        ));
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

    #[test]
    fn test_a78_header_struct_roundtrip_and_properties() {
        let cfg = Config {
            title: Some("TYPED STRUCT TEST".to_string()),
            controller_1: ControllerType::LightGun,
            controller_2: ControllerType::Paddle,
            tv_type: TvType::Pal,
            save_device: SaveDevice::SaveKey,
            slot_passthrough: SlotPassthrough::Xm,
            mapper: 2,
            mapper_opts: 0x42,
            audio: 0x0801,
            interrupt: 0x1234,
            ..Config::default()
        };

        let header = A78Header::from_config(&cfg, 65536);
        assert_eq!(header.title, "TYPED STRUCT TEST");
        assert_eq!(header.rom_size, 65536);
        assert_eq!(header.controller_1, ControllerType::LightGun);
        assert_eq!(header.controller_2, ControllerType::Paddle);
        assert_eq!(header.tv_type, TvType::Pal);
        assert_eq!(header.save_device, SaveDevice::SaveKey);
        assert_eq!(header.slot_passthrough, SlotPassthrough::Xm);
        assert_eq!(header.mapper, 2);
        assert_eq!(header.mapper_opts, 0x42);
        assert_eq!(header.audio.raw(), 0x0801);
        assert!(header.audio.has_ym2149());
        assert!(header.audio.has_pokey_4000());
        assert!(!header.audio.has_pokey_4500());
        assert_eq!(header.interrupt, 0x1234);

        let bytes = header.to_bytes();
        let parsed = A78Header::from_bytes(&bytes).unwrap();
        assert_eq!(header, parsed);
    }

    #[test]
    fn test_audio_flags_builder() {
        let mut audio = AudioFlags::default();
        assert_eq!(audio.raw(), 0);
        assert!(!audio.has_ym2149());

        audio.set_ym2149(true);
        assert!(audio.has_ym2149());
        assert_eq!(audio.raw(), AudioFlags::YM2149);

        audio.set_pokey_4000(true);
        assert!(audio.has_pokey_4000());
        assert_eq!(audio.raw(), AudioFlags::YM2149 | AudioFlags::POKEY_4000);

        audio.set_pokey_4500(true);
        assert!(audio.has_pokey_4500());

        audio.set_ym2149(false);
        assert!(!audio.has_ym2149());
        assert_eq!(audio.raw(), AudioFlags::POKEY_4000 | AudioFlags::POKEY_4500);
    }

    #[test]
    fn test_rom_image_with_and_without_header() {
        let payload = vec![0xEA; 4096];

        // Raw payload without header
        let raw_rom = RomImage::from_bytes(&payload).unwrap();
        assert!(!raw_rom.has_header());
        assert_eq!(raw_rom.payload(), payload.as_slice());
        assert_eq!(raw_rom.to_bytes(), payload);

        // Headered ROM
        let cfg = Config::default();
        let header = A78Header::from_config(&cfg, 4096);
        let headered_rom = RomImage {
            header: Some(header),
            payload: payload.clone(),
        };
        assert!(headered_rom.has_header());
        let file_bytes = headered_rom.to_bytes();
        assert_eq!(file_bytes.len(), HEADER_LEN + payload.len());

        let parsed_rom = RomImage::from_bytes(&file_bytes).unwrap();
        assert!(parsed_rom.has_header());
        assert_eq!(parsed_rom.payload(), payload.as_slice());
        assert_eq!(parsed_rom.header.unwrap().rom_size, 4096);
    }

    #[test]
    fn test_header_error_display() {
        let err = HeaderError::FileTooSmall {
            actual: 50,
            expected: 128,
        };
        assert_eq!(
            err.to_string(),
            "File too small for .a78 header (got 50 bytes, expected >= 128)"
        );

        let err = HeaderError::UnknownController { port: 1, value: 99 };
        assert_eq!(err.to_string(), "Unknown Controller 1 value: 99");
    }

    #[test]
    fn test_prepare_mapper_payload_linear_32k() {
        // Less than 32 KB: padded with 0xFF at start so vectors stay top-aligned
        let raw = vec![0x42; 16384];
        let (payload, notes) = RomImage::prepare_mapper_payload(0, &raw);
        assert_eq!(payload.len(), 32768);
        assert_eq!(&payload[..16384], &[0xFF; 16384]);
        assert_eq!(&payload[16384..], raw.as_slice());
        assert!(notes.is_empty());

        // Exactly 32 KB
        let raw_32k = vec![0x11; 32768];
        let (payload, notes) = RomImage::prepare_mapper_payload(0, &raw_32k);
        assert_eq!(payload.len(), 32768);
        assert_eq!(payload, raw_32k);
        assert!(notes.is_empty());

        // Greater than 32 KB: truncated to top 32 KB with warning note
        let mut raw_48k = vec![0x00; 16384];
        raw_48k.extend_from_slice(&vec![0xAA; 32768]);
        let (payload, notes) = RomImage::prepare_mapper_payload(0, &raw_48k);
        assert_eq!(payload.len(), 32768);
        assert_eq!(payload, vec![0xAA; 32768]);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("only the top 32 KB will be kept"));
    }

    #[test]
    fn test_prepare_mapper_payload_banked_and_custom() {
        // Mapper 1 (128 KB standard): no warnings
        let raw_128k = vec![0; 128 * 1024];
        let (payload, notes) = RomImage::prepare_mapper_payload(1, &raw_128k);
        assert_eq!(payload.len(), 128 * 1024);
        assert!(notes.is_empty());

        // Mapper 1 unexpected size
        let raw_16k = vec![0; 16 * 1024];
        let (_, notes) = RomImage::prepare_mapper_payload(1, &raw_16k);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("Mapper 1"));

        // Custom mapper
        let (_, notes) = RomImage::prepare_mapper_payload(42, &raw_16k);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("custom/experimental mapper ID 42"));
    }

    #[test]
    fn test_config_associated_defaults() {
        assert_eq!(Config::default_version(), 4);
        assert_eq!(Config::default_audio(), AudioFlags::YM2149);
        let cfg = Config::default();
        assert_eq!(cfg.version, Config::DEFAULT_VERSION);
        assert_eq!(cfg.audio, Config::DEFAULT_AUDIO);
    }
}
