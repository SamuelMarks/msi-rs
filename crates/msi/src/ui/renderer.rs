//! Immediate-Mode GUI Renderer & Multi-Backend Display System (`msi-gui-egui`).
//!
//! Grounded directly in `egui` immediate mode UI architecture and Windows Installer SDK specifications:
//! - Absolute coordinate positioning via `ui.put(rect, widget)` mapping MSI dialog units directly to pixels.
//! - Multi-Backend Display Support:
//!   - `GpuAccelerated`: Native GPU acceleration via `eframe` (wgpu / glow) on macOS, Linux, and Windows.
//!   - `SoftwareRasterizer`: Fallback software frame buffer (`tiny-skia` / `softbuffer`) for environments lacking GPU.
//!   - `TerminalTui`: Terminal wizard for SSH and headless consoles.
//! - High-fidelity layout mapping for `WiX` Mondo, `InstallDir`, and `FeatureTree` wizards.

use crate::ui::controls::ControlType;
use crate::ui::engine::UiEngine;
use crate::ui::layout::{FontMetrics, PixelRect};
use crate::ui::theme::{Color32, WizardTheme};

/// Display rendering backend type for multi-platform presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisplayBackendType {
    /// Hardware GPU acceleration via `eframe` (wgpu / glow) on macOS, Linux, Windows.
    #[default]
    GpuAccelerated,
    /// Software rasterizer (`tiny-skia` / `softbuffer`) for headless or server environments.
    SoftwareRasterizer,
    /// Interactive terminal text wizard (curses / `ratatui`) for SSH consoles.
    TerminalTui,
}

impl DisplayBackendType {
    /// Returns the display backend identifier name.
    ///
    /// # Returns
    ///
    /// Computed `&'static str` value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuAccelerated => "gpu-accelerated (eframe/wgpu)",
            Self::SoftwareRasterizer => "software-rasterizer (softbuffer)",
            Self::TerminalTui => "terminal-tui (ratatui)",
        }
    }
}

/// Abstract immediate-mode widget type placed at absolute pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum UiWidget {
    /// Push button with display text and default styling.
    Button {
        /// Button label.
        text: String,
        /// Whether this button is default for Enter key.
        is_default: bool,
        /// Whether button is enabled.
        is_enabled: bool,
    },
    /// Text label with font size and color.
    Label {
        /// Label text.
        text: String,
        /// Font size in pixels.
        font_size: f32,
        /// Text color.
        color: Color32,
    },
    /// Check box with checked state.
    CheckBox {
        /// Label text.
        text: String,
        /// Checked flag.
        checked: bool,
        /// Enabled flag.
        is_enabled: bool,
    },
    /// Text edit input box.
    Edit {
        /// Text value.
        text: String,
        /// Whether masked as password.
        is_password: bool,
        /// Enabled flag.
        is_enabled: bool,
    },
    /// Progress bar with percentage complete (0.0 to 1.0).
    ProgressBar {
        /// Fraction complete (0.0 to 1.0).
        fraction: f32,
    },
    /// Separator line.
    Separator {
        /// Color.
        color: Color32,
    },
    /// Banner image placeholder.
    Image {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
    },
    /// Scrollable text viewport for RTF or plain-text license agreements.
    ScrollableText {
        /// Content text.
        text: String,
        /// Current scroll offset in lines.
        scroll_offset: usize,
        /// Total lines in text.
        total_lines: usize,
        /// Whether content is Rich Text Format (RTF).
        is_rtf: bool,
    },
    /// Hierarchical tree control for feature selection.
    SelectionTree {
        /// Tree nodes.
        nodes: Vec<crate::ui::controls::SelectionTreeNode>,
    },
    /// Volume cost table listing disk space usage per mount/drive.
    VolumeCostList {
        /// Volume cost entries.
        entries: Vec<crate::ui::controls::VolumeCostEntry>,
    },
    /// Path edit field paired with directory picker.
    PathEdit {
        /// Selected directory path.
        path: String,
        /// Enabled flag.
        is_enabled: bool,
    },
    /// Radio button option within a group.
    RadioButton {
        /// Display text.
        text: String,
        /// Selected state.
        selected: bool,
        /// Enabled flag.
        is_enabled: bool,
        /// Group identifier.
        group: String,
        /// Radio button value.
        value: String,
    },
}

/// Low-level 2D drawing primitive instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// Solid filled rectangle.
    FillRect {
        /// Destination pixel rectangle.
        rect: PixelRect,
        /// Fill color.
        color: Color32,
    },
    /// Outlined rectangle.
    StrokeRect {
        /// Destination pixel rectangle.
        rect: PixelRect,
        /// Outline color.
        color: Color32,
        /// Stroke width.
        width: f32,
    },
    /// Rendered text string.
    DrawText {
        /// (X, Y) start position.
        pos: (i32, i32),
        /// String text.
        text: String,
        /// Text color.
        color: Color32,
        /// Font size.
        font_size: f32,
    },
    /// Solid line segment.
    DrawLine {
        /// Start coordinate.
        start: (i32, i32),
        /// End coordinate.
        end: (i32, i32),
        /// Line color.
        color: Color32,
        /// Stroke width.
        width: f32,
    },
}

/// Layout mapper generating immediate-mode `ui.put(rect, widget)` instructions from active dialogs.
#[derive(Debug, Clone)]
pub struct EguiLayoutMapper {
    /// Font metrics for DLU-to-pixel conversions.
    font_metrics: FontMetrics,
    /// Active wizard styling theme.
    theme: WizardTheme,
}

impl EguiLayoutMapper {
    /// Creates a new [`EguiLayoutMapper`].
    ///
    /// # Arguments
    ///
    /// * `theme` - Visual wizard theme.
    /// * `font_metrics` - Sizing metrics.
    ///
    /// # Returns
    ///
    /// A new [`EguiLayoutMapper`].
    #[must_use]
    pub const fn new(theme: WizardTheme, font_metrics: FontMetrics) -> Self {
        Self {
            font_metrics,
            theme,
        }
    }

    /// Generates absolute coordinate widget placements and drawing commands for the active dialog.
    ///
    /// # Arguments
    ///
    /// * `engine` - Headless UI engine with active dialog.
    /// * `container_width` - Parent window width.
    /// * `container_height` - Parent window height.
    ///
    /// # Returns
    ///
    /// Tuple of `(dialog_pixel_bounds, widget_placements, draw_commands)`.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn map_active_dialog(
        &self,
        engine: &UiEngine,
        container_width: i32,
        container_height: i32,
    ) -> (PixelRect, Vec<(PixelRect, UiWidget)>, Vec<DrawCommand>) {
        let Some(dialog) = engine.active_dialog() else {
            return (PixelRect::default(), Vec::new(), Vec::new());
        };

        let dialog_bounds =
            dialog.pixel_bounds(container_width, container_height, &self.font_metrics);
        let mut widgets = Vec::new();
        let mut draws = Vec::new();

        // 1. Draw dialog window background
        draws.push(DrawCommand::FillRect {
            rect: dialog_bounds,
            color: self.theme.window_bg,
        });

        // 2. Draw top banner area if styled
        let banner_rect = PixelRect::new(
            dialog_bounds.x,
            dialog_bounds.y,
            dialog_bounds.width,
            i32::try_from(self.theme.banner_height).unwrap_or(58),
        );
        draws.push(DrawCommand::FillRect {
            rect: banner_rect,
            color: self.theme.banner_bg,
        });
        draws.push(DrawCommand::DrawLine {
            start: (banner_rect.x, banner_rect.y + banner_rect.height),
            end: (
                banner_rect.x + banner_rect.width,
                banner_rect.y + banner_rect.height,
            ),
            color: Color32::LINE_GRAY,
            width: 1.0,
        });

        // 3. Draw dialog title in banner
        if let Some(ref title) = dialog.title {
            draws.push(DrawCommand::DrawText {
                pos: (dialog_bounds.x + 16, dialog_bounds.y + 20),
                text: title.clone(),
                color: Color32::BLACK,
                font_size: self.theme.title_font_size,
            });
        }

        // 4. Map each control from DLU to relative/absolute pixel coordinate
        let control_defs = engine.get_dialog_controls(&dialog.name);
        for def in control_defs {
            let ctrl_pixel_rel = def.rect().to_pixel_rect(&self.font_metrics);
            let ctrl_bounds = PixelRect::new(
                dialog_bounds.x + ctrl_pixel_rel.x,
                dialog_bounds.y + ctrl_pixel_rel.y,
                ctrl_pixel_rel.width,
                ctrl_pixel_rel.height,
            );

            let state = engine.get_control_state(&dialog.name, def.control());
            let (is_visible, is_enabled, current_text) = state.map_or_else(
                || {
                    (
                        def.is_visible_by_default(),
                        def.is_enabled_by_default(),
                        def.text_template().unwrap_or("").to_string(),
                    )
                },
                |s| (s.is_visible, s.is_enabled, s.current_text.clone()),
            );

            if !is_visible {
                continue;
            }

            let widget = match def.control_type() {
                ControlType::PushButton => {
                    let clean_label = crate::ui::font_rasterizer::strip_accelerator(&current_text);
                    let is_default = state.is_some_and(|s| s.is_default);
                    let (bg_col, border_col, txt_col) = if !is_enabled {
                        (
                            Color32::from_rgb(240, 242, 246),
                            Color32::LINE_GRAY,
                            Color32::from_rgb(155, 162, 174),
                        )
                    } else if is_default {
                        (Color32::ACCENT_BLUE, Color32::ACCENT_BLUE, Color32::WHITE)
                    } else {
                        (Color32::WHITE, Color32::LINE_GRAY, Color32::BLACK)
                    };
                    draws.push(DrawCommand::FillRect {
                        rect: ctrl_bounds,
                        color: bg_col,
                    });
                    draws.push(DrawCommand::StrokeRect {
                        rect: ctrl_bounds,
                        color: border_col,
                        width: 1.0,
                    });
                    draws.push(DrawCommand::DrawText {
                        pos: (ctrl_bounds.x + 8, ctrl_bounds.y + 4),
                        text: clean_label.clone(),
                        color: txt_col,
                        font_size: self.theme.base_font_size,
                    });
                    UiWidget::Button {
                        text: clean_label,
                        is_default,
                        is_enabled,
                    }
                }
                ControlType::CheckBox => {
                    let clean_label = crate::ui::font_rasterizer::strip_accelerator(&current_text);
                    let checked = state.is_some_and(|s| s.bound_value.as_deref() == Some("1"));
                    let check_box_rect = PixelRect::new(ctrl_bounds.x, ctrl_bounds.y + 2, 12, 12);
                    draws.push(DrawCommand::StrokeRect {
                        rect: check_box_rect,
                        color: Color32::BLACK,
                        width: 1.0,
                    });
                    if checked {
                        draws.push(DrawCommand::FillRect {
                            rect: PixelRect::new(ctrl_bounds.x + 2, ctrl_bounds.y + 4, 8, 8),
                            color: Color32::ACCENT_BLUE,
                        });
                    }
                    draws.push(DrawCommand::DrawText {
                        pos: (ctrl_bounds.x + 16, ctrl_bounds.y + 2),
                        text: clean_label.clone(),
                        color: self.theme.text_color,
                        font_size: self.theme.base_font_size,
                    });
                    UiWidget::CheckBox {
                        text: clean_label,
                        checked,
                        is_enabled,
                    }
                }
                ControlType::Edit | ControlType::MaskedEdit | ControlType::PathEdit => {
                    let prop = def.property_name().unwrap_or("");
                    let is_path_edit = def.control_type() == ControlType::PathEdit
                        || prop.ends_with("DIR")
                        || prop.ends_with("FOLDER")
                        || prop.ends_with("PATH");
                    draws.push(DrawCommand::FillRect {
                        rect: ctrl_bounds,
                        color: Color32::WHITE,
                    });
                    if is_path_edit {
                        draws.push(DrawCommand::StrokeRect {
                            rect: ctrl_bounds,
                            color: Color32::LINE_GRAY,
                            width: 1.0,
                        });
                        draws.push(DrawCommand::DrawText {
                            pos: (ctrl_bounds.x + 4, ctrl_bounds.y + 4),
                            text: current_text.clone(),
                            color: Color32::BLACK,
                            font_size: self.theme.base_font_size,
                        });
                        UiWidget::PathEdit {
                            path: current_text,
                            is_enabled,
                        }
                    } else {
                        let is_pwd = def.is_password_input();
                        draws.push(DrawCommand::StrokeRect {
                            rect: ctrl_bounds,
                            color: Color32::LINE_GRAY,
                            width: 1.0,
                        });
                        let display_text = if is_pwd {
                            "*".repeat(current_text.len())
                        } else {
                            current_text.clone()
                        };
                        draws.push(DrawCommand::DrawText {
                            pos: (ctrl_bounds.x + 4, ctrl_bounds.y + 4),
                            text: display_text,
                            color: Color32::BLACK,
                            font_size: self.theme.base_font_size,
                        });
                        UiWidget::Edit {
                            text: current_text,
                            is_password: is_pwd,
                            is_enabled,
                        }
                    }
                }
                ControlType::ProgressBar => {
                    let pct = state.map_or(0, |s| s.progress_percent);
                    let frac = f32::from(u16::try_from(pct).unwrap_or(0)) / 100.0;
                    draws.push(DrawCommand::FillRect {
                        rect: ctrl_bounds,
                        color: Color32::from_rgb(230, 234, 240),
                    });
                    draws.push(DrawCommand::StrokeRect {
                        rect: ctrl_bounds,
                        color: Color32::LINE_GRAY,
                        width: 1.0,
                    });
                    let fill_w = ((ctrl_bounds.width as f32) * frac) as i32;
                    if fill_w > 0 {
                        let fill_rect = PixelRect::new(
                            ctrl_bounds.x,
                            ctrl_bounds.y,
                            fill_w,
                            ctrl_bounds.height,
                        );
                        draws.push(DrawCommand::FillRect {
                            rect: fill_rect,
                            color: Color32::ACCENT_BLUE,
                        });
                    }
                    if !engine.action_log().is_empty() {
                        let log_box = PixelRect::new(
                            ctrl_bounds.x,
                            ctrl_bounds.y + ctrl_bounds.height + 24,
                            ctrl_bounds.width,
                            110,
                        );
                        draws.push(DrawCommand::FillRect {
                            rect: log_box,
                            color: Color32::WHITE,
                        });
                        draws.push(DrawCommand::StrokeRect {
                            rect: log_box,
                            color: Color32::LINE_GRAY,
                            width: 1.0,
                        });
                        for (idx, entry) in engine.action_log().iter().take(5).enumerate() {
                            draws.push(DrawCommand::DrawText {
                                pos: (log_box.x + 8, log_box.y + 6 + (idx as i32 * 18)),
                                text: entry.clone(),
                                color: self.theme.text_color,
                                font_size: self.theme.base_font_size,
                            });
                        }
                    }
                    UiWidget::ProgressBar { fraction: frac }
                }
                ControlType::Line => {
                    draws.push(DrawCommand::DrawLine {
                        start: (ctrl_bounds.x, ctrl_bounds.y),
                        end: (
                            ctrl_bounds.x + ctrl_bounds.width,
                            ctrl_bounds.y + ctrl_bounds.height,
                        ),
                        color: Color32::LINE_GRAY,
                        width: 1.0,
                    });
                    UiWidget::Separator {
                        color: Color32::LINE_GRAY,
                    }
                }
                ControlType::Bitmap | ControlType::Icon => {
                    if def.property_name() == Some("WixUI_Bmp_Dialog") || ctrl_bounds.height > 200 {
                        let sidebar_w = 180.min(dialog_bounds.width / 3);
                        let sidebar_h = (dialog_bounds.height - 46).max(60);
                        draws.push(DrawCommand::FillRect {
                            rect: PixelRect::new(
                                dialog_bounds.x,
                                dialog_bounds.y,
                                sidebar_w,
                                sidebar_h,
                            ),
                            color: Color32::from_rgb(11, 50, 112),
                        });
                        draws.push(DrawCommand::FillRect {
                            rect: PixelRect::new(
                                dialog_bounds.x,
                                dialog_bounds.y,
                                sidebar_w,
                                sidebar_h / 3,
                            ),
                            color: Color32::from_rgb(82, 148, 226),
                        });
                        draws.push(DrawCommand::DrawText {
                            pos: (dialog_bounds.x + 16, dialog_bounds.y + (sidebar_h / 3) + 20),
                            text: "msi-rs".to_string(),
                            color: Color32::WHITE,
                            font_size: self.theme.title_font_size,
                        });
                    } else {
                        let icon_rect = PixelRect::new(
                            dialog_bounds.x + dialog_bounds.width - 52,
                            dialog_bounds.y + 12,
                            34,
                            34,
                        );
                        draws.push(DrawCommand::FillRect {
                            rect: icon_rect,
                            color: Color32::ACCENT_BLUE,
                        });
                    }
                    UiWidget::Image {
                        width: u32::try_from(ctrl_bounds.width).unwrap_or(0),
                        height: u32::try_from(ctrl_bounds.height).unwrap_or(0),
                    }
                }
                ControlType::ScrollableText => {
                    let is_rtf = current_text.starts_with(r"{\rtf");
                    let clean_doc = if is_rtf {
                        crate::ui::font_rasterizer::strip_rtf_to_plaintext(&current_text)
                    } else {
                        current_text.clone()
                    };
                    let total_lines = clean_doc.lines().count();
                    draws.push(DrawCommand::FillRect {
                        rect: ctrl_bounds,
                        color: Color32::WHITE,
                    });
                    draws.push(DrawCommand::StrokeRect {
                        rect: ctrl_bounds,
                        color: Color32::LINE_GRAY,
                        width: 1.0,
                    });
                    for (line_idx, line) in clean_doc
                        .lines()
                        .take(ctrl_bounds.height as usize / 16)
                        .enumerate()
                    {
                        draws.push(DrawCommand::DrawText {
                            pos: (
                                ctrl_bounds.x + 4,
                                ctrl_bounds.y + 4 + (line_idx as i32 * 16),
                            ),
                            text: line.to_string(),
                            color: Color32::BLACK,
                            font_size: self.theme.base_font_size,
                        });
                    }
                    UiWidget::ScrollableText {
                        text: clean_doc,
                        scroll_offset: 0,
                        total_lines,
                        is_rtf,
                    }
                }
                ControlType::SelectionTree => {
                    draws.push(DrawCommand::FillRect {
                        rect: ctrl_bounds,
                        color: Color32::WHITE,
                    });
                    draws.push(DrawCommand::StrokeRect {
                        rect: ctrl_bounds,
                        color: Color32::LINE_GRAY,
                        width: 1.0,
                    });
                    let nodes = state.map_or_else(Vec::new, |s| s.tree_nodes.clone());
                    let max_rows = (ctrl_bounds.height as usize / 20).max(1);
                    let mut flat_rows = Vec::new();
                    for node in &nodes {
                        flat_rows.push((0i32, node));
                        for child in &node.children {
                            flat_rows.push((14i32, child));
                        }
                    }
                    for (row_idx, (indent, item)) in
                        flat_rows.into_iter().take(max_rows).enumerate()
                    {
                        let mark = if item.install_state
                            == crate::execution::properties::InstallState::Local
                        {
                            "[X]"
                        } else {
                            "[ ]"
                        };
                        draws.push(DrawCommand::DrawText {
                            pos: (
                                ctrl_bounds.x + 8 + indent,
                                ctrl_bounds.y + 4 + (row_idx as i32 * 20),
                            ),
                            text: format!("├─ {mark} {}", item.title),
                            color: Color32::BLACK,
                            font_size: self.theme.base_font_size,
                        });
                    }
                    UiWidget::SelectionTree { nodes }
                }
                ControlType::VolumeCostList => {
                    draws.push(DrawCommand::FillRect {
                        rect: ctrl_bounds,
                        color: Color32::WHITE,
                    });
                    draws.push(DrawCommand::StrokeRect {
                        rect: ctrl_bounds,
                        color: Color32::LINE_GRAY,
                        width: 1.0,
                    });
                    let entries = state.map_or_else(Vec::new, |s| s.volume_entries.clone());
                    draws.push(DrawCommand::DrawText {
                        pos: (ctrl_bounds.x + 4, ctrl_bounds.y + 4),
                        text: "Volume | Disk Size | Required | Available".to_string(),
                        color: Color32::BLACK,
                        font_size: self.theme.base_font_size,
                    });
                    UiWidget::VolumeCostList { entries }
                }
                ControlType::RadioButtonGroup => {
                    let clean_label = if current_text.is_empty()
                        && def.property_name() == Some("LicenseAccepted")
                    {
                        "I accept the terms in the License Agreement".to_string()
                    } else {
                        crate::ui::font_rasterizer::strip_accelerator(&current_text)
                    };
                    let is_selected = state.is_some_and(|s| {
                        s.bound_value.as_deref() == Some(&current_text)
                            || s.bound_value.as_deref() == Some("1")
                    });
                    let radio_mark = if is_selected { "(•) " } else { "( ) " };
                    draws.push(DrawCommand::DrawText {
                        pos: (ctrl_bounds.x, ctrl_bounds.y + 2),
                        text: format!("{radio_mark}{clean_label}"),
                        color: self.theme.text_color,
                        font_size: self.theme.base_font_size,
                    });
                    UiWidget::RadioButton {
                        text: clean_label.clone(),
                        selected: is_selected,
                        is_enabled,
                        group: def.control().to_string(),
                        value: clean_label,
                    }
                }
                ControlType::Billboard
                | ControlType::Text
                | ControlType::GroupBox
                | ControlType::ComboBox
                | ControlType::DirectoryCombo
                | ControlType::VolumeSelectCombo
                | ControlType::ListBox
                | ControlType::DirectoryList
                | ControlType::ListView => {
                    let (clean_text, _is_bold, size_override) =
                        crate::ui::font_rasterizer::strip_wix_font_tags(&current_text);
                    let font_size = size_override.unwrap_or(self.theme.base_font_size);
                    if def.control() == "Box" {
                        draws.push(DrawCommand::StrokeRect {
                            rect: ctrl_bounds,
                            color: Color32::LINE_GRAY,
                            width: 1.0,
                        });
                    }
                    draws.push(DrawCommand::DrawText {
                        pos: (ctrl_bounds.x, ctrl_bounds.y + 2),
                        text: clean_text.clone(),
                        color: self.theme.text_color,
                        font_size,
                    });
                    UiWidget::Label {
                        text: clean_text,
                        font_size,
                        color: self.theme.text_color,
                    }
                }
            };

            widgets.push((ctrl_bounds, widget));
        }

        (dialog_bounds, widgets, draws)
    }
}

/// Software frame buffer rasterizer providing pure software rendering without GPU or display server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareBuffer {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// RGBA pixel buffer (size = width * height).
    pub pixels: Vec<Color32>,
}

impl SoftwareBuffer {
    /// Creates a new [`SoftwareBuffer`] initialized to transparent.
    ///
    /// # Arguments
    ///
    /// * `width` - Buffer width in pixels.
    /// * `height` - Buffer height in pixels.
    ///
    /// # Returns
    ///
    /// A new [`SoftwareBuffer`].
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![Color32::TRANSPARENT; size],
        }
    }

    /// Clears the entire buffer with a solid background color.
    ///
    /// # Arguments
    ///
    /// * `color` - RGBA or RGB color value.
    pub fn clear(&mut self, color: Color32) {
        self.pixels.fill(color);
    }

    /// Fills a rectangular region with solid color with clipping boundaries.
    ///
    /// # Arguments
    ///
    /// * `rect` - Bounding rectangle coordinates.
    /// * `color` - RGBA or RGB color value.
    #[allow(clippy::cast_sign_loss)]
    pub fn fill_rect(&mut self, rect: PixelRect, color: Color32) {
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = ((rect.x + rect.width).max(0) as u32).min(self.width);
        let y1 = ((rect.y + rect.height).max(0) as u32).min(self.height);

        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y as usize) * (self.width as usize) + (x as usize);
                self.pixels[idx] = color;
            }
        }
    }

    /// Draws an outline rectangle.
    ///
    /// # Arguments
    ///
    /// * `rect` - Bounding rectangle coordinates.
    /// * `color` - RGBA or RGB color value.
    pub fn stroke_rect(&mut self, rect: PixelRect, color: Color32) {
        let top = PixelRect::new(rect.x, rect.y, rect.width, 1);
        let bottom = PixelRect::new(rect.x, rect.y + rect.height - 1, rect.width, 1);
        let left = PixelRect::new(rect.x, rect.y, 1, rect.height);
        let right = PixelRect::new(rect.x + rect.width - 1, rect.y, 1, rect.height);

        self.fill_rect(top, color);
        self.fill_rect(bottom, color);
        self.fill_rect(left, color);
        self.fill_rect(right, color);
    }

    /// Returns the pixel color at coordinates (x, y) if within bounds.
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal coordinate in dialog units or pixels.
    /// * `y` - Vertical coordinate in dialog units or pixels.
    ///
    /// # Returns
    ///
    /// `Some(Color32)` if found, or `None` if absent.
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color32> {
        if x < self.width && y < self.height {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            self.pixels.get(idx).copied()
        } else {
            None
        }
    }

    /// Renders a list of [`DrawCommand`] instructions into the software buffer.
    ///
    /// # Arguments
    ///
    /// * `commands` - The commands value.
    pub fn render_commands(&mut self, commands: &[DrawCommand]) {
        for cmd in commands {
            match cmd {
                DrawCommand::FillRect { rect, color } => self.fill_rect(*rect, *color),
                DrawCommand::StrokeRect { rect, color, .. } => self.stroke_rect(*rect, *color),
                DrawCommand::DrawLine {
                    start, end, color, ..
                } => {
                    let min_x = start.0.min(end.0);
                    let min_y = start.1.min(end.1);
                    let w = (end.0 - start.0).abs().max(1);
                    let h = (end.1 - start.1).abs().max(1);
                    self.fill_rect(PixelRect::new(min_x, min_y, w, h), *color);
                }
                DrawCommand::DrawText {
                    pos,
                    text,
                    color,
                    font_size,
                } => {
                    let style = crate::ui::font_rasterizer::TextStyleDefinition::new(
                        "Segoe UI", *font_size, *color, 0,
                    );
                    crate::ui::font_rasterizer::FontVectorRasterizer::render_text(
                        self, text, &style, pos.0, pos.1, None,
                    );
                }
            }
        }
    }

    /// Encodes the software buffer into a standard 8-bit RGBA PNG image byte vector.
    ///
    /// # Returns
    ///
    /// Encoded PNG bytes on success.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MsiError`] if zlib compression fails.
    #[must_use]
    pub fn to_png_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&self.width.to_be_bytes());
        ihdr.extend_from_slice(&self.height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        write_png_chunk(&mut out, *b"IHDR", &ihdr);

        let row_bytes = (self.width as usize) * 4 + 1;
        let mut raw_data = Vec::with_capacity(row_bytes * (self.height as usize));
        for y in 0..self.height {
            raw_data.push(0); // Filter type 0 (None)
            for x in 0..self.width {
                let idx = (y as usize) * (self.width as usize) + (x as usize);
                let px = self
                    .pixels
                    .get(idx)
                    .copied()
                    .unwrap_or(Color32::TRANSPARENT);
                raw_data.extend_from_slice(&[px.r, px.g, px.b, px.a]);
            }
        }

        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw_data, 6);
        write_png_chunk(&mut out, *b"IDAT", &compressed);
        write_png_chunk(&mut out, *b"IEND", &[]);

        out
    }

    /// Writes the software buffer directly to a PNG file on disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MsiError`] if file I/O fails.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> crate::error::Result<()> {
        let bytes = self.to_png_bytes();
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

/// Writes a single PNG chunk (`length || type || data || crc32`) to `out`.
///
/// # Arguments
///
/// * `out` - Output buffer.
/// * `chunk_type` - 4-byte ASCII chunk identifier.
/// * `data` - Chunk payload bytes.
#[allow(clippy::cast_possible_truncation)]
fn write_png_chunk(out: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&chunk_type);
    out.extend_from_slice(data);
    let mut crc_buf = Vec::with_capacity(4 + data.len());
    crc_buf.extend_from_slice(&chunk_type);
    crc_buf.extend_from_slice(data);
    let crc = crate::cab::csum::compute_crc32(&crc_buf);
    out.extend_from_slice(&crc.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::properties::EvaluationContext;
    use crate::ui::controls::ControlDefinition;
    use crate::ui::engine::{DialogDefinition, DIALOG_ATTR_VISIBLE};
    use crate::ui::layout::DluRect;

    /// Tests `DisplayBackendType` identifiers and representations.
    #[test]
    fn test_display_backends() {
        assert_eq!(
            DisplayBackendType::GpuAccelerated.as_str(),
            "gpu-accelerated (eframe/wgpu)"
        );
        assert_eq!(
            DisplayBackendType::SoftwareRasterizer.as_str(),
            "software-rasterizer (softbuffer)"
        );
        assert_eq!(
            DisplayBackendType::TerminalTui.as_str(),
            "terminal-tui (ratatui)"
        );
        assert_eq!(
            DisplayBackendType::default(),
            DisplayBackendType::GpuAccelerated
        );
    }

    /// Tests `EguiLayoutMapper` mapping active dialog controls to absolute pixel coordinates.
    #[test]
    fn test_layout_mapper() {
        let theme = WizardTheme::mondo();
        let font_metrics = FontMetrics::default();
        let mapper = EguiLayoutMapper::new(theme, font_metrics);

        let mut engine = UiEngine::new(EvaluationContext::new());
        engine.add_dialog(DialogDefinition {
            name: "TestDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Test Title".to_string()),
            control_first: "Btn1".to_string(),
            control_default: Some("Btn1".to_string()),
            control_cancel: None,
        });

        engine.add_control(
            ControlDefinition::new(
                "TestDlg",
                "Btn1",
                ControlType::PushButton,
                DluRect::new(10, 20, 50, 15),
                3,
            )
            .text("Click Me"),
        );

        assert!(engine.set_active_dialog("TestDlg").is_ok());

        let (bounds, widgets, draws) = mapper.map_active_dialog(&engine, 1024, 768);
        assert!(bounds.width > 0);
        assert!(bounds.height > 0);
        assert_eq!(widgets.len(), 1);
        assert_ne!(draws.len(), 0);

        // Check button widget placement
        let (btn_rect, btn_widget) = &widgets[0];
        assert_eq!(btn_rect.x, bounds.x + 20); // 10 DLUs * 2px
        assert_eq!(
            btn_widget,
            &UiWidget::Button {
                text: "Click Me".to_string(),
                is_default: false,
                is_enabled: true,
            }
        );
    }

    /// Tests `EguiLayoutMapper` when there is no active dialog or when dialog has no title.
    #[test]
    fn test_layout_mapper_no_dialog_and_no_title() {
        let mapper = EguiLayoutMapper::new(WizardTheme::mondo(), FontMetrics::default());
        let mut engine = UiEngine::new(EvaluationContext::new());

        // 1. No active dialog
        let (bounds, widgets, draws) = mapper.map_active_dialog(&engine, 800, 600);
        assert_eq!(bounds, PixelRect::default());
        assert_eq!(widgets.len(), 0);
        assert_eq!(draws.len(), 0);

        // 2. Dialog with no title
        engine.add_dialog(DialogDefinition {
            name: "NoTitleDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 150,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        });
        assert!(engine.set_active_dialog("NoTitleDlg").is_ok());
        let (bounds2, widgets2, draws2) = mapper.map_active_dialog(&engine, 800, 600);
        assert!(bounds2.width > 0);
        assert_eq!(widgets2.len(), 0);
        assert_ne!(draws2.len(), 0);
    }

    /// Helper to populate a selection tree node on a dialog control.
    ///
    /// # Arguments
    ///
    /// * `engine` - Active UI engine.
    /// * `dlg` - Dialog identifier name.
    /// * `ctrl` - Control identifier name.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or [`MsiError::Validation`] if the control state is missing.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if the specified control does not exist.
    #[allow(clippy::or_fun_call)]
    fn add_selection_tree_node(
        engine: &mut UiEngine,
        dlg: &str,
        ctrl: &str,
    ) -> crate::error::Result<()> {
        let tre_state =
            engine
                .get_control_state_mut(dlg, ctrl)
                .ok_or(crate::error::MsiError::Validation {
                    element: "Tre".to_string(),
                    reason: "missing state".to_string(),
                })?;
        tre_state
            .tree_nodes
            .push(crate::ui::controls::SelectionTreeNode::new(
                "Feat1",
                "Main Feature",
                1024,
            ));
        Ok(())
    }

    /// Tests mapping all control types (`CheckBox`, `Edit`, `ProgressBar`, `Line`, `Bitmap`, `Text`, and invisible controls).
    ///
    /// # Returns
    ///
    /// Computed `crate::error::Result<()>` value.
    #[test]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::unnecessary_wraps)]
    fn test_layout_mapper_all_control_variants() -> crate::error::Result<()> {
        let mapper = EguiLayoutMapper::new(WizardTheme::mondo(), FontMetrics::default());
        let mut engine = UiEngine::new(EvaluationContext::new());

        engine.add_dialog(DialogDefinition {
            name: "ControlsDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 400,
            height: 300,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Controls".to_string()),
            control_first: "Cb1".to_string(),
            control_default: None,
            control_cancel: None,
        });

        // CheckBox (attribute 3 = visible + enabled)
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "Cb1",
                ControlType::CheckBox,
                DluRect::new(10, 10, 100, 14),
                3,
            )
            .text("Accept Terms")
            .property("ACCEPT"),
        );

        // Edit masked password (attribute 0x0200)
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "Pwd",
                ControlType::Edit,
                DluRect::new(10, 30, 100, 14),
                3 | 0x0200,
            )
            .text("secret"),
        );

        // ProgressBar
        engine.add_control(ControlDefinition::new(
            "ControlsDlg",
            "Prog",
            ControlType::ProgressBar,
            DluRect::new(10, 50, 200, 14),
            3,
        ));

        // Separator Line
        engine.add_control(ControlDefinition::new(
            "ControlsDlg",
            "Sep",
            ControlType::Line,
            DluRect::new(10, 70, 200, 2),
            3,
        ));

        // Bitmap image placeholder
        engine.add_control(ControlDefinition::new(
            "ControlsDlg",
            "Bmp",
            ControlType::Bitmap,
            DluRect::new(10, 80, 50, 50),
            3,
        ));

        // Text label
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "Lbl",
                ControlType::Text,
                DluRect::new(10, 140, 100, 14),
                3,
            )
            .text("Static Description"),
        );

        // PushButton
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "OkBtn",
                ControlType::PushButton,
                DluRect::new(10, 160, 50, 15),
                3,
            )
            .text("OK"),
        );

        // Multi-pattern label variants
        let extra_types = [
            ("Rbg", ControlType::RadioButtonGroup),
            ("Cmb", ControlType::ComboBox),
            ("Lst", ControlType::ListBox),
            ("Lsv", ControlType::ListView),
            ("Scr", ControlType::ScrollableText),
            ("Vol", ControlType::VolumeCostList),
            ("Tre", ControlType::SelectionTree),
        ];
        for (name, ctype) in extra_types {
            engine.add_control(
                ControlDefinition::new(
                    "ControlsDlg",
                    name,
                    ctype,
                    DluRect::new(10, 180, 100, 14),
                    3,
                )
                .text("Item"),
            );
        }

        // Invisible control (attribute 0 = not visible)
        engine.add_control(ControlDefinition::new(
            "ControlsDlg",
            "Hidden",
            ControlType::PushButton,
            DluRect::new(10, 200, 50, 15),
            0,
        ));

        // PathEdit variants (Edit controls with property ending in DIR, FOLDER, PATH)
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "EditDir",
                ControlType::Edit,
                DluRect::new(10, 220, 100, 14),
                3,
            )
            .property("INSTALLDIR")
            .text("C:\\Program Files\\App"),
        );
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "EditFolder",
                ControlType::Edit,
                DluRect::new(10, 240, 100, 14),
                3,
            )
            .property("TARGETFOLDER")
            .text("C:\\Target"),
        );
        engine.add_control(
            ControlDefinition::new(
                "ControlsDlg",
                "EditPath",
                ControlType::Edit,
                DluRect::new(10, 260, 100, 14),
                3,
            )
            .property("MYPATH")
            .text("C:\\Path"),
        );

        assert!(add_selection_tree_node(&mut engine, "ControlsDlg", "Tre").is_ok());
        assert!(add_selection_tree_node(&mut engine, "ControlsDlg", "NoSuchControl").is_err());

        // Configure properties and conditions to trigger dynamic state mutations
        engine.context_mut().set_property("ACCEPT", "1");
        engine.add_condition(crate::ui::events::ControlCondition {
            dialog: "ControlsDlg".to_string(),
            control: "OkBtn".to_string(),
            action: crate::ui::events::ControlConditionAction::Default,
            condition: "1".to_string(),
        });

        assert!(engine.set_active_dialog("ControlsDlg").is_ok());
        engine.update_progress(50);

        // 1. Map with Some(state)
        let (_bounds, widgets, draws) = mapper.map_active_dialog(&engine, 1024, 768);
        assert_eq!(widgets.len(), 17); // 17 visible controls, 1 hidden skipped

        assert_eq!(
            widgets[0].1,
            UiWidget::CheckBox {
                text: "Accept Terms".to_string(),
                checked: true,
                is_enabled: true,
            }
        );
        assert_eq!(
            widgets[1].1,
            UiWidget::Edit {
                text: "secret".to_string(),
                is_password: true,
                is_enabled: true,
            }
        );
        assert_eq!(widgets[2].1, UiWidget::ProgressBar { fraction: 0.5 });
        assert_eq!(
            widgets[3].1,
            UiWidget::Separator {
                color: Color32::LINE_GRAY,
            }
        );
        assert_eq!(
            widgets[4].1,
            UiWidget::Image {
                width: 100,
                height: 100,
            }
        );
        assert_eq!(
            widgets[5].1,
            UiWidget::Label {
                text: "Static Description".to_string(),
                font_size: WizardTheme::mondo().base_font_size,
                color: WizardTheme::mondo().text_color,
            }
        );
        assert_eq!(
            widgets[6].1,
            UiWidget::Button {
                text: "OK".to_string(),
                is_default: true,
                is_enabled: true,
            }
        );
        assert_eq!(
            widgets[14].1,
            UiWidget::PathEdit {
                path: "C:\\Program Files\\App".to_string(),
                is_enabled: true,
            }
        );
        assert_eq!(
            widgets[15].1,
            UiWidget::PathEdit {
                path: "C:\\Target".to_string(),
                is_enabled: true,
            }
        );
        assert_eq!(
            widgets[16].1,
            UiWidget::PathEdit {
                path: "C:\\Path".to_string(),
                is_enabled: true,
            }
        );
        assert!(draws.iter().any(|d| match d {
            DrawCommand::DrawText { text, .. } => text.contains("Main Feature"),
            _ => false,
        }));

        // 2. Map with None (states cleared)
        engine.clear_control_states();
        let (_bounds2, widgets2, _draws2) = mapper.map_active_dialog(&engine, 1024, 768);
        assert_eq!(widgets2.len(), 17);
        assert_eq!(
            widgets2[0].1,
            UiWidget::CheckBox {
                text: "Accept Terms".to_string(),
                checked: false,
                is_enabled: true,
            }
        );
        assert_eq!(widgets2[2].1, UiWidget::ProgressBar { fraction: 0.0 });
        assert_eq!(
            widgets2[6].1,
            UiWidget::Button {
                text: "OK".to_string(),
                is_default: false,
                is_enabled: true,
            }
        );
        Ok(())
    }

    /// Tests `SoftwareBuffer` rasterization, filling, stroking, text commands, and clipping.
    #[test]
    fn test_software_buffer_rasterizer() {
        let mut buffer = SoftwareBuffer::new(100, 100);
        assert_eq!(buffer.get_pixel(0, 0), Some(Color32::TRANSPARENT));

        buffer.clear(Color32::WHITE);
        assert_eq!(buffer.get_pixel(50, 50), Some(Color32::WHITE));

        // Fill rect with clipping boundaries (negative x/y)
        let rect = PixelRect::new(-10, -10, 30, 30);
        buffer.fill_rect(rect, Color32::ACCENT_BLUE);
        assert_eq!(buffer.get_pixel(5, 5), Some(Color32::ACCENT_BLUE));
        assert_eq!(buffer.get_pixel(50, 50), Some(Color32::WHITE));

        // Stroke rect
        let outline_rect = PixelRect::new(40, 40, 10, 10);
        buffer.stroke_rect(outline_rect, Color32::BLACK);
        assert_eq!(buffer.get_pixel(40, 40), Some(Color32::BLACK));

        // Render commands including StrokeRect, DrawLine (both directions), and DrawText
        let draws = vec![
            DrawCommand::FillRect {
                rect: PixelRect::new(0, 0, 5, 5),
                color: Color32::FOCUS_RING,
            },
            DrawCommand::StrokeRect {
                rect: PixelRect::new(20, 20, 10, 10),
                color: Color32::from_rgb(255, 0, 0),
                width: 1.0,
            },
            DrawCommand::DrawLine {
                start: (10, 10),
                end: (0, 0),
                color: Color32::BLACK,
                width: 1.0,
            },
            DrawCommand::DrawText {
                pos: (2, 2),
                text: "Hi".to_string(),
                color: Color32::BLACK,
                font_size: 10.0,
            },
        ];
        buffer.render_commands(&draws);
        assert_eq!(buffer.get_pixel(0, 0), Some(Color32::BLACK));
        assert_eq!(buffer.get_pixel(20, 20), Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(buffer.get_pixel(200, 200), None); // Out of bounds X and Y
        assert_eq!(buffer.get_pixel(10, 200), None); // In bounds X, out of bounds Y
        assert_eq!(buffer.get_pixel(200, 10), None); // Out of bounds X, in bounds Y

        let png = buffer.to_png_bytes();
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");

        let temp_path = std::env::temp_dir().join("msi_rs_test_buffer.png");
        assert!(buffer.save_png(&temp_path).is_ok());
        let _ = std::fs::remove_file(&temp_path);
        assert!(buffer
            .save_png("/nonexistent_dir_msi_rs_9999/test.png")
            .is_err());
    }

    /// Tests extended layout mapper branches for sidebar bitmaps, disabled buttons,
    /// RTF scrollable text, hierarchical selection trees, radio defaults, and action logs.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_layout_mapper_extended_controls() {
        use crate::execution::properties::InstallState;
        use crate::ui::controls::SelectionTreeNode;

        let mut engine = UiEngine::new(EvaluationContext::new());
        engine.add_dialog(DialogDefinition {
            name: "ExtDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 370,
            height: 270,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Extended Dialog".to_string()),
            control_first: "DisabledBtn".to_string(),
            control_default: None,
            control_cancel: None,
        });
        engine.add_control(
            ControlDefinition::new(
                "ExtDlg",
                "DisabledBtn",
                ControlType::PushButton,
                DluRect::new(10, 10, 56, 17),
                1, // Visible, not enabled
            )
            .text("&Disabled"),
        );
        engine.add_control(
            ControlDefinition::new(
                "ExtDlg",
                "DialogBmp",
                ControlType::Bitmap,
                DluRect::new(0, 0, 493, 312),
                1,
            )
            .property("WixUI_Bmp_Dialog"),
        );
        engine.add_control(
            ControlDefinition::new(
                "ExtDlg",
                "RtfScroll",
                ControlType::ScrollableText,
                DluRect::new(140, 20, 200, 60),
                3,
            )
            .text(r"{\rtf1\ansi Line A\par Line B}"),
        );
        engine.add_control(
            ControlDefinition::new(
                "ExtDlg",
                "LicRadio",
                ControlType::RadioButtonGroup,
                DluRect::new(140, 90, 200, 20),
                3,
            )
            .property("LicenseAccepted"),
        );
        engine.add_control(
            ControlDefinition::new(
                "ExtDlg",
                "Box",
                ControlType::Text,
                DluRect::new(210, 120, 140, 60),
                3,
            )
            .text(r"{\WixUI_Font_Title}Feature Description"),
        );
        engine.add_control(ControlDefinition::new(
            "ExtDlg",
            "Tree",
            ControlType::SelectionTree,
            DluRect::new(20, 120, 175, 45),
            3,
        ));
        engine.add_control(ControlDefinition::new(
            "ExtDlg",
            "Prog",
            ControlType::ProgressBar,
            DluRect::new(20, 200, 300, 10),
            3,
        ));

        engine.context_mut().set_property("LicenseAccepted", "1");
        engine.append_action_log("Action 1: CostInitialize");
        assert!(engine.set_active_dialog("ExtDlg").is_ok());

        let mut root_local = SelectionTreeNode::new("Root1", "Root Local", 1024);
        let mut child_local = SelectionTreeNode::new("Child1", "Child Local", 512);
        child_local.install_state = InstallState::Local;
        let mut child_absent = SelectionTreeNode::new("Child2", "Child Absent", 512);
        child_absent.install_state = InstallState::Absent;
        let child_overflow = SelectionTreeNode::new("Child3", "Child Overflow", 512);
        root_local.add_child(child_local);
        root_local.add_child(child_absent);
        root_local.add_child(child_overflow);

        let mut root_absent = SelectionTreeNode::new("Root2", "Root Absent", 1024);
        root_absent.install_state = InstallState::Absent;
        let root_overflow = SelectionTreeNode::new("Root3", "Root Overflow", 1024);

        assert!(engine
            .get_control_state_mut("ExtDlg", "Tree")
            .is_some_and(|st| {
                st.tree_nodes = vec![root_local, root_absent, root_overflow];
                true
            }));
        let mapper = EguiLayoutMapper::new(WizardTheme::mondo(), FontMetrics::default());
        let (_bounds, widgets, draws) = mapper.map_active_dialog(&engine, 800, 600);
        assert_eq!(widgets.len(), 7);
        assert!(draws.iter().any(|d| match d {
            DrawCommand::DrawText { text, .. } => text.contains("Action 1: CostInitialize"),
            _ => false,
        }));

        // Also test SelectionTree when root_absent is first and fits within max_rows
        let mut r_abs = SelectionTreeNode::new("R_Abs", "Absent First", 512);
        r_abs.install_state = InstallState::Absent;
        assert!(engine
            .get_control_state_mut("ExtDlg", "Tree")
            .is_some_and(|st| {
                st.tree_nodes = vec![r_abs];
                true
            }));
        let (_b2, _w2, draws2) = mapper.map_active_dialog(&engine, 800, 600);
        assert!(draws2.iter().any(|d| matches!(
            d,
            DrawCommand::DrawText { text, .. } if text.contains("[ ] Absent First")
        )));
    }

    /// Tests trait implementations for renderer types.
    #[test]
    fn test_renderer_types_and_traits() {
        let mapper = EguiLayoutMapper::new(WizardTheme::mondo(), FontMetrics::default());
        let mapper_dbg = format!("{mapper:?}");
        assert!(mapper_dbg.contains("EguiLayoutMapper"));

        let widget = UiWidget::Separator {
            color: Color32::LINE_GRAY,
        };
        #[allow(clippy::redundant_clone)]
        let cloned_widget = widget.clone();
        assert_eq!(widget, cloned_widget);
        assert!(format!("{widget:?}").contains("Separator"), "format check");

        let draw = DrawCommand::FillRect {
            rect: PixelRect::new(0, 0, 10, 10),
            color: Color32::WHITE,
        };
        #[allow(clippy::redundant_clone)]
        let cloned_draw = draw.clone();
        assert_eq!(draw, cloned_draw);
        assert!(format!("{draw:?}").contains("FillRect"), "format check");

        let buffer = SoftwareBuffer::new(10, 10);
        #[allow(clippy::redundant_clone)]
        let cloned_buffer = buffer.clone();
        assert_eq!(buffer, cloned_buffer);
        let buf_dbg = format!("{buffer:?}");
        assert!(buf_dbg.contains("SoftwareBuffer"));
    }
}
