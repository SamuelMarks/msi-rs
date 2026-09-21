//! MSI Dialog Layout Math & Geometry.
//!
//! Grounded directly in official Windows Installer SDK specifications:
//! - Dialog units (DLUs) to pixel conversion:
//!   - `PixelX = (DialogX * AverageCharWidth) / 4`
//!   - `PixelY = (DialogY * FontHeight) / 8`
//! - Inverse pixel to dialog units conversion:
//!   - `DialogX = (PixelX * 4) / AverageCharWidth`
//!   - `DialogY = (PixelY * 8) / FontHeight`
//! - Center alignment algorithms for `HCentering` and `VCentering` flags.

/// Default font metric: Average character width in pixels (standard 8-point MS Sans Serif / Tahoma).
pub const DEFAULT_AVERAGE_CHAR_WIDTH: u32 = 8;

/// Default font metric: Font height in pixels (standard 8-point MS Sans Serif / Tahoma).
pub const DEFAULT_FONT_HEIGHT: u32 = 16;

/// Font metrics specifying character sizing for dialog unit to pixel conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    /// Average character width in pixels.
    pub average_char_width: u32,
    /// Line or font height in pixels.
    pub font_height: u32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            average_char_width: DEFAULT_AVERAGE_CHAR_WIDTH,
            font_height: DEFAULT_FONT_HEIGHT,
        }
    }
}

impl FontMetrics {
    /// Creates a new [`FontMetrics`] specification.
    ///
    /// # Arguments
    ///
    /// * `average_char_width` - Average character width in pixels (minimum 1).
    /// * `font_height` - Font height in pixels (minimum 1).
    ///
    /// # Returns
    ///
    /// A new [`FontMetrics`].
    #[must_use]
    pub const fn new(average_char_width: u32, font_height: u32) -> Self {
        let w = if average_char_width == 0 {
            1
        } else {
            average_char_width
        };
        let h = if font_height == 0 { 1 } else { font_height };
        Self {
            average_char_width: w,
            font_height: h,
        }
    }

    /// Converts horizontal dialog units to pixels.
    ///
    /// Formula: `PixelX = (DialogX * AverageCharWidth) / 4`
    ///
    /// # Arguments
    ///
    /// * `dlu_x` - Horizontal coordinate or width in dialog units.
    ///
    /// # Returns
    ///
    /// Pixel coordinate or width.
    #[must_use]
    pub fn dlu_to_pixel_x(&self, dlu_x: i32) -> i32 {
        let w = i32::try_from(self.average_char_width).unwrap_or(1);
        (dlu_x * w) / 4
    }

    /// Converts vertical dialog units to pixels.
    ///
    /// Formula: `PixelY = (DialogY * FontHeight) / 8`
    ///
    /// # Arguments
    ///
    /// * `dlu_y` - Vertical coordinate or height in dialog units.
    ///
    /// # Returns
    ///
    /// Pixel coordinate or height.
    #[must_use]
    pub fn dlu_to_pixel_y(&self, dlu_y: i32) -> i32 {
        let h = i32::try_from(self.font_height).unwrap_or(1);
        (dlu_y * h) / 8
    }

    /// Converts horizontal pixels to dialog units.
    ///
    /// Formula: `DialogX = (PixelX * 4) / AverageCharWidth`
    ///
    /// # Arguments
    ///
    /// * `pixel_x` - Pixel coordinate or width.
    ///
    /// # Returns
    ///
    /// Dialog units coordinate or width.
    #[must_use]
    pub fn pixel_to_dlu_x(&self, pixel_x: i32) -> i32 {
        let w = i32::try_from(self.average_char_width).unwrap_or(1);
        (pixel_x * 4) / w
    }

    /// Converts vertical pixels to dialog units.
    ///
    /// Formula: `DialogY = (PixelY * 8) / FontHeight`
    ///
    /// # Arguments
    ///
    /// * `pixel_y` - Pixel coordinate or height.
    ///
    /// # Returns
    ///
    /// Dialog units coordinate or height.
    #[must_use]
    pub fn pixel_to_dlu_y(&self, pixel_y: i32) -> i32 {
        let h = i32::try_from(self.font_height).unwrap_or(1);
        (pixel_y * 8) / h
    }
}

/// Geometric rectangle represented in MSI dialog units (DLUs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DluRect {
    /// X origin coordinate in dialog units.
    pub x: i16,
    /// Y origin coordinate in dialog units.
    pub y: i16,
    /// Width in dialog units.
    pub width: i16,
    /// Height in dialog units.
    pub height: i16,
}

impl DluRect {
    /// Creates a new [`DluRect`].
    ///
    /// # Arguments
    ///
    /// * `x` - X origin in DLUs.
    /// * `y` - Y origin in DLUs.
    /// * `width` - Width in DLUs.
    /// * `height` - Height in DLUs.
    ///
    /// # Returns
    ///
    /// A new [`DluRect`].
    #[must_use]
    pub const fn new(x: i16, y: i16, width: i16, height: i16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Converts this dialog unit rectangle into absolute pixel coordinates.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Font metrics used for conversion.
    ///
    /// # Returns
    ///
    /// A [`PixelRect`].
    #[must_use]
    pub fn to_pixel_rect(&self, metrics: &FontMetrics) -> PixelRect {
        PixelRect {
            x: metrics.dlu_to_pixel_x(i32::from(self.x)),
            y: metrics.dlu_to_pixel_y(i32::from(self.y)),
            width: metrics.dlu_to_pixel_x(i32::from(self.width)),
            height: metrics.dlu_to_pixel_y(i32::from(self.height)),
        }
    }
}

/// Geometric rectangle represented in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PixelRect {
    /// X origin coordinate in pixels.
    pub x: i32,
    /// Y origin coordinate in pixels.
    pub y: i32,
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
}

impl PixelRect {
    /// Creates a new [`PixelRect`].
    ///
    /// # Arguments
    ///
    /// * `x` - X origin in pixels.
    /// * `y` - Y origin in pixels.
    /// * `width` - Width in pixels.
    /// * `height` - Height in pixels.
    ///
    /// # Returns
    ///
    /// A new [`PixelRect`].
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Applies horizontal and vertical centering within a parent or screen container.
    ///
    /// # Arguments
    ///
    /// * `container_width` - Container width in pixels.
    /// * `container_height` - Container height in pixels.
    /// * `h_centering` - Whether horizontal centering is requested (50 = 50% or flag enabled).
    /// * `v_centering` - Whether vertical centering is requested (50 = 50% or flag enabled).
    ///
    /// # Returns
    ///
    /// A centered [`PixelRect`].
    #[must_use]
    pub fn apply_centering(
        mut self,
        container_width: i32,
        container_height: i32,
        h_centering: i16,
        v_centering: i16,
    ) -> Self {
        if h_centering > 0 {
            let offset_pct = if h_centering >= 100 {
                50
            } else {
                i32::from(h_centering)
            };
            self.x = (container_width - self.width) * offset_pct / 100;
        }
        if v_centering > 0 {
            let offset_pct = if v_centering >= 100 {
                50
            } else {
                i32::from(v_centering)
            };
            self.y = (container_height - self.height) * offset_pct / 100;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests dialog unit to pixel and inverse conversions.
    #[test]
    fn test_font_metrics_conversions() {
        let metrics = FontMetrics::default(); // 8px char width, 16px char height
        assert_eq!(metrics.average_char_width, 8);
        assert_eq!(metrics.font_height, 16);

        // Horizontal: (X * 8) / 4 = X * 2
        assert_eq!(metrics.dlu_to_pixel_x(0), 0);
        assert_eq!(metrics.dlu_to_pixel_x(50), 100);
        assert_eq!(metrics.pixel_to_dlu_x(100), 50);

        // Vertical: (Y * 16) / 8 = Y * 2
        assert_eq!(metrics.dlu_to_pixel_y(0), 0);
        assert_eq!(metrics.dlu_to_pixel_y(25), 50);
        assert_eq!(metrics.pixel_to_dlu_y(50), 25);

        // Custom metrics fallback on 0
        let custom = FontMetrics::new(0, 0);
        assert_eq!(custom.average_char_width, 1);
        assert_eq!(custom.font_height, 1);
    }

    /// Tests rectangle conversion and centering.
    #[test]
    fn test_dlu_rect_and_pixel_rect_centering() {
        let metrics = FontMetrics::new(8, 16);
        let dlu_rect = DluRect::new(10, 20, 180, 120);

        let pixel_rect = dlu_rect.to_pixel_rect(&metrics);
        assert_eq!(pixel_rect.x, 20); // 10 * 2
        assert_eq!(pixel_rect.y, 40); // 20 * 2
        assert_eq!(pixel_rect.width, 360); // 180 * 2
        assert_eq!(pixel_rect.height, 240); // 120 * 2

        // Center within a 1024x768 screen with 50% centering
        let centered = pixel_rect.apply_centering(1024, 768, 50, 50);
        assert_eq!(centered.x, (1024 - 360) / 2); // 332
        assert_eq!(centered.y, (768 - 240) / 2); // 264
        assert_eq!(centered.width, 360);
        assert_eq!(centered.height, 240);

        // If centering is 0, preserve original coordinates
        let uncentered = pixel_rect.apply_centering(1024, 768, 0, 0);
        assert_eq!(uncentered.x, 20);
        assert_eq!(uncentered.y, 40);

        // If centering is >= 100, clamp to 50%
        let clamped = pixel_rect.apply_centering(1000, 800, 100, 150);
        assert_eq!(clamped.x, (1000 - 360) / 2);
        assert_eq!(clamped.y, (800 - 240) / 2);
    }
}
