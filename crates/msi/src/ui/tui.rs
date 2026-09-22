//! Alternative Terminal TUI Backend (`msi-gui-tui`).
//!
//! Grounded directly in POSIX terminal standards, curses / `ratatui` architecture, and Windows Installer SDK specifications:
//! - Headless interactive terminal text wizard backed by the exact same `msi-ui-core` state machine.
//! - Keyboard navigation: `Tab` / `BackTab` focus cycles, `Enter` default action, `Escape` cancel dialog, `Space` toggle.
//! - Box-drawing character borders, progress bars, edit fields, and focus highlighting.

use crate::error::Result;
use crate::ui::controls::ControlType;
use crate::ui::engine::UiEngine;
use crate::ui::events::DialogReturnCode;

/// Terminal keyboard input event for driving TUI wizard interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TuiKey {
    /// Forward tab navigation to next control.
    Tab,
    /// Reverse tab navigation to previous control (`Shift+Tab`).
    BackTab,
    /// Enter / Return key triggering default or focused action.
    Enter,
    /// Escape key triggering cancel action.
    Escape,
    /// Space key toggling checkbox or radio selection.
    Space,
    /// Up arrow navigation.
    Up,
    /// Down arrow navigation.
    Down,
    /// Left arrow navigation.
    Left,
    /// Right arrow navigation.
    Right,
    /// Printable character input for text edit boxes.
    Char(char),
    /// Function key (F1 through F12).
    F(u8),
    /// Home navigation key.
    Home,
    /// End navigation key.
    End,
    /// Page Up navigation key.
    PageUp,
    /// Page Down navigation key.
    PageDown,
    /// Backspace deletion key.
    Backspace,
}

/// Terminal input and window events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Keyboard key press event.
    Key(TuiKey),
    /// Terminal window resize event.
    Resize {
        /// Updated number of columns.
        cols: u16,
        /// Updated number of rows.
        rows: u16,
    },
}

/// ANSI terminal controller generating standard escape sequences for raw mode and screen switching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalController;

impl TerminalController {
    /// ANSI sequence to enter the alternate screen buffer.
    pub const ENTER_ALTERNATE_SCREEN: &'static str = "\x1b[?1049h";
    /// ANSI sequence to leave the alternate screen buffer and return to user shell.
    pub const LEAVE_ALTERNATE_SCREEN: &'static str = "\x1b[?1049l";
    /// ANSI sequence to hide terminal cursor.
    pub const HIDE_CURSOR: &'static str = "\x1b[?25l";
    /// ANSI sequence to show terminal cursor.
    pub const SHOW_CURSOR: &'static str = "\x1b[?25h";
    /// ANSI sequence to clear the screen and home the cursor.
    pub const CLEAR_SCREEN: &'static str = "\x1b[2J\x1b[H";

    /// Generates ANSI sequence to position cursor at zero-based coordinates `(col, row)`.
    ///
    /// # Arguments
    ///
    /// * `col` - Zero-based column index.
    /// * `row` - Zero-based row index.
    ///
    /// # Returns
    ///
    /// Formatted ANSI cursor positioning string.
    #[must_use]
    pub fn move_cursor(col: u16, row: u16) -> String {
        format!("\x1b[{};{}H", row + 1, col + 1)
    }

    /// Parses a raw ANSI byte sequence into a [`TuiKey`] and the number of consumed bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw byte buffer received from stdin.
    ///
    /// # Returns
    ///
    /// Tuple of `(TuiKey, consumed_byte_count)` if a valid key was parsed, or `None`.
    #[must_use]
    pub const fn parse_key(bytes: &[u8]) -> Option<(TuiKey, usize)> {
        if bytes.is_empty() {
            return None;
        }

        match bytes[0] {
            b'\t' => Some((TuiKey::Tab, 1)),
            b'\r' | b'\n' => Some((TuiKey::Enter, 1)),
            b' ' => Some((TuiKey::Space, 1)),
            0x1B => {
                if bytes.len() == 1 {
                    Some((TuiKey::Escape, 1))
                } else if bytes.len() >= 3 && bytes[1] == b'[' {
                    match bytes[2] {
                        b'A' => Some((TuiKey::Up, 3)),
                        b'B' => Some((TuiKey::Down, 3)),
                        b'C' => Some((TuiKey::Right, 3)),
                        b'D' => Some((TuiKey::Left, 3)),
                        b'Z' => Some((TuiKey::BackTab, 3)),
                        _ => Some((TuiKey::Escape, 1)),
                    }
                } else {
                    Some((TuiKey::Escape, 1))
                }
            }
            b if b.is_ascii_graphic() => Some((TuiKey::Char(b as char), 1)),
            _ => None,
        }
    }
}

/// RAII terminal safety guard ensuring cursor visibility and alternate screen teardown on panic.
#[derive(Debug, Default)]
pub struct TerminalSafetyGuard {
    /// Active state flag.
    active: bool,
}

impl TerminalSafetyGuard {
    /// Creates a new active [`TerminalSafetyGuard`].
    ///
    /// # Returns
    ///
    /// A new [`TerminalSafetyGuard`].
    #[must_use]
    pub const fn new() -> Self {
        Self { active: true }
    }

    /// Disarms the guard, indicating clean manual teardown.
    pub const fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for TerminalSafetyGuard {
    fn drop(&mut self) {
        if self.active {
            let mut stdout = std::io::stdout();
            drop(std::io::Write::write_all(
                &mut stdout,
                TerminalController::SHOW_CURSOR.as_bytes(),
            ));
            drop(std::io::Write::write_all(
                &mut stdout,
                TerminalController::LEAVE_ALTERNATE_SCREEN.as_bytes(),
            ));
            drop(std::io::Write::flush(&mut stdout));
        }
    }
}

/// In-memory terminal character grid canvas for text UI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBuffer {
    /// Width in character columns.
    pub width: u16,
    /// Height in character rows.
    pub height: u16,
    /// Character grid cells.
    pub cells: Vec<char>,
}

impl TerminalBuffer {
    /// Creates a new [`TerminalBuffer`] filled with space characters.
    ///
    /// # Arguments
    ///
    /// * `width` - Grid columns.
    /// * `height` - Grid rows.
    ///
    /// # Returns
    ///
    /// A new [`TerminalBuffer`].
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            cells: vec![' '; size],
        }
    }

    /// Sets a character at grid coordinates `(x, y)` if within boundaries.
    pub fn set_char(&mut self, x: u16, y: u16, ch: char) {
        if x < self.width && y < self.height {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            self.cells[idx] = ch;
        }
    }

    /// Writes a horizontal string at `(x, y)`.
    pub fn draw_string(&mut self, x: u16, y: u16, text: &str) {
        for (i, ch) in text.chars().enumerate() {
            let col = x.saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
            self.set_char(col, y, ch);
        }
    }

    /// Draws an outlined box using Unicode box-drawing characters.
    pub fn draw_box(&mut self, x: u16, y: u16, w: u16, h: u16, title: Option<&str>) {
        if w < 2 || h < 2 {
            return;
        }

        // Corners
        self.set_char(x, y, '┌');
        self.set_char(x + w - 1, y, '┐');
        self.set_char(x, y + h - 1, '└');
        self.set_char(x + w - 1, y + h - 1, '┘');

        // Horizontal borders
        for col in (x + 1)..(x + w - 1) {
            self.set_char(col, y, '─');
            self.set_char(col, y + h - 1, '─');
        }

        // Vertical borders
        for row in (y + 1)..(y + h - 1) {
            self.set_char(x, row, '│');
            self.set_char(x + w - 1, row, '│');
        }

        // Title text centered on top border
        if let Some(t) = title {
            let title_fmt = format!(" {t} ");
            let title_len = u16::try_from(title_fmt.len()).unwrap_or(u16::MAX);
            let start_x = x + (w.saturating_sub(title_len) / 2);
            self.draw_string(start_x, y, &title_fmt);
        }
    }

    /// Converts the character buffer into a multiline formatted string for terminal output.
    #[must_use]
    pub fn render_to_string(&self) -> String {
        let mut out = String::with_capacity((self.width as usize + 1) * self.height as usize);
        for row in 0..self.height {
            for col in 0..self.width {
                let idx = (row as usize) * (self.width as usize) + (col as usize);
                out.push(self.cells[idx]);
            }
            out.push('\n');
        }
        out
    }
}

/// Interactive terminal wizard controller backed by [`UiEngine`].
#[derive(Debug)]
pub struct TerminalWizard {
    /// Underlying headless UI state machine.
    engine: UiEngine,
    /// Currently focused control index within the active dialog.
    focused_index: usize,
}

impl TerminalWizard {
    /// Creates a new [`TerminalWizard`] wrapping a [`UiEngine`].
    ///
    /// # Arguments
    ///
    /// * `engine` - Pre-configured [`UiEngine`].
    ///
    /// # Returns
    ///
    /// A new [`TerminalWizard`].
    #[must_use]
    pub const fn new(engine: UiEngine) -> Self {
        Self {
            engine,
            focused_index: 0,
        }
    }

    /// Returns a reference to the underlying [`UiEngine`].
    #[must_use]
    pub const fn engine(&self) -> &UiEngine {
        &self.engine
    }

    /// Returns a mutable reference to the underlying [`UiEngine`].
    pub const fn engine_mut(&mut self) -> &mut UiEngine {
        &mut self.engine
    }

    /// Processes keyboard input, updating control focus or triggering dialog events.
    ///
    /// # Arguments
    ///
    /// * `key` - Pressed [`TuiKey`].
    ///
    /// # Returns
    ///
    /// `Some(DialogReturnCode)` if installer execution finishes, or `None` if interaction continues.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on event execution failure.
    pub fn handle_key(&mut self, key: TuiKey) -> Result<Option<DialogReturnCode>> {
        let Some(dialog) = self.engine.active_dialog().cloned() else {
            return Ok(None);
        };
        let controls = self.engine.get_dialog_controls(&dialog.name).to_vec();
        if controls.is_empty() {
            return Ok(None);
        }

        match key {
            TuiKey::Tab => {
                self.focused_index = (self.focused_index + 1) % controls.len();
                Ok(None)
            }
            TuiKey::BackTab => {
                self.focused_index = if self.focused_index == 0 {
                    controls.len().saturating_sub(1)
                } else {
                    self.focused_index - 1
                };
                Ok(None)
            }
            TuiKey::Enter => {
                let target_ctrl = if let Some(ref def_name) = dialog.control_default {
                    def_name.as_str()
                } else if let Some(ctrl) = controls.get(self.focused_index) {
                    ctrl.control()
                } else {
                    ""
                };
                if target_ctrl.is_empty() {
                    Ok(None)
                } else {
                    self.engine.click_control(&dialog.name, target_ctrl)
                }
            }
            TuiKey::Escape => {
                if let Some(ref cancel_name) = dialog.control_cancel {
                    self.engine.click_control(&dialog.name, cancel_name)
                } else {
                    Ok(None)
                }
            }
            TuiKey::Space => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::CheckBox {
                        if let Some(prop) = ctrl.property_name() {
                            let cur_val = self.engine.context().get_property(prop).unwrap_or("0");
                            let new_val = if cur_val == "1" { "0" } else { "1" };
                            self.engine.context_mut().set_property(prop, new_val);
                            self.engine.evaluate_conditions_and_formatting()?;
                        }
                    } else if ctrl.control_type() == ControlType::PushButton {
                        return self.engine.click_control(&dialog.name, ctrl.control());
                    }
                }
                Ok(None)
            }
            TuiKey::Up
            | TuiKey::Down
            | TuiKey::Left
            | TuiKey::Right
            | TuiKey::Char(_)
            | TuiKey::F(_)
            | TuiKey::Home
            | TuiKey::End
            | TuiKey::PageUp
            | TuiKey::PageDown
            | TuiKey::Backspace => Ok(None),
        }
    }

    /// Handles a high-level [`TerminalEvent`], either dispatching a key or updating dimensions on resize.
    ///
    /// # Arguments
    ///
    /// * `event` - Terminal input or resize event.
    /// * `cols` - Current terminal width in characters.
    /// * `rows` - Current terminal height in characters.
    ///
    /// # Returns
    ///
    /// A tuple of optional completion [`DialogReturnCode`] and the newly rendered [`TerminalBuffer`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on event dispatch failure.
    pub fn handle_event(
        &mut self,
        event: TerminalEvent,
        cols: u16,
        rows: u16,
    ) -> Result<(Option<DialogReturnCode>, TerminalBuffer)> {
        match event {
            TerminalEvent::Key(key) => {
                let code = self.handle_key(key)?;
                let buf = self.render_frame(cols, rows);
                Ok((code, buf))
            }
            TerminalEvent::Resize {
                cols: new_cols,
                rows: new_rows,
            } => {
                let buf = self.render_frame(new_cols, new_rows);
                Ok((None, buf))
            }
        }
    }

    /// Runs a live interactive terminal event stream reading from an input source and writing frames.
    ///
    /// # Arguments
    ///
    /// * `input` - Character/byte input reader.
    /// * `output` - Output terminal writer.
    /// * `initial_cols` - Terminal columns.
    /// * `initial_rows` - Terminal rows.
    ///
    /// # Returns
    ///
    /// The final [`DialogReturnCode`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on I/O or execution failure.
    pub fn run_event_stream<R: std::io::Read, W: std::io::Write>(
        &mut self,
        mut input: R,
        mut output: W,
        initial_cols: u16,
        initial_rows: u16,
    ) -> Result<DialogReturnCode> {
        let mut guard = TerminalSafetyGuard::new();
        output.write_all(TerminalController::ENTER_ALTERNATE_SCREEN.as_bytes())?;
        output.write_all(TerminalController::HIDE_CURSOR.as_bytes())?;
        output.write_all(TerminalController::CLEAR_SCREEN.as_bytes())?;

        let initial_frame = self.render_frame(initial_cols, initial_rows);
        output.write_all(initial_frame.render_to_string().as_bytes())?;
        output.flush()?;

        let mut byte_buf = [0u8; 32];
        let mut return_code = DialogReturnCode::Return;

        loop {
            let bytes_read = match input.read(&mut byte_buf) {
                Ok(n) if n > 0 => n,
                _ => break,
            };

            let mut cursor = 0;
            while cursor < bytes_read {
                if let Some((key, consumed)) =
                    TerminalController::parse_key(&byte_buf[cursor..bytes_read])
                {
                    cursor += consumed;
                    let (code_opt, frame) =
                        self.handle_event(TerminalEvent::Key(key), initial_cols, initial_rows)?;
                    output.write_all(TerminalController::CLEAR_SCREEN.as_bytes())?;
                    output.write_all(frame.render_to_string().as_bytes())?;
                    output.flush()?;

                    if let Some(code) = code_opt {
                        return_code = code;
                        guard.disarm();
                        output.write_all(TerminalController::SHOW_CURSOR.as_bytes())?;
                        output.write_all(TerminalController::LEAVE_ALTERNATE_SCREEN.as_bytes())?;
                        output.flush()?;
                        return Ok(return_code);
                    }
                } else {
                    cursor += 1;
                }
            }
        }

        guard.disarm();
        output.write_all(TerminalController::SHOW_CURSOR.as_bytes())?;
        output.write_all(TerminalController::LEAVE_ALTERNATE_SCREEN.as_bytes())?;
        output.flush()?;
        Ok(return_code)
    }

    /// Renders the active dialog into a formatted [`TerminalBuffer`].
    ///
    /// # Arguments
    ///
    /// * `cols` - Terminal width in characters.
    /// * `rows` - Terminal height in characters.
    ///
    /// # Returns
    ///
    /// Rendered [`TerminalBuffer`].
    #[must_use]
    pub fn render_frame(&self, cols: u16, rows: u16) -> TerminalBuffer {
        let mut buf = TerminalBuffer::new(cols, rows);
        let Some(dialog) = self.engine.active_dialog() else {
            return buf;
        };

        let box_w = cols.min(70);
        let box_h = rows.min(20);
        let box_x = (cols.saturating_sub(box_w)) / 2;
        let box_y = (rows.saturating_sub(box_h)) / 2;

        buf.draw_box(box_x, box_y, box_w, box_h, dialog.title.as_deref());

        let controls = self.engine.get_dialog_controls(&dialog.name);
        let mut line_offset = 2;

        for (idx, ctrl) in controls.iter().enumerate() {
            if line_offset + 1 >= box_h - 1 {
                break;
            }

            let state = self.engine.get_control_state(&dialog.name, ctrl.control());
            let is_visible = state.map_or_else(|| ctrl.is_visible_by_default(), |s| s.is_visible);
            if !is_visible {
                continue;
            }

            let is_focused = idx == self.focused_index;
            let text = state.map_or_else(
                || ctrl.text_template().unwrap_or("").to_string(),
                |s| s.current_text.clone(),
            );

            let row_y = box_y + line_offset;
            let col_x = box_x + 4;

            match ctrl.control_type() {
                ControlType::PushButton => {
                    let btn_str = if is_focused {
                        format!("[> {text} <]")
                    } else {
                        format!("[ {text} ]")
                    };
                    buf.draw_string(col_x, row_y, &btn_str);
                    line_offset += 2;
                }
                ControlType::CheckBox => {
                    let checked = state.is_some_and(|s| s.bound_value.as_deref() == Some("1"));
                    let mark = if checked { 'X' } else { ' ' };
                    let focus_cursor = if is_focused { '>' } else { ' ' };
                    let check_str = format!("{focus_cursor} [{mark}] {text}");
                    buf.draw_string(col_x, row_y, &check_str);
                    line_offset += 2;
                }
                ControlType::Edit => {
                    let focus_cursor = if is_focused { '>' } else { ' ' };
                    let edit_str = format!("{focus_cursor} [ {text} ]");
                    buf.draw_string(col_x, row_y, &edit_str);
                    line_offset += 2;
                }
                ControlType::ProgressBar => {
                    let pct = state.map_or(0, |s| s.progress_percent);
                    let filled = (usize::try_from(pct).unwrap_or(0) * 30) / 100;
                    let mut bar = "█".repeat(filled);
                    bar.push_str(&"░".repeat(30usize.saturating_sub(filled)));
                    let prog_str = format!("[{bar}] {pct}%");
                    buf.draw_string(col_x, row_y, &prog_str);
                    line_offset += 2;
                }
                ControlType::RadioButtonGroup
                | ControlType::Text
                | ControlType::ComboBox
                | ControlType::ListBox
                | ControlType::ListView
                | ControlType::Bitmap
                | ControlType::Line
                | ControlType::ScrollableText
                | ControlType::VolumeCostList
                | ControlType::SelectionTree => {
                    buf.draw_string(col_x, row_y, &text);
                    line_offset += 1;
                }
            }
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::properties::EvaluationContext;
    use crate::ui::controls::ControlDefinition;
    use crate::ui::engine::{DialogDefinition, DIALOG_ATTR_VISIBLE};
    use crate::ui::events::{ControlEvent, ControlEventType};
    use crate::ui::layout::DluRect;

    /// Tests `TerminalBuffer` dimensions, string rendering, and box drawing.
    #[test]
    fn test_terminal_buffer() {
        let mut buf = TerminalBuffer::new(40, 10);
        assert_eq!(buf.width, 40);
        assert_eq!(buf.height, 10);

        buf.draw_box(0, 0, 40, 10, Some("Wizard Setup"));
        assert_eq!(buf.cells[0], '┌');
        assert_eq!(buf.cells[39], '┐');

        // Test draw_box with title = None on a second buffer
        let mut buf2 = TerminalBuffer::new(30, 10);
        buf2.draw_box(0, 0, 20, 10, None);
        assert!(buf2.render_to_string().contains('┌'));

        // Test out of bounds set_char: x out of bounds, y out of bounds, and x in bounds with y out of bounds
        buf.set_char(100, 100, 'X');
        buf.set_char(10, 100, 'Y');

        // Test draw_box too small (returns early)
        buf.draw_box(0, 0, 1, 1, None);
        buf.draw_box(0, 0, 10, 1, None);

        let rendered = buf.render_to_string();
        assert!(rendered.contains("Wizard Setup"));
        assert!(rendered.contains('│'));
    }

    /// Tests `TerminalWizard` keyboard navigation, control types rendering, and event handling.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_terminal_wizard_navigation() -> Result<()> {
        let mut engine = UiEngine::new(EvaluationContext::new());
        engine.add_dialog(DialogDefinition {
            name: "WelcomeDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Welcome".to_string()),
            control_first: "EulaCheck".to_string(),
            control_default: Some("NextBtn".to_string()),
            control_cancel: Some("CancelBtn".to_string()),
        });

        engine.add_control(
            ControlDefinition::new(
                "WelcomeDlg",
                "EulaCheck",
                ControlType::CheckBox,
                DluRect::new(10, 10, 100, 20),
                3,
            )
            .property("ACCEPT_EULA")
            .text("I accept"),
        );
        engine.add_control(
            ControlDefinition::new(
                "WelcomeDlg",
                "NextBtn",
                ControlType::PushButton,
                DluRect::new(10, 40, 50, 20),
                3,
            )
            .text("Next"),
        );
        engine.add_control(
            ControlDefinition::new(
                "WelcomeDlg",
                "CancelBtn",
                ControlType::PushButton,
                DluRect::new(70, 40, 50, 20),
                3,
            )
            .text("Cancel"),
        );
        engine.add_control(
            ControlDefinition::new(
                "WelcomeDlg",
                "UnboundCheck",
                ControlType::CheckBox,
                DluRect::new(10, 70, 100, 20),
                3,
            )
            .text("Unbound Checkbox"),
        );

        engine.add_event(ControlEvent::new(
            "WelcomeDlg",
            "NextBtn",
            ControlEventType::EndDialog(DialogReturnCode::Return),
            None,
            1,
        ));
        engine.add_event(ControlEvent::new(
            "WelcomeDlg",
            "CancelBtn",
            ControlEventType::EndDialog(DialogReturnCode::Exit),
            None,
            1,
        ));

        engine.set_active_dialog("WelcomeDlg")?;
        let mut wizard = TerminalWizard::new(engine);

        // Initial render frame
        let frame = wizard.render_frame(80, 24);
        let rendered_str = frame.render_to_string();
        assert!(rendered_str.contains("Welcome"));
        assert!(rendered_str.contains("[ ] I accept"));

        // Space key on focused EulaCheck toggles it from 0 to 1
        wizard.handle_key(TuiKey::Space)?;
        assert_eq!(
            wizard.engine().context().get_property("ACCEPT_EULA"),
            Some("1")
        );

        // Render frame when checkbox is checked ([X])
        let frame_checked = wizard.render_frame(80, 24);
        assert!(frame_checked.render_to_string().contains("[X] I accept"));

        // Space key again toggles it from 1 to 0
        wizard.handle_key(TuiKey::Space)?;
        assert_eq!(
            wizard.engine().context().get_property("ACCEPT_EULA"),
            Some("0")
        );
        let frame_unchecked = wizard.render_frame(80, 24);
        assert!(frame_unchecked.render_to_string().contains("[ ] I accept"));

        // Tab key cycles to NextBtn (index 1)
        wizard.handle_key(TuiKey::Tab)?;
        assert_eq!(wizard.focused_index, 1);

        // Space key on PushButton triggers click_control -> returns Return
        let space_btn_res = wizard.handle_key(TuiKey::Space)?;
        assert_eq!(space_btn_res, Some(DialogReturnCode::Return));

        // Tab to CancelBtn (index 2)
        wizard.engine_mut().set_active_dialog("WelcomeDlg")?;
        wizard.handle_key(TuiKey::Tab)?;
        assert_eq!(wizard.focused_index, 2);

        // BackTab when focused_index > 0 decrements index to 1
        wizard.handle_key(TuiKey::BackTab)?;
        assert_eq!(wizard.focused_index, 1);

        // BackTab when focused_index == 0 wraps around to controls.len() - 1
        wizard.focused_index = 0;
        wizard.handle_key(TuiKey::BackTab)?;
        assert_eq!(wizard.focused_index, 3); // 4 controls now

        // Focus UnboundCheck (index 3) and press Space (exercises None branch of property_name)
        assert_eq!(wizard.handle_key(TuiKey::Space)?, None);

        // Space key when focused_index is out of bounds
        wizard.focused_index = 999;
        assert_eq!(wizard.handle_key(TuiKey::Space)?, None);

        // Enter key activates default (NextBtn) -> returns Return
        let res = wizard.handle_key(TuiKey::Enter)?;
        assert_eq!(res, Some(DialogReturnCode::Return));

        // Escape activates cancel -> returns Exit
        wizard.engine_mut().set_active_dialog("WelcomeDlg")?;
        let esc_res = wizard.handle_key(TuiKey::Escape)?;
        assert_eq!(esc_res, Some(DialogReturnCode::Exit));

        // Navigation arrow keys and Char keys
        assert_eq!(wizard.handle_key(TuiKey::Up)?, None);
        assert_eq!(wizard.handle_key(TuiKey::Down)?, None);
        assert_eq!(wizard.handle_key(TuiKey::Left)?, None);
        assert_eq!(wizard.handle_key(TuiKey::Right)?, None);
        assert_eq!(wizard.handle_key(TuiKey::Char('q'))?, None);

        // Dialog without default control and without cancel control
        let mut engine_nodef = UiEngine::new(EvaluationContext::new());
        engine_nodef.add_dialog(DialogDefinition {
            name: "NoDefDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: "Btn1".to_string(),
            control_default: None,
            control_cancel: None,
        });
        engine_nodef.add_control(
            ControlDefinition::new(
                "NoDefDlg",
                "Btn1",
                ControlType::PushButton,
                DluRect::new(10, 10, 50, 20),
                0,
            )
            .text("ClickMe"),
        );
        engine_nodef.add_event(ControlEvent::new(
            "NoDefDlg",
            "Btn1",
            ControlEventType::EndDialog(DialogReturnCode::Retry),
            None,
            1,
        ));
        engine_nodef.set_active_dialog("NoDefDlg")?;
        let mut wiz_nodef = TerminalWizard::new(engine_nodef);

        // Enter triggers focused control when control_default is None
        assert_eq!(
            wiz_nodef.handle_key(TuiKey::Enter)?,
            Some(DialogReturnCode::Retry)
        );

        // Enter when control_default is None and focused_index is out of bounds
        wiz_nodef.focused_index = 999;
        assert_eq!(wiz_nodef.handle_key(TuiKey::Enter)?, None);

        // Escape returns None when control_cancel is None
        wiz_nodef.engine_mut().set_active_dialog("NoDefDlg")?;
        assert_eq!(wiz_nodef.handle_key(TuiKey::Escape)?, None);

        // Test render_frame when control_states are cleared (exercises state == None fallback)
        wizard.engine_mut().clear_control_states();
        let frame_no_states = wizard.render_frame(80, 24);
        assert_ne!(frame_no_states.render_to_string(), "");

        // When active dialog has no controls
        let mut engine_empty_ctrls = UiEngine::new(EvaluationContext::new());
        engine_empty_ctrls.add_dialog(DialogDefinition {
            name: "EmptyDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 100,
            attributes: DIALOG_ATTR_VISIBLE,
            title: None,
            control_first: String::new(),
            control_default: None,
            control_cancel: None,
        });
        engine_empty_ctrls.set_active_dialog("EmptyDlg")?;
        let mut wiz_empty = TerminalWizard::new(engine_empty_ctrls);
        assert_eq!(wiz_empty.handle_key(TuiKey::Tab)?, None);

        // When no active dialog is set
        let mut wiz_no_dlg = TerminalWizard::new(UiEngine::new(EvaluationContext::new()));
        assert_eq!(wiz_no_dlg.handle_key(TuiKey::Tab)?, None);
        let empty_frame = wiz_no_dlg.render_frame(80, 24);
        assert_eq!(empty_frame.width, 80);

        Ok(())
    }

    /// Tests rendering all supported control types, hidden controls, progress bar, edit, and overflow.
    #[test]
    fn test_terminal_wizard_render_all_control_types() -> Result<()> {
        let mut engine = UiEngine::new(EvaluationContext::new());
        engine.add_dialog(DialogDefinition {
            name: "AllControlsDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("All Controls".to_string()),
            control_first: "Edit1".to_string(),
            control_default: None,
            control_cancel: None,
        });

        // Edit control
        engine.add_control(
            ControlDefinition::new(
                "AllControlsDlg",
                "Edit1",
                ControlType::Edit,
                DluRect::new(10, 10, 80, 20),
                3,
            )
            .text("EditableText"),
        );
        // ProgressBar
        engine.add_control(ControlDefinition::new(
            "AllControlsDlg",
            "Prog1",
            ControlType::ProgressBar,
            DluRect::new(10, 30, 80, 20),
            3,
        ));
        // Text control
        engine.add_control(
            ControlDefinition::new(
                "AllControlsDlg",
                "Text1",
                ControlType::Text,
                DluRect::new(10, 50, 80, 20),
                3,
            )
            .text("Static Label"),
        );
        // RadioButtonGroup
        engine.add_control(
            ControlDefinition::new(
                "AllControlsDlg",
                "Radio1",
                ControlType::RadioButtonGroup,
                DluRect::new(10, 70, 80, 20),
                3,
            )
            .text("Option 1"),
        );
        // Hidden control (should be skipped)
        let hidden_ctrl = ControlDefinition::new(
            "AllControlsDlg",
            "Hidden1",
            ControlType::PushButton,
            DluRect::new(10, 90, 80, 20),
            0,
        )
        .text("Hidden");
        engine.add_control(hidden_ctrl);

        // Many controls to overflow dialog height box
        for i in 0..15 {
            engine.add_control(
                ControlDefinition::new(
                    "AllControlsDlg",
                    format!("ExtraCtrl{i}"),
                    ControlType::Text,
                    DluRect::new(10, 100 + i * 10, 80, 20),
                    3,
                )
                .text(format!("Extra Label {i}")),
            );
        }

        engine.set_active_dialog("AllControlsDlg")?;
        engine.update_progress(50);

        let mut wizard = TerminalWizard::new(engine);
        let frame = wizard.render_frame(80, 20);
        let s = frame.render_to_string();

        assert!(s.contains("EditableText"));
        assert!(s.contains("50%"));
        assert!(s.contains("Static Label"));
        assert!(!s.contains("Hidden"));

        // Render when Edit1 is NOT focused (exercises is_focused == false under ControlType::Edit)
        wizard.focused_index = 1;
        let frame_unfocused = wizard.render_frame(80, 20);
        assert!(frame_unfocused
            .render_to_string()
            .contains("  [ EditableText ]"));

        // Test Space key on non-checkbox, non-pushbutton control (e.g. Edit) -> None
        wizard.focused_index = 0; // Edit1
        assert_eq!(wizard.handle_key(TuiKey::Space)?, None);

        Ok(())
    }

    /// Tests `TerminalController`, key parsing, ANSI sequences, and `TerminalSafetyGuard`.
    #[test]
    fn test_terminal_controller_and_safety_guard() {
        // ANSI constants
        assert_eq!(TerminalController::ENTER_ALTERNATE_SCREEN, "\x1b[?1049h");
        assert_eq!(TerminalController::LEAVE_ALTERNATE_SCREEN, "\x1b[?1049l");
        assert_eq!(TerminalController::HIDE_CURSOR, "\x1b[?25l");
        assert_eq!(TerminalController::SHOW_CURSOR, "\x1b[?25h");
        assert_eq!(TerminalController::CLEAR_SCREEN, "\x1b[2J\x1b[H");

        // Cursor movement
        assert_eq!(TerminalController::move_cursor(0, 0), "\x1b[1;1H");
        assert_eq!(TerminalController::move_cursor(10, 5), "\x1b[6;11H");

        // Key parsing
        assert_eq!(TerminalController::parse_key(b""), None);
        assert_eq!(TerminalController::parse_key(b"\t"), Some((TuiKey::Tab, 1)));
        assert_eq!(
            TerminalController::parse_key(b"\r"),
            Some((TuiKey::Enter, 1))
        );
        assert_eq!(
            TerminalController::parse_key(b"\n"),
            Some((TuiKey::Enter, 1))
        );
        assert_eq!(
            TerminalController::parse_key(b" "),
            Some((TuiKey::Space, 1))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b"),
            Some((TuiKey::Escape, 1))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b[A"),
            Some((TuiKey::Up, 3))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b[B"),
            Some((TuiKey::Down, 3))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b[C"),
            Some((TuiKey::Right, 3))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b[D"),
            Some((TuiKey::Left, 3))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b[Z"),
            Some((TuiKey::BackTab, 3))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1b[X"),
            Some((TuiKey::Escape, 1))
        );
        assert_eq!(
            TerminalController::parse_key(b"\x1bO"),
            Some((TuiKey::Escape, 1))
        );
        assert_eq!(
            TerminalController::parse_key(b"a"),
            Some((TuiKey::Char('a'), 1))
        );
        assert_eq!(TerminalController::parse_key(b"\x00"), None);

        // TerminalEvent
        let key_ev = TerminalEvent::Key(TuiKey::Tab);
        let resize_ev = TerminalEvent::Resize {
            cols: 120,
            rows: 40,
        };
        assert_eq!(key_ev, TerminalEvent::Key(TuiKey::Tab));
        assert_eq!(
            resize_ev,
            TerminalEvent::Resize {
                cols: 120,
                rows: 40
            }
        );

        // TerminalSafetyGuard: drop when disarmed
        let mut guard = TerminalSafetyGuard::new();
        guard.disarm();
        drop(guard);

        let default_guard = TerminalSafetyGuard::default();
        drop(default_guard);

        // TerminalSafetyGuard: drop when active
        let armed_guard = TerminalSafetyGuard::new();
        drop(armed_guard);
    }

    /// Tests `handle_event` and `run_event_stream` on `TerminalWizard`.
    #[test]
    fn test_terminal_wizard_event_stream() -> Result<()> {
        struct FailingReader;
        impl std::io::Read for FailingReader {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("read error"))
            }
        }

        let mut engine = UiEngine::new(EvaluationContext::new());
        engine.add_dialog(DialogDefinition {
            name: "MainDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Main Dialog".to_string()),
            control_first: "FinishBtn".to_string(),
            control_default: Some("FinishBtn".to_string()),
            control_cancel: None,
        });
        engine.add_control(
            ControlDefinition::new(
                "MainDlg",
                "FinishBtn",
                ControlType::PushButton,
                DluRect::new(10, 10, 50, 20),
                3,
            )
            .text("Finish"),
        );
        engine.add_event(ControlEvent::new(
            "MainDlg",
            "FinishBtn",
            ControlEventType::EndDialog(DialogReturnCode::Exit),
            None,
            1,
        ));
        engine.set_active_dialog("MainDlg")?;

        let mut wizard = TerminalWizard::new(engine);

        // Test handle_event resize
        let (res, buf) = wizard.handle_event(
            TerminalEvent::Resize {
                cols: 100,
                rows: 30,
            },
            100,
            30,
        )?;
        assert_eq!(res, None);
        assert_eq!(buf.width, 100);
        assert_eq!(buf.height, 30);

        // Test run_event_stream with Tab key followed by unparseable byte and Enter key
        let input_bytes = b"\t\x1b\xff\r";
        let mut output_bytes = Vec::new();
        let code = wizard.run_event_stream(&input_bytes[..], &mut output_bytes, 80, 24)?;
        assert_eq!(code, DialogReturnCode::Exit);
        assert_ne!(output_bytes.len(), 0);

        // Test run_event_stream with empty input (reaches EOF without DialogReturnCode)
        let empty_input: &[u8] = b"";
        let mut out2 = Vec::new();
        let code2 = wizard.run_event_stream(empty_input, &mut out2, 80, 24)?;
        assert_eq!(code2, DialogReturnCode::Return);

        // Test run_event_stream with Tab key only (exercises cursor < bytes_read exiting inner loop)
        let tab_input = b"\t";
        let mut out4 = Vec::new();
        let code4 = wizard.run_event_stream(&tab_input[..], &mut out4, 80, 24)?;
        assert_eq!(code4, DialogReturnCode::Return);

        // Test run_event_stream with reader returning an I/O error
        let mut out3 = Vec::new();
        let code3 = wizard.run_event_stream(&mut FailingReader, &mut out3, 80, 24)?;
        assert_eq!(code3, DialogReturnCode::Return);

        Ok(())
    }
}
