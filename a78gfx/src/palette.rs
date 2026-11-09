use crate::{AssemblerWriter, GfxError, IndexedGraphic, MariaMode};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// A 24-bit RGB color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl fmt::Display for ColorRgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl ColorRgb {
    /// Creates a new RGB color.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Calculates perceptual squared color distance using the redmean algorithm.
    ///
    /// The redmean formula adjusts Euclidean weights based on the average red level
    /// to approximate human eye sensitivity across color spectrums.
    #[must_use]
    pub fn distance_squared(&self, other: &Self) -> u32 {
        let r_mean = u32::midpoint(u32::from(self.r), u32::from(other.r));
        let dr = i32::from(self.r) - i32::from(other.r);
        let dg = i32::from(self.g) - i32::from(other.g);
        let db = i32::from(self.b) - i32::from(other.b);

        let dr2 = (dr * dr) as u32;
        let dg2 = (dg * dg) as u32;
        let db2 = (db * db) as u32;

        let weight_r = 2 + (r_mean >> 8);
        let weight_g = 4;
        let weight_b = 3;

        weight_r * dr2 + weight_g * dg2 + weight_b * db2
    }

    /// Finds the index of the closest color in a slice of candidate colors.
    #[must_use]
    pub fn nearest_index(&self, palette: &[Self]) -> usize {
        if palette.is_empty() {
            return 0;
        }
        let mut best_idx = 0;
        let mut min_dist = u32::MAX;
        for (i, c) in palette.iter().enumerate() {
            let dist = self.distance_squared(c);
            if dist < min_dist {
                min_dist = dist;
                best_idx = i;
            }
        }
        best_idx
    }

    /// Maps this RGB color to the closest standard Atari 7800 MariaColor ($00..$FF).
    #[must_use]
    pub fn to_nearest_maria_color(&self) -> MariaColor {
        MariaColor::from_rgb(self)
    }
}

/// A 32-bit RGBA color with 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba {
    /// Creates a new RGBA color.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Drops the alpha channel and returns the RGB components.
    #[must_use]
    pub const fn to_rgb(&self) -> ColorRgb {
        ColorRgb::new(self.r, self.g, self.b)
    }
}

/// Represents an Atari 7800 hardware color byte (`$CH`), where high nibble `C` is hue (0..15)
/// and low nibble `H` is luminance (0..15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MariaColor(pub u8);

impl MariaColor {
    /// Returns the raw hardware register byte value ($00..$FF).
    #[must_use]
    pub const fn as_byte(&self) -> u8 {
        self.0
    }

    /// Hue component (0..15).
    #[must_use]
    pub const fn hue(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }

    /// Luminance component (0..15).
    #[must_use]
    pub const fn luminance(&self) -> u8 {
        self.0 & 0x0F
    }

    /// Returns the standard RGB representation of this Atari 7800 color.
    #[must_use]
    pub fn to_rgb(&self) -> ColorRgb {
        ATARI_7800_NTSC_PALETTE[usize::from(self.0)]
    }

    /// Maps an RGB color to the closest Atari 7800 hardware color code ($00..$FF).
    #[must_use]
    pub fn from_rgb(rgb: &ColorRgb) -> Self {
        let idx = rgb.nearest_index(&ATARI_7800_NTSC_PALETTE);
        Self(idx as u8)
    }

    /// Formats the color code as a hexadecimal assembly string (e.g. `"$36"`).
    #[must_use]
    pub fn format_hex(&self) -> String {
        format!("${:02X}", self.0)
    }
}

impl fmt::Display for MariaColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${:02X}", self.0)
    }
}

/// A named color entry in a palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorEntry {
    pub rgb: ColorRgb,
    pub name: String,
}

/// Represents a loaded project palette containing color entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPalette {
    pub name: String,
    pub entries: Vec<ColorEntry>,
}

impl ProjectPalette {
    /// Creates a new palette with a given name and color entries.
    #[must_use]
    pub fn new(name: impl Into<String>, entries: Vec<ColorEntry>) -> Self {
        Self {
            name: name.into(),
            entries,
        }
    }

    /// Creates an empty palette.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            entries: Vec::new(),
        }
    }

    /// Number of color entries in the palette.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the palette has no color entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the background color (entry 0 or black if empty).
    #[must_use]
    pub fn background(&self) -> ColorRgb {
        self.entries
            .first()
            .map_or(ColorRgb::new(0, 0, 0), |e| e.rgb)
    }

    /// Extracts the 4-color subpalette `[c0, c1, c2, c3]` for subpalette slot `slot` (0..=7).
    ///
    /// - `c0`: Background / transparent color.
    /// - `c1..=c3`: Active foreground sprite colors.
    #[must_use]
    pub fn get_subpalette(&self, slot: usize) -> [ColorRgb; 4] {
        let slot = slot.min(7);
        let n = self.entries.len();

        if n == 0 {
            return [
                ColorRgb::new(0, 0, 0),
                ColorRgb::new(255, 255, 255),
                ColorRgb::new(170, 170, 170),
                ColorRgb::new(85, 85, 85),
            ];
        }

        // Dedicated 4-color palette for a single sprite
        if n <= 4 {
            let c0 = self
                .entries
                .first()
                .map_or(ColorRgb::new(0, 0, 0), |e| e.rgb);
            let c1 = self
                .entries
                .get(1)
                .map_or(ColorRgb::new(255, 255, 255), |e| e.rgb);
            let c2 = self
                .entries
                .get(2)
                .map_or(ColorRgb::new(170, 170, 170), |e| e.rgb);
            let c3 = self
                .entries
                .get(3)
                .map_or(ColorRgb::new(85, 85, 85), |e| e.rgb);
            return [c0, c1, c2, c3];
        }

        // Full 7800 palette layout: 1 background + 8 * 3 colors (25 total entries)
        if n == 25 {
            let c0 = self.entries[0].rgb;
            let base = 1 + slot * 3;
            let c1 = self.entries[base].rgb;
            let c2 = self.entries[base + 1].rgb;
            let c3 = self.entries[base + 2].rgb;
            return [c0, c1, c2, c3];
        }

        // 8 palettes of 4 colors (32 total entries)
        let chunk_start = slot * 4;
        if chunk_start + 3 < n {
            return [
                self.entries[chunk_start].rgb,
                self.entries[chunk_start + 1].rgb,
                self.entries[chunk_start + 2].rgb,
                self.entries[chunk_start + 3].rgb,
            ];
        }

        // Chunk layout with shared background (entry 0 + slot * 3)
        let base = 1 + slot * 3;
        if base + 2 < n {
            return [
                self.entries[0].rgb,
                self.entries[base].rgb,
                self.entries[base + 1].rgb,
                self.entries[base + 2].rgb,
            ];
        }

        // Fallback: take first 4 entries
        let c0 = self.entries[0].rgb;
        let c1 = self.entries.get(1).map_or(c0, |e| e.rgb);
        let c2 = self.entries.get(2).map_or(c1, |e| e.rgb);
        let c3 = self.entries.get(3).map_or(c2, |e| e.rgb);
        [c0, c1, c2, c3]
    }

    /// Converts the subpalette for `slot` into Atari 7800 hardware color bytes.
    #[must_use]
    pub fn hardware_subpalette(&self, slot: usize) -> [MariaColor; 4] {
        let rgb_pal = self.get_subpalette(slot);
        [
            MariaColor::from_rgb(&rgb_pal[0]),
            MariaColor::from_rgb(&rgb_pal[1]),
            MariaColor::from_rgb(&rgb_pal[2]),
            MariaColor::from_rgb(&rgb_pal[3]),
        ]
    }

    /// Emits assembler equate definitions for this subpalette's hardware color registers.
    #[must_use]
    pub fn format_subpalette_equates(
        &self,
        writer: &dyn AssemblerWriter,
        prefix: &str,
        slot: usize,
    ) -> String {
        use std::fmt::Write as _;
        let hw = self.hardware_subpalette(slot);
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{}",
            writer.format_comment(&format!(
                "Hardware Palette {slot} Equates (from '{}')",
                self.name
            ))
        );
        let _ = writeln!(
            out,
            "{}",
            writer.format_equate(&format!("{prefix}_pal{slot}_c1"), &hw[1].format_hex())
        );
        let _ = writeln!(
            out,
            "{}",
            writer.format_equate(&format!("{prefix}_pal{slot}_c2"), &hw[2].format_hex())
        );
        let _ = writeln!(
            out,
            "{}",
            writer.format_equate(&format!("{prefix}_pal{slot}_c3"), &hw[3].format_hex())
        );
        out
    }
}

/// Abstract decoder trait for palette file formats.
pub trait PaletteDecoder: fmt::Debug {
    /// Decodes a palette from any reader implementing [`BufRead`].
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails.
    fn decode<R: BufRead>(&self, reader: R) -> Result<ProjectPalette, GfxError>;

    /// File extensions associated with this format (e.g. `["gpl"]`).
    fn extensions(&self) -> &'static [&'static str];
}

/// Supported palette file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteFormat {
    /// GIMP Palette (.gpl), also exported by Aseprite, LibreSprite, and Photoshop plugins.
    Gpl,
}

impl fmt::Display for PaletteFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpl => write!(f, "GIMP Palette (.gpl)"),
        }
    }
}

impl PaletteFormat {
    /// Detects palette format from file extension or content magic.
    ///
    /// # Errors
    /// Returns [`GfxError::UnsupportedFormat`] if format cannot be identified.
    pub fn detect<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let path_ref = path.as_ref();
        if let Some(ext) = path_ref.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("gpl") {
                return Ok(Self::Gpl);
            }
        }

        if let Ok(file) = File::open(path_ref) {
            let mut reader = BufReader::new(file);
            let mut first_line = String::new();
            if reader.read_line(&mut first_line).is_ok()
                && first_line.trim().eq_ignore_ascii_case("gimp palette")
            {
                return Ok(Self::Gpl);
            }
        }

        Err(GfxError::UnsupportedFormat(format!(
            "Could not determine palette format for '{}'",
            path_ref.display()
        )))
    }

    /// Decodes a palette using this format.
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails.
    pub fn decode<R: BufRead>(&self, reader: R) -> Result<ProjectPalette, GfxError> {
        match self {
            Self::Gpl => GplDecoder.decode(reader),
        }
    }
}

/// Decoder for the GIMP Palette (.gpl) format.
#[derive(Debug, Clone, Copy, Default)]
pub struct GplDecoder;

impl PaletteDecoder for GplDecoder {
    fn decode<R: BufRead>(&self, reader: R) -> Result<ProjectPalette, GfxError> {
        let mut palette_name = String::from("Unnamed");
        let mut entries = Vec::new();
        let mut header_found = false;

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if !header_found {
                if trimmed.eq_ignore_ascii_case("gimp palette") {
                    header_found = true;
                    continue;
                }
                return Err(GfxError::InvalidPalette(format!(
                    "Invalid GPL header: expected 'GIMP Palette', got '{trimmed}'"
                )));
            }

            if let Some(rest) = trimmed.strip_prefix("Name:") {
                palette_name = rest.trim().to_string();
                continue;
            }

            if trimmed.starts_with("Columns:") || trimmed.starts_with('#') {
                continue;
            }

            // Parse color line: R G B [Name]
            let mut parts = trimmed.split_whitespace();
            let r_str = parts.next().ok_or_else(|| {
                GfxError::InvalidPalette(format!("Missing red component in line '{trimmed}'"))
            })?;
            let g_str = parts.next().ok_or_else(|| {
                GfxError::InvalidPalette(format!("Missing green component in line '{trimmed}'"))
            })?;
            let b_str = parts.next().ok_or_else(|| {
                GfxError::InvalidPalette(format!("Missing blue component in line '{trimmed}'"))
            })?;

            let r: u8 = r_str.parse().map_err(|_| {
                GfxError::InvalidPalette(format!("Invalid red value '{r_str}' in '{trimmed}'"))
            })?;
            let g: u8 = g_str.parse().map_err(|_| {
                GfxError::InvalidPalette(format!("Invalid green value '{g_str}' in '{trimmed}'"))
            })?;
            let b: u8 = b_str.parse().map_err(|_| {
                GfxError::InvalidPalette(format!("Invalid blue value '{b_str}' in '{trimmed}'"))
            })?;

            let name: String = parts.collect::<Vec<_>>().join(" ");
            entries.push(ColorEntry {
                rgb: ColorRgb::new(r, g, b),
                name,
            });
        }

        if !header_found {
            return Err(GfxError::InvalidPalette(
                "Empty file or missing 'GIMP Palette' header".into(),
            ));
        }

        Ok(ProjectPalette::new(palette_name, entries))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gpl"]
    }
}

/// Abstract container representing a loaded palette file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteFile {
    pub path: Option<PathBuf>,
    pub format: PaletteFormat,
    pub palette: ProjectPalette,
}

impl PaletteFile {
    /// Load a palette file from disk using automatic format detection.
    ///
    /// # Errors
    /// Returns [`GfxError`] if file reading or decoding fails.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let p = path.as_ref();
        let format = PaletteFormat::detect(p)?;
        let file = File::open(p)?;
        let reader = BufReader::new(file);
        let palette = format.decode(reader)?;
        Ok(Self {
            path: Some(p.to_path_buf()),
            format,
            palette,
        })
    }

    /// Load a palette from any reader implementing [`BufRead`] with an explicit format.
    ///
    /// # Errors
    /// Returns [`GfxError`] if decoding fails.
    pub fn from_reader<R: BufRead>(reader: R, format: PaletteFormat) -> Result<Self, GfxError> {
        let palette = format.decode(reader)?;
        Ok(Self {
            path: None,
            format,
            palette,
        })
    }

    /// Consumes this `PaletteFile` and returns the inner [`ProjectPalette`].
    #[must_use]
    pub fn into_palette(self) -> ProjectPalette {
        self.palette
    }
}

/// An unquantized 2D raster of 32-bit RGBA pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaGraphic {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<ColorRgba>,
}

/// Canvas offset of the source's top-left so the two centers coincide.
///
/// Leaving this exact (not rounded to a pixel) is what keeps a symmetric source symmetric.
/// Snapping the origin walks the sprite by a pixel between nearby scale factors.
fn centered_origin(src_extent: usize, dst_extent: usize, factor: f64) -> f64 {
    (dst_extent as f64) * 0.5 - (src_extent as f64) * 0.5 * factor
}

/// Coverage of one connected source shape inside a single output pixel.
struct ComponentCoverage {
    id: u32,
    colors: [f64; 4],
}

impl ComponentCoverage {
    fn ink(&self) -> f64 {
        self.colors[1] + self.colors[2] + self.colors[3]
    }

    /// Palette index with the most coverage. Index 0 is never returned.
    fn color(&self) -> Option<u8> {
        let mut best_i = 0_usize;
        let mut best_w = 0.0_f64;
        for (i, &weight) in self.colors.iter().enumerate().skip(1) {
            if weight > best_w {
                best_w = weight;
                best_i = i;
            }
        }
        if best_i == 0 {
            None
        } else {
            Some(best_i as u8)
        }
    }
}

/// 4-connected labels for non-zero `mask` pixels. Label 0 is background.
fn component_labels(mask: &[u8], width: usize) -> Vec<u32> {
    let mut labels = vec![0_u32; mask.len()];
    if width == 0 {
        return labels;
    }
    let height = mask.len() / width;
    let mut next_id = 1_u32;
    let mut stack = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let start = y * width + x;
            if mask[start] == 0 || labels[start] != 0 {
                continue;
            }
            let id = next_id;
            next_id = next_id.saturating_add(1);
            labels[start] = id;
            stack.push((x, y));
            while let Some((cx, cy)) = stack.pop() {
                let neighbors = [
                    (cx.wrapping_sub(1), cy, cx > 0),
                    (cx + 1, cy, cx + 1 < width),
                    (cx, cy.wrapping_sub(1), cy > 0),
                    (cx, cy + 1, cy + 1 < height),
                ];
                for (nx, ny, inside) in neighbors {
                    if !inside {
                        continue;
                    }
                    let ni = ny * width + nx;
                    if mask[ni] != 0 && labels[ni] == 0 {
                        labels[ni] = id;
                        stack.push((nx, ny));
                    }
                }
            }
        }
    }
    labels
}

/// Open a one-pixel slot wherever two different shapes sit on neighbouring pixels.
///
/// The slot is made by sliding the left shape left, or the right shape right, into an
/// empty pixel. If neither side has room, the longer of the two runs gives up its
/// touching pixel.
fn open_component_gaps(ids: &mut [u32], colors: &mut [u8]) {
    let width = ids.len();
    for _ in 0..width {
        let mut opened = false;
        for i in 0..width.saturating_sub(1) {
            let left = ids[i];
            let right = ids[i + 1];
            if left == 0 || right == 0 || left == right {
                continue;
            }
            let mut start = i;
            while start > 0 && ids[start - 1] == left {
                start -= 1;
            }
            if start > 0 && ids[start - 1] == 0 {
                ids[start - 1] = left;
                colors[start - 1] = colors[start];
                ids[i] = 0;
                colors[i] = 0;
                opened = true;
                break;
            }
            let mut end = i + 1;
            while end + 1 < width && ids[end + 1] == right {
                end += 1;
            }
            if end + 1 < width && ids[end + 1] == 0 {
                ids[end + 1] = right;
                colors[end + 1] = colors[end];
                ids[i + 1] = 0;
                colors[i + 1] = 0;
                opened = true;
                break;
            }
            let left_len = i - start + 1;
            let right_len = end - (i + 1) + 1;
            if left_len >= right_len {
                ids[i] = 0;
                colors[i] = 0;
            } else {
                ids[i + 1] = 0;
                colors[i + 1] = 0;
            }
            opened = true;
            break;
        }
        if !opened {
            break;
        }
    }
}

impl RgbaGraphic {
    /// Creates a new graphic filled with transparent pixels.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![ColorRgba::new(0, 0, 0, 0); width * height],
        }
    }

    /// Reads pixel at `(x, y)`.
    #[must_use]
    pub fn get_pixel(&self, x: usize, y: usize) -> ColorRgba {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            ColorRgba::new(0, 0, 0, 0)
        }
    }

    /// Loads a PNG from disk as unquantized RGBA.
    ///
    /// # Errors
    /// Returns [`GfxError`] if the file cannot be read or decoded.
    pub fn load_png<P: AsRef<Path>>(path: P) -> Result<Self, GfxError> {
        let file = File::open(path)?;
        Self::from_png_reader(BufReader::new(file))
    }

    /// Resamples into a `new_width` x `new_height` canvas with a premultiplied-alpha box
    /// (area-weighted) filter, so partly covered pixels keep partial alpha instead of being
    /// dropped or kept whole.
    #[must_use]
    pub fn resize(&self, new_width: usize, new_height: usize) -> Self {
        if new_width == 0 || new_height == 0 || self.width == 0 || self.height == 0 {
            return Self::new(new_width, new_height);
        }

        let factor_x = new_width as f64 / self.width as f64;
        let factor_y = new_height as f64 / self.height as f64;
        self.scale_area(new_width, new_height, factor_x, factor_y)
    }

    /// Applies pixel aspect ratio (PAR) correction for the target hardware graphics mode.
    ///
    /// For 160A mode where CRT pixels are 2:1 wide, compresses horizontal resolution by 0.5x
    /// so a square-pixel source appears square on the CRT. 320 mode is left unchanged.
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

    /// Resamples into an `out_width` x `out_height` canvas with a box (area-weighted) filter.
    ///
    /// `factor_x` / `factor_y` are output pixels per source pixel. A factor below the
    /// canvas-to-source ratio shrinks the source around the middle and leaves a transparent
    /// border; exactly 1.0 pixel covers the whole canvas. Each output pixel averages the
    /// source area it covers in premultiplied alpha, so partly covered pixels get partial
    /// alpha instead of being dropped or kept whole (as nearest-neighbour sampling does).
    /// The source center maps exactly onto the canvas center, so a symmetric source stays
    /// symmetric and neighbouring scale factors do not jump by a pixel.
    #[must_use]
    pub fn scale_area(
        &self,
        out_width: usize,
        out_height: usize,
        factor_x: f64,
        factor_y: f64,
    ) -> Self {
        let mut out = Self::new(out_width, out_height);
        if factor_x <= f64::EPSILON
            || factor_y <= f64::EPSILON
            || self.width == 0
            || self.height == 0
        {
            return out;
        }

        let origin_x = centered_origin(self.width, out_width, factor_x);
        let origin_y = centered_origin(self.height, out_height, factor_y);

        for dy in 0..out_height {
            let y0 = (dy as f64 - origin_y) / factor_y;
            let y1 = ((dy + 1) as f64 - origin_y) / factor_y;
            for dx in 0..out_width {
                let x0 = (dx as f64 - origin_x) / factor_x;
                let x1 = ((dx + 1) as f64 - origin_x) / factor_x;

                let footprint = (x1 - x0) * (y1 - y0);
                let mut sum_a = 0.0_f64;
                let mut sum_r = 0.0_f64;
                let mut sum_g = 0.0_f64;
                let mut sum_b = 0.0_f64;

                let sy_lo = y0.floor().max(0.0) as usize;
                let sy_hi = (y1.ceil().max(0.0) as usize).min(self.height);
                let sx_lo = x0.floor().max(0.0) as usize;
                let sx_hi = (x1.ceil().max(0.0) as usize).min(self.width);

                for sy in sy_lo..sy_hi {
                    let wy = (y1.min((sy + 1) as f64) - y0.max(sy as f64)).max(0.0);
                    for sx in sx_lo..sx_hi {
                        let wx = (x1.min((sx + 1) as f64) - x0.max(sx as f64)).max(0.0);
                        let p = self.pixels[sy * self.width + sx];
                        let wa = wx * wy * f64::from(p.a);
                        sum_a += wa;
                        sum_r += wa * f64::from(p.r);
                        sum_g += wa * f64::from(p.g);
                        sum_b += wa * f64::from(p.b);
                    }
                }

                if sum_a > 0.0 && footprint > 0.0 {
                    let to_u8 = |v: f64| v.round().clamp(0.0, 255.0) as u8;
                    out.pixels[dy * out_width + dx] = ColorRgba::new(
                        to_u8(sum_r / sum_a),
                        to_u8(sum_g / sum_a),
                        to_u8(sum_b / sum_a),
                        to_u8(sum_a / footprint),
                    );
                }
            }
        }

        out
    }

    /// Box-filters into an indexed canvas, keeping the palette color that covers the most of
    /// each output pixel.
    ///
    /// Source pixels with alpha below 128 are transparent. An output pixel becomes transparent
    /// when its ink coverage, on the 0..255 scale, rounds to less than `coverage_threshold`.
    /// Coverage is measured per connected shape, not summed across shapes: the slot between
    /// the Fuji's bars is not ink just because both bars spill into it. If two shapes would
    /// still land on neighbouring pixels, they are shifted apart by one pixel so the slot
    /// stays open. The winning color is a real palette entry: covered colors are not averaged.
    /// The same center mapping as [`Self::scale_area`] is used, so a symmetric source stays
    /// put as the factor changes.
    #[must_use]
    pub fn scale_area_indexed(
        &self,
        out_width: usize,
        out_height: usize,
        factor_x: f64,
        factor_y: f64,
        palette: &ProjectPalette,
        subpalette_slot: usize,
        mode: MariaMode,
        coverage_threshold: u8,
    ) -> IndexedGraphic {
        let mut out = IndexedGraphic::new(out_width, out_height);
        if factor_x <= f64::EPSILON
            || factor_y <= f64::EPSILON
            || self.width == 0
            || self.height == 0
            || out_width == 0
            || out_height == 0
        {
            return out;
        }

        let src_index = self.classify_for_scale(palette, subpalette_slot, mode);
        let components = component_labels(&src_index, self.width);
        let origin_x = centered_origin(self.width, out_width, factor_x);
        let origin_y = centered_origin(self.height, out_height, factor_y);
        let color_slots = match mode {
            MariaMode::Mode160A => 3,
            MariaMode::Mode320 => 1,
        };

        let mut row_ids = vec![0_u32; out_width];
        let mut row_colors = vec![0_u8; out_width];
        for dy in 0..out_height {
            let y0 = (dy as f64 - origin_y) / factor_y;
            let y1 = ((dy + 1) as f64 - origin_y) / factor_y;
            let sy_lo = y0.floor().max(0.0) as usize;
            let sy_hi = (y1.ceil().max(0.0) as usize).min(self.height);
            row_ids.fill(0);
            row_colors.fill(0);
            for dx in 0..out_width {
                let x0 = (dx as f64 - origin_x) / factor_x;
                let x1 = ((dx + 1) as f64 - origin_x) / factor_x;
                let footprint = (x1 - x0) * (y1 - y0);
                if footprint <= 0.0 {
                    continue;
                }

                let sx_lo = x0.floor().max(0.0) as usize;
                let sx_hi = (x1.ceil().max(0.0) as usize).min(self.width);
                let mut parts: Vec<ComponentCoverage> = Vec::new();
                for sy in sy_lo..sy_hi {
                    let wy = (y1.min((sy + 1) as f64) - y0.max(sy as f64)).max(0.0);
                    if wy <= 0.0 {
                        continue;
                    }
                    for sx in sx_lo..sx_hi {
                        let wx = (x1.min((sx + 1) as f64) - x0.max(sx as f64)).max(0.0);
                        if wx <= 0.0 {
                            continue;
                        }
                        let i = sy * self.width + sx;
                        let idx = usize::from(src_index[i]);
                        if idx == 0 || idx > color_slots {
                            continue;
                        }
                        let weight = wx * wy * f64::from(self.pixels[i].a);
                        let comp = components[i];
                        if let Some(part) = parts.iter_mut().find(|part| part.id == comp) {
                            part.colors[idx] += weight;
                        } else {
                            let mut colors = [0.0; 4];
                            colors[idx] += weight;
                            parts.push(ComponentCoverage { id: comp, colors });
                        }
                    }
                }

                let Some(best) = parts.iter().max_by(|a, b| {
                    a.ink()
                        .partial_cmp(&b.ink())
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) else {
                    continue;
                };
                let coverage = best.ink() / footprint;
                if coverage.round() < f64::from(coverage_threshold) {
                    continue;
                }
                if let Some(color) = best.color() {
                    row_ids[dx] = best.id;
                    row_colors[dx] = color;
                }
            }
            open_component_gaps(&mut row_ids, &mut row_colors);
            let dst = dy * out_width;
            out.pixels[dst..dst + out_width].copy_from_slice(&row_colors);
        }

        out
    }

    /// Palette index per source pixel, using the same matte rules as [`Self::to_indexed`].
    /// Index 0 is transparent. Opaque pixels map to 1..=3 in 160A and to 1 in 320 when the
    /// source has any transparent pixel.
    fn classify_for_scale(
        &self,
        palette: &ProjectPalette,
        subpalette_slot: usize,
        mode: MariaMode,
    ) -> Vec<u8> {
        const MATTE: u8 = 128;
        let subpal = palette.get_subpalette(subpalette_slot);
        let has_alpha = self.pixels.iter().any(|p| p.a < MATTE);
        let mut src_index = Vec::with_capacity(self.pixels.len());

        match mode {
            MariaMode::Mode160A => {
                let visible = [subpal[1], subpal[2], subpal[3]];
                let all = [subpal[0], subpal[1], subpal[2], subpal[3]];
                for p in &self.pixels {
                    if p.a < MATTE {
                        src_index.push(0);
                    } else if has_alpha {
                        let best = p.to_rgb().nearest_index(&visible);
                        src_index.push((best as u8) + 1);
                    } else {
                        let best = p.to_rgb().nearest_index(&all);
                        src_index.push(best as u8);
                    }
                }
            }
            MariaMode::Mode320 => {
                let pair = [subpal[0], subpal[1]];
                for p in &self.pixels {
                    if p.a < MATTE {
                        src_index.push(0);
                    } else if has_alpha {
                        src_index.push(1);
                    } else {
                        let best = p.to_rgb().nearest_index(&pair);
                        src_index.push(best as u8);
                    }
                }
            }
        }

        src_index
    }

    /// Decodes an image from any reader implementing [`BufRead`] + [`std::io::Seek`] into an `RgbaGraphic`.
    ///
    /// # Errors
    /// Returns [`GfxError`] if reading or decoding fails.
    pub fn from_png_reader<R: BufRead + std::io::Seek>(reader: R) -> Result<Self, GfxError> {
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

        let mut pixels = Vec::with_capacity(width * height);

        match info.color_type {
            png::ColorType::Rgba => match info.bit_depth {
                png::BitDepth::Eight => {
                    for chunk in raw_buf.chunks_exact(4) {
                        pixels.push(ColorRgba::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                    }
                }
                _ => {
                    return Err(GfxError::UnsupportedColorType(
                        "Only 8-bit per channel RGBA is supported".into(),
                    ));
                }
            },
            png::ColorType::Rgb => match info.bit_depth {
                png::BitDepth::Eight => {
                    for chunk in raw_buf.chunks_exact(3) {
                        pixels.push(ColorRgba::new(chunk[0], chunk[1], chunk[2], 255));
                    }
                }
                _ => {
                    return Err(GfxError::UnsupportedColorType(
                        "Only 8-bit per channel RGB is supported".into(),
                    ));
                }
            },
            png::ColorType::Grayscale => {
                for &byte in &raw_buf {
                    pixels.push(ColorRgba::new(byte, byte, byte, 255));
                }
            }
            png::ColorType::GrayscaleAlpha => {
                for chunk in raw_buf.chunks_exact(2) {
                    let g = chunk[0];
                    let a = chunk[1];
                    pixels.push(ColorRgba::new(g, g, g, a));
                }
            }
            png::ColorType::Indexed => {
                let palette_bytes = info.palette.as_deref().ok_or_else(|| {
                    GfxError::UnsupportedColorType("Indexed PNG missing PLTE palette chunk".into())
                })?;
                let trns_bytes = info.trns.as_deref();

                let mut indices = Vec::with_capacity(width * height);
                match info.bit_depth {
                    png::BitDepth::One => {
                        for byte in &raw_buf {
                            for bit in (0..8).rev() {
                                if indices.len() < width * height {
                                    indices.push((byte >> bit) & 0x01);
                                }
                            }
                        }
                    }
                    png::BitDepth::Two => {
                        for byte in &raw_buf {
                            for shift in [6, 4, 2, 0] {
                                if indices.len() < width * height {
                                    indices.push((byte >> shift) & 0x03);
                                }
                            }
                        }
                    }
                    png::BitDepth::Four => {
                        for byte in &raw_buf {
                            if indices.len() < width * height {
                                indices.push((byte >> 4) & 0x0F);
                            }
                            if indices.len() < width * height {
                                indices.push(byte & 0x0F);
                            }
                        }
                    }
                    png::BitDepth::Eight => {
                        indices = raw_buf;
                    }
                    png::BitDepth::Sixteen => {
                        return Err(GfxError::UnsupportedColorType(
                            "16-bit depth indexed PNG not supported".into(),
                        ));
                    }
                }

                for idx in indices {
                    let entry_idx = usize::from(idx);
                    let pal_offset = entry_idx * 3;
                    if pal_offset + 2 < palette_bytes.len() {
                        let red = palette_bytes[pal_offset];
                        let green = palette_bytes[pal_offset + 1];
                        let blue = palette_bytes[pal_offset + 2];
                        let alpha = trns_bytes
                            .and_then(|t| t.get(entry_idx).copied())
                            .unwrap_or(255);
                        pixels.push(ColorRgba::new(red, green, blue, alpha));
                    } else {
                        pixels.push(ColorRgba::new(0, 0, 0, 0));
                    }
                }
            }
        }

        if pixels.len() != width * height {
            pixels.resize(width * height, ColorRgba::new(0, 0, 0, 0));
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Quantizes this image into an [`IndexedGraphic`] using the provided palette and subpalette slot.
    ///
    /// For 160A mode:
    /// - Pixels with `a < 128` map to index 0 (transparent).
    /// - If the image has any transparent pixels, opaque pixels are matched against visible
    ///   colors `[c1, c2, c3]` to prevent opaque areas from becoming holes.
    /// - If the image is fully opaque, pixels are matched against all 4 colors `[c0, c1, c2, c3]`,
    ///   allowing solid background colors matching `c0` to become index 0.
    #[must_use]
    pub fn to_indexed(
        &self,
        palette: &ProjectPalette,
        subpalette_slot: usize,
        mode: MariaMode,
    ) -> IndexedGraphic {
        self.to_indexed_with_threshold(palette, subpalette_slot, mode, 128)
    }

    /// Same as [`Self::to_indexed`], but pixels with `a < alpha_threshold` become transparent
    /// (index 0). Lower thresholds keep thin or partially covered strokes (for example in
    /// area-scaled frames); higher thresholds drop them.
    #[must_use]
    pub fn to_indexed_with_threshold(
        &self,
        palette: &ProjectPalette,
        subpalette_slot: usize,
        mode: MariaMode,
        alpha_threshold: u8,
    ) -> IndexedGraphic {
        let subpal = palette.get_subpalette(subpalette_slot);
        let has_alpha = self.pixels.iter().any(|p| p.a < alpha_threshold);

        let mut indexed_px = Vec::with_capacity(self.width * self.height);

        match mode {
            MariaMode::Mode160A => {
                let visible = [subpal[1], subpal[2], subpal[3]];
                let all = [subpal[0], subpal[1], subpal[2], subpal[3]];

                for p in &self.pixels {
                    if p.a < alpha_threshold {
                        indexed_px.push(0);
                    } else if has_alpha {
                        let best = p.to_rgb().nearest_index(&visible);
                        indexed_px.push((best as u8) + 1);
                    } else {
                        let best = p.to_rgb().nearest_index(&all);
                        indexed_px.push(best as u8);
                    }
                }
            }
            MariaMode::Mode320 => {
                let pair = [subpal[0], subpal[1]];

                for p in &self.pixels {
                    if p.a < alpha_threshold {
                        indexed_px.push(0);
                    } else if has_alpha {
                        indexed_px.push(1);
                    } else {
                        let best = p.to_rgb().nearest_index(&pair);
                        indexed_px.push(best as u8);
                    }
                }
            }
        }

        IndexedGraphic {
            width: self.width,
            height: self.height,
            pixels: indexed_px,
        }
    }
}

/// Canonical standard Atari 7800 NTSC palette table (256 entries mapping $00..$FF to RGB).
pub const ATARI_7800_NTSC_PALETTE: [ColorRgb; 256] = [
    ColorRgb::new(0, 0, 0),       // $00
    ColorRgb::new(0, 0, 0),       // $01
    ColorRgb::new(36, 36, 36),    // $02
    ColorRgb::new(36, 36, 36),    // $03
    ColorRgb::new(71, 71, 71),    // $04
    ColorRgb::new(71, 71, 71),    // $05
    ColorRgb::new(107, 107, 107), // $06
    ColorRgb::new(107, 107, 107), // $07
    ColorRgb::new(145, 145, 145), // $08
    ColorRgb::new(145, 145, 145), // $09
    ColorRgb::new(181, 181, 181), // $0A
    ColorRgb::new(181, 181, 181), // $0B
    ColorRgb::new(217, 217, 217), // $0C
    ColorRgb::new(217, 217, 217), // $0D
    ColorRgb::new(255, 255, 255), // $0E
    ColorRgb::new(255, 255, 255), // $0F
    ColorRgb::new(91, 0, 0),      // $10
    ColorRgb::new(91, 0, 0),      // $11
    ColorRgb::new(126, 0, 0),     // $12
    ColorRgb::new(126, 0, 0),     // $13
    ColorRgb::new(162, 32, 34),   // $14
    ColorRgb::new(162, 32, 34),   // $15
    ColorRgb::new(198, 68, 70),   // $16
    ColorRgb::new(198, 68, 70),   // $17
    ColorRgb::new(236, 106, 108), // $18
    ColorRgb::new(236, 106, 108), // $19
    ColorRgb::new(255, 142, 144), // $1A
    ColorRgb::new(255, 142, 144), // $1B
    ColorRgb::new(255, 178, 179), // $1C
    ColorRgb::new(255, 178, 179), // $1D
    ColorRgb::new(255, 216, 218), // $1E
    ColorRgb::new(255, 216, 218), // $1F
    ColorRgb::new(104, 0, 0),     // $20
    ColorRgb::new(104, 0, 0),     // $21
    ColorRgb::new(140, 0, 0),     // $22
    ColorRgb::new(140, 0, 0),     // $23
    ColorRgb::new(176, 22, 12),   // $24
    ColorRgb::new(176, 22, 12),   // $25
    ColorRgb::new(211, 57, 48),   // $26
    ColorRgb::new(211, 57, 48),   // $27
    ColorRgb::new(250, 96, 86),   // $28
    ColorRgb::new(250, 96, 86),   // $29
    ColorRgb::new(255, 131, 122), // $2A
    ColorRgb::new(255, 131, 122), // $2B
    ColorRgb::new(255, 167, 157), // $2C
    ColorRgb::new(255, 167, 157), // $2D
    ColorRgb::new(255, 205, 196), // $2E
    ColorRgb::new(255, 205, 196), // $2F
    ColorRgb::new(111, 0, 0),     // $30
    ColorRgb::new(111, 0, 0),     // $31
    ColorRgb::new(146, 0, 0),     // $32
    ColorRgb::new(146, 0, 0),     // $33
    ColorRgb::new(182, 15, 0),    // $34
    ColorRgb::new(182, 15, 0),    // $35
    ColorRgb::new(218, 51, 28),   // $36
    ColorRgb::new(218, 51, 28),   // $37
    ColorRgb::new(255, 89, 66),   // $38
    ColorRgb::new(255, 89, 66),   // $39
    ColorRgb::new(255, 125, 102), // $3A
    ColorRgb::new(255, 125, 102), // $3B
    ColorRgb::new(255, 161, 137), // $3C
    ColorRgb::new(255, 161, 137), // $3D
    ColorRgb::new(255, 199, 176), // $3E
    ColorRgb::new(255, 199, 176), // $3F
    ColorRgb::new(110, 0, 0),     // $40
    ColorRgb::new(110, 0, 0),     // $41
    ColorRgb::new(146, 0, 0),     // $42
    ColorRgb::new(146, 0, 0),     // $43
    ColorRgb::new(182, 13, 0),    // $44
    ColorRgb::new(182, 13, 0),    // $45
    ColorRgb::new(217, 49, 12),   // $46
    ColorRgb::new(217, 49, 12),   // $47
    ColorRgb::new(255, 87, 50),   // $48
    ColorRgb::new(255, 87, 50),   // $49
    ColorRgb::new(255, 123, 86),  // $4A
    ColorRgb::new(255, 123, 86),  // $4B
    ColorRgb::new(255, 159, 122), // $4C
    ColorRgb::new(255, 159, 122), // $4D
    ColorRgb::new(255, 197, 160), // $4E
    ColorRgb::new(255, 197, 160), // $4F
    ColorRgb::new(102, 0, 0),     // $50
    ColorRgb::new(102, 0, 0),     // $51
    ColorRgb::new(138, 0, 8),     // $52
    ColorRgb::new(138, 0, 8),     // $53
    ColorRgb::new(174, 18, 44),   // $54
    ColorRgb::new(174, 18, 44),   // $55
    ColorRgb::new(209, 54, 80),   // $56
    ColorRgb::new(209, 54, 80),   // $57
    ColorRgb::new(247, 92, 118),  // $58
    ColorRgb::new(247, 92, 118),  // $59
    ColorRgb::new(255, 128, 154), // $5A
    ColorRgb::new(255, 128, 154), // $5B
    ColorRgb::new(255, 164, 190), // $5C
    ColorRgb::new(255, 164, 190), // $5D
    ColorRgb::new(255, 202, 228), // $5E
    ColorRgb::new(255, 202, 228), // $5F
    ColorRgb::new(88, 0, 19),     // $60
    ColorRgb::new(88, 0, 19),     // $61
    ColorRgb::new(124, 0, 55),    // $62
    ColorRgb::new(124, 0, 55),    // $63
    ColorRgb::new(159, 29, 91),   // $64
    ColorRgb::new(159, 29, 91),   // $65
    ColorRgb::new(195, 65, 127),  // $66
    ColorRgb::new(195, 65, 127),  // $67
    ColorRgb::new(233, 103, 165), // $68
    ColorRgb::new(233, 103, 165), // $69
    ColorRgb::new(255, 139, 201), // $6A
    ColorRgb::new(255, 139, 201), // $6B
    ColorRgb::new(255, 175, 237), // $6C
    ColorRgb::new(255, 175, 237), // $6D
    ColorRgb::new(255, 213, 255), // $6E
    ColorRgb::new(255, 213, 255), // $6F
    ColorRgb::new(69, 0, 67),     // $70
    ColorRgb::new(69, 0, 67),     // $71
    ColorRgb::new(104, 0, 103),   // $72
    ColorRgb::new(104, 0, 103),   // $73
    ColorRgb::new(140, 44, 139),  // $74
    ColorRgb::new(140, 44, 139),  // $75
    ColorRgb::new(176, 79, 175),  // $76
    ColorRgb::new(176, 79, 175),  // $77
    ColorRgb::new(214, 117, 213), // $78
    ColorRgb::new(214, 117, 213), // $79
    ColorRgb::new(250, 153, 249), // $7A
    ColorRgb::new(250, 153, 249), // $7B
    ColorRgb::new(255, 189, 255), // $7C
    ColorRgb::new(255, 189, 255), // $7D
    ColorRgb::new(255, 227, 255), // $7E
    ColorRgb::new(255, 227, 255), // $7F
    ColorRgb::new(47, 0, 114),    // $80
    ColorRgb::new(47, 0, 114),    // $81
    ColorRgb::new(82, 0, 149),    // $82
    ColorRgb::new(82, 0, 149),    // $83
    ColorRgb::new(118, 59, 185),  // $84
    ColorRgb::new(118, 59, 185),  // $85
    ColorRgb::new(154, 95, 221),  // $86
    ColorRgb::new(154, 95, 221),  // $87
    ColorRgb::new(192, 133, 255), // $88
    ColorRgb::new(192, 133, 255), // $89
    ColorRgb::new(228, 169, 255), // $8A
    ColorRgb::new(228, 169, 255), // $8B
    ColorRgb::new(255, 205, 255), // $8C
    ColorRgb::new(255, 205, 255), // $8D
    ColorRgb::new(255, 243, 255), // $8E
    ColorRgb::new(255, 243, 255), // $8F
    ColorRgb::new(23, 10, 153),   // $90
    ColorRgb::new(23, 10, 153),   // $91
    ColorRgb::new(59, 45, 189),   // $92
    ColorRgb::new(59, 45, 189),   // $93
    ColorRgb::new(95, 81, 225),   // $94
    ColorRgb::new(95, 81, 225),   // $95
    ColorRgb::new(130, 117, 255), // $96
    ColorRgb::new(130, 117, 255), // $97
    ColorRgb::new(169, 155, 255), // $98
    ColorRgb::new(169, 155, 255), // $99
    ColorRgb::new(205, 191, 255), // $9A
    ColorRgb::new(205, 191, 255), // $9B
    ColorRgb::new(241, 227, 255), // $9C
    ColorRgb::new(241, 227, 255), // $9D
    ColorRgb::new(255, 255, 255), // $9E
    ColorRgb::new(255, 255, 255), // $9F
    ColorRgb::new(0, 31, 180),    // $A0
    ColorRgb::new(0, 31, 180),    // $A1
    ColorRgb::new(36, 67, 216),   // $A2
    ColorRgb::new(36, 67, 216),   // $A3
    ColorRgb::new(71, 103, 252),  // $A4
    ColorRgb::new(71, 103, 252),  // $A5
    ColorRgb::new(107, 138, 255), // $A6
    ColorRgb::new(107, 138, 255), // $A7
    ColorRgb::new(145, 176, 255), // $A8
    ColorRgb::new(145, 176, 255), // $A9
    ColorRgb::new(181, 212, 255), // $AA
    ColorRgb::new(181, 212, 255), // $AB
    ColorRgb::new(217, 248, 255), // $AC
    ColorRgb::new(217, 248, 255), // $AD
    ColorRgb::new(255, 255, 255), // $AE
    ColorRgb::new(255, 255, 255), // $AF
    ColorRgb::new(0, 53, 191),    // $B0
    ColorRgb::new(0, 53, 191),    // $B1
    ColorRgb::new(13, 89, 226),   // $B2
    ColorRgb::new(13, 89, 226),   // $B3
    ColorRgb::new(49, 124, 255),  // $B4
    ColorRgb::new(49, 124, 255),  // $B5
    ColorRgb::new(85, 160, 255),  // $B6
    ColorRgb::new(85, 160, 255),  // $B7
    ColorRgb::new(123, 198, 255), // $B8
    ColorRgb::new(123, 198, 255), // $B9
    ColorRgb::new(159, 234, 255), // $BA
    ColorRgb::new(159, 234, 255), // $BB
    ColorRgb::new(195, 255, 255), // $BC
    ColorRgb::new(195, 255, 255), // $BD
    ColorRgb::new(233, 255, 255), // $BE
    ColorRgb::new(233, 255, 255), // $BF
    ColorRgb::new(0, 71, 183),    // $C0
    ColorRgb::new(0, 71, 183),    // $C1
    ColorRgb::new(0, 107, 218),   // $C2
    ColorRgb::new(0, 107, 218),   // $C3
    ColorRgb::new(31, 142, 254),  // $C4
    ColorRgb::new(31, 142, 254),  // $C5
    ColorRgb::new(66, 178, 255),  // $C6
    ColorRgb::new(66, 178, 255),  // $C7
    ColorRgb::new(104, 216, 255), // $C8
    ColorRgb::new(104, 216, 255), // $C9
    ColorRgb::new(140, 252, 255), // $CA
    ColorRgb::new(140, 252, 255), // $CB
    ColorRgb::new(176, 255, 255), // $CC
    ColorRgb::new(176, 255, 255), // $CD
    ColorRgb::new(214, 255, 255), // $CE
    ColorRgb::new(214, 255, 255), // $CF
    ColorRgb::new(0, 81, 156),    // $D0
    ColorRgb::new(0, 81, 156),    // $D1
    ColorRgb::new(0, 117, 192),   // $D2
    ColorRgb::new(0, 117, 192),   // $D3
    ColorRgb::new(18, 152, 227),  // $D4
    ColorRgb::new(18, 152, 227),  // $D5
    ColorRgb::new(54, 188, 255),  // $D6
    ColorRgb::new(54, 188, 255),  // $D7
    ColorRgb::new(92, 226, 255),  // $D8
    ColorRgb::new(92, 226, 255),  // $D9
    ColorRgb::new(128, 255, 255), // $DA
    ColorRgb::new(128, 255, 255), // $DB
    ColorRgb::new(164, 255, 255), // $DC
    ColorRgb::new(164, 255, 255), // $DD
    ColorRgb::new(202, 255, 255), // $DE
    ColorRgb::new(202, 255, 255), // $DF
    ColorRgb::new(0, 83, 111),    // $E0
    ColorRgb::new(0, 83, 111),    // $E1
    ColorRgb::new(0, 119, 147),   // $E2
    ColorRgb::new(0, 119, 147),   // $E3
    ColorRgb::new(16, 154, 183),  // $E4
    ColorRgb::new(16, 154, 183),  // $E5
    ColorRgb::new(52, 190, 218),  // $E6
    ColorRgb::new(52, 190, 218),  // $E7
    ColorRgb::new(90, 228, 255),  // $E8
    ColorRgb::new(90, 228, 255),  // $E9
    ColorRgb::new(126, 255, 255), // $EA
    ColorRgb::new(126, 255, 255), // $EB
    ColorRgb::new(162, 255, 255), // $EC
    ColorRgb::new(162, 255, 255), // $ED
    ColorRgb::new(200, 255, 255), // $EE
    ColorRgb::new(200, 255, 255), // $EF
    ColorRgb::new(0, 75, 52),     // $F0
    ColorRgb::new(0, 75, 52),     // $F1
    ColorRgb::new(0, 111, 88),    // $F2
    ColorRgb::new(0, 111, 88),    // $F3
    ColorRgb::new(32, 147, 123),  // $F4
    ColorRgb::new(32, 147, 123),  // $F5
    ColorRgb::new(68, 182, 159),  // $F6
    ColorRgb::new(68, 182, 159),  // $F7
    ColorRgb::new(106, 220, 197), // $F8
    ColorRgb::new(106, 220, 197), // $F9
    ColorRgb::new(142, 255, 233), // $FA
    ColorRgb::new(142, 255, 233), // $FB
    ColorRgb::new(178, 255, 255), // $FC
    ColorRgb::new(178, 255, 255), // $FD
    ColorRgb::new(216, 255, 255), // $FE
    ColorRgb::new(216, 255, 255), // $FF
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_distance() {
        let black = ColorRgb::new(0, 0, 0);
        let dark_gray = ColorRgb::new(20, 20, 20);
        let white = ColorRgb::new(255, 255, 255);

        assert!(black.distance_squared(&dark_gray) < black.distance_squared(&white));
    }

    #[test]
    fn test_nearest_index() {
        let palette = [
            ColorRgb::new(0, 0, 0),       // 0: black
            ColorRgb::new(255, 0, 0),     // 1: red
            ColorRgb::new(0, 255, 0),     // 2: green
            ColorRgb::new(255, 255, 255), // 3: white
        ];

        let target = ColorRgb::new(240, 10, 10);
        assert_eq!(target.nearest_index(&palette), 1);

        let target2 = ColorRgb::new(10, 10, 10);
        assert_eq!(target2.nearest_index(&palette), 0);
    }

    #[test]
    fn test_parse_gpl() {
        let sample = "GIMP Palette\n\
                      Name: Test Pal\n\
                      Columns: 4\n\
                      # Comment line\n\
                        0   0   0 Transparent\n\
                      218  51  28 Red\n\
                      255 125 102 Orange\n\
                      255 255 255 White\n";

        let reader = std::io::Cursor::new(sample.as_bytes());
        let decoder = GplDecoder;
        let pal = decoder.decode(reader).unwrap();

        assert_eq!(pal.name, "Test Pal");
        assert_eq!(pal.entries.len(), 4);
        assert_eq!(pal.entries[0].rgb, ColorRgb::new(0, 0, 0));
        assert_eq!(pal.entries[1].rgb, ColorRgb::new(218, 51, 28));
        assert_eq!(pal.entries[2].rgb, ColorRgb::new(255, 125, 102));
        assert_eq!(pal.entries[3].rgb, ColorRgb::new(255, 255, 255));
    }

    #[test]
    fn test_subpalette_extraction_4color() {
        let entries = vec![
            ColorEntry {
                rgb: ColorRgb::new(0, 0, 0),
                name: "Black".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(255, 0, 0),
                name: "Red".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(0, 255, 0),
                name: "Green".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(0, 0, 255),
                name: "Blue".into(),
            },
        ];
        let pal = ProjectPalette::new("Test", entries);
        let sub = pal.get_subpalette(0);
        assert_eq!(sub[0], ColorRgb::new(0, 0, 0));
        assert_eq!(sub[1], ColorRgb::new(255, 0, 0));
        assert_eq!(sub[2], ColorRgb::new(0, 255, 0));
        assert_eq!(sub[3], ColorRgb::new(0, 0, 255));
    }

    #[test]
    fn test_maria_color_hardware_lookup() {
        let black = ColorRgb::new(0, 0, 0);
        let mc = MariaColor::from_rgb(&black);
        assert_eq!(mc.0, 0x00);

        let white = ColorRgb::new(255, 255, 255);
        let mc_w = MariaColor::from_rgb(&white);
        assert!(mc_w.luminance() >= 14);
    }

    #[test]
    fn test_rgba_to_indexed_quantization() {
        let entries = vec![
            ColorEntry {
                rgb: ColorRgb::new(0, 0, 0),
                name: "Black".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(255, 0, 0),
                name: "Red".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(0, 255, 0),
                name: "Green".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(255, 255, 255),
                name: "White".into(),
            },
        ];
        let pal = ProjectPalette::new("Test", entries);

        let mut rgba = RgbaGraphic::new(4, 1);
        // Pixel 0: transparent
        rgba.pixels[0] = ColorRgba::new(0, 0, 0, 0);
        // Pixel 1: close to red
        rgba.pixels[1] = ColorRgba::new(240, 10, 10, 255);
        // Pixel 2: close to green
        rgba.pixels[2] = ColorRgba::new(10, 240, 10, 255);
        // Pixel 3: close to white
        rgba.pixels[3] = ColorRgba::new(250, 250, 250, 255);

        let indexed = rgba.to_indexed(&pal, 0, MariaMode::Mode160A);
        assert_eq!(indexed.pixels[0], 0); // transparent
        assert_eq!(indexed.pixels[1], 1); // red
        assert_eq!(indexed.pixels[2], 2); // green
        assert_eq!(indexed.pixels[3], 3); // white
    }

    #[test]
    fn test_palette_format_detection() {
        assert_eq!(
            PaletteFormat::detect("sprite.gpl").unwrap(),
            PaletteFormat::Gpl
        );
        assert_eq!(
            PaletteFormat::detect("SPRITE.GPL").unwrap(),
            PaletteFormat::Gpl
        );
        assert!(PaletteFormat::detect("sprite.unknown").is_err());
    }

    #[test]
    fn test_format_subpalette_equates() {
        let entries = vec![
            ColorEntry {
                rgb: ColorRgb::new(0, 0, 0),
                name: "Black".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(255, 0, 0),
                name: "Red".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(0, 255, 0),
                name: "Green".into(),
            },
            ColorEntry {
                rgb: ColorRgb::new(0, 0, 255),
                name: "Blue".into(),
            },
        ];
        let pal = ProjectPalette::new("TestPal", entries);
        let writer = crate::Ca65Writer;
        let equates = pal.format_subpalette_equates(&writer, "hero", 2);

        assert!(equates.contains("hero_pal2_c1 ="));
        assert!(equates.contains("hero_pal2_c2 ="));
        assert!(equates.contains("hero_pal2_c3 ="));
    }

    fn rgba_solid(width: usize, height: usize, px: ColorRgba) -> RgbaGraphic {
        let mut g = RgbaGraphic::new(width, height);
        g.pixels.fill(px);
        g
    }

    #[test]
    fn test_scale_area_identity() {
        let mut src = RgbaGraphic::new(4, 4);
        src.pixels[5] = ColorRgba::new(255, 0, 0, 255);
        src.pixels[10] = ColorRgba::new(0, 255, 0, 128);
        let out = src.scale_area(4, 4, 1.0, 1.0);
        assert_eq!(out, src);
    }

    #[test]
    fn test_scale_area_partial_coverage_gives_partial_alpha() {
        // One opaque red pixel beside one transparent pixel, squeezed into one pixel.
        let mut src = RgbaGraphic::new(2, 1);
        src.pixels[0] = ColorRgba::new(255, 0, 0, 255);
        let out = src.scale_area(1, 1, 0.5, 1.0);
        let p = out.pixels[0];
        assert_eq!(
            (p.r, p.g, p.b),
            (255, 0, 0),
            "color must not be dimmed by transparent area"
        );
        assert!((127..=128).contains(&p.a), "alpha was {}", p.a);
    }

    #[test]
    fn test_scale_area_shrinks_around_center() {
        let src = rgba_solid(4, 4, ColorRgba::new(255, 0, 0, 255));
        let out = src.scale_area(4, 4, 0.5, 0.5);
        for y in 0..4 {
            for x in 0..4 {
                let inside = (1..3).contains(&x) && (1..3).contains(&y);
                let a = out.get_pixel(x, y).a;
                assert_eq!(a, if inside { 255 } else { 0 }, "pixel ({x},{y})");
            }
        }
    }

    #[test]
    fn test_scale_area_to_smaller_canvas() {
        // A 128-wide source baked onto a 64-wide canvas at full size (factor 0.5).
        let src = rgba_solid(128, 128, ColorRgba::new(255, 0, 0, 255));
        let out = src.scale_area(64, 64, 0.5, 0.5);
        assert!(out.pixels.iter().all(|p| p.a == 255));
    }

    #[test]
    fn test_rgba_apply_par_correction() {
        let src = rgba_solid(16, 8, ColorRgba::new(255, 0, 0, 255));
        let corrected = src.apply_par_correction(MariaMode::Mode160A);
        assert_eq!((corrected.width, corrected.height), (8, 8));
        assert!(corrected.pixels.iter().all(|p| p.a == 255));

        let unchanged = src.apply_par_correction(MariaMode::Mode320);
        assert_eq!((unchanged.width, unchanged.height), (16, 8));
    }

    #[test]
    fn test_to_indexed_alpha_threshold() {
        let pal = ProjectPalette::new(
            "T",
            vec![
                ColorEntry {
                    rgb: ColorRgb::new(0, 0, 0),
                    name: "bg".into(),
                },
                ColorEntry {
                    rgb: ColorRgb::new(255, 0, 0),
                    name: "red".into(),
                },
            ],
        );
        let mut src = RgbaGraphic::new(2, 1);
        src.pixels[0] = ColorRgba::new(255, 0, 0, 100);
        src.pixels[1] = ColorRgba::new(255, 0, 0, 255);

        let strict = src.to_indexed_with_threshold(&pal, 0, MariaMode::Mode160A, 128);
        assert_eq!(strict.pixels, vec![0, 1]);
        let lenient = src.to_indexed_with_threshold(&pal, 0, MariaMode::Mode160A, 64);
        assert_eq!(lenient.pixels, vec![1, 1]);
        assert_eq!(src.to_indexed(&pal, 0, MariaMode::Mode160A), strict);
    }

    #[test]
    fn test_scale_area_keeps_a_symmetric_source_symmetric() {
        // Diamond centered on an even canvas. Factor 0.7 does not land on a whole-pixel origin,
        // which is the case that shifts if the origin is rounded.
        let mut src = RgbaGraphic::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let dx = (x as i32 * 2 - 15).unsigned_abs();
                let dy = (y as i32 * 2 - 15).unsigned_abs();
                if dx + dy <= 14 {
                    src.pixels[y * 16 + x] = ColorRgba::new(255, 0, 0, 255);
                }
            }
        }
        let out = src.scale_area(16, 16, 0.7, 0.7);
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(
                    out.get_pixel(x, y),
                    out.get_pixel(15 - x, y),
                    "horizontal mirror ({x},{y})"
                );
                assert_eq!(
                    out.get_pixel(x, y),
                    out.get_pixel(x, 15 - y),
                    "vertical mirror ({x},{y})"
                );
            }
        }
    }

    fn demo_palette() -> ProjectPalette {
        ProjectPalette::new(
            "T",
            vec![
                ColorEntry {
                    rgb: ColorRgb::new(0, 0, 0),
                    name: "bg".into(),
                },
                ColorEntry {
                    rgb: ColorRgb::new(128, 0, 128),
                    name: "purple".into(),
                },
                ColorEntry {
                    rgb: ColorRgb::new(255, 0, 0),
                    name: "red".into(),
                },
                ColorEntry {
                    rgb: ColorRgb::new(0, 0, 255),
                    name: "blue".into(),
                },
            ],
        )
    }

    #[test]
    fn test_scale_area_indexed_picks_majority_not_blend() {
        let pal = demo_palette();
        let mut src = RgbaGraphic::new(4, 1);
        let red = ColorRgba::new(255, 0, 0, 255);
        src.pixels[..3].fill(red);
        src.pixels[3] = ColorRgba::new(0, 0, 255, 255);
        let out = src.scale_area_indexed(1, 1, 0.25, 1.0, &pal, 0, MariaMode::Mode160A, 128);
        assert_eq!(out.pixels, vec![2], "three red pixels must beat one blue");

        // Equal red and blue would average to purple. Majority must not invent that color.
        let mut split = RgbaGraphic::new(2, 1);
        split.pixels[0] = red;
        split.pixels[1] = ColorRgba::new(0, 0, 255, 255);
        let tied = split.scale_area_indexed(1, 1, 0.5, 1.0, &pal, 0, MariaMode::Mode160A, 128);
        assert_eq!(tied.pixels, vec![2]);
    }

    #[test]
    fn test_scale_area_indexed_keeps_half_covered_column() {
        let pal = demo_palette();
        let mut src = RgbaGraphic::new(8, 8);
        for y in 0..8 {
            src.pixels[y * 8 + 2] = ColorRgba::new(255, 0, 0, 255);
        }
        let out = src.scale_area_indexed(8, 8, 0.5, 0.5, &pal, 0, MariaMode::Mode160A, 128);
        assert!(
            out.pixels.contains(&2),
            "a one-pixel stroke at half coverage must survive"
        );
    }

    #[test]
    fn test_scale_area_indexed_keeps_gap_between_separate_bars() {
        let pal = demo_palette();
        let mut src = RgbaGraphic::new(8, 4);
        let red = ColorRgba::new(255, 0, 0, 255);
        for y in 0..4 {
            // Columns 2-3 and 5-6 are two bars. Column 4 is the slot between them.
            src.pixels[y * 8 + 2] = red;
            src.pixels[y * 8 + 3] = red;
            src.pixels[y * 8 + 5] = red;
            src.pixels[y * 8 + 6] = red;
        }
        // Half scale maps each bar pair onto its own output pixel, and those pixels touch.
        let out = src.scale_area_indexed(4, 4, 0.5, 1.0, &pal, 0, MariaMode::Mode160A, 128);
        for y in 0..4 {
            let row = &out.pixels[y * 4..(y + 1) * 4];
            let ink: Vec<usize> = row
                .iter()
                .enumerate()
                .filter(|(_, pixel)| **pixel != 0)
                .map(|(x, _)| x)
                .collect();
            assert!(
                ink.windows(2).any(|pair| pair[1] > pair[0] + 1),
                "row {y} joined the bars: {row:?}"
            );
        }
    }

    #[test]
    fn test_full_25_color_7800_palette() {
        let mut entries = Vec::with_capacity(25);
        entries.push(ColorEntry {
            rgb: ColorRgb::new(0, 0, 0),
            name: "BG".into(),
        });
        for i in 1..=24 {
            entries.push(ColorEntry {
                rgb: ColorRgb::new(i as u8 * 10, i as u8 * 10, i as u8 * 10),
                name: format!("C{i}"),
            });
        }
        let pal = ProjectPalette::new("Full7800", entries);
        let sub0 = pal.get_subpalette(0);
        assert_eq!(sub0[0], ColorRgb::new(0, 0, 0));
        assert_eq!(sub0[1], ColorRgb::new(10, 10, 10));
        assert_eq!(sub0[2], ColorRgb::new(20, 20, 20));
        assert_eq!(sub0[3], ColorRgb::new(30, 30, 30));

        let sub1 = pal.get_subpalette(1);
        assert_eq!(sub1[0], ColorRgb::new(0, 0, 0));
        assert_eq!(sub1[1], ColorRgb::new(40, 40, 40));
        assert_eq!(sub1[2], ColorRgb::new(50, 50, 50));
        assert_eq!(sub1[3], ColorRgb::new(60, 60, 60));
    }
}
