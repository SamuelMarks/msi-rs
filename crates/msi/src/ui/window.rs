//! Live Native Desktop GUI Windowing Runtime and AccessKit Bridge.
//!
//! Grounded directly in `egui` immediate-mode GUI architecture, `eframe` windowing, and
//! Windows Installer SDK specifications:
//! - Window configuration with non-resizable constraints and title formatting.
//! - Multi-Backend hardware acceleration:
//!   - `WgpuDirectXMetalVulkan`: Modern GPU rendering via `wgpu`.
//!   - `GlowLegacyOpenGl`: Legacy OpenGL rendering via `glow`.
//!   - `SoftbufferHeadlessRasterizer`: Pure software rasterizer fallback (`softbuffer`).
//! - High-fidelity `WiX` styling presets: `WixUI_Mondo`, `WixUI_InstallDir`, `WixUI_FeatureTree`.
//! - Accessible navigation:
//!   - Full keyboard tab navigation cycling focus across visible controls.
//!   - Enter key default button activation and Escape key cancellation.
//!   - High-contrast focus ring rendering (`Color32::FOCUS_RING`).
//!   - Screen reader accessibility integration via `AccessKit` node tree publishing.

use crate::error::Result;
use crate::ui::controls::ControlType;
use crate::ui::engine::UiEngine;
use crate::ui::events::DialogReturnCode;
use crate::ui::layout::{FontMetrics, PixelRect};
use crate::ui::renderer::{DrawCommand, EguiLayoutMapper, UiWidget};
use crate::ui::theme::{Color32, WizardStyle, WizardTheme};

/// Window creation configuration specifying desktop window properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    /// Window title text displayed in the OS title bar.
    pub title: String,
    /// Window width in pixels.
    pub width: u32,
    /// Window height in pixels.
    pub height: u32,
    /// Whether window is resizable (standard MSI dialogs are non-resizable).
    pub resizable: bool,
    /// Whether the window should be centered on screen on display.
    pub center_on_screen: bool,
    /// Optional window icon byte buffer (PNG / ICO).
    pub icon_bytes: Option<Vec<u8>>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Windows Installer Setup".to_string(),
            width: 493,
            height: 360,
            resizable: false,
            center_on_screen: true,
            icon_bytes: None,
        }
    }
}

/// Supported GUI display and hardware acceleration backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GuiHardwareBackend {
    /// Modern hardware-accelerated GPU pipeline (`wgpu`: DirectX 12, Metal, Vulkan).
    #[default]
    WgpuDirectXMetalVulkan,
    /// Legacy hardware OpenGL backend (`glow`).
    GlowLegacyOpenGl,
    /// Headless or pure-CPU software rasterizer fallback (`softbuffer`).
    SoftbufferHeadlessRasterizer,
}

impl GuiHardwareBackend {
    /// Returns the descriptive name of this backend.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WgpuDirectXMetalVulkan => "wgpu (DirectX 12 / Metal / Vulkan)",
            Self::GlowLegacyOpenGl => "glow (OpenGL 2.1+ / ES 2.0)",
            Self::SoftbufferHeadlessRasterizer => "softbuffer (pure CPU rasterizer)",
        }
    }
}

/// Accessible semantic role of a UI node corresponding to AccessKit specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleRole {
    /// Push button.
    Button,
    /// Check box toggle.
    CheckBox,
    /// Text edit field.
    TextInput,
    /// Progress indicator.
    ProgressBar,
    /// Static read-only text or label.
    StaticText,
    /// Top-level modal or modeless dialog.
    Dialog,
    /// Application desktop window.
    Window,
}

/// An accessible node representation published for screen readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleNode {
    /// Unique node identifier.
    pub id: usize,
    /// Accessible semantic role.
    pub role: AccessibleRole,
    /// Accessible text label.
    pub label: String,
    /// Optional accessible description or tooltip.
    pub description: Option<String>,
    /// Bounding rectangle in canvas pixel coordinates.
    pub bounds: PixelRect,
    /// Whether node currently possesses keyboard focus.
    pub is_focused: bool,
    /// Whether node is enabled for interaction.
    pub is_enabled: bool,
    /// Optional boolean checked state for check boxes.
    pub is_checked: Option<bool>,
}

/// Bridge publishing accessible control hierarchies for assistive screen readers.
#[derive(Debug, Clone, Default)]
pub struct AccessKitBridge {
    /// List of published accessible nodes for the current frame.
    nodes: Vec<AccessibleNode>,
}

impl AccessKitBridge {
    /// Creates a new [`AccessKitBridge`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears and publishes a new set of accessible nodes for the active frame.
    ///
    /// # Arguments
    ///
    /// * `nodes` - Vector of [`AccessibleNode`].
    pub fn update_nodes(&mut self, nodes: Vec<AccessibleNode>) {
        self.nodes = nodes;
    }

    /// Returns the list of currently published accessible nodes.
    ///
    /// # Returns
    ///
    /// Slice of [`AccessibleNode`].
    #[must_use]
    pub fn nodes(&self) -> &[AccessibleNode] {
        &self.nodes
    }
}

/// User input events delivered to the immediate-mode desktop GUI loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiInputEvent {
    /// Mouse primary click at specified canvas pixel coordinates.
    MouseClick {
        /// X coordinate.
        x: i32,
        /// Y coordinate.
        y: i32,
    },
    /// Tab key pressed to navigate to next tab stop.
    TabNext,
    /// Shift+Tab pressed to navigate to previous tab stop.
    TabPrev,
    /// Enter key pressed to activate the default button.
    SubmitDefault,
    /// Escape key pressed to trigger dialog cancellation.
    CancelEscape,
}

/// Lifecycle events of a desktop GUI window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLifecycleEvent {
    /// Window has opened and initialized on screen.
    Opened,
    /// Window received focus.
    Focused,
    /// Window lost focus.
    Unfocused,
    /// Window minimized.
    Minimized,
    /// Window restored from minimized state.
    Restored,
    /// Window resize requested with target dimensions (constrained by non-resizable rules).
    ResizeRequested {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
    /// Window close requested by user (titlebar close button or Alt+F4).
    CloseRequested,
}

/// The live desktop GUI windowing runtime orchestrating the immediate-mode render loop.
#[derive(Debug)]
pub struct GuiDesktopRuntime {
    /// Inner headless UI engine managing dialog state machines.
    engine: UiEngine,
    /// Layout mapper mapping MSI dialogs to widgets and draw commands.
    layout_mapper: EguiLayoutMapper,
    /// Active window configuration.
    window_config: WindowConfig,
    /// Hardware acceleration backend.
    hardware_backend: GuiHardwareBackend,
    /// Visuals styling theme.
    theme: WizardTheme,
    /// Screen reader AccessKit bridge.
    access_bridge: AccessKitBridge,
    /// Index of currently focused control in tab stop order.
    focused_tab_index: usize,
}

impl GuiDesktopRuntime {
    /// Creates a new [`GuiDesktopRuntime`].
    ///
    /// # Arguments
    ///
    /// * `engine` - Configured [`UiEngine`].
    /// * `theme` - Visual styling theme.
    /// * `hardware_backend` - Hardware acceleration backend.
    ///
    /// # Returns
    ///
    /// A new [`GuiDesktopRuntime`].
    #[must_use]
    pub fn new(engine: UiEngine, theme: WizardTheme, hardware_backend: GuiHardwareBackend) -> Self {
        let font_metrics = FontMetrics::default();
        let layout_mapper = EguiLayoutMapper::new(theme.clone(), font_metrics);
        let window_config = WindowConfig {
            title: theme.style.as_str().to_string(),
            width: theme.banner_width,
            height: 360,
            resizable: false,
            center_on_screen: true,
            icon_bytes: None,
        };

        Self {
            engine,
            layout_mapper,
            window_config,
            hardware_backend,
            theme,
            access_bridge: AccessKitBridge::new(),
            focused_tab_index: 0,
        }
    }

    /// Automatically selects the best available hardware backend, cascading from
    /// `Wgpu` to `Glow` to `Softbuffer` on initialization failure.
    ///
    /// # Arguments
    ///
    /// * `requested` - Preferred initial backend.
    /// * `wgpu_available` - Whether modern GPU is available.
    /// * `glow_available` - Whether legacy OpenGL context can be acquired.
    ///
    /// # Returns
    ///
    /// Resulting active [`GuiHardwareBackend`].
    #[must_use]
    pub const fn select_backend_with_fallback(
        requested: GuiHardwareBackend,
        wgpu_available: bool,
        glow_available: bool,
    ) -> GuiHardwareBackend {
        match requested {
            GuiHardwareBackend::WgpuDirectXMetalVulkan => {
                if wgpu_available {
                    GuiHardwareBackend::WgpuDirectXMetalVulkan
                } else if glow_available {
                    GuiHardwareBackend::GlowLegacyOpenGl
                } else {
                    GuiHardwareBackend::SoftbufferHeadlessRasterizer
                }
            }
            GuiHardwareBackend::GlowLegacyOpenGl => {
                if glow_available {
                    GuiHardwareBackend::GlowLegacyOpenGl
                } else {
                    GuiHardwareBackend::SoftbufferHeadlessRasterizer
                }
            }
            GuiHardwareBackend::SoftbufferHeadlessRasterizer => {
                GuiHardwareBackend::SoftbufferHeadlessRasterizer
            }
        }
    }

    /// Enforces non-resizable modal dialog constraints, returning the constrained dimensions.
    ///
    /// # Arguments
    ///
    /// * `requested_w` - Requested width.
    /// * `requested_h` - Requested height.
    ///
    /// # Returns
    ///
    /// Tuple of constrained `(width, height)`.
    #[must_use]
    pub const fn enforce_window_constraints(
        &self,
        requested_w: u32,
        requested_h: u32,
    ) -> (u32, u32) {
        if self.window_config.resizable {
            (requested_w, requested_h)
        } else {
            (self.window_config.width, self.window_config.height)
        }
    }

    /// Processes a window lifecycle event.
    ///
    /// # Arguments
    ///
    /// * `event` - Window lifecycle event.
    ///
    /// # Returns
    ///
    /// Optional [`DialogReturnCode`] if the event triggers dialog termination.
    pub fn process_lifecycle_event(
        &mut self,
        event: WindowLifecycleEvent,
    ) -> Option<DialogReturnCode> {
        match event {
            WindowLifecycleEvent::CloseRequested => {
                let code = self
                    .engine
                    .active_dialog()
                    .and_then(|dlg| {
                        dlg.control_cancel
                            .as_deref()
                            .map(|c| (dlg.name.clone(), c.to_string()))
                    })
                    .and_then(|(dlg_name, cancel_ctrl)| {
                        self.engine
                            .click_control(&dlg_name, &cancel_ctrl)
                            .ok()
                            .flatten()
                    });
                Some(code.unwrap_or(DialogReturnCode::Exit))
            }
            _ => None,
        }
    }

    /// Returns a reference to the active window configuration.
    #[must_use]
    pub const fn window_config(&self) -> &WindowConfig {
        &self.window_config
    }

    /// Returns a mutable reference to the active window configuration.
    pub const fn window_config_mut(&mut self) -> &mut WindowConfig {
        &mut self.window_config
    }

    /// Returns the active hardware acceleration backend.
    #[must_use]
    pub const fn hardware_backend(&self) -> GuiHardwareBackend {
        self.hardware_backend
    }

    /// Returns a reference to the active wizard theme.
    #[must_use]
    pub const fn theme(&self) -> &WizardTheme {
        &self.theme
    }

    /// Applies a new wizard styling preset to the runtime visuals.
    ///
    /// # Arguments
    ///
    /// * `style` - Visual style preset ([`WizardStyle`]).
    pub fn apply_style_preset(&mut self, style: WizardStyle) {
        self.theme = match style {
            WizardStyle::Mondo => WizardTheme::mondo(),
            WizardStyle::InstallDir => WizardTheme::install_dir(),
            WizardStyle::FeatureTree => WizardTheme::feature_tree(),
            WizardStyle::Minimal => WizardTheme::minimal(),
        };
        self.layout_mapper = EguiLayoutMapper::new(self.theme.clone(), FontMetrics::default());
        self.window_config.title = style.as_str().to_string();
    }

    /// Returns a reference to the inner [`UiEngine`].
    #[must_use]
    pub const fn engine(&self) -> &UiEngine {
        &self.engine
    }

    /// Returns a mutable reference to the inner [`UiEngine`].
    pub const fn engine_mut(&mut self) -> &mut UiEngine {
        &mut self.engine
    }

    /// Returns a reference to the screen reader [`AccessKitBridge`].
    #[must_use]
    pub const fn access_bridge(&self) -> &AccessKitBridge {
        &self.access_bridge
    }

    /// Executes one frame of the immediate-mode render loop.
    ///
    /// Generates:
    /// - Widget placements for `ui.put(rect, widget)`.
    /// - Drawing commands for 2D software rendering.
    /// - Visual focus ring overlay at the focused control coordinates.
    /// - Accessible node tree published to `AccessKitBridge`.
    ///
    /// # Returns
    ///
    /// Tuple of `(dialog_bounds, widget_placements, draw_commands)`.
    #[allow(clippy::cast_possible_wrap)]
    pub fn render_frame(&mut self) -> (PixelRect, Vec<(PixelRect, UiWidget)>, Vec<DrawCommand>) {
        let (dialog_bounds, widgets, mut draws) = self.layout_mapper.map_active_dialog(
            &self.engine,
            self.window_config.width as i32,
            self.window_config.height as i32,
        );

        // Build accessible nodes and calculate tab stops
        let mut access_nodes = Vec::new();
        access_nodes.push(AccessibleNode {
            id: 1,
            role: AccessibleRole::Window,
            label: self.window_config.title.clone(),
            description: None,
            bounds: PixelRect::new(
                0,
                0,
                self.window_config.width as i32,
                self.window_config.height as i32,
            ),
            is_focused: false,
            is_enabled: true,
            is_checked: None,
        });

        let mut tab_stops: Vec<(usize, PixelRect)> = Vec::new();

        for (idx, (rect, widget)) in widgets.iter().enumerate() {
            let node_id = idx + 2;
            let (role, label, is_enabled, checked) = match widget {
                UiWidget::Button {
                    text, is_enabled, ..
                } => (AccessibleRole::Button, text.clone(), *is_enabled, None),
                UiWidget::Label { text, .. } => {
                    (AccessibleRole::StaticText, text.clone(), true, None)
                }
                UiWidget::CheckBox {
                    text,
                    checked,
                    is_enabled,
                } => (
                    AccessibleRole::CheckBox,
                    text.clone(),
                    *is_enabled,
                    Some(*checked),
                ),
                UiWidget::Edit {
                    text, is_enabled, ..
                } => (AccessibleRole::TextInput, text.clone(), *is_enabled, None),
                UiWidget::ProgressBar { fraction } => (
                    AccessibleRole::ProgressBar,
                    format!("{:.0}%", fraction * 100.0),
                    true,
                    None,
                ),
                UiWidget::Separator { .. } | UiWidget::Image { .. } => continue,
            };

            let is_focused = tab_stops.len() == self.focused_tab_index;
            if is_enabled
                && matches!(
                    role,
                    AccessibleRole::Button | AccessibleRole::CheckBox | AccessibleRole::TextInput
                )
            {
                tab_stops.push((node_id, *rect));
            }

            access_nodes.push(AccessibleNode {
                id: node_id,
                role,
                label,
                description: None,
                bounds: *rect,
                is_focused,
                is_enabled,
                is_checked: checked,
            });
        }

        // Draw visual focus ring around focused control
        if let Some((_, focus_rect)) = tab_stops.get(self.focused_tab_index) {
            let ring_rect = PixelRect::new(
                focus_rect.x - 2,
                focus_rect.y - 2,
                focus_rect.width + 4,
                focus_rect.height + 4,
            );
            draws.push(DrawCommand::StrokeRect {
                rect: ring_rect,
                color: Color32::FOCUS_RING,
                width: self.theme.focus_ring_width,
            });
        }

        self.access_bridge.update_nodes(access_nodes);
        (dialog_bounds, widgets, draws)
    }

    /// Dispatches an input event into the runtime loop.
    ///
    /// # Arguments
    ///
    /// * `event` - Input event to process.
    ///
    /// # Returns
    ///
    /// Optional [`DialogReturnCode`] if a control event closed the dialog.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] if condition or action evaluation fails.
    #[allow(clippy::cast_possible_wrap)]
    pub fn process_event(&mut self, event: GuiInputEvent) -> Result<Option<DialogReturnCode>> {
        let Some(dlg) = self.engine.active_dialog().cloned() else {
            return Ok(None);
        };
        let active_dlg = dlg.name;

        match event {
            GuiInputEvent::MouseClick { x, y } => {
                let controls = self.engine.get_dialog_controls(&active_dlg).to_vec();
                let font_metrics = FontMetrics::default();
                for ctrl in controls {
                    let pixel_rect = ctrl.rect().to_pixel_rect(&font_metrics);
                    if x >= pixel_rect.x
                        && x < pixel_rect.x + pixel_rect.width
                        && y >= pixel_rect.y
                        && y < pixel_rect.y + pixel_rect.height
                    {
                        return self.engine.click_control(&active_dlg, ctrl.control());
                    }
                }
                Ok(None)
            }
            GuiInputEvent::TabNext => {
                let tab_count = self.get_interactive_control_count(&active_dlg);
                if tab_count > 0 {
                    self.focused_tab_index = (self.focused_tab_index + 1) % tab_count;
                }
                Ok(None)
            }
            GuiInputEvent::TabPrev => {
                let tab_count = self.get_interactive_control_count(&active_dlg);
                if tab_count > 0 {
                    self.focused_tab_index = (self.focused_tab_index + tab_count - 1) % tab_count;
                }
                Ok(None)
            }
            GuiInputEvent::SubmitDefault => {
                if let Some(ref def_ctrl) = dlg.control_default {
                    return self.engine.click_control(&active_dlg, def_ctrl);
                }
                Ok(None)
            }
            GuiInputEvent::CancelEscape => {
                if let Some(ref cancel_ctrl) = dlg.control_cancel {
                    return self.engine.click_control(&active_dlg, cancel_ctrl);
                }
                Ok(Some(DialogReturnCode::Exit))
            }
        }
    }

    /// Counts the interactive enabled controls eligible for tab stops in the active dialog.
    fn get_interactive_control_count(&self, dialog: &str) -> usize {
        let mut count = 0;
        for ctrl in self.engine.get_dialog_controls(dialog) {
            let is_interactive = matches!(
                ctrl.control_type(),
                ControlType::PushButton | ControlType::CheckBox | ControlType::Edit
            );
            let state = self.engine.get_control_state(dialog, ctrl.control());
            let (is_enabled, is_visible) = state.map_or_else(
                || (ctrl.is_enabled_by_default(), ctrl.is_visible_by_default()),
                |s| (s.is_enabled, s.is_visible),
            );
            if is_interactive && is_enabled && is_visible {
                count += 1;
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::controls::ControlDefinition;
    use crate::ui::engine::DialogDefinition;

    /// Helper creating a populated test [`UiEngine`] with a Welcome dialog.
    fn create_test_engine() -> UiEngine {
        let context = crate::execution::properties::EvaluationContext::new();
        let mut engine = UiEngine::new(context);
        engine.add_dialog(DialogDefinition {
            name: "WelcomeDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 370,
            height: 270,
            attributes: crate::ui::engine::DIALOG_ATTR_VISIBLE
                | crate::ui::engine::DIALOG_ATTR_MODAL,
            title: Some("Welcome to Setup".to_string()),
            control_first: "NextButton".to_string(),
            control_default: Some("NextButton".to_string()),
            control_cancel: Some("CancelButton".to_string()),
        });

        let next_btn = ControlDefinition::new(
            "WelcomeDlg",
            "NextButton",
            ControlType::PushButton,
            crate::ui::layout::DluRect::new(200, 240, 50, 17),
            3,
        )
        .text("Next >");
        let cancel_btn = ControlDefinition::new(
            "WelcomeDlg",
            "CancelButton",
            ControlType::PushButton,
            crate::ui::layout::DluRect::new(260, 240, 50, 17),
            3,
        )
        .text("Cancel");

        // Non-interactive control to test is_interactive = false
        let label = ControlDefinition::new(
            "WelcomeDlg",
            "WelcomeText",
            ControlType::Text,
            crate::ui::layout::DluRect::new(20, 20, 200, 20),
            3,
        )
        .text("Welcome");

        // Disabled interactive control to test is_enabled = false
        let disabled_btn = ControlDefinition::new(
            "WelcomeDlg",
            "DisabledNext",
            ControlType::PushButton,
            crate::ui::layout::DluRect::new(140, 240, 50, 17),
            1,
        )
        .text("Disabled");

        // Invisible interactive control to test is_visible = false
        let hidden_btn = ControlDefinition::new(
            "WelcomeDlg",
            "HiddenBtn",
            ControlType::PushButton,
            crate::ui::layout::DluRect::new(80, 240, 50, 17),
            2,
        )
        .text("Hidden");

        engine.add_control(next_btn);
        engine.add_control(cancel_btn);
        engine.add_control(label);
        engine.add_control(disabled_btn);
        engine.add_control(hidden_btn);

        engine.add_event(crate::ui::events::ControlEvent::new(
            "WelcomeDlg",
            "CancelButton",
            crate::ui::events::ControlEventType::EndDialog(DialogReturnCode::Exit),
            None,
            1,
        ));

        let _ = engine.set_active_dialog("WelcomeDlg");
        engine
    }

    /// Tests default configuration values for [`WindowConfig`].
    #[test]
    fn test_window_config_defaults() {
        let cfg = WindowConfig::default();
        assert!(!cfg.resizable);
        assert!(cfg.center_on_screen);
        assert_eq!(cfg.width, 493);
        assert_eq!(cfg.height, 360);
        assert!(cfg.icon_bytes.is_none());
    }

    /// Tests descriptive string representations of all [`GuiHardwareBackend`] options.
    #[test]
    fn test_gui_hardware_backends_display() {
        assert!(GuiHardwareBackend::WgpuDirectXMetalVulkan
            .as_str()
            .contains("wgpu"));
        assert!(GuiHardwareBackend::GlowLegacyOpenGl
            .as_str()
            .contains("glow"));
        assert!(GuiHardwareBackend::SoftbufferHeadlessRasterizer
            .as_str()
            .contains("softbuffer"));
        assert_eq!(
            GuiHardwareBackend::default(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan
        );
    }

    /// Tests rendering a frame, focus ring generation, and getters.
    #[test]
    fn test_gui_runtime_render_frame_and_focus_ring() {
        let engine = create_test_engine();
        let mut runtime = GuiDesktopRuntime::new(
            engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan,
        );

        assert_eq!(runtime.window_config().width, runtime.theme().banner_width);
        assert_eq!(
            runtime.hardware_backend(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan
        );
        assert_eq!(
            runtime.engine().active_dialog().map(|d| d.name.as_str()),
            Some("WelcomeDlg")
        );
        assert_eq!(runtime.engine_mut().action_log().len(), 0);

        let (bounds, widgets, draws) = runtime.render_frame();
        assert!(bounds.width > 0);
        assert_eq!(widgets.len(), 4);
        assert_ne!(draws.len(), 0);

        // When focused_tab_index is beyond tab stops, focus ring is not drawn
        runtime.focused_tab_index = 999;
        let (_, _, draws_no_focus) = runtime.render_frame();
        assert_eq!(draws.len(), draws_no_focus.len() + 1);
        assert!(runtime.access_bridge().nodes().len() >= 2);
    }

    /// Tests processing input events: tab traversal, submit, cancel, and mouse clicks.
    #[test]
    fn test_gui_runtime_input_events() {
        let engine = create_test_engine();
        let mut runtime = GuiDesktopRuntime::new(
            engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan,
        );

        // Tab navigation
        assert!(runtime.process_event(GuiInputEvent::TabNext).is_ok());
        assert_eq!(runtime.focused_tab_index, 1);
        assert!(runtime.process_event(GuiInputEvent::TabPrev).is_ok());
        assert_eq!(runtime.focused_tab_index, 0);

        // Cancel Escape
        let res_cancel = runtime.process_event(GuiInputEvent::CancelEscape);
        assert_eq!(res_cancel, Ok(Some(DialogReturnCode::Exit)));

        // Submit Default
        let res_default = runtime.process_event(GuiInputEvent::SubmitDefault);
        assert!(res_default.is_ok());

        // Mouse click on button (both X and Y in bounds)
        let res_click = runtime.process_event(GuiInputEvent::MouseClick { x: 530, y: 490 });
        assert_eq!(res_click, Ok(Some(DialogReturnCode::Exit)));

        // Mouse click: X in bounds, Y above control
        let res_y_above = runtime.process_event(GuiInputEvent::MouseClick { x: 530, y: 10 });
        assert_eq!(res_y_above, Ok(None));

        // Mouse click: X in bounds, Y below control
        let res_y_below = runtime.process_event(GuiInputEvent::MouseClick { x: 530, y: 9999 });
        assert_eq!(res_y_below, Ok(None));

        // Mouse click missing all controls
        let res_miss = runtime.process_event(GuiInputEvent::MouseClick { x: 10, y: 10 });
        assert_eq!(res_miss, Ok(None));

        // Test TabNext after clearing control states to exercise None fallback in get_interactive_control_count
        runtime.engine_mut().clear_control_states();
        assert!(runtime.process_event(GuiInputEvent::TabNext).is_ok());
    }

    /// Tests input event handling edge cases: no active dialog, no default, no cancel, and empty tab stops.
    #[test]
    fn test_gui_runtime_input_event_edge_cases() {
        let context = crate::execution::properties::EvaluationContext::new();
        let mut engine = UiEngine::new(context);

        // 1. No active dialog
        let mut runtime_no_dlg = GuiDesktopRuntime::new(
            engine.clone(),
            WizardTheme::mondo(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan,
        );
        assert_eq!(
            runtime_no_dlg.process_event(GuiInputEvent::TabNext),
            Ok(None)
        );
        assert_eq!(
            runtime_no_dlg.process_event(GuiInputEvent::SubmitDefault),
            Ok(None)
        );

        // 2. Dialog with no default button and no cancel button
        engine.add_dialog(DialogDefinition {
            name: "BareDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: crate::ui::engine::DIALOG_ATTR_VISIBLE,
            title: Some("Bare".to_string()),
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        });
        assert!(engine.set_active_dialog("BareDlg").is_ok());

        let mut runtime_bare = GuiDesktopRuntime::new(
            engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan,
        );

        // Tab with 0 interactive controls
        runtime_bare.engine_mut().clear_control_states();
        assert_eq!(runtime_bare.process_event(GuiInputEvent::TabNext), Ok(None));
        assert_eq!(runtime_bare.process_event(GuiInputEvent::TabPrev), Ok(None));
        assert_eq!(
            runtime_bare.process_event(GuiInputEvent::SubmitDefault),
            Ok(None)
        );
        assert_eq!(
            runtime_bare.process_event(GuiInputEvent::CancelEscape),
            Ok(Some(DialogReturnCode::Exit))
        );
    }

    /// Tests widget variants including `CheckBox`, `Edit`, `ProgressBar`, `Separator`, `Image`, and disabled controls.
    #[test]
    fn test_gui_runtime_with_all_widget_types() {
        let mut engine = create_test_engine();
        let chk = ControlDefinition::new(
            "WelcomeDlg",
            "AgreeCheck",
            ControlType::CheckBox,
            crate::ui::layout::DluRect::new(20, 100, 100, 15),
            3,
        )
        .text("I agree");

        let edit = ControlDefinition::new(
            "WelcomeDlg",
            "NameEdit",
            ControlType::Edit,
            crate::ui::layout::DluRect::new(20, 130, 100, 15),
            3,
        )
        .text("Name");

        let prog = ControlDefinition::new(
            "WelcomeDlg",
            "InstallProgress",
            ControlType::ProgressBar,
            crate::ui::layout::DluRect::new(20, 160, 200, 15),
            3,
        );

        let sep = ControlDefinition::new(
            "WelcomeDlg",
            "SepLine",
            ControlType::Line,
            crate::ui::layout::DluRect::new(20, 180, 200, 2),
            3,
        );

        let bmp = ControlDefinition::new(
            "WelcomeDlg",
            "BmpImg",
            ControlType::Bitmap,
            crate::ui::layout::DluRect::new(20, 190, 50, 50),
            3,
        );

        let lbl = ControlDefinition::new(
            "WelcomeDlg",
            "Lbl",
            ControlType::Text,
            crate::ui::layout::DluRect::new(20, 240, 100, 14),
            3,
        )
        .text("Static Label");

        // Disabled PushButton (attribute 1: visible, not enabled)
        let disabled_btn = ControlDefinition::new(
            "WelcomeDlg",
            "DisabledBtn",
            ControlType::PushButton,
            crate::ui::layout::DluRect::new(20, 250, 50, 15),
            1,
        )
        .text("Disabled");

        engine.add_control(chk);
        engine.add_control(edit);
        engine.add_control(prog);
        engine.add_control(sep);
        engine.add_control(bmp);
        engine.add_control(lbl);
        engine.add_control(disabled_btn);

        let mut runtime = GuiDesktopRuntime::new(
            engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
        );

        let (_, widgets, _) = runtime.render_frame();
        assert_eq!(widgets.len(), 11);
        assert!(runtime.access_bridge().nodes().len() >= 7);
    }

    /// Tests applying all visual style presets.
    #[test]
    fn test_apply_style_presets() {
        let engine = create_test_engine();
        let mut runtime = GuiDesktopRuntime::new(
            engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan,
        );

        runtime.apply_style_preset(WizardStyle::InstallDir);
        assert_eq!(runtime.theme().style, WizardStyle::InstallDir);

        runtime.apply_style_preset(WizardStyle::FeatureTree);
        assert_eq!(runtime.theme().style, WizardStyle::FeatureTree);

        runtime.apply_style_preset(WizardStyle::Mondo);
        assert_eq!(runtime.theme().style, WizardStyle::Mondo);

        runtime.apply_style_preset(WizardStyle::Minimal);
        assert_eq!(runtime.theme().style, WizardStyle::Minimal);
    }

    /// Tests trait implementations for window types.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_window_types_and_traits() {
        let mut bridge = AccessKitBridge::new();
        assert_eq!(bridge.nodes().len(), 0);

        let node = AccessibleNode {
            id: 1,
            role: AccessibleRole::Dialog,
            label: "Test".to_string(),
            description: Some("Desc".to_string()),
            bounds: PixelRect::new(0, 0, 100, 100),
            is_focused: true,
            is_enabled: true,
            is_checked: Some(false),
        };
        let cloned_node = node.clone();
        assert_eq!(node, cloned_node);
        assert!(format!("{node:?}").contains("AccessibleNode"));
        assert!(format!("{:?}", AccessibleRole::Window).contains("Window"));

        bridge.update_nodes(vec![node]);
        assert_eq!(bridge.nodes().len(), 1);
        let cloned_bridge = bridge.clone();
        assert_eq!(cloned_bridge.nodes().len(), 1);
        assert!(format!("{bridge:?}").contains("AccessKitBridge"));

        let event = GuiInputEvent::MouseClick { x: 5, y: 10 };
        let cloned_event = event;
        assert_eq!(event, cloned_event);
        assert!(format!("{event:?}").contains("MouseClick"));

        let config = WindowConfig::default();
        let cloned_config = config.clone();
        assert_eq!(config, cloned_config);
        assert!(format!("{config:?}").contains("WindowConfig"));

        let engine = create_test_engine();
        let mut runtime = GuiDesktopRuntime::new(
            engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::WgpuDirectXMetalVulkan,
        );
        assert!(format!("{runtime:?}").contains("GuiDesktopRuntime"));

        // Test backend fallback cascade
        assert_eq!(
            GuiDesktopRuntime::select_backend_with_fallback(
                GuiHardwareBackend::WgpuDirectXMetalVulkan,
                true,
                true
            ),
            GuiHardwareBackend::WgpuDirectXMetalVulkan
        );
        assert_eq!(
            GuiDesktopRuntime::select_backend_with_fallback(
                GuiHardwareBackend::WgpuDirectXMetalVulkan,
                false,
                true
            ),
            GuiHardwareBackend::GlowLegacyOpenGl
        );
        assert_eq!(
            GuiDesktopRuntime::select_backend_with_fallback(
                GuiHardwareBackend::WgpuDirectXMetalVulkan,
                false,
                false
            ),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer
        );
        assert_eq!(
            GuiDesktopRuntime::select_backend_with_fallback(
                GuiHardwareBackend::GlowLegacyOpenGl,
                false,
                true
            ),
            GuiHardwareBackend::GlowLegacyOpenGl
        );
        assert_eq!(
            GuiDesktopRuntime::select_backend_with_fallback(
                GuiHardwareBackend::GlowLegacyOpenGl,
                false,
                false
            ),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer
        );
        assert_eq!(
            GuiDesktopRuntime::select_backend_with_fallback(
                GuiHardwareBackend::SoftbufferHeadlessRasterizer,
                true,
                true
            ),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer
        );

        // Test non-resizable window constraint enforcement
        assert_eq!(
            runtime.enforce_window_constraints(800, 600),
            (
                runtime.window_config().width,
                runtime.window_config().height
            )
        );

        // Test resizable window constraint enforcement
        runtime.window_config_mut().resizable = true;
        assert_eq!(runtime.enforce_window_constraints(1200, 900), (1200, 900));

        // Test lifecycle events
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::Opened),
            None
        );
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::Focused),
            None
        );
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::Unfocused),
            None
        );
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::Minimized),
            None
        );
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::Restored),
            None
        );
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::ResizeRequested {
                width: 1024,
                height: 768
            }),
            None
        );
        assert_eq!(
            runtime.process_lifecycle_event(WindowLifecycleEvent::CloseRequested),
            Some(DialogReturnCode::Exit)
        );

        // Test CloseRequested when cancel button has registered event
        let mut cancel_engine = create_test_engine();
        let _ = cancel_engine.set_active_dialog("WelcomeDlg");
        cancel_engine.add_event(crate::ui::events::ControlEvent::new(
            "WelcomeDlg",
            "CancelButton",
            crate::ui::events::ControlEventType::EndDialog(DialogReturnCode::Retry),
            None,
            0,
        ));
        let mut cancel_runtime = GuiDesktopRuntime::new(
            cancel_engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
        );
        assert_eq!(
            cancel_runtime.process_lifecycle_event(WindowLifecycleEvent::CloseRequested),
            Some(DialogReturnCode::Retry)
        );

        // Test CloseRequested when active dialog has NO cancel control
        let mut no_cancel_engine =
            UiEngine::new(crate::execution::properties::EvaluationContext::new());
        no_cancel_engine.add_dialog(DialogDefinition {
            name: "NoCancelDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: crate::ui::engine::DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        });
        let _ = no_cancel_engine.set_active_dialog("NoCancelDlg");
        let mut no_cancel_runtime = GuiDesktopRuntime::new(
            no_cancel_engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
        );
        assert_eq!(
            no_cancel_runtime.process_lifecycle_event(WindowLifecycleEvent::CloseRequested),
            Some(DialogReturnCode::Exit)
        );

        // Test CloseRequested when cancel button does not exist (click_control returns Err)
        let mut err_cancel_engine =
            UiEngine::new(crate::execution::properties::EvaluationContext::new());
        err_cancel_engine.add_dialog(DialogDefinition {
            name: "ErrCancelDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: crate::ui::engine::DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: Some("MissingCancelBtn".to_string()),
        });
        let _ = err_cancel_engine.set_active_dialog("ErrCancelDlg");
        let mut err_cancel_runtime = GuiDesktopRuntime::new(
            err_cancel_engine,
            WizardTheme::mondo(),
            GuiHardwareBackend::SoftbufferHeadlessRasterizer,
        );
        assert_eq!(
            err_cancel_runtime.process_lifecycle_event(WindowLifecycleEvent::CloseRequested),
            Some(DialogReturnCode::Exit)
        );

        let lc_ev = WindowLifecycleEvent::ResizeRequested {
            width: 500,
            height: 400,
        };
        assert_eq!(lc_ev, lc_ev.clone());
        assert!(format!("{lc_ev:?}").contains("ResizeRequested"));
    }
}
