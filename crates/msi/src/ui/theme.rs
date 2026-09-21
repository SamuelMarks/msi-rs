//! MSI Wizard Themes, High-Fidelity Styling & Accessibility Specifications.
//!
//! Grounded directly in official Windows Installer and `WiX` Toolset UI library specifications:
//! - `WiX` Wizard Themes:
//!   - `WixUI_Mondo`: Full wizard with license agreement, setup type selection (Typical, Complete, Custom), and feature customization.
//!   - `WixUI_InstallDir`: Streamlined wizard with license agreement and target installation directory browser.
//!   - `WixUI_FeatureTree`: Wizard with license agreement leading directly to hierarchical feature tree selection.
//!   - `WixUI_Minimal`: Single-dialog agreement and immediate installation.
//! - High-fidelity color palettes, banner dimensions, button styling, and separator lines.
//! - Comprehensive accessibility metadata: full keyboard tab order, focus ring highlighting, and screen reader labels.

/// 32-bit RGBA color representation for UI rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color32 {
    /// Red component (0..=255).
    pub r: u8,
    /// Green component (0..=255).
    pub g: u8,
    /// Blue component (0..=255).
    pub b: u8,
    /// Alpha component (0..=255).
    pub a: u8,
}

impl Color32 {
    /// Creates a new fully opaque [`Color32`] from RGB components.
    #[must_use]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Creates a new [`Color32`] from RGBA components.
    #[must_use]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Pure white (`#FFFFFF`).
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);
    /// Pure black (`#000000`).
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    /// Transparent color (`#00000000`).
    pub const TRANSPARENT: Self = Self::from_rgba(0, 0, 0, 0);
    /// Classic Windows dialog background gray (`#F0F0F0`).
    pub const WINDOW_BG: Self = Self::from_rgb(240, 240, 240);
    /// Button border gray (`#ADADAD`).
    pub const BUTTON_BORDER: Self = Self::from_rgb(173, 173, 173);
    /// Button default surface (`#E1E1E1`).
    pub const BUTTON_BG: Self = Self::from_rgb(225, 225, 225);
    /// Button hover surface (`#E5F1FB`).
    pub const BUTTON_HOVER: Self = Self::from_rgb(229, 241, 251);
    /// Windows Installer accent blue (`#0078D7`).
    pub const ACCENT_BLUE: Self = Self::from_rgb(0, 120, 215);
    /// High-contrast keyboard focus ring color (`#005A9E`).
    pub const FOCUS_RING: Self = Self::from_rgb(0, 90, 158);
    /// Banner header background white (`#FFFFFF`).
    pub const BANNER_BG: Self = Self::from_rgb(255, 255, 255);
    /// Etched separator line gray (`#DFDFDF`).
    pub const LINE_GRAY: Self = Self::from_rgb(223, 223, 223);
}

/// Classic `WiX` Toolset wizard styling flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WizardStyle {
    /// `WixUI_Mondo`: Complete wizard with `SetupType` selection and Feature customization.
    #[default]
    Mondo,
    /// `WixUI_InstallDir`: Streamlined wizard specifying a single target directory.
    InstallDir,
    /// `WixUI_FeatureTree`: Bypasses setup type and presents the selection tree directly.
    FeatureTree,
    /// `WixUI_Minimal`: Minimal installer without optional setup screens.
    Minimal,
}

impl WizardStyle {
    /// Returns the standard `WiX` dialog set identifier name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mondo => "WixUI_Mondo",
            Self::InstallDir => "WixUI_InstallDir",
            Self::FeatureTree => "WixUI_FeatureTree",
            Self::Minimal => "WixUI_Minimal",
        }
    }
}

/// Visual styling specifications replicating the authentic Windows Installer wizard aesthetic.
#[derive(Debug, Clone, PartialEq)]
pub struct WizardTheme {
    /// Active wizard style preset.
    pub style: WizardStyle,
    /// Top banner width in pixels.
    pub banner_width: u32,
    /// Top banner height in pixels (typically 58px).
    pub banner_height: u32,
    /// Standard dialog width in DLUs (typically 370).
    pub dialog_width_dlu: i16,
    /// Standard dialog height in DLUs (typically 270).
    pub dialog_height_dlu: i16,
    /// Main window background color.
    pub window_bg: Color32,
    /// Banner background color.
    pub banner_bg: Color32,
    /// Body text color.
    pub text_color: Color32,
    /// Default push button background color.
    pub button_bg: Color32,
    /// Button border outline color.
    pub button_border: Color32,
    /// Focus ring outline color for keyboard navigation.
    pub focus_ring_color: Color32,
    /// Focus ring stroke thickness in pixels.
    pub focus_ring_width: f32,
    /// Base UI font size in points/pixels.
    pub base_font_size: f32,
    /// Title header font size in points/pixels.
    pub title_font_size: f32,
}

impl Default for WizardTheme {
    fn default() -> Self {
        Self::mondo()
    }
}

impl WizardTheme {
    /// Creates a [`WizardTheme`] configured with `WixUI_Mondo` styling.
    #[must_use]
    pub const fn mondo() -> Self {
        Self {
            style: WizardStyle::Mondo,
            banner_width: 493,
            banner_height: 58,
            dialog_width_dlu: 370,
            dialog_height_dlu: 270,
            window_bg: Color32::WINDOW_BG,
            banner_bg: Color32::BANNER_BG,
            text_color: Color32::BLACK,
            button_bg: Color32::BUTTON_BG,
            button_border: Color32::BUTTON_BORDER,
            focus_ring_color: Color32::FOCUS_RING,
            focus_ring_width: 2.0,
            base_font_size: 12.0,
            title_font_size: 14.0,
        }
    }

    /// Creates a [`WizardTheme`] configured with `WixUI_InstallDir` styling.
    #[must_use]
    pub const fn install_dir() -> Self {
        let mut t = Self::mondo();
        t.style = WizardStyle::InstallDir;
        t
    }

    /// Creates a [`WizardTheme`] configured with `WixUI_FeatureTree` styling.
    #[must_use]
    pub const fn feature_tree() -> Self {
        let mut t = Self::mondo();
        t.style = WizardStyle::FeatureTree;
        t
    }

    /// Creates a [`WizardTheme`] configured with `WixUI_Minimal` styling.
    #[must_use]
    pub const fn minimal() -> Self {
        let mut t = Self::mondo();
        t.style = WizardStyle::Minimal;
        t
    }
}

/// Accessibility metadata for UI controls and dialog elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityInfo {
    /// Accessible text label read aloud by screen readers.
    pub label: String,
    /// Optional accessible description / tooltip.
    pub description: Option<String>,
    /// Order index in sequential keyboard Tab navigation.
    pub tab_index: u32,
    /// Whether this control currently holds active keyboard focus.
    pub has_focus: bool,
    /// Accessible role classification (e.g. "button", "checkbox", "edit", "tree", "link").
    pub role: &'static str,
}

impl AccessibilityInfo {
    /// Creates a new [`AccessibilityInfo`].
    ///
    /// # Arguments
    ///
    /// * `label` - Screen reader label.
    /// * `role` - Accessible role.
    /// * `tab_index` - Tab navigation order.
    ///
    /// # Returns
    ///
    /// A new [`AccessibilityInfo`].
    #[must_use]
    pub fn new(label: impl Into<String>, role: &'static str, tab_index: u32) -> Self {
        Self {
            label: label.into(),
            description: None,
            tab_index,
            has_focus: false,
            role,
        }
    }

    /// Sets the accessible description.
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Sets focus state.
    #[must_use]
    pub const fn with_focus(mut self, focus: bool) -> Self {
        self.has_focus = focus;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `Color32` constructors and standard constants.
    #[test]
    fn test_color32() {
        let c = Color32::from_rgb(10, 20, 30);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
        assert_eq!(c.a, 255);

        let c_alpha = Color32::from_rgba(10, 20, 30, 128);
        assert_eq!(c_alpha.a, 128);

        assert_eq!(Color32::WHITE, Color32::from_rgb(255, 255, 255));
        assert_eq!(Color32::BLACK, Color32::from_rgb(0, 0, 0));
        assert_eq!(Color32::TRANSPARENT, Color32::from_rgba(0, 0, 0, 0));
    }

    /// Tests `WizardStyle` string representation and theme presets.
    #[test]
    fn test_wizard_themes() {
        assert_eq!(WizardStyle::Mondo.as_str(), "WixUI_Mondo");
        assert_eq!(WizardStyle::InstallDir.as_str(), "WixUI_InstallDir");
        assert_eq!(WizardStyle::FeatureTree.as_str(), "WixUI_FeatureTree");
        assert_eq!(WizardStyle::Minimal.as_str(), "WixUI_Minimal");

        let mondo = WizardTheme::default();
        assert_eq!(mondo.style, WizardStyle::Mondo);
        assert_eq!(mondo.dialog_width_dlu, 370);
        assert_eq!(mondo.dialog_height_dlu, 270);
        assert_eq!(mondo.banner_height, 58);

        let idir = WizardTheme::install_dir();
        assert_eq!(idir.style, WizardStyle::InstallDir);

        let ftree = WizardTheme::feature_tree();
        assert_eq!(ftree.style, WizardStyle::FeatureTree);

        let min = WizardTheme::minimal();
        assert_eq!(min.style, WizardStyle::Minimal);
    }

    /// Tests `AccessibilityInfo` creation and focus handling.
    #[test]
    fn test_accessibility_info() {
        let acc = AccessibilityInfo::new("Next", "button", 1)
            .description("Click to advance to the next wizard step")
            .with_focus(true);

        assert_eq!(acc.label, "Next");
        assert_eq!(acc.role, "button");
        assert_eq!(acc.tab_index, 1);
        assert_eq!(
            acc.description.as_deref(),
            Some("Click to advance to the next wizard step")
        );
        assert!(acc.has_focus);
    }
}
