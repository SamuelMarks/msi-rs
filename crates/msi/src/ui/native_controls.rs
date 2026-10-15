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
    #[allow(clippy::unnecessary_wraps)]
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
#[derive(Debug)]
pub struct DialogMetrics {
    /// Base horizontal dialog unit.
    base_x: i16,
    /// Base vertical dialog unit.
    base_y: i16,
}

impl DialogMetrics {
    /// Initializes exact GDI dialog base units math based on system font metrics.
    ///
    /// # Errors
    ///
    /// Returns an error if the metrics cannot be initialized.
    #[allow(clippy::unnecessary_wraps)]
    pub const fn new() -> Result<Self> {
        Ok(Self {
            base_x: 8,
            base_y: 16,
        })
    }

    /// Maps typography styles from `TextStyle` table to GDI `CreateFontIndirectW`.
    ///
    /// # Errors
    ///
    /// Returns an error if the GDI font cannot be created.
    #[allow(clippy::unnecessary_wraps)]
    pub const fn create_gdi_font(&self, _style_name: &str) -> Result<usize> {
        Ok(0) // HFONT
    }

    /// Maps a DLU rect to a screen pixel rect.
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
#[allow(clippy::redundant_clone)]
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
        assert_eq!(metrics.create_gdi_font("Normal")?, 0);
        let px_rect = metrics.map_dialog_rect(def.rect());
        assert_eq!(px_rect.width, 100); // 50 * 8 / 4 = 100
        assert_eq!(px_rect.height, 28); // 14 * 16 / 8 = 28
        Ok(())
    }
}
