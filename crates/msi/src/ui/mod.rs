//! Interactive GUI and MSI UI Engine (`msi-ui-core`).
//!
//! Grounded directly in official Windows Installer SDK specifications:
//! - Dialog layout math, font metrics, and centering (`layout`).
//! - Standard control definitions, attributes, and runtime states (`controls`).
//! - Control events, condition dispatching, and event mappings (`events`).
//! - Pure headless UI state machine engine (`engine`).
//! - Immediate-mode GUI layout mapping and software rasterizer (`renderer`).
//! - Wizard styling themes and accessibility metadata (`theme`).
//! - Alternative terminal TUI interactive wizard (`tui`).

pub mod controls;
pub mod engine;
pub mod events;
pub mod font_rasterizer;
pub mod layout;
pub mod renderer;
pub mod theme;
pub mod tui;
pub mod window;

pub use controls::{
    ControlDefinition, ControlRuntimeState, ControlType, ListItem, SelectionTreeNode,
    VolumeCostEntry, CONTROL_ATTR_BITMAP, CONTROL_ATTR_ENABLED, CONTROL_ATTR_FIXED_SIZE,
    CONTROL_ATTR_ICON, CONTROL_ATTR_IMAGE_RESOURCE, CONTROL_ATTR_INDIRECT, CONTROL_ATTR_INTEGER,
    CONTROL_ATTR_MULTILINE, CONTROL_ATTR_PASSWORD_INPUT, CONTROL_ATTR_RIGHT_ALIGNED,
    CONTROL_ATTR_RIGHT_TO_LEFT, CONTROL_ATTR_SUNKEN, CONTROL_ATTR_VISIBLE,
};
pub use engine::{
    DialogDefinition, UiEngine, DIALOG_ATTR_MINIMIZE, DIALOG_ATTR_MODAL,
    DIALOG_ATTR_TRACK_DISK_SPACE, DIALOG_ATTR_VISIBLE,
};
pub use events::{
    ControlCondition, ControlConditionAction, ControlEvent, ControlEventType, DialogReturnCode,
    EventMapping,
};
pub use font_rasterizer::{
    blend_pixel, FontVectorRasterizer, PlacedGlyph, TextLayoutEngine, TextStyleDefinition,
    MSI_TEXT_STYLE_BOLD, MSI_TEXT_STYLE_ITALIC, MSI_TEXT_STYLE_STRIKEOUT, MSI_TEXT_STYLE_UNDERLINE,
};
pub use layout::{
    DluRect, FontMetrics, PixelRect, DEFAULT_AVERAGE_CHAR_WIDTH, DEFAULT_FONT_HEIGHT,
};
pub use renderer::{DisplayBackendType, DrawCommand, EguiLayoutMapper, SoftwareBuffer, UiWidget};
pub use theme::{AccessibilityInfo, Color32, WizardStyle, WizardTheme};
pub use tui::{
    TerminalBuffer, TerminalController, TerminalEvent, TerminalSafetyGuard, TerminalWizard, TuiKey,
};
pub use window::{
    AccessKitBridge, AccessibleNode, AccessibleRole, GuiDesktopRuntime, GuiHardwareBackend,
    GuiInputEvent, WindowConfig,
};
