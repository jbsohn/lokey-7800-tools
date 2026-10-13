use std::fmt;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

pub mod assembler;
pub mod format;
pub mod palette;
pub use assembler::{AssemblerType, AssemblerWriter, Ca65Writer, DasmWriter, MadsWriter};
pub use format::{GfxFile, GfxFormat, GraphicDecoder, PngDecoder};
pub use palette::{
    ColorEntry, ColorRgb, ColorRgba, GplDecoder, MariaColor, PaletteDecoder, PaletteFile,
    PaletteFormat, ProjectPalette, RgbaGraphic, ATARI_7800_NTSC_PALETTE,
};

#[derive(Debug)]
pub enum GfxError {
    Io(std::io::Error),
    Png(png::DecodingError),
    InvalidDimensions(String),
    UnsupportedColorType(String),
    InvalidPixelIndex(u8),
    Conversion(String),
    UnsupportedFormat(String),
    InvalidPalette(String),
}

impl fmt::Display for GfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Png(e) => write!(f, "PNG decoding error: {e}"),
            Self::InvalidDimensions(msg) => write!(f, "Invalid dimensions: {msg}"),
            Self::UnsupportedColorType(msg) => write!(f, "Unsupported color type: {msg}"),
            Self::InvalidPixelIndex(idx) => write!(f, "Pixel index out of bounds: {idx}"),
            Self::Conversion(msg) => write!(f, "Conversion error: {msg}"),
            Self::UnsupportedFormat(msg) => write!(f, "Unsupported format: {msg}"),
            Self::InvalidPalette(msg) => write!(f, "Palette error: {msg}"),
        }
    }
}

impl std::error::Error for GfxError {}

impl From<std::io::Error> for GfxError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<png::DecodingError> for GfxError {
    fn from(e: png::DecodingError) -> Self {
        Self::Png(e)
    }
}

/// Supported MARIA graphics modes for bit packing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MariaMode {
    /// 160A mode: 160 pixels wide across scanline, 4 pixels per byte (2 bits each: 0..=3).
    Mode160A,
    /// 320A / 320B / 320C mode: 320 pixels wide, 8 pixels per byte (1 bit each: 0..=1).
    Mode320,
}

/// An in-memory 2D raster of palette-indexed pixels (0..=3 for 160A, 0..=1 for 320).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedGraphic {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl IndexedGraphic {
    /// Creates a new graphic filled with index 0 (transparent/background).
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            pixels: vec![0; size],
        }
    }

    /// Load a graphic from disk using automatic format detection.
    ///
    /// # Errors
    /// Returns [`GfxError`] if loading or decoding fails.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let file = GfxFile::load(path)?;
        Ok(file.into_graphic())
    }

    /// Load an indexed PNG from disk.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if reading the file fails, or [`GfxError::Png`] if decoding fails.
    pub fn load_png<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_png_reader(reader)
    }

    /// Load an indexed PNG from any reader implementing `BufRead` + `Seek`.
    ///
    /// # Errors
    /// Returns [`GfxError::Png`] if decoding fails, or [`GfxError::UnsupportedColorType`] if color format is unsupported.
    pub fn from_png_reader<R: std::io::BufRead + std::io::Seek>(
        reader: R,
    ) -> Result<Self, GfxError> {
        let decoder = png::Decoder::new(reader);
        let mut png_reader = decoder.read_info()?;
        let info = png_reader.info().clone();

        let width = usize::try_from(info.width)
            .map_err(|_| GfxError::InvalidDimensions("Width exceeds memory bounds".into()))?;
        let height = usize::try_from(info.height)
            .map_err(|_| GfxError::InvalidDimensions("Height exceeds memory bounds".into()))?;

        if width == 0 || height == 0 {
            return Err(GfxError::InvalidDimensions(
                "Width and height must be greater than zero".into(),
            ));
        }

        let buf_size = png_reader.output_buffer_size().unwrap_or(width * height);
        let mut raw_buf = vec![0u8; buf_size];
        let output_info = png_reader.next_frame(&mut raw_buf)?;
        raw_buf.truncate(output_info.buffer_size());

        // Extract pixel indices based on color type & bit depth
        let pixels = match info.color_type {
            png::ColorType::Indexed => match info.bit_depth {
                png::BitDepth::One => {
                    let mut px = Vec::with_capacity(width * height);
                    for byte in &raw_buf {
                        for bit in (0..8).rev() {
                            if px.len() < width * height {
                                px.push((byte >> bit) & 0x01);
                            }
                        }
                    }
                    px
                }
                png::BitDepth::Two => {
                    let mut px = Vec::with_capacity(width * height);
                    for byte in &raw_buf {
                        for shift in [6, 4, 2, 0] {
                            if px.len() < width * height {
                                px.push((byte >> shift) & 0x03);
                            }
                        }
                    }
                    px
                }
                png::BitDepth::Four => {
                    let mut px = Vec::with_capacity(width * height);
                    for byte in &raw_buf {
                        if px.len() < width * height {
                            px.push((byte >> 4) & 0x0F);
                        }
                        if px.len() < width * height {
                            px.push(byte & 0x0F);
                        }
                    }
                    px
                }
                png::BitDepth::Eight => raw_buf,
                png::BitDepth::Sixteen => {
                    return Err(GfxError::UnsupportedColorType(
                        "16-bit depth indexed PNG not supported".into(),
                    ));
                }
            },
            png::ColorType::Grayscale => match info.bit_depth {
                png::BitDepth::One | png::BitDepth::Two | png::BitDepth::Eight => raw_buf,
                _ => {
                    return Err(GfxError::UnsupportedColorType(
                        "Unsupported grayscale bit depth".into(),
                    ));
                }
            },
            png::ColorType::Rgba => match info.bit_depth {
                png::BitDepth::Eight => {
                    let mut px = Vec::with_capacity(width * height);
                    for chunk in raw_buf.chunks_exact(4) {
                        let alpha = chunk[3];
                        if alpha < 128 {
                            px.push(0);
                        } else {
                            let r = chunk[0];
                            let g = chunk[1];
                            let b = chunk[2];
                            if r > 240 && g > 240 && b > 240 {
                                px.push(3);
                            } else if alpha < 200 {
                                px.push(1);
                            } else {
                                px.push(2);
                            }
                        }
                    }
                    px
                }
                _ => {
                    return Err(GfxError::UnsupportedColorType(
                        "Only 8-bit per channel RGBA is supported".into(),
                    ));
                }
            },
            png::ColorType::Rgb => match info.bit_depth {
                png::BitDepth::Eight => {
                    let mut px = Vec::with_capacity(width * height);
                    for chunk in raw_buf.chunks_exact(3) {
                        let r = chunk[0];
                        let g = chunk[1];
                        let b = chunk[2];
                        if r < 16 && g < 16 && b < 16 {
                            px.push(0);
                        } else if r > 240 && g > 240 && b > 240 {
                            px.push(3);
                        } else {
                            px.push(2);
                        }
                    }
                    px
                }
                _ => {
                    return Err(GfxError::UnsupportedColorType(
                        "Only 8-bit per channel RGB is supported".into(),
                    ));
                }
            },
            other @ png::ColorType::GrayscaleAlpha => {
                return Err(GfxError::UnsupportedColorType(format!(
                    "Expected Indexed, Grayscale, or RGB/RGBA PNG, got {other:?}"
                )));
            }
        };

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Read pixel at coordinate `(x, y)`. Returns 0 if out of bounds or negative.
    #[must_use]
    pub fn get_pixel(&self, x: i64, y: i64) -> u8 {
        if x < 0 || y < 0 {
            0
        } else {
            let ux = x as usize;
            let uy = y as usize;
            if ux >= self.width || uy >= self.height {
                0
            } else {
                self.pixels[uy * self.width + ux]
            }
        }
    }

    /// Set pixel at coordinate `(x, y)`. Ignores if out of bounds or negative.
    pub fn set_pixel(&mut self, x: i64, y: i64, val: u8) {
        if x >= 0 && y >= 0 {
            let ux = x as usize;
            let uy = y as usize;
            if ux < self.width && uy < self.height {
                self.pixels[uy * self.width + ux] = val;
            }
        }
    }

    /// Horizontal flip (X-mirror).
    #[must_use]
    pub fn flip_h(&self) -> Self {
        let mut flipped = Self::new(self.width, self.height);
        let max_x = (self.width as i64) - 1;
        for y in 0..self.height {
            let iy = y as i64;
            for x in 0..self.width {
                let ix = x as i64;
                let val = self.get_pixel(ix, iy);
                flipped.set_pixel(max_x - ix, iy, val);
            }
        }
        flipped
    }

    /// Vertical flip (Y-mirror).
    #[must_use]
    pub fn flip_v(&self) -> Self {
        let mut flipped = Self::new(self.width, self.height);
        let max_y = (self.height as i64) - 1;
        for y in 0..self.height {
            let iy = y as i64;
            for x in 0..self.width {
                let ix = x as i64;
                let val = self.get_pixel(ix, iy);
                flipped.set_pixel(ix, max_y - iy, val);
            }
        }
        flipped
    }

    /// Nearest-neighbor scaling.
    /// `scale_x` and `scale_y` are positive multipliers (e.g. 0.5 for 50%, 2.0 for 200%).
    #[must_use]
    pub fn scale(&self, scale_x: f32, scale_y: f32) -> Self {
        let new_w = ((self.width as f32 * scale_x).round() as usize).max(1);
        let new_h = ((self.height as f32 * scale_y).round() as usize).max(1);

        let mut result = Self::new(new_w, new_h);

        for y in 0..new_h {
            let src_y = (y as f32 / scale_y).floor() as i64;
            for x in 0..new_w {
                let src_x = (x as f32 / scale_x).floor() as i64;
                let val = self.get_pixel(src_x, src_y);
                result.set_pixel(x as i64, y as i64, val);
            }
        }

        result
    }

    /// Scale image around its center while keeping canvas dimensions unchanged.
    /// Pixels outside the scaled sprite boundary become 0 (transparent).
    #[must_use]
    pub fn scale_centered(&self, scale_x: f32, scale_y: f32) -> Self {
        if scale_x <= f32::EPSILON || scale_y <= f32::EPSILON {
            return Self::new(self.width, self.height);
        }

        let mut scaled = Self::new(self.width, self.height);
        let w = self.width as f64;
        let h = self.height as f64;
        let cx = (w - 1.0) * 0.5;
        let cy = (h - 1.0) * 0.5;
        let sx = f64::from(scale_x);
        let sy = f64::from(scale_y);

        for dy in 0..self.height {
            let y_rel = (dy as f64) - cy;
            let src_y = (y_rel / sy) + cy;
            let sample_y = src_y.round() as i64;

            for dx in 0..self.width {
                let x_rel = (dx as f64) - cx;
                let src_x = (x_rel / sx) + cx;
                let sample_x = src_x.round() as i64;

                let val = self.get_pixel(sample_x, sample_y);
                scaled.set_pixel(dx as i64, dy as i64, val);
            }
        }

        scaled
    }

    /// Resizes the graphic to `(new_width, new_height)` using area box sampling.
    #[must_use]
    pub fn resize(&self, new_width: usize, new_height: usize) -> Self {
        if new_width == 0 || new_height == 0 || self.width == 0 || self.height == 0 {
            return Self::new(new_width, new_height);
        }

        let mut out = Self::new(new_width, new_height);
        let scale_x = (self.width as f64) / (new_width as f64);
        let scale_y = (self.height as f64) / (new_height as f64);

        for dy in 0..new_height {
            let src_y_start = (dy as f64 * scale_y).floor() as i64;
            let src_y_end = (((dy + 1) as f64 * scale_y).ceil() as i64).min(self.height as i64);

            for dx in 0..new_width {
                let src_x_start = (dx as f64 * scale_x).floor() as i64;
                let src_x_end = (((dx + 1) as f64 * scale_x).ceil() as i64).min(self.width as i64);

                let mut counts = [0usize; 256];
                let mut total = 0usize;

                for sy in src_y_start..src_y_end {
                    for sx in src_x_start..src_x_end {
                        let val = self.get_pixel(sx, sy);
                        counts[val as usize] += 1;
                        total += 1;
                    }
                }

                if total == 0 {
                    continue;
                }

                let non_zero_count: usize = counts[1..].iter().sum();
                if (non_zero_count * 100) / total >= 25 {
                    let mut best_val = 2u8;
                    let mut max_count = 0usize;
                    for (val, &count) in counts.iter().enumerate().skip(1) {
                        if count > max_count {
                            max_count = count;
                            best_val = val as u8;
                        }
                    }
                    out.set_pixel(dx as i64, dy as i64, best_val);
                } else {
                    out.set_pixel(dx as i64, dy as i64, 0);
                }
            }
        }

        out
    }

    /// Discrete rotation around image center using inverse rotation mapping.
    ///
    /// - `degrees`: clockwise angle (e.g. 45.0, 90.0).
    /// - `par_correction`: if true, compensates for 160A mode's 2:1 rectangular pixel aspect ratio.
    #[must_use]
    pub fn rotate(&self, degrees: f32, par_correction: bool) -> Self {
        let rad = f64::from(degrees).to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        // Target canvas dimensions
        let w = self.width as f64;
        let h = self.height as f64;

        // Pixel aspect ratio scale: 160A pixels are twice as wide as they are tall
        let par_scale_x = if par_correction { 2.0 } else { 1.0 };

        let cx = (w - 1.0) * 0.5;
        let cy = (h - 1.0) * 0.5;

        let mut rotated = Self::new(self.width, self.height);

        for dy in 0..self.height {
            let y_rel = (dy as f64) - cy;
            for dx in 0..self.width {
                let x_rel = ((dx as f64) - cx) * par_scale_x;

                // Inverse rotate to find sample coordinates in source space
                let src_x_unscaled = x_rel * cos_a + y_rel * sin_a;
                let src_y = -x_rel * sin_a + y_rel * cos_a + cy;
                let src_x = (src_x_unscaled / par_scale_x) + cx;

                let sample_x = src_x.round() as i64;
                let sample_y = src_y.round() as i64;

                let val = self.get_pixel(sample_x, sample_y);
                rotated.set_pixel(dx as i64, dy as i64, val);
            }
        }

        rotated
    }

    /// Applies pixel aspect ratio (PAR) correction for the target hardware graphics mode.
    ///
    /// For 160A mode where CRT pixels are 2:1 wide, compresses horizontal
    /// resolution by 0.5x so a square-pixel source appears square on CRT.
    /// For 320 mode where CRT pixels are roughly 1:1, leaves dimensions unchanged.
    #[must_use]
    pub fn apply_par_correction(&self, mode: MariaMode) -> Self {
        match mode {
            MariaMode::Mode160A => {
                let new_width = (self.width / 2).max(1);
                self.resize(new_width, self.height)
            }
            MariaMode::Mode320 => self.clone(),
        }
    }

    /// Converts this 2D graphic into a hardware-ready `MariaSprite`.
    ///
    /// # Errors
    /// Returns [`GfxError::Conversion`] if any pixel indices exceed the mode's palette bounds,
    /// or if the dimensions exceed hardware byte bounds (`u8`).
    pub fn to_maria_sprite(&self, mode: MariaMode) -> Result<MariaSprite, GfxError> {
        let height_lines = u8::try_from(self.height).map_err(|_| {
            GfxError::Conversion(format!(
                "Height {} exceeds maximum hardware lines per sprite (255)",
                self.height
            ))
        })?;

        match mode {
            MariaMode::Mode160A => {
                if !self.pixels.iter().all(|&p| p <= 3) {
                    return Err(GfxError::Conversion(
                        "Graphic contains pixel indices > 3, not compatible with 160A mode".into(),
                    ));
                }

                // 4 pixels per byte in 160A
                let width_bytes = u8::try_from(self.width.div_ceil(4)).map_err(|_| {
                    GfxError::Conversion(format!(
                        "Width {} exceeds maximum hardware bytes (255)",
                        self.width
                    ))
                })?;

                // Each scanline is packed into `width_bytes`.
                // MARIA hardware down-counters decrement from (height-1) down to 0:
                // Scanline 0 (top line of zone) fetches page (addr_hi + height - 1).
                // Scanline H-1 (bottom line of zone) fetches page (addr_hi + 0).
                // Therefore, pages in memory are ordered bottom-up:
                // Page 0 is the bottom scanline, Page H-1 is the top scanline.
                let mut pages = Vec::with_capacity(height_lines as usize);

                for y in (0..self.height).rev() {
                    let iy = i64::try_from(y).unwrap_or(0);
                    let mut line_bytes = Vec::with_capacity(width_bytes as usize);
                    for byte_col in 0..i64::from(width_bytes) {
                        let p0 = self.get_pixel(byte_col * 4, iy) & 0x03;
                        let p1 = self.get_pixel(byte_col * 4 + 1, iy) & 0x03;
                        let p2 = self.get_pixel(byte_col * 4 + 2, iy) & 0x03;
                        let p3 = self.get_pixel(byte_col * 4 + 3, iy) & 0x03;

                        let packed = (p0 << 6) | (p1 << 4) | (p2 << 2) | p3;
                        line_bytes.push(packed);
                    }
                    pages.push(line_bytes);
                }

                Ok(MariaSprite {
                    mode,
                    width_bytes,
                    height_lines,
                    subpalette: 0,
                    scanline_bytes: pages,
                })
            }
            MariaMode::Mode320 => {
                // 8 pixels per byte in 320 mode (1 bit per pixel: 0 = transparent, 1 = color)
                let width_bytes = u8::try_from(self.width.div_ceil(8)).map_err(|_| {
                    GfxError::Conversion(format!(
                        "Width {} exceeds maximum hardware bytes (255)",
                        self.width
                    ))
                })?;

                let mut pages = Vec::with_capacity(height_lines as usize);

                for y in (0..self.height).rev() {
                    let iy = i64::try_from(y).unwrap_or(0);
                    let mut line_bytes = Vec::with_capacity(width_bytes as usize);
                    for byte_col in 0..i64::from(width_bytes) {
                        let mut packed = 0u8;
                        for bit in 0..8 {
                            let val = self.get_pixel(byte_col * 8 + bit, iy);
                            let p = u8::from(val > 0);
                            packed |= p << (7 - bit);
                        }
                        line_bytes.push(packed);
                    }
                    pages.push(line_bytes);
                }

                Ok(MariaSprite {
                    mode,
                    width_bytes,
                    height_lines,
                    subpalette: 0,
                    scanline_bytes: pages,
                })
            }
        }
    }
}

/// Represents a graphic encoded for the Atari 7800 MARIA DMA coprocessor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MariaSprite {
    pub mode: MariaMode,
    pub width_bytes: u8,
    pub height_lines: u8,
    /// Subpalette index (0..7) to target for 160A sprites (embedded in bits 7..5 of PAL_W).
    pub subpalette: u8,
    /// Vector of scanlines (0..height_lines-1), each containing `width_bytes` of packed graphic data.
    pub scanline_bytes: Vec<Vec<u8>>,
}

impl MariaSprite {
    /// Sets the subpalette slot index (0..7) and returns the updated sprite.
    #[must_use]
    pub fn with_subpalette(mut self, subpalette: u8) -> Self {
        self.subpalette = subpalette & 0x07;
        self
    }
    /// Formats the sprite as ca65 assembly source code with labels and `.byte` directives.
    #[must_use]
    pub fn to_ca65_assembly(&self, label: &str) -> String {
        Ca65Writer.write_sprite_asm(self, label)
    }

    /// Formats an include header (.inc) with equates and optional binary inclusion directive.
    #[must_use]
    pub fn write_inc(
        &self,
        writer: &dyn AssemblerWriter,
        label: &str,
        bin_filename: Option<&str>,
    ) -> String {
        writer.write_sprite_inc(self, label, bin_filename)
    }

    /// Save an include header (.inc) to disk.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_inc<P: AsRef<Path>>(
        &self,
        writer: &dyn AssemblerWriter,
        path: P,
        label: &str,
        bin_filename: Option<&str>,
    ) -> Result<(), GfxError> {
        let text = self.write_inc(writer, label, bin_filename);
        let mut file = File::create(path)?;
        file.write_all(text.as_bytes())?;
        Ok(())
    }

    /// Serializes the sprite into a 256-byte page-strided raw binary blob.
    ///
    /// In MARIA DMA addressing:
    /// Scanline 0 is at offset: `page * 256 + base_x`
    /// Scanline 1 is at offset: `(page + 1) * 256 + base_x`
    /// ...
    /// Scanline N is at offset: `(page + N) * 256 + base_x`
    ///
    /// If `page_strided` is true, each scanline begins at offset `line_idx * 256`.
    /// If false, scanlines are contiguous (for simple single-line tests or custom packing).
    #[must_use]
    pub fn to_binary(&self, page_strided: bool) -> Vec<u8> {
        if page_strided {
            // 256-byte page-strided buffer
            let total_size = (self.height_lines as usize) * 256;
            let mut out = vec![0u8; total_size];
            for (line_idx, line) in self.scanline_bytes.iter().enumerate() {
                let start = line_idx * 256;
                let end = start + line.len();
                out[start..end].copy_from_slice(line);
            }
            out
        } else {
            // Contiguous packing
            let mut out =
                Vec::with_capacity((self.width_bytes as usize) * (self.height_lines as usize));
            for line in &self.scanline_bytes {
                out.extend_from_slice(line);
            }
            out
        }
    }

    /// Save the sprite directly to a binary file.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_binary<P: AsRef<Path>>(&self, path: P, page_strided: bool) -> Result<(), GfxError> {
        let data = self.to_binary(page_strided);
        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    /// Save the sprite as ca65 assembly text.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_ca65<P: AsRef<Path>>(&self, path: P, label: &str) -> Result<(), GfxError> {
        let asm = self.to_ca65_assembly(label);
        let mut file = File::create(path)?;
        file.write_all(asm.as_bytes())?;
        Ok(())
    }
}

/// Metadata for a single frame within a `MariaSpriteBank`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MariaBankFrame {
    pub label: String,
    pub width_bytes: u8,
    pub offset_in_page: u8,
    pub pal_w: u8,
}

/// A bank of multiple sprite frames interleaved across 256-byte pages.
///
/// In MARIA hardware DMA:
/// Scanline Y is fetched from `(base_page + Y) * 256 + frame.offset_in_page`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MariaSpriteBank {
    pub mode: MariaMode,
    pub height_lines: u8,
    pub frames: Vec<MariaBankFrame>,
    /// Scanline pages: exactly `height_lines` pages, each 256 bytes wide.
    pub pages: Vec<Vec<u8>>,
}

impl MariaSpriteBank {
    /// Create an empty sprite bank for a given mode and height.
    #[must_use]
    pub fn new(mode: MariaMode, height_lines: u8) -> Self {
        Self {
            mode,
            height_lines,
            frames: Vec::new(),
            pages: vec![vec![0u8; 256]; usize::from(height_lines)],
        }
    }

    /// Add a frame to the bank.
    ///
    /// # Errors
    /// Returns [`GfxError::Conversion`] if the sprite height does not match the bank height,
    /// or if the total width of all frames exceeds the 256-byte page width limit.
    pub fn add_frame(
        &mut self,
        sprite: &MariaSprite,
        label: impl Into<String>,
    ) -> Result<usize, GfxError> {
        if sprite.height_lines != self.height_lines {
            return Err(GfxError::Conversion(format!(
                "Sprite height {} does not match bank height {}",
                sprite.height_lines, self.height_lines
            )));
        }

        let current_offset: usize = self.frames.iter().map(|f| usize::from(f.width_bytes)).sum();
        let next_end = current_offset + usize::from(sprite.width_bytes);
        if next_end > 256 {
            return Err(GfxError::Conversion(format!(
                "Total frame width {next_end} exceeds 256-byte page limit"
            )));
        }

        for (line_idx, line) in sprite.scanline_bytes.iter().enumerate() {
            self.pages[line_idx][current_offset..next_end].copy_from_slice(line);
        }

        let width_neg = (-i8::try_from(sprite.width_bytes).unwrap_or(0)) as u8;
        let pal_w = (width_neg & 0x1F) | ((sprite.subpalette & 0x07) << 5);

        self.frames.push(MariaBankFrame {
            label: label.into(),
            width_bytes: sprite.width_bytes,
            offset_in_page: current_offset as u8,
            pal_w,
        });

        Ok(self.frames.len() - 1)
    }

    /// Bake a 360-degree discrete rotation sequence into a `MariaSpriteBank`.
    ///
    /// # Errors
    /// Returns [`GfxError::Conversion`] if `num_angles` is 0 or conversion/packing fails.
    pub fn bake_rotation_sequence(
        graphic: &IndexedGraphic,
        num_angles: usize,
        par: bool,
        mode: MariaMode,
        label_prefix: &str,
    ) -> Result<Self, GfxError> {
        if num_angles == 0 {
            return Err(GfxError::Conversion("num_angles must be at least 1".into()));
        }

        let first_sprite = graphic.to_maria_sprite(mode)?;
        let mut bank = Self::new(mode, first_sprite.height_lines);

        let step_deg = 360.0 / (num_angles as f32);
        for i in 0..num_angles {
            let deg = (i as f32) * step_deg;
            let rot = graphic.rotate(deg, par);
            let rot_sprite = rot.to_maria_sprite(mode)?;
            bank.add_frame(&rot_sprite, format!("{label_prefix}_{i:02}"))?;
        }

        Ok(bank)
    }

    /// Formats the bank lookup tables and page data as ca65 assembly source code.
    #[must_use]
    pub fn to_ca65_assembly(&self, label: &str) -> String {
        Ca65Writer.write_sprite_bank_asm(self, label)
    }

    /// Formats only the ca65 header equates and lookup tables (.inc file).
    #[must_use]
    pub fn to_ca65_inc(&self, label: &str) -> String {
        Ca65Writer.write_sprite_bank_inc(self, label, None)
    }

    /// Formats an include header (.inc) with equates, frame tables, and optional binary inclusion directive.
    #[must_use]
    pub fn write_inc(
        &self,
        writer: &dyn AssemblerWriter,
        label: &str,
        bin_filename: Option<&str>,
    ) -> String {
        writer.write_sprite_bank_inc(self, label, bin_filename)
    }

    /// Save an include header (.inc) to disk.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_inc<P: AsRef<Path>>(
        &self,
        writer: &dyn AssemblerWriter,
        path: P,
        label: &str,
        bin_filename: Option<&str>,
    ) -> Result<(), GfxError> {
        let text = self.write_inc(writer, label, bin_filename);
        let mut file = File::create(path)?;
        file.write_all(text.as_bytes())?;
        Ok(())
    }

    /// Serializes the bank into a raw 256-byte page-strided binary buffer.
    #[must_use]
    pub fn to_binary(&self) -> Vec<u8> {
        let total_size = usize::from(self.height_lines) * 256;
        let mut out = Vec::with_capacity(total_size);
        for page in &self.pages {
            out.extend_from_slice(page);
        }
        out
    }

    /// Save the binary bank directly to disk.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_binary<P: AsRef<Path>>(&self, path: P) -> Result<(), GfxError> {
        let data = self.to_binary();
        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    /// Save the full ca65 assembly text to disk.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_ca65<P: AsRef<Path>>(&self, path: P, label: &str) -> Result<(), GfxError> {
        let asm = self.to_ca65_assembly(label);
        let mut file = File::create(path)?;
        file.write_all(asm.as_bytes())?;
        Ok(())
    }

    /// Save the ca65 include header (.inc) to disk.
    ///
    /// # Errors
    /// Returns [`GfxError::Io`] if file creation or writing fails.
    pub fn save_ca65_inc<P: AsRef<Path>>(&self, path: P, label: &str) -> Result<(), GfxError> {
        let inc = self.to_ca65_inc(label);
        let mut file = File::create(path)?;
        file.write_all(inc.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_pixel_access() {
        let mut gfx = IndexedGraphic::new(8, 8);
        assert_eq!(gfx.get_pixel(0, 0), 0);
        gfx.set_pixel(3, 4, 2);
        assert_eq!(gfx.get_pixel(3, 4), 2);
        assert_eq!(gfx.get_pixel(8, 8), 0); // Out of bounds
    }

    #[test]
    fn test_flips() {
        let mut gfx = IndexedGraphic::new(4, 2);
        // Line 0: [1, 2, 0, 0]
        // Line 1: [0, 0, 3, 0]
        gfx.set_pixel(0, 0, 1);
        gfx.set_pixel(1, 0, 2);
        gfx.set_pixel(2, 1, 3);

        let h_flip = gfx.flip_h();
        assert_eq!(h_flip.get_pixel(3, 0), 1);
        assert_eq!(h_flip.get_pixel(2, 0), 2);
        assert_eq!(h_flip.get_pixel(1, 1), 3);

        let v_flip = gfx.flip_v();
        assert_eq!(v_flip.get_pixel(0, 1), 1);
        assert_eq!(v_flip.get_pixel(1, 1), 2);
        assert_eq!(v_flip.get_pixel(2, 0), 3);
    }

    #[test]
    fn test_scale_down_and_up() {
        let mut gfx = IndexedGraphic::new(4, 4);
        gfx.set_pixel(0, 0, 1);
        gfx.set_pixel(1, 0, 1);
        gfx.set_pixel(0, 1, 1);
        gfx.set_pixel(1, 1, 1);

        let scaled_down = gfx.scale(0.5, 0.5);
        assert_eq!(scaled_down.width, 2);
        assert_eq!(scaled_down.height, 2);
        assert_eq!(scaled_down.get_pixel(0, 0), 1);

        let scaled_up = gfx.scale(2.0, 2.0);
        assert_eq!(scaled_up.width, 8);
        assert_eq!(scaled_up.height, 8);
    }

    #[test]
    fn test_rotate_90_degrees() {
        let mut gfx = IndexedGraphic::new(5, 5);
        // Top-center pixel
        gfx.set_pixel(2, 0, 3);

        // 90 deg clockwise without PAR correction
        let rotated = gfx.rotate(90.0, false);
        // Should land near right-center: (4, 2)
        assert_eq!(rotated.get_pixel(4, 2), 3);
    }

    #[test]
    fn test_to_maria_sprite_160a() {
        let mut gfx = IndexedGraphic::new(4, 2);
        // Line 0: [0, 2, 0, 0] -> bits 00, 10, 00, 00 -> 0x20
        gfx.set_pixel(1, 0, 2);
        // Line 1: [1, 2, 3, 0] -> bits 01, 10, 11, 00 -> 0x6C
        gfx.set_pixel(0, 1, 1);
        gfx.set_pixel(1, 1, 2);
        gfx.set_pixel(2, 1, 3);

        let sprite = gfx.to_maria_sprite(MariaMode::Mode160A).unwrap();
        assert_eq!(sprite.width_bytes, 1);
        assert_eq!(sprite.height_lines, 2);
        // Bottom-up order: page 0 is bottom line (0x6C), page 1 is top line (0x20)
        assert_eq!(sprite.scanline_bytes[0], vec![0x6C]);
        assert_eq!(sprite.scanline_bytes[1], vec![0x20]);

        let asm = sprite.to_ca65_assembly("test_sprite");
        assert!(asm.contains("test_sprite_line_00:\n    .byte $6C"));
        assert!(asm.contains("test_sprite_line_01:\n    .byte $20"));

        let contiguous = sprite.to_binary(false);
        assert_eq!(contiguous, vec![0x6C, 0x20]);

        let strided = sprite.to_binary(true);
        assert_eq!(strided.len(), 512); // 2 lines * 256
        assert_eq!(strided[0], 0x6C);
        assert_eq!(strided[256], 0x20);
    }

    #[test]
    fn test_scale_centered() {
        let mut gfx = IndexedGraphic::new(8, 8);
        // Put a 2x2 square in the center (3,3) to (4,4)
        gfx.set_pixel(3, 3, 2);
        gfx.set_pixel(4, 3, 2);
        gfx.set_pixel(3, 4, 2);
        gfx.set_pixel(4, 4, 2);

        // Scale by 1.0 keeps canvas and center intact
        let unchanged = gfx.scale_centered(1.0, 1.0);
        assert_eq!(unchanged.width, 8);
        assert_eq!(unchanged.height, 8);
        assert_eq!(unchanged.get_pixel(3, 3), 2);

        // Scale by 2.0 expands the center block
        let scaled_up = gfx.scale_centered(2.0, 2.0);
        assert_eq!(scaled_up.width, 8);
        assert_eq!(scaled_up.height, 8);
        assert_eq!(scaled_up.get_pixel(3, 3), 2);
        assert_eq!(scaled_up.get_pixel(2, 2), 2);
    }

    #[test]
    fn test_sprite_bank_rotations() {
        let mut gfx = IndexedGraphic::new(16, 16);
        // Draw a simple arrow / ship pointing up
        gfx.set_pixel(8, 2, 3);
        gfx.set_pixel(7, 3, 3);
        gfx.set_pixel(8, 3, 3);
        gfx.set_pixel(9, 3, 3);

        let bank =
            MariaSpriteBank::bake_rotation_sequence(&gfx, 16, true, MariaMode::Mode160A, "ship")
                .unwrap();

        assert_eq!(bank.frames.len(), 16);
        assert_eq!(bank.height_lines, 16);
        assert_eq!(bank.pages.len(), 16);

        // In 160A mode, 16 pixels = 4 bytes per frame.
        // 16 frames * 4 bytes = 64 bytes total width per page.
        for (i, frame) in bank.frames.iter().enumerate() {
            assert_eq!(frame.width_bytes, 4);
            assert_eq!(frame.offset_in_page, (i * 4) as u8);
        }

        let bin = bank.to_binary();
        assert_eq!(bin.len(), 16 * 256); // 16 pages * 256 bytes = 4096 bytes

        let asm = bank.to_ca65_assembly("ship_bank");
        assert!(asm.contains("ship_bank_frame_count = 16"));
        assert!(asm.contains("ship_bank_page_00:"));
        assert!(asm.contains("ship_bank_frame_lo:"));

        let inc = bank.to_ca65_inc("ship_bank");
        assert!(inc.contains("ship_bank_frame_count = 16"));
        assert!(inc.contains("ship_bank_frame_lo:"));
    }

    #[test]
    fn test_bake_atari_asset() {
        let asset_path = std::path::Path::new("../examples/transform_sprite/assets/atari.png");
        let alt_path = std::path::Path::new("examples/transform_sprite/assets/atari.png");
        let path = if asset_path.exists() {
            asset_path
        } else if alt_path.exists() {
            alt_path
        } else {
            return;
        };

        let gfx = IndexedGraphic::load_png(path).expect("Failed to load canonical atari PNG");
        assert_eq!(gfx.width, 16);
        assert_eq!(gfx.height, 16);

        let bank =
            MariaSpriteBank::bake_rotation_sequence(&gfx, 16, true, MariaMode::Mode160A, "atari")
                .expect("Failed to bake rotation sequence");

        assert_eq!(bank.frames.len(), 16);
        let temp_dir = std::env::temp_dir();
        let out_file = temp_dir.join("atari_bank_test.s");
        bank.save_ca65(&out_file, "atari_bank")
            .expect("save atari_bank.s");
        assert!(out_file.exists());
        let _ = std::fs::remove_file(out_file);
    }

    #[test]
    fn test_apply_par_correction() {
        let graphic = IndexedGraphic::new(20, 16);
        let corrected_160a = graphic.apply_par_correction(MariaMode::Mode160A);
        assert_eq!(corrected_160a.width, 10);
        assert_eq!(corrected_160a.height, 16);

        let corrected_320 = graphic.apply_par_correction(MariaMode::Mode320);
        assert_eq!(corrected_320.width, 20);
        assert_eq!(corrected_320.height, 16);
    }

    #[test]
    fn test_assembler_writers_sprite_inc() {
        let mut gfx = IndexedGraphic::new(4, 2);
        gfx.set_pixel(1, 0, 2);
        let sprite = gfx.to_maria_sprite(MariaMode::Mode160A).unwrap();

        let ca65 = Ca65Writer;
        let ca65_inc = sprite.write_inc(&ca65, "ship", Some("ship.bin"));
        assert!(ca65_inc.contains("ship_width_pixels = 4"));
        assert!(ca65_inc.contains("ship_width_bytes = 1"));
        assert!(ca65_inc.contains("ship_height_lines = 2"));
        assert!(ca65_inc.contains("ship_pal_w = $1F"));
        assert!(ca65_inc.contains("ship_size_bytes = 512"));
        assert!(ca65_inc.contains("ship_mode = 0"));
        assert!(ca65_inc.contains(".align $100"));
        assert!(ca65_inc.contains("ship_pages:"));
        assert!(ca65_inc.contains(".incbin \"ship.bin\""));

        let dasm = DasmWriter;
        let dasm_inc = sprite.write_inc(&dasm, "ship", Some("ship.bin"));
        assert!(dasm_inc.contains("ship_width_pixels EQU 4"));
        assert!(dasm_inc.contains("ship_width_bytes EQU 1"));
        assert!(dasm_inc.contains("ship_height_lines EQU 2"));
        assert!(dasm_inc.contains("ship_mode EQU 0"));
        assert!(dasm_inc.contains("ALIGN 256"));
        assert!(dasm_inc.contains("ship_pages:"));
        assert!(dasm_inc.contains("INCBIN \"ship.bin\""));

        let mads = MadsWriter;
        let mads_inc = sprite.write_inc(&mads, "ship", Some("ship.bin"));
        assert!(mads_inc.contains("ship_width_pixels = 4"));
        assert!(mads_inc.contains("ship_width_bytes = 1"));
        assert!(mads_inc.contains("ship_height_lines = 2"));
        assert!(mads_inc.contains("ship_mode = 0"));
        assert!(mads_inc.contains(".align $100"));
        assert!(mads_inc.contains("ship_pages:"));
        assert!(mads_inc.contains("ins \"ship.bin\""));
    }

    #[test]
    fn test_assembler_writers_bank_inc() {
        let mut gfx = IndexedGraphic::new(8, 8);
        gfx.set_pixel(2, 2, 2);
        let bank =
            MariaSpriteBank::bake_rotation_sequence(&gfx, 4, false, MariaMode::Mode160A, "star")
                .unwrap();

        let ca65 = Ca65Writer;
        let ca65_inc = bank.write_inc(&ca65, "star_bank", Some("star_bank.bin"));
        assert!(ca65_inc.contains("star_bank_frame_count = 4"));
        assert!(ca65_inc.contains("star_bank_height_lines = 8"));
        assert!(ca65_inc.contains("star_bank_size_bytes = 2048"));
        assert!(ca65_inc.contains("star_bank_mode = 0"));
        assert!(ca65_inc.contains("star_bank_frame_lo:"));
        assert!(ca65_inc.contains(".byte $00, $02, $04, $06"));
        assert!(ca65_inc.contains("star_bank_frame_width_bytes:"));
        assert!(ca65_inc.contains("star_bank_frame_width_pixels:"));
        assert!(ca65_inc.contains("star_bank_frame_width = star_bank_frame_width_bytes"));
        assert!(ca65_inc.contains(".align $100"));
        assert!(ca65_inc.contains("star_bank_pages:"));
        assert!(ca65_inc.contains(".incbin \"star_bank.bin\""));

        let dasm = DasmWriter;
        let dasm_inc = bank.write_inc(&dasm, "star_bank", Some("star_bank.bin"));
        assert!(dasm_inc.contains("star_bank_frame_count EQU 4"));
        assert!(dasm_inc.contains("ALIGN 256"));
        assert!(dasm_inc.contains("INCBIN \"star_bank.bin\""));
        assert!(dasm_inc.contains("HEX 00 02 04 06"));
    }

    #[test]
    fn test_assembler_type_creation() {
        let ca65_writer = AssemblerType::Ca65.create_writer();
        assert_eq!(ca65_writer.name(), "ca65");

        let dasm_writer = AssemblerType::Dasm.create_writer();
        assert_eq!(dasm_writer.name(), "dasm");

        let mads_writer = AssemblerType::Mads.create_writer();
        assert_eq!(mads_writer.name(), "mads");
    }
}
