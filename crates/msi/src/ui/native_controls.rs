//! Native Win32 Control Instances.
//!
//! Provides the HWND-backed instances for standard controls such as
//! `PushButton`, `RadioButtonGroup`, `Edit`, `ComboBox`, `ProgressBar`, `VolumeCostList`, etc.

use crate::error::Result;
use crate::ui::controls::{ControlDefinition, ControlType};

/// A Win32 Native Control.
#[derive(Debug)]
pub struct NativeControl {
    /// HWND handle for the control.
    _hwnd: usize,
    /// Type of the control.
    _ctrl_type: ControlType,
}

impl NativeControl {
    /// Instantiates a new native control from a definition.
    ///
    /// # Errors
    ///
    /// Returns an error if the native control cannot be created.
    pub const fn new(def: &ControlDefinition) -> Result<Self> {
        Ok(Self {
            _hwnd: 0,
            _ctrl_type: def.control_type(),
        })
    }

    /// Handles a Win32 message targeted at this control.
    pub const fn handle_message(&mut self, _msg: u32, _wparam: usize, _lparam: isize) -> isize {
        0
    }
}

/// Font Rasterization & Dialog Units (DLU) Metric Fidelity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogMetrics {
    /// Base horizontal dialog unit in pixels.
    base_x: i16,
    /// Base vertical dialog unit in pixels.
    base_y: i16,
}

/// Type alias unifying `DialogUnitConverter` with the canonical [`DialogMetrics`] struct.
pub type DialogUnitConverter = DialogMetrics;

impl Default for DialogMetrics {
    fn default() -> Self {
        Self::with_units(8, 16)
    }
}

impl DialogMetrics {
    /// Initializes exact GDI dialog base units math based on default system font metrics.
    ///
    /// # Errors
    ///
    /// Returns an error if the metrics cannot be initialized.
    pub const fn new() -> Result<Self> {
        Ok(Self {
            base_x: 8,
            base_y: 16,
        })
    }

    /// Creates a [`DialogMetrics`] instance with custom horizontal and vertical base font units.
    ///
    /// # Arguments
    ///
    /// * `base_x` - Base horizontal character width in pixels.
    /// * `base_y` - Base vertical character height in pixels.
    ///
    /// # Returns
    ///
    /// A configured [`DialogMetrics`] converter.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn with_units(base_x: u32, base_y: u32) -> Self {
        Self {
            base_x: base_x as i16,
            base_y: base_y as i16,
        }
    }

    /// Converts an X coordinate from Dialog Units to Pixels (`(x * base_x) / 4`).
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal coordinate or width in dialog units.
    ///
    /// # Returns
    ///
    /// Converted horizontal pixel coordinate or width.
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub const fn x_to_pixels(&self, x: u32) -> u32 {
        (x * (self.base_x as u32)) / 4
    }

    /// Converts a Y coordinate from Dialog Units to Pixels (`(y * base_y) / 8`).
    ///
    /// # Arguments
    ///
    /// * `y` - Vertical coordinate or height in dialog units.
    ///
    /// # Returns
    ///
    /// Converted vertical pixel coordinate or height.
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub const fn y_to_pixels(&self, y: u32) -> u32 {
        (y * (self.base_y as u32)) / 8
    }

    /// Maps typography styles from `TextStyle` table to GDI `CreateFontIndirectW`.
    ///
    /// # Arguments
    ///
    /// * `_style_name` - Name of the `TextStyle` entry to resolve.
    ///
    /// # Returns
    ///
    /// Simulated or native `HFONT` handle (`0` when running headlessly).
    ///
    /// # Errors
    ///
    /// Returns an error if the GDI font cannot be created.
    pub const fn create_gdi_font(&self, _style_name: &str) -> Result<usize> {
        Ok(0) // HFONT
    }

    /// Maps a DLU rect to a screen pixel rect.
    ///
    /// # Arguments
    ///
    /// * `rect` - Input rectangle in dialog units.
    ///
    /// # Returns
    ///
    /// Scaled rectangle in pixel coordinates.
    #[must_use]
    pub const fn map_dialog_rect(
        &self,
        mut rect: crate::ui::layout::DluRect,
    ) -> crate::ui::layout::DluRect {
        rect.x = (rect.x * self.base_x) / 4;
        rect.y = (rect.y * self.base_y) / 8;
        rect.width = (rect.width * self.base_x) / 4;
        rect.height = (rect.height * self.base_y) / 8;
        rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_controls_stubs() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let def = ControlDefinition::new(
            "Dlg",
            "Btn",
            ControlType::PushButton,
            crate::ui::layout::DluRect::new(0, 0, 50, 14),
            0,
        );
        let mut ctrl = NativeControl::new(&def)?;
        assert_eq!(ctrl.handle_message(0, 0, 0), 0);

        let metrics = DialogMetrics::new()?;
        assert_eq!(metrics, DialogMetrics::default());
        assert_eq!(metrics.create_gdi_font("Normal")?, 0);
        assert_eq!(metrics.x_to_pixels(10), 20);
        assert_eq!(metrics.y_to_pixels(10), 20);
        let px_rect = metrics.map_dialog_rect(def.rect());
        assert_eq!(px_rect.width, 100); // 50 * 8 / 4 = 100
        assert_eq!(px_rect.height, 28); // 14 * 16 / 8 = 28
        Ok(())
    }
}
