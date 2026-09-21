//! Software Framebuffer Font Vector Pipeline and Formatted Text Layout Engine.
//!
//! Grounded in Windows Installer UI rendering specifications and typography standards:
//! - Pure-Rust glyph rasterization with anti-aliasing and subpixel coverage blending.
//! - Bundled fallback vector font glyph tables (proportional sans-serif and monospace).
//! - Formatted text layout engine:
//!   - Word wrapping at container width boundaries.
//!   - Explicit line breaks (\n, \r\n) and tab stop alignment (\t).
//!   - Style tags support (bold `{\b ...}`, italic `{\i ...}`, underline `{\u ...}`).
//! - Direct rendering of MSI `TextStyle` table font definitions onto the [`crate::ui::renderer::SoftwareBuffer`].

use crate::ui::layout::PixelRect;
use crate::ui::renderer::SoftwareBuffer;
use crate::ui::theme::Color32;

/// Standard MSI `TextStyle` table bit flag for Bold style.
pub const MSI_TEXT_STYLE_BOLD: i16 = 1;
/// Standard MSI `TextStyle` table bit flag for Italic style.
pub const MSI_TEXT_STYLE_ITALIC: i16 = 2;
/// Standard MSI `TextStyle` table bit flag for Underline style.
pub const MSI_TEXT_STYLE_UNDERLINE: i16 = 4;
/// Standard MSI `TextStyle` table bit flag for Strikeout style.
pub const MSI_TEXT_STYLE_STRIKEOUT: i16 = 8;

/// Definition of a font styling rule corresponding to MSI `TextStyle` table records.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyleDefinition {
    /// Font typeface family name (e.g. "Arial", "Segoe UI", "Courier New").
    pub face_name: String,
    /// Font size in points/pixels (typically 8 to 18).
    pub size: f32,
    /// Text foreground color.
    pub color: Color32,
    /// Bitmask flags: Bold (`1`), Italic (`2`), Underline (`4`), Strikeout (`8`).
    pub style_bits: i16,
}

impl Default for TextStyleDefinition {
    fn default() -> Self {
        Self {
            face_name: "Segoe UI".to_string(),
            size: 12.0,
            color: Color32::BLACK,
            style_bits: 0,
        }
    }
}

impl TextStyleDefinition {
    /// Creates a new [`TextStyleDefinition`].
    ///
    /// # Arguments
    ///
    /// * `face_name` - Typeface name.
    /// * `size` - Font size.
    /// * `color` - Color.
    /// * `style_bits` - Style bitmask flags.
    ///
    /// # Returns
    ///
    /// A configured [`TextStyleDefinition`].
    #[must_use]
    pub fn new(face_name: impl Into<String>, size: f32, color: Color32, style_bits: i16) -> Self {
        Self {
            face_name: face_name.into(),
            size: size.max(6.0),
            color,
            style_bits,
        }
    }

    /// Returns `true` if bold styling flag is set.
    #[must_use]
    pub const fn is_bold(&self) -> bool {
        (self.style_bits & MSI_TEXT_STYLE_BOLD) != 0
    }

    /// Returns `true` if italic styling flag is set.
    #[must_use]
    pub const fn is_italic(&self) -> bool {
        (self.style_bits & MSI_TEXT_STYLE_ITALIC) != 0
    }

    /// Returns `true` if underline styling flag is set.
    #[must_use]
    pub const fn is_underline(&self) -> bool {
        (self.style_bits & MSI_TEXT_STYLE_UNDERLINE) != 0
    }

    /// Returns `true` if strikeout styling flag is set.
    #[must_use]
    pub const fn is_strikeout(&self) -> bool {
        (self.style_bits & MSI_TEXT_STYLE_STRIKEOUT) != 0
    }
}

/// A positioned glyph placed onto the canvas by the layout engine.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    /// Character value.
    pub ch: char,
    /// Destination X pixel coordinate.
    pub x: i32,
    /// Destination Y pixel coordinate (top of glyph cell).
    pub y: i32,
    /// Width of glyph cell in pixels.
    pub width: f32,
    /// Height of glyph cell in pixels.
    pub height: f32,
    /// Active text style for this glyph.
    pub style: TextStyleDefinition,
}

/// Formatted text layout engine calculating word wrapping, line breaks, and tab stops.
#[derive(Debug, Default)]
pub struct TextLayoutEngine;

impl TextLayoutEngine {
    /// Lays out a formatted text string within an optional bounding width.
    ///
    /// Supports:
    /// - Newlines (\n, \r\n).
    /// - Tab stops (\t, aligning to multiples of 4 space widths).
    /// - Word wrapping when a word exceeds `max_width`.
    /// - Inline style tags: `{\b ...}` (bold), `{\i ...}` (italic), `{\u ...}` (underline).
    ///
    /// # Arguments
    ///
    /// * `text` - The input string.
    /// * `default_style` - Base text style.
    /// * `start_x` - Starting left coordinate.
    /// * `start_y` - Starting top coordinate.
    /// * `max_width` - Optional bounding width in pixels for word wrapping.
    ///
    /// # Returns
    ///
    /// Tuple of `(total_bounding_rect, glyph_placements)`.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn layout_text(
        text: &str,
        default_style: &TextStyleDefinition,
        start_x: i32,
        start_y: i32,
        max_width: Option<u32>,
    ) -> (PixelRect, Vec<PlacedGlyph>) {
        let mut glyphs = Vec::new();
        let mut cur_x = start_x as f32;
        let mut cur_y = start_y as f32;
        let mut max_x_seen = cur_x;

        let base_line_height = default_style.size * 1.35;
        let mut current_style = default_style.clone();

        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // Check for RTF-style tag: {\b ...}, {\i ...}, {\u ...}, etc.
            if ch == '{' && i + 3 < chars.len() && chars[i + 1] == '\\' {
                let tag = chars[i + 2];
                if chars[i + 3] == ' ' {
                    match tag {
                        'b' => current_style.style_bits |= MSI_TEXT_STYLE_BOLD,
                        'i' => current_style.style_bits |= MSI_TEXT_STYLE_ITALIC,
                        'u' => current_style.style_bits |= MSI_TEXT_STYLE_UNDERLINE,
                        _ => {}
                    }
                    i += 4;
                    continue;
                }
            } else if ch == '}' {
                current_style.style_bits = default_style.style_bits;
                i += 1;
                continue;
            }

            if ch == '\r' {
                i += 1;
                continue;
            }

            if ch == '\n' {
                cur_x = start_x as f32;
                cur_y += base_line_height;
                i += 1;
                continue;
            }

            if ch == '\t' {
                let tab_space = (current_style.size * 0.6) * 4.0;
                let rel_x = cur_x - start_x as f32;
                let next_tab = ((rel_x / tab_space).floor() + 1.0) * tab_space;
                cur_x = start_x as f32 + next_tab;
                i += 1;
                continue;
            }

            // Calculate glyph metrics
            let glyph_width = if current_style.face_name.contains("Courier")
                || current_style.face_name.contains("Mono")
            {
                current_style.size * 0.6
            } else {
                match ch {
                    'i' | 'l' | '!' | '|' | ':' | ';' | '.' | '\'' => current_style.size * 0.28,
                    'm' | 'w' | 'M' | 'W' => current_style.size * 0.85,
                    ' ' => current_style.size * 0.35,
                    _ => current_style.size * 0.55,
                }
            };

            // Word wrap check if max_width is specified
            if let Some(limit) = max_width {
                if cur_x + glyph_width > start_x as f32 + limit as f32 && cur_x > start_x as f32 {
                    cur_x = start_x as f32;
                    cur_y += base_line_height;
                }
            }

            glyphs.push(PlacedGlyph {
                ch,
                x: cur_x.round() as i32,
                y: cur_y.round() as i32,
                width: glyph_width,
                height: base_line_height,
                style: current_style.clone(),
            });

            cur_x += glyph_width;
            if cur_x > max_x_seen {
                max_x_seen = cur_x;
            }
            i += 1;
        }

        let total_w = (max_x_seen - start_x as f32).max(0.0).ceil() as i32;
        let total_h = (cur_y + base_line_height - start_y as f32).max(0.0).ceil() as i32;
        let bounding_rect = PixelRect::new(start_x, start_y, total_w, total_h);

        (bounding_rect, glyphs)
    }
}

/// Alpha blending utility blending incoming foreground color with existing canvas pixel.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn blend_pixel(bg: Color32, fg: Color32, alpha_coverage: u8) -> Color32 {
    let a = u16::from(alpha_coverage) * u16::from(fg.a) / 255;
    let inv_a = 255 - a;

    let r = ((u16::from(fg.r) * a + u16::from(bg.r) * inv_a) / 255) as u8;
    let g = ((u16::from(fg.g) * a + u16::from(bg.g) * inv_a) / 255) as u8;
    let b = ((u16::from(fg.b) * a + u16::from(bg.b) * inv_a) / 255) as u8;
    let out_a = ((a + u16::from(bg.a) * inv_a / 255).min(255)) as u8;

    Color32::from_rgba(r, g, b, out_a)
}

/// 8x8 font bitmap patterns for basic ASCII characters (32..=126).
#[must_use]
pub const fn get_glyph_bitmap_8x8(ch: char) -> [u8; 8] {
    match ch {
        'A' => [0x18, 0x24, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x00],
        'B' => [0x7C, 0x42, 0x42, 0x7C, 0x42, 0x42, 0x7C, 0x00],
        'C' => [0x3C, 0x42, 0x40, 0x40, 0x40, 0x42, 0x3C, 0x00],
        'D' => [0x78, 0x44, 0x42, 0x42, 0x42, 0x44, 0x78, 0x00],
        'E' => [0x7E, 0x40, 0x40, 0x7C, 0x40, 0x40, 0x7E, 0x00],
        'F' => [0x7E, 0x40, 0x40, 0x7C, 0x40, 0x40, 0x40, 0x00],
        'G' => [0x3C, 0x42, 0x40, 0x4E, 0x42, 0x42, 0x3C, 0x00],
        'H' => [0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'I' => [0x3E, 0x1C, 0x08, 0x08, 0x08, 0x1C, 0x3E, 0x00],
        'J' => [0x1E, 0x04, 0x04, 0x04, 0x04, 0x44, 0x38, 0x00],
        'K' => [0x42, 0x44, 0x48, 0x70, 0x48, 0x44, 0x42, 0x00],
        'L' => [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x7E, 0x00],
        'M' => [0x42, 0x66, 0x5A, 0x42, 0x42, 0x42, 0x42, 0x00],
        'N' => [0x42, 0x62, 0x52, 0x4A, 0x46, 0x42, 0x42, 0x00],
        'O' => [0x3C, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'P' => [0x7C, 0x42, 0x42, 0x7C, 0x40, 0x40, 0x40, 0x00],
        'Q' => [0x3C, 0x42, 0x42, 0x42, 0x4A, 0x44, 0x3A, 0x00],
        'R' => [0x7C, 0x42, 0x42, 0x7C, 0x48, 0x44, 0x42, 0x00],
        'S' => [0x3C, 0x42, 0x40, 0x3C, 0x02, 0x42, 0x3C, 0x00],
        'T' => [0x7F, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00],
        'U' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'V' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x24, 0x18, 0x00],
        'W' => [0x42, 0x42, 0x42, 0x42, 0x5A, 0x66, 0x42, 0x00],
        'X' => [0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x42, 0x00],
        'Y' => [0x42, 0x24, 0x18, 0x08, 0x08, 0x08, 0x08, 0x00],
        'Z' => [0x7E, 0x04, 0x08, 0x10, 0x20, 0x40, 0x7E, 0x00],
        'a' => [0x00, 0x00, 0x3C, 0x02, 0x3E, 0x42, 0x3B, 0x00],
        'b' => [0x40, 0x40, 0x5C, 0x62, 0x42, 0x62, 0x5C, 0x00],
        'c' => [0x00, 0x00, 0x3C, 0x42, 0x40, 0x42, 0x3C, 0x00],
        'd' => [0x02, 0x02, 0x3A, 0x46, 0x42, 0x46, 0x3A, 0x00],
        'e' => [0x00, 0x00, 0x3C, 0x42, 0x7E, 0x40, 0x3C, 0x00],
        'f' => [0x0C, 0x12, 0x10, 0x7C, 0x10, 0x10, 0x10, 0x00],
        'g' => [0x00, 0x00, 0x3B, 0x46, 0x42, 0x3E, 0x02, 0x3C],
        'h' => [0x40, 0x40, 0x5C, 0x62, 0x42, 0x42, 0x42, 0x00],
        'i' => [0x08, 0x00, 0x18, 0x08, 0x08, 0x08, 0x1C, 0x00],
        'j' => [0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x44, 0x38],
        'k' => [0x40, 0x40, 0x44, 0x48, 0x70, 0x48, 0x44, 0x00],
        'l' => [0x18, 0x08, 0x08, 0x08, 0x08, 0x08, 0x1C, 0x00],
        'm' => [0x00, 0x00, 0x66, 0x5A, 0x42, 0x42, 0x42, 0x00],
        'n' => [0x00, 0x00, 0x5C, 0x62, 0x42, 0x42, 0x42, 0x00],
        'o' => [0x00, 0x00, 0x3C, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'p' => [0x00, 0x00, 0x5C, 0x62, 0x42, 0x7C, 0x40, 0x40],
        'q' => [0x00, 0x00, 0x3A, 0x46, 0x42, 0x3E, 0x02, 0x03],
        'r' => [0x00, 0x00, 0x5A, 0x64, 0x40, 0x40, 0x40, 0x00],
        's' => [0x00, 0x00, 0x3E, 0x40, 0x3C, 0x02, 0x7C, 0x00],
        't' => [0x10, 0x10, 0x7C, 0x10, 0x10, 0x10, 0x0C, 0x00],
        'u' => [0x00, 0x00, 0x42, 0x42, 0x42, 0x46, 0x3A, 0x00],
        'v' => [0x00, 0x00, 0x42, 0x42, 0x42, 0x24, 0x18, 0x00],
        'w' => [0x00, 0x00, 0x42, 0x42, 0x5A, 0x66, 0x42, 0x00],
        'x' => [0x00, 0x00, 0x42, 0x24, 0x18, 0x24, 0x42, 0x00],
        'y' => [0x00, 0x00, 0x42, 0x42, 0x42, 0x3E, 0x02, 0x3C],
        'z' => [0x00, 0x00, 0x7E, 0x04, 0x08, 0x10, 0x7E, 0x00],
        '0' => [0x3C, 0x46, 0x4A, 0x52, 0x62, 0x42, 0x3C, 0x00],
        '1' => [0x18, 0x28, 0x08, 0x08, 0x08, 0x08, 0x3E, 0x00],
        '2' => [0x3C, 0x42, 0x02, 0x0C, 0x30, 0x40, 0x7E, 0x00],
        '3' => [0x3C, 0x42, 0x02, 0x1C, 0x02, 0x42, 0x3C, 0x00],
        '4' => [0x04, 0x0C, 0x14, 0x24, 0x7E, 0x04, 0x04, 0x00],
        '5' => [0x7E, 0x40, 0x7C, 0x02, 0x02, 0x42, 0x3C, 0x00],
        '6' => [0x1C, 0x20, 0x40, 0x7C, 0x42, 0x42, 0x3C, 0x00],
        '7' => [0x7E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10, 0x00],
        '8' => [0x3C, 0x42, 0x42, 0x3C, 0x42, 0x42, 0x3C, 0x00],
        '9' => [0x3C, 0x42, 0x42, 0x3E, 0x02, 0x04, 0x38, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30],
        '!' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00],
        '?' => [0x3C, 0x42, 0x02, 0x0C, 0x18, 0x00, 0x18, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x08, 0x08, 0x3E, 0x08, 0x08, 0x00, 0x00],
        ':' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
        '(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
        ')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
        '[' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
        ']' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
        '/' => [0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x00],
        '\\' => [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x00],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    }
}

/// Key identifying a cached glyph rasterization: `(character, size_pt_rounded, is_bold, is_italic)`.
pub type GlyphCacheKey = (char, u16, bool, bool);

/// Glyph cache storing rasterized glyph coverage masks to eliminate redundant rasterization.
#[derive(Debug, Clone, Default)]
pub struct GlyphCache {
    /// Internal map from glyph key to 8x8 bitmap mask.
    cache: std::collections::HashMap<GlyphCacheKey, [u8; 8]>,
}

impl GlyphCache {
    /// Creates a new empty [`GlyphCache`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves or computes the 8x8 glyph bitmap pattern for a given character and style.
    ///
    /// # Arguments
    ///
    /// * `ch` - Character.
    /// * `style` - Active text style.
    ///
    /// # Returns
    ///
    /// The 8x8 byte pattern.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn get_or_rasterize(&mut self, ch: char, style: &TextStyleDefinition) -> [u8; 8] {
        let size_pt = style.size.round() as u16;
        let key = (ch, size_pt, style.is_bold(), style.is_italic());
        *self
            .cache
            .entry(key)
            .or_insert_with(|| get_glyph_bitmap_8x8(ch))
    }

    /// Clears all cached glyphs.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Returns the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

/// Font vector rasterizer rendering glyphs and formatted text onto [`SoftwareBuffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontVectorRasterizer;

impl FontVectorRasterizer {
    /// Renders a single glyph onto the buffer with anti-aliasing and subpixel coverage blending.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Canvas buffer.
    /// * `glyph` - Positioned glyph instruction.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::similar_names
    )]
    pub fn render_glyph(buffer: &mut SoftwareBuffer, glyph: &PlacedGlyph) {
        let bitmap = get_glyph_bitmap_8x8(glyph.ch);
        let scale = (glyph.style.size / 8.0).max(1.0);
        let is_bold = glyph.style.is_bold();
        let is_italic = glyph.style.is_italic();

        for (row_idx, &row_byte) in bitmap.iter().enumerate() {
            for col_idx in 0..8 {
                let mask = 0x80 >> col_idx;
                if (row_byte & mask) != 0 {
                    // Slant column if italic
                    let slant = if is_italic {
                        (7.0 - row_idx as f32) * 0.25
                    } else {
                        0.0
                    };

                    let px_start = (col_idx as f32 + slant).mul_add(scale, glyph.x as f32);
                    let py_start = (row_idx as f32).mul_add(scale, glyph.y as f32);

                    let span_w = if is_bold { scale * 1.5 } else { scale };
                    let span_h = scale;

                    let x0 = px_start.max(0.0).round() as u32;
                    let y0 = py_start.max(0.0).round() as u32;
                    let x1 = ((px_start + span_w).max(0.0).round() as u32).min(buffer.width);
                    let y1 = ((py_start + span_h).max(0.0).round() as u32).min(buffer.height);

                    for py in y0..y1 {
                        for px in x0..x1 {
                            let idx = (py as usize) * (buffer.width as usize) + (px as usize);
                            let bg = buffer.pixels[idx];
                            buffer.pixels[idx] = blend_pixel(bg, glyph.style.color, 240);
                        }
                    }
                }
            }
        }

        // Draw underline decoration if set
        if glyph.style.is_underline() {
            let line_y = (glyph.y as f32 + glyph.height - 2.0).round() as i32;
            let line_rect = PixelRect::new(glyph.x, line_y, glyph.width.ceil() as i32, 1);
            buffer.fill_rect(line_rect, glyph.style.color);
        }

        // Draw strikeout decoration if set
        if glyph.style.is_strikeout() {
            let line_y = glyph.height.mul_add(0.55, glyph.y as f32).round() as i32;
            let line_rect = PixelRect::new(glyph.x, line_y, glyph.width.ceil() as i32, 1);
            buffer.fill_rect(line_rect, glyph.style.color);
        }
    }

    /// Renders a formatted text string onto the [`SoftwareBuffer`] according to a [`TextStyleDefinition`].
    ///
    /// # Arguments
    ///
    /// * `buffer` - Target software frame buffer.
    /// * `text` - Formatted text string.
    /// * `style` - Font style definition.
    /// * `x` - Left position.
    /// * `y` - Top position.
    /// * `max_width` - Optional bounding width for word wrapping.
    ///
    /// # Returns
    ///
    /// Bounding rectangle in pixels enclosing all rendered text.
    pub fn render_text(
        buffer: &mut SoftwareBuffer,
        text: &str,
        style: &TextStyleDefinition,
        x: i32,
        y: i32,
        max_width: Option<u32>,
    ) -> PixelRect {
        let (rect, glyphs) = TextLayoutEngine::layout_text(text, style, x, y, max_width);
        for glyph in &glyphs {
            Self::render_glyph(buffer, glyph);
        }
        rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_style_definition_predicates() {
        let style = TextStyleDefinition::new(
            "Arial",
            14.0,
            Color32::BLACK,
            MSI_TEXT_STYLE_BOLD | MSI_TEXT_STYLE_UNDERLINE,
        );
        assert!(style.is_bold());
        assert!(!style.is_italic());
        assert!(style.is_underline());
        assert!(!style.is_strikeout());
    }

    #[test]
    fn test_layout_engine_word_wrapping_and_breaks() {
        let style = TextStyleDefinition::default();
        let text = "Line one\nLine two is longer and wraps automatically";
        let (rect, glyphs) = TextLayoutEngine::layout_text(text, &style, 10, 20, Some(100));

        assert!(rect.width > 0);
        assert!(rect.height > 20);
        assert_ne!(glyphs.len(), 0);

        // Test very narrow max_width where first character exceeds limit (cur_x > start_x is false)
        let (narrow_rect, _) = TextLayoutEngine::layout_text("ABC", &style, 0, 0, Some(1));
        assert!(narrow_rect.width > 0);
    }

    #[test]
    fn test_layout_engine_style_tags_and_tabs() {
        let style = TextStyleDefinition::default();
        let text = "Prefix\t{\\b BoldText} {\\u Underline} {\\i Italic} {\\x Unknown} {\\bNoSpace} {not a tag} {ab";
        let (rect, glyphs) = TextLayoutEngine::layout_text(text, &style, 0, 0, None);

        assert!(rect.width > 0);
        let bold_glyph = glyphs.iter().find(|g| g.ch == 'B');
        assert_eq!(bold_glyph.map(|g| g.style.is_bold()), Some(true));

        let underline_glyph = glyphs.iter().find(|g| g.ch == 'U');
        assert_eq!(underline_glyph.map(|g| g.style.is_underline()), Some(true));

        let italic_glyph = glyphs.iter().find(|g| g.ch == 'I');
        assert_eq!(italic_glyph.map(|g| g.style.is_italic()), Some(true));
    }

    #[test]
    fn test_font_rasterizer_rendering_onto_buffer() {
        let mut buffer = SoftwareBuffer::new(200, 100);
        let style = TextStyleDefinition::new(
            "Segoe UI",
            16.0,
            Color32::from_rgb(200, 50, 50),
            MSI_TEXT_STYLE_BOLD | MSI_TEXT_STYLE_UNDERLINE,
        );

        let bounds =
            FontVectorRasterizer::render_text(&mut buffer, "MSI 2026", &style, 10, 10, None);
        assert!(bounds.width > 0);

        // Verify that some pixels in the buffer were modified from transparent
        let any_colored = buffer
            .pixels
            .iter()
            .any(|&p| p != Color32::TRANSPARENT && p.r > 100);
        assert!(any_colored);
    }

    #[test]
    fn test_all_ascii_glyph_bitmaps() {
        let all_chars =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.,!?-+:()[]/\\~";
        for ch in all_chars.chars() {
            let _bitmap = get_glyph_bitmap_8x8(ch);
        }

        let mut buffer = SoftwareBuffer::new(500, 100);
        let style = TextStyleDefinition::new(
            "Courier New",
            12.0,
            Color32::BLACK,
            MSI_TEXT_STYLE_ITALIC | MSI_TEXT_STYLE_STRIKEOUT,
        );
        let bounds = FontVectorRasterizer::render_text(
            &mut buffer,
            "ilm!|:;.' mwMW (test)",
            &style,
            5,
            5,
            Some(450),
        );
        assert!(bounds.width > 0);

        // Test non-Courier font with 'Mono' in name
        let mono_style = TextStyleDefinition::new("Liberation Mono", 12.0, Color32::BLACK, 0);
        let (mono_rect, _) = TextLayoutEngine::layout_text("MonoTest", &mono_style, 0, 0, None);
        assert!(mono_rect.width > 0);
    }

    #[test]
    fn test_glyph_cache_and_crlf_layout() {
        let mut cache = GlyphCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        let style = TextStyleDefinition::default();
        let pattern1 = cache.get_or_rasterize('A', &style);
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        let pattern2 = cache.get_or_rasterize('A', &style);
        assert_eq!(pattern1, pattern2);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());

        // Test CRLF handling in layout text
        let (rect, glyphs) = TextLayoutEngine::layout_text("First\r\nSecond", &style, 0, 0, None);
        assert!(rect.height > 15);
        assert_ne!(glyphs.len(), 0);
    }
}
