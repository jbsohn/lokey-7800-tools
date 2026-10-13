use crate::{GfxError, IndexedGraphic};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek};
use std::path::{Path, PathBuf};

/// Supported graphics file formats for import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GfxFormat {
    /// Portable Network Graphics (indexed, grayscale, or RGBA)
    Png,
    /// Windows/OS/2 Bitmap (.bmp)
    Bmp,
    /// Graphics Interchange Format (.gif)
    Gif,
    /// Classic Atari ST Degas Elite uncompressed / compressed (.pi1, .pc1)
    AtariStDegas,
    /// Amiga / Deluxe Paint Interchange File Format (.iff, .lbm)
    AmigaIff,
    /// Raw indexed binary stream (.bin, .raw)
    RawBinary,
}

impl fmt::Display for GfxFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Png => write!(f, "PNG"),
            Self::Bmp => write!(f, "BMP"),
            Self::Gif => write!(f, "GIF"),
            Self::AtariStDegas => write!(f, "Atari ST Degas Elite"),
            Self::AmigaIff => write!(f, "Amiga IFF / ILBM"),
            Self::RawBinary => write!(f, "Raw Binary"),
        }
    }
}

impl GfxFormat {
    /// Detect format from file path extension or file magic bytes.
    ///
    /// # Errors
    /// Returns [`GfxError::UnsupportedFormat`] if format cannot be recognized.
    pub fn detect<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let path_ref = path.as_ref();

        // 1. Check extension first
        if let Some(ext) = path_ref.extension().and_then(|e| e.to_str()) {
            match ext.to_ascii_lowercase().as_str() {
                "png" => return Ok(Self::Png),
                "bmp" => return Ok(Self::Bmp),
                "gif" => return Ok(Self::Gif),
                "pi1" | "pc1" => return Ok(Self::AtariStDegas),
                "iff" | "lbm" => return Ok(Self::AmigaIff),
                "bin" | "raw" => return Ok(Self::RawBinary),
                _ => {}
            }
        }

        // 2. Fall back to inspecting magic bytes if file exists
        if let Ok(mut file) = File::open(path_ref) {
            let mut magic = [0u8; 8];
            if std::io::Read::read(&mut file, &mut magic).is_ok() {
                if let Some(fmt) = Self::detect_magic(&magic) {
                    return Ok(fmt);
                }
            }
        }

        Err(GfxError::UnsupportedFormat(format!(
            "Could not determine graphics format for '{}'",
            path_ref.display()
        )))
    }

    /// Identify format based on leading magic bytes.
    #[must_use]
    pub fn detect_magic(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 8 && bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
        {
            return Some(Self::Png);
        }
        if bytes.len() >= 2 && bytes.starts_with(b"BM") {
            return Some(Self::Bmp);
        }
        if bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
            return Some(Self::Gif);
        }
        if bytes.len() >= 4 && bytes.starts_with(b"FORM") {
            return Some(Self::AmigaIff);
        }
        None
    }

    /// Decode graphic data using this format.
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails or if the format is not yet implemented.
    pub fn decode<R: BufRead + Seek>(&self, reader: R) -> Result<IndexedGraphic, GfxError> {
        match self {
            Self::Png => PngDecoder.decode(reader),
            Self::Bmp => Err(GfxError::UnsupportedFormat(
                "BMP decoder not yet implemented".into(),
            )),
            Self::Gif => Err(GfxError::UnsupportedFormat(
                "GIF decoder not yet implemented".into(),
            )),
            Self::AtariStDegas => Err(GfxError::UnsupportedFormat(
                "Atari ST Degas decoder not yet implemented".into(),
            )),
            Self::AmigaIff => Err(GfxError::UnsupportedFormat(
                "Amiga IFF decoder not yet implemented".into(),
            )),
            Self::RawBinary => Err(GfxError::UnsupportedFormat(
                "Raw binary decoder not yet implemented".into(),
            )),
        }
    }

    /// Decode graphic data and quantize against a project palette.
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails or if the format does not support palette quantization.
    pub fn decode_with_palette<R: BufRead + Seek>(
        &self,
        reader: R,
        palette: &crate::ProjectPalette,
        subpalette_slot: usize,
        mode: crate::MariaMode,
    ) -> Result<IndexedGraphic, GfxError> {
        match self {
            Self::Png => {
                let rgba = crate::RgbaGraphic::from_png_reader(reader)?;
                Ok(rgba.to_indexed(palette, subpalette_slot, mode))
            }
            _ => Err(GfxError::UnsupportedFormat(format!(
                "Format {self} does not support palette quantization yet"
            ))),
        }
    }
}

/// Abstract decoder trait for graphics file formats.
pub trait GraphicDecoder: std::fmt::Debug {
    /// Decodes an image from any reader implementing [`BufRead`] and [`Seek`].
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails.
    fn decode<R: BufRead + Seek>(&self, reader: R) -> Result<IndexedGraphic, GfxError>;

    /// File extensions associated with this format (e.g. `["png"]`).
    fn extensions(&self) -> &'static [&'static str];
}

/// Decoder implementation for PNG graphics.
#[derive(Debug, Clone, Copy, Default)]
pub struct PngDecoder;

impl GraphicDecoder for PngDecoder {
    fn decode<R: BufRead + Seek>(&self, reader: R) -> Result<IndexedGraphic, GfxError> {
        IndexedGraphic::from_png_reader(reader)
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["png"]
    }
}

/// Abstract container representing a loaded graphics file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GfxFile {
    /// Source file path on disk, if loaded from a file.
    pub path: Option<PathBuf>,
    /// Format of the graphics file.
    pub format: GfxFormat,
    /// Format-agnostic 2D raster of palette-indexed pixels.
    pub graphic: IndexedGraphic,
}

impl GfxFile {
    /// Creates a new `GfxFile` with an in-memory graphic and format.
    #[must_use]
    pub fn new(graphic: IndexedGraphic, format: GfxFormat) -> Self {
        Self {
            path: None,
            format,
            graphic,
        }
    }

    /// Load a graphics file from disk, auto-detecting the format from the file extension or magic bytes.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if reading fails, or [`GfxError::UnsupportedFormat`] if format is unknown or unsupported.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let p = path.as_ref();
        let format = GfxFormat::detect(p)?;
        let file = File::open(p)?;
        let reader = BufReader::new(file);
        let graphic = format.decode(reader)?;
        Ok(Self {
            path: Some(p.to_path_buf()),
            format,
            graphic,
        })
    }

    /// Load a graphics file from disk and quantize it using a project palette.
    ///
    /// # Errors
    /// Returns [`GfxError`] if reading, decoding, or quantization fails.
    pub fn load_with_palette<P: AsRef<Path>>(
        path: P,
        palette: &crate::ProjectPalette,
        subpalette_slot: usize,
        mode: crate::MariaMode,
    ) -> Result<Self, GfxError> {
        let p = path.as_ref();
        let format = GfxFormat::detect(p)?;
        let file = File::open(p)?;
        let reader = BufReader::new(file);
        let graphic = format.decode_with_palette(reader, palette, subpalette_slot, mode)?;
        Ok(Self {
            path: Some(p.to_path_buf()),
            format,
            graphic,
        })
    }

    /// Load a graphics file from disk with an explicitly specified format.
    ///
    /// # Errors
    /// Returns [`GfxError`] if reading or decoding fails.
    pub fn load_with_format<P: AsRef<Path>>(path: P, format: GfxFormat) -> Result<Self, GfxError> {
        let p = path.as_ref();
        let file = File::open(p)?;
        let reader = BufReader::new(file);
        let graphic = format.decode(reader)?;
        Ok(Self {
            path: Some(p.to_path_buf()),
            format,
            graphic,
        })
    }

    /// Load a graphics file from any reader implementing [`BufRead`] and [`Seek`] with an explicit format.
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails.
    pub fn from_reader<R: BufRead + Seek>(reader: R, format: GfxFormat) -> Result<Self, GfxError> {
        let graphic = format.decode(reader)?;
        Ok(Self {
            path: None,
            format,
            graphic,
        })
    }

    /// Consumes this `GfxFile` and returns the inner [`IndexedGraphic`].
    #[must_use]
    pub fn into_graphic(self) -> IndexedGraphic {
        self.graphic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection_by_extension() {
        assert_eq!(GfxFormat::detect("ship.png").unwrap(), GfxFormat::Png);
        assert_eq!(GfxFormat::detect("IMAGE.PNG").unwrap(), GfxFormat::Png);
        assert_eq!(GfxFormat::detect("tile.bmp").unwrap(), GfxFormat::Bmp);
        assert_eq!(GfxFormat::detect("anim.gif").unwrap(), GfxFormat::Gif);
        assert_eq!(
            GfxFormat::detect("art.pi1").unwrap(),
            GfxFormat::AtariStDegas
        );
        assert_eq!(
            GfxFormat::detect("art.pc1").unwrap(),
            GfxFormat::AtariStDegas
        );
        assert_eq!(
            GfxFormat::detect("screen.iff").unwrap(),
            GfxFormat::AmigaIff
        );
        assert_eq!(GfxFormat::detect("brush.lbm").unwrap(), GfxFormat::AmigaIff);
        assert_eq!(GfxFormat::detect("data.bin").unwrap(), GfxFormat::RawBinary);
        assert_eq!(GfxFormat::detect("data.raw").unwrap(), GfxFormat::RawBinary);
        assert!(GfxFormat::detect("unknown.xyz").is_err());
    }

    #[test]
    fn test_format_magic_detection() {
        assert_eq!(
            GfxFormat::detect_magic(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some(GfxFormat::Png)
        );
        assert_eq!(GfxFormat::detect_magic(b"BM1234"), Some(GfxFormat::Bmp));
        assert_eq!(GfxFormat::detect_magic(b"GIF89a..."), Some(GfxFormat::Gif));
        assert_eq!(
            GfxFormat::detect_magic(b"FORMBODY"),
            Some(GfxFormat::AmigaIff)
        );
        assert_eq!(GfxFormat::detect_magic(b"randomdata"), None);
    }

    #[test]
    fn test_format_display() {
        assert_eq!(GfxFormat::Png.to_string(), "PNG");
        assert_eq!(GfxFormat::AtariStDegas.to_string(), "Atari ST Degas Elite");
    }

    #[test]
    fn test_png_decoder_extensions() {
        let decoder = PngDecoder;
        assert_eq!(decoder.extensions(), &["png"]);
    }
}
