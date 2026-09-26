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
    /// Vertical scroll offset for scrollable controls (lines).
    scroll_offset: usize,
    /// Cursor character index within the active edit field.
    cursor_pos: usize,
    /// Whether the real-time live diagnostics log view is toggled open (F2).
    diagnostics_log_open: bool,
    /// Real-time step description text displayed during installation progress.
    action_text: Option<String>,
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
    pub fn new(engine: UiEngine) -> Self {
        let focused_index = engine.active_dialog().map_or(0, |dlg| {
            engine
                .get_dialog_controls(&dlg.name)
                .iter()
                .position(|c| c.control() == dlg.control_first)
                .unwrap_or(0)
        });
        Self {
            engine,
            focused_index,
            scroll_offset: 0,
            cursor_pos: 0,
            diagnostics_log_open: false,
            action_text: None,
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

    /// Returns whether the diagnostics log view is toggled open.
    #[must_use]
    pub const fn is_diagnostics_log_open(&self) -> bool {
        self.diagnostics_log_open
    }

    /// Toggles the diagnostics log view.
    pub const fn toggle_diagnostics_log(&mut self) {
        self.diagnostics_log_open = !self.diagnostics_log_open;
    }

    /// Sets the real-time action description text.
    ///
    /// # Arguments
    ///
    /// * `text` - Action description string (e.g. `"Installing service MySQL..."`).
    pub fn set_action_text(&mut self, text: impl Into<String>) {
        self.action_text = Some(text.into());
    }

    /// Returns the current real-time action description text, if set.
    #[must_use]
    pub fn action_text(&self) -> Option<&str> {
        self.action_text.as_deref()
    }

    /// Returns the active vertical scroll offset.
    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets the vertical scroll offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - Vertical scroll offset in lines.
    pub const fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Returns the current text cursor position within the active edit control.
    #[must_use]
    pub const fn cursor_pos(&self) -> usize {
        self.cursor_pos
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
    #[allow(clippy::too_many_lines)]
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
                self.cursor_pos = 0;
                Ok(None)
            }
            TuiKey::BackTab => {
                self.focused_index = if self.focused_index == 0 {
                    controls.len().saturating_sub(1)
                } else {
                    self.focused_index - 1
                };
                self.cursor_pos = 0;
                Ok(None)
            }
            TuiKey::Enter => {
                let target_ctrl = if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::PushButton {
                        ctrl.control()
                    } else if let Some(ref def_name) = dialog.control_default {
                        def_name.as_str()
                    } else {
                        ctrl.control()
                    }
                } else if let Some(ref def_name) = dialog.control_default {
                    def_name.as_str()
                } else {
                    ""
                };
                if target_ctrl.is_empty() {
                    Ok(None)
                } else {
                    self.scroll_offset = 0;
                    self.cursor_pos = 0;
                    let prev_dlg = dialog.name.clone();
                    let res = self.engine.click_control(&dialog.name, target_ctrl)?;
                    let cur_dlg = self.engine.active_dialog();
                    if let Some(d) = cur_dlg.filter(|d| d.name != prev_dlg) {
                        self.focused_index = self
                            .engine
                            .get_dialog_controls(&d.name)
                            .iter()
                            .position(|c| c.control() == d.control_first)
                            .unwrap_or(0);
                    }
                    Ok(res)
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
                    match ctrl.control_type() {
                        ControlType::CheckBox => {
                            self.engine.toggle_checkbox(&dialog.name, ctrl.control())?;
                        }
                        ControlType::RadioButtonGroup => {
                            let val = ctrl.text_template().unwrap_or("").to_string();
                            self.engine
                                .select_radio_button(&dialog.name, ctrl.control(), &val)?;
                        }
                        ControlType::PushButton => {
                            return self.engine.click_control(&dialog.name, ctrl.control());
                        }
                        _ => {}
                    }
                }
                Ok(None)
            }
            TuiKey::Char(c) => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::Edit {
                        let cur_text = self
                            .engine
                            .get_control_state(&dialog.name, ctrl.control())
                            .map_or_else(
                                || ctrl.text_template().unwrap_or("").to_string(),
                                |s| s.current_text.clone(),
                            );
                        let mut new_text = cur_text;
                        if self.cursor_pos >= new_text.len() {
                            new_text.push(c);
                            self.cursor_pos = new_text.len();
                        } else {
                            new_text.insert(self.cursor_pos, c);
                            self.cursor_pos += 1;
                        }
                        self.engine.update_control_value(
                            &dialog.name,
                            ctrl.control(),
                            &new_text,
                        )?;
                    }
                }
                Ok(None)
            }
            TuiKey::Backspace => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::Edit {
                        let cur_text = self
                            .engine
                            .get_control_state(&dialog.name, ctrl.control())
                            .map_or_else(
                                || ctrl.text_template().unwrap_or("").to_string(),
                                |s| s.current_text.clone(),
                            );
                        let mut new_text = cur_text;
                        if self.cursor_pos > 0 && !new_text.is_empty() {
                            let remove_idx = (self.cursor_pos - 1).min(new_text.len() - 1);
                            new_text.remove(remove_idx);
                            self.cursor_pos = remove_idx;
                            self.engine.update_control_value(
                                &dialog.name,
                                ctrl.control(),
                                &new_text,
                            )?;
                        }
                    }
                }
                Ok(None)
            }
            TuiKey::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                Ok(None)
            }
            TuiKey::Right => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    let len = self
                        .engine
                        .get_control_state(&dialog.name, ctrl.control())
                        .map_or_else(
                            || ctrl.text_template().map_or(0, str::len),
                            |s| s.current_text.len(),
                        );
                    if self.cursor_pos < len {
                        self.cursor_pos += 1;
                    }
                }
                Ok(None)
            }
            TuiKey::Up => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::ScrollableText {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    } else if ctrl.control_type() == ControlType::RadioButtonGroup {
                        let val = ctrl.text_template().unwrap_or("").to_string();
                        self.engine
                            .select_radio_button(&dialog.name, ctrl.control(), &val)?;
                    }
                }
                Ok(None)
            }
            TuiKey::Down => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::ScrollableText {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                    } else if ctrl.control_type() == ControlType::RadioButtonGroup {
                        let val = ctrl.text_template().unwrap_or("").to_string();
                        self.engine
                            .select_radio_button(&dialog.name, ctrl.control(), &val)?;
                    }
                }
                Ok(None)
            }
            TuiKey::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                Ok(None)
            }
            TuiKey::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
                Ok(None)
            }
            TuiKey::Home => {
                self.scroll_offset = 0;
                self.cursor_pos = 0;
                Ok(None)
            }
            TuiKey::End => {
                if let Some(ctrl) = controls.get(self.focused_index) {
                    if ctrl.control_type() == ControlType::Edit {
                        let len = self
                            .engine
                            .get_control_state(&dialog.name, ctrl.control())
                            .map_or_else(
                                || ctrl.text_template().map_or(0, str::len),
                                |s| s.current_text.len(),
                            );
                        self.cursor_pos = len;
                    } else if ctrl.control_type() == ControlType::ScrollableText {
                        let total = ctrl.text_template().unwrap_or("").lines().count();
                        self.scroll_offset = total.saturating_sub(10);
                    }
                }
                Ok(None)
            }
            TuiKey::F(2) => {
                self.toggle_diagnostics_log();
                Ok(None)
            }
            TuiKey::F(_) => Ok(None),
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
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn render_frame(&self, cols: u16, rows: u16) -> TerminalBuffer {
        let mut buf = TerminalBuffer::new(cols, rows);

        // If diagnostics console is open, overlay it
        if self.diagnostics_log_open {
            let log_w = cols.min(76);
            let log_h = rows.min(18);
            let log_x = cols.saturating_sub(log_w) / 2;
            let log_y = rows.saturating_sub(log_h) / 2;
            buf.draw_box(
                log_x,
                log_y,
                log_w,
                log_h,
                Some("Diagnostics Log [F2: Close]"),
            );
            let logs = self.engine.action_log();
            let max_lines = (log_h.saturating_sub(3)) as usize;
            let start = logs.len().saturating_sub(max_lines);
            for (i, entry) in logs.iter().skip(start).enumerate() {
                buf.draw_string(log_x + 2, log_y + 2 + i as u16, entry);
            }
            return buf;
        }

        let Some(dialog) = self.engine.active_dialog() else {
            return buf;
        };

        let box_w = cols.min(70);
        let box_h = rows.min(20);
        let box_x = (cols.saturating_sub(box_w)) / 2;
        let box_y = (rows.saturating_sub(box_h)) / 2;

        let formatted_title = dialog.title.as_deref().map(|t| {
            self.engine
                .context()
                .format_string(t)
                .unwrap_or_else(|_| t.to_string())
        });
        buf.draw_box(box_x, box_y, box_w, box_h, formatted_title.as_deref());

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
                ControlType::RadioButtonGroup => {
                    let is_selected =
                        state.is_some_and(|s| s.bound_value.as_deref() == Some(&text));
                    let mark = if is_selected { "(•)" } else { "( )" };
                    let focus_cursor = if is_focused { '>' } else { ' ' };
                    let radio_str = format!("{focus_cursor} {mark} {text}");
                    buf.draw_string(col_x, row_y, &radio_str);
                    line_offset += 2;
                }
                ControlType::Edit => {
                    let is_pwd = ctrl.is_password_input();
                    let display_text = if is_pwd {
                        "*".repeat(text.len())
                    } else {
                        text.clone()
                    };
                    let mut invalid_feedback = String::new();
                    if let Some(prop) = ctrl.property_name() {
                        if prop.contains("PORT") && !text.is_empty() && text.parse::<u16>().is_err()
                        {
                            invalid_feedback = " [!] Invalid port".to_string();
                        }
                    }
                    let focus_cursor = if is_focused { '>' } else { ' ' };
                    let edit_str = format!("{focus_cursor} [ {display_text} ]{invalid_feedback}");
                    buf.draw_string(col_x, row_y, &edit_str);
                    line_offset += 2;
                }
                ControlType::ScrollableText => {
                    let all_lines: Vec<&str> = text.lines().collect();
                    let total_lines = all_lines.len();
                    let max_display_lines = (box_h.saturating_sub(line_offset + 3)) as usize;
                    let start = self.scroll_offset.min(total_lines.saturating_sub(1));
                    let end = (start + max_display_lines).min(total_lines);
                    for line in &all_lines[start..end] {
                        buf.draw_string(col_x, box_y + line_offset, line);
                        line_offset += 1;
                    }
                    let scroll_ind = format!(
                        "[Lines {}-{end} of {total_lines} - Up/Down/PageDown to scroll]",
                        start + 1
                    );
                    buf.draw_string(col_x, box_y + line_offset, &scroll_ind);
                    line_offset += 2;
                }
                ControlType::SelectionTree => {
                    let nodes = state.map_or_else(Vec::new, |s| s.tree_nodes.clone());
                    let focus_cursor = if is_focused { '>' } else { ' ' };
                    buf.draw_string(
                        col_x,
                        row_y,
                        &format!("{focus_cursor} Components Checklist:"),
                    );
                    line_offset += 1;
                    for node in nodes.iter().take(4) {
                        if line_offset + 1 >= box_h - 1 {
                            break;
                        }
                        buf.draw_string(
                            col_x + 2,
                            box_y + line_offset,
                            &format!("├─ [X] {}", node.title),
                        );
                        line_offset += 1;
                    }
                    line_offset += 1;
                }
                ControlType::ProgressBar => {
                    let pct = state.map_or(0, |s| s.progress_percent);
                    let filled = (usize::try_from(pct).unwrap_or(0) * 30) / 100;
                    let mut bar = "█".repeat(filled);
                    bar.push_str(&"░".repeat(30usize.saturating_sub(filled)));
                    let prog_str = format!("[{bar}] {pct}%");
                    buf.draw_string(col_x, row_y, &prog_str);
                    line_offset += 1;
                    if let Some(ref action) = self.action_text {
                        buf.draw_string(col_x, box_y + line_offset, action);
                        line_offset += 1;
                    }
                    line_offset += 1;
                }
                ControlType::Text
                | ControlType::ComboBox
                | ControlType::ListBox
                | ControlType::ListView
                | ControlType::Bitmap
                | ControlType::Line
                | ControlType::VolumeCostList => {
                    buf.draw_string(col_x, row_y, &text);
                    line_offset += 1;
                }
            }
        }

        buf
    }

    /// Renders an exit summary screen displaying final installation status, ports, and services.
    ///
    /// # Arguments
    ///
    /// * `cols` - Terminal width in characters.
    /// * `rows` - Terminal height in characters.
    /// * `product_name` - Product name string.
    /// * `ports` - Service and listening port mappings.
    /// * `services` - Active service names.
    ///
    /// # Returns
    ///
    /// Rendered [`TerminalBuffer`].
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn render_exit_summary(
        &self,
        cols: u16,
        rows: u16,
        product_name: &str,
        ports: &[(String, u16)],
        services: &[String],
    ) -> TerminalBuffer {
        let mut buf = TerminalBuffer::new(cols, rows);
        let box_w = cols.min(72);
        let box_h = rows.min(22);
        let box_x = cols.saturating_sub(box_w) / 2;
        let box_y = rows.saturating_sub(box_h) / 2;

        let title = format!("{product_name} - Setup Complete");
        buf.draw_box(box_x, box_y, box_w, box_h, Some(&title));

        buf.draw_string(box_x + 3, box_y + 2, "Installation finished successfully!");
        buf.draw_string(box_x + 3, box_y + 4, "Active Services:");
        for (i, s) in services.iter().enumerate().take(5) {
            buf.draw_string(box_x + 5, box_y + 5 + i as u16, &format!("• {s} (running)"));
        }

        let port_y = box_y + 6 + services.len().min(5) as u16;
        buf.draw_string(box_x + 3, port_y, "Listening Endpoints:");
        for (i, (svc, port)) in ports.iter().enumerate().take(5) {
            buf.draw_string(
                box_x + 5,
                port_y + 1 + i as u16,
                &format!("• {svc}: http://localhost:{port}"),
            );
        }

        buf.draw_string(
            box_x + 3,
            box_y + box_h - 2,
            "Press Enter or Finish to exit.",
        );
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

        // Enter when control_default is None and focused_index is 0 (RetryBtn)
        wiz_nodef.focused_index = 0;
        assert_eq!(
            wiz_nodef.handle_key(TuiKey::Enter)?,
            Some(DialogReturnCode::Retry)
        );

        // Reactivate NoDefDlg and enter on non-PushButton control when control_default is None (hits line 419 else { ctrl.control() })
        wiz_nodef.engine_mut().set_active_dialog("NoDefDlg")?;
        let edit_nodef = ControlDefinition::new(
            "NoDefDlg",
            "EditNoDef",
            ControlType::Edit,
            DluRect::new(10, 10, 80, 15),
            3,
        );
        wiz_nodef.engine_mut().add_control(edit_nodef);
        wiz_nodef.focused_index = 1; // EditNoDef
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
    #[allow(clippy::too_many_lines)]
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

        // Password edit control
        let pwd_ctrl = ControlDefinition::new(
            "AllControlsDlg",
            "Pwd1",
            ControlType::Edit,
            DluRect::new(10, 85, 80, 20),
            3 | crate::ui::controls::CONTROL_ATTR_PASSWORD_INPUT,
        )
        .text("secret");
        engine.add_control(pwd_ctrl);

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
        assert!(s.contains("******"));
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

        // Test ScrollableText rendering
        let mut scroll_engine = UiEngine::new(EvaluationContext::new());
        scroll_engine.add_dialog(DialogDefinition {
            name: "ScrollDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("License Terms".to_string()),
            control_first: "LicenseText".to_string(),
            control_default: None,
            control_cancel: None,
        });

        let license_body = "Line 1: Terms\nLine 2: Conditions\nLine 3: Permissions\nLine 4: Liability\nLine 5: Warranty\nLine 6: Termination\nLine 7: End";
        scroll_engine.add_control(
            ControlDefinition::new(
                "ScrollDlg",
                "LicenseText",
                ControlType::ScrollableText,
                DluRect::new(10, 10, 80, 80),
                3,
            )
            .text(license_body),
        );
        scroll_engine.set_active_dialog("ScrollDlg")?;

        let scroll_wizard = TerminalWizard::new(scroll_engine);
        // Render with standard height to draw text and the scroll indicator
        let scroll_frame = scroll_wizard.render_frame(80, 24);
        let scroll_str = scroll_frame.render_to_string();
        assert!(scroll_str.contains("Line 1: Terms"));
        assert!(scroll_str.contains("Up/Down/PageDown to scroll"));

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

    /// Tests rich controls (`ScrollableText`, `SelectionTree`, Radio, Edit cursor, diagnostics log, summary).
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_terminal_wizard_rich_controls_and_diagnostics() -> Result<()> {
        let mut context = EvaluationContext::new();
        context.set_property("MY_PORT", "abc"); // invalid port
        context.set_property("SECRET_PWD", "pass");
        let mut engine = UiEngine::new(context);

        engine.add_dialog(DialogDefinition {
            name: "RichDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 300,
            height: 200,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("Rich Controls".to_string()),
            control_first: "PortInput".to_string(),
            control_default: Some("OkBtn".to_string()),
            control_cancel: Some("CancelBtn".to_string()),
        });

        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "PortInput",
                ControlType::Edit,
                DluRect::new(10, 10, 80, 15),
                3,
            )
            .property("MY_PORT")
            .text("abc"),
        );
        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "UserInput",
                ControlType::Edit,
                DluRect::new(10, 25, 80, 15),
                3,
            )
            .property("USER_NAME")
            .text("admin"),
        );
        engine.add_control(ControlDefinition::new(
            "RichDlg",
            "CompTree",
            ControlType::SelectionTree,
            DluRect::new(10, 45, 80, 20),
            3,
        ));
        engine.add_control(ControlDefinition::new(
            "RichDlg",
            "Progress",
            ControlType::ProgressBar,
            DluRect::new(10, 70, 80, 15),
            3,
        ));
        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "ModeOpt",
                ControlType::RadioButtonGroup,
                DluRect::new(10, 90, 80, 15),
                3,
            )
            .text("Option X"),
        );
        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "LogOpt",
                ControlType::CheckBox,
                DluRect::new(10, 110, 80, 15),
                3,
            )
            .property("ENABLE_LOGGING")
            .text("Enable Logging"),
        );
        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "OkBtn",
                ControlType::PushButton,
                DluRect::new(10, 130, 50, 15),
                3,
            )
            .text("OK"),
        );
        engine.add_event(ControlEvent::new(
            "RichDlg",
            "OkBtn",
            ControlEventType::EndDialog(DialogReturnCode::Return),
            None,
            1,
        ));
        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "CancelBtn",
                ControlType::PushButton,
                DluRect::new(70, 130, 50, 15),
                3,
            )
            .text("Cancel"),
        );
        engine.add_event(ControlEvent::new(
            "RichDlg",
            "CancelBtn",
            ControlEventType::EndDialog(DialogReturnCode::Exit),
            None,
            1,
        ));
        let scroll_lines = (1..=20)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        engine.add_control(
            ControlDefinition::new(
                "RichDlg",
                "ScrollDoc",
                ControlType::ScrollableText,
                DluRect::new(10, 150, 80, 40),
                3,
            )
            .text(scroll_lines),
        );

        for name in ["CompTree", "Nonexistent"] {
            if let Some(tree_state) = engine.get_control_state_mut("RichDlg", name) {
                tree_state
                    .tree_nodes
                    .push(crate::ui::controls::SelectionTreeNode::new(
                        "Feat",
                        "Core Feature",
                        1024,
                    ));
                tree_state
                    .tree_nodes
                    .push(crate::ui::controls::SelectionTreeNode::new(
                        "Extra",
                        "Extra Feature",
                        2048,
                    ));
            }
        }
        engine.update_progress(60);

        engine.set_active_dialog("RichDlg")?;
        let mut wizard = TerminalWizard::new(engine);

        // Edit control testing: cursor movement, backspace, delete, typing
        assert_eq!(wizard.cursor_pos(), 0);
        wizard.handle_key(TuiKey::Right)?;
        assert_eq!(wizard.cursor_pos(), 1);
        wizard.handle_key(TuiKey::Left)?;
        assert_eq!(wizard.cursor_pos(), 0);
        wizard.handle_key(TuiKey::Left)?;
        assert_eq!(wizard.cursor_pos(), 0); // Left at 0
        wizard.handle_key(TuiKey::Backspace)?;
        assert_eq!(wizard.cursor_pos(), 0); // Backspace at 0
        wizard.handle_key(TuiKey::Char('9'))?;
        assert_eq!(wizard.cursor_pos(), 1);
        wizard.handle_key(TuiKey::Home)?;
        assert_eq!(wizard.cursor_pos(), 0);
        wizard.handle_key(TuiKey::Char('1'))?; // Insert in middle
        assert_eq!(wizard.cursor_pos(), 1);
        wizard.handle_key(TuiKey::End)?;
        assert!(wizard.cursor_pos() > 1);
        wizard.handle_key(TuiKey::Right)?; // Right at end (cursor_pos >= len)

        // F keys
        wizard.handle_key(TuiKey::F(2))?;
        assert!(wizard.is_diagnostics_log_open());
        wizard.handle_key(TuiKey::F(2))?;
        assert!(!wizard.is_diagnostics_log_open());
        wizard.handle_key(TuiKey::F(5))?;

        // Invalid port feedback rendering
        wizard.set_action_text("Copying files...");
        let invalid_buf = wizard.render_frame(80, 50).render_to_string();
        assert!(invalid_buf.contains("[!] Invalid port"));
        assert!(invalid_buf.contains("Core Feature")); // SelectionTree rendering
        assert!(invalid_buf.contains("60%")); // ProgressBar rendering
        assert!(invalid_buf.contains("Copying files..."));

        // Valid port rendering (tests !text.is_empty() and text.parse::<u16>().is_ok())
        wizard
            .engine_mut()
            .update_control_value("RichDlg", "PortInput", "8080")?;
        let valid_buf = wizard.render_frame(80, 50).render_to_string();
        assert!(!valid_buf.contains("[!] Invalid port"));

        // Focus ScrollDoc (index 8)
        wizard.focused_index = 8;
        assert_eq!(wizard.scroll_offset(), 0);
        wizard.set_scroll_offset(2);
        assert_eq!(wizard.scroll_offset(), 2);
        wizard.handle_key(TuiKey::Up)?;
        assert_eq!(wizard.scroll_offset(), 1);
        wizard.handle_key(TuiKey::Down)?;
        assert_eq!(wizard.scroll_offset(), 2);
        wizard.handle_key(TuiKey::PageDown)?;
        assert_eq!(wizard.scroll_offset(), 12);
        wizard.handle_key(TuiKey::PageUp)?;
        assert_eq!(wizard.scroll_offset(), 2);
        wizard.handle_key(TuiKey::Home)?;
        assert_eq!(wizard.scroll_offset(), 0);
        wizard.handle_key(TuiKey::End)?;
        assert!(wizard.scroll_offset() > 0);

        // Focus CompTree (index 2 - SelectionTree focused branch)
        wizard.focused_index = 2;
        let _tree_focused = wizard.render_frame(80, 50);

        // Test render_frame with height 10 to trigger line 878 break in SelectionTree
        assert_eq!(wizard.render_frame(80, 10).height, 10);

        // Focus ModeOpt (index 4 - RadioButtonGroup focused branch)
        wizard.focused_index = 4;
        let _mode_focused = wizard.render_frame(80, 50);
        wizard.handle_key(TuiKey::Up)?;
        wizard.handle_key(TuiKey::Down)?;
        wizard.handle_key(TuiKey::Space)?;

        // Focus LogOpt (index 5 - CheckBox)
        wizard.focused_index = 5;
        wizard.handle_key(TuiKey::Space)?;

        // Focus OkBtn (index 6 - PushButton)
        wizard.focused_index = 6;
        wizard.handle_key(TuiKey::Left)?;
        wizard.handle_key(TuiKey::Home)?;
        wizard.handle_key(TuiKey::PageUp)?;
        wizard.handle_key(TuiKey::PageDown)?;
        wizard.handle_key(TuiKey::End)?;
        wizard.handle_key(TuiKey::Up)?; // Up on PushButton (hits lines 552/564 else)
        wizard.handle_key(TuiKey::Down)?; // Down on PushButton
        wizard.handle_key(TuiKey::Backspace)?; // Backspace on PushButton (hits line 477 false)

        // Render frame while OkBtn is focused (renders all other controls with is_focused == false)
        let _unfocused_buf = wizard.render_frame(80, 50);

        // Test Escape on dialog with control_cancel defined (CancelBtn)
        assert_eq!(
            wizard.handle_key(TuiKey::Escape)?,
            Some(DialogReturnCode::Exit)
        );

        // Reactivate RichDlg and test Enter on PortInput (activating default control OkBtn)
        assert!(wizard.engine_mut().set_active_dialog("RichDlg").is_ok());
        wizard.focused_index = 0;
        assert_eq!(
            wizard.handle_key(TuiKey::Enter)?,
            Some(DialogReturnCode::Return)
        );

        // Reactivate RichDlg and test Enter when focused_index is out of range (activating default control)
        assert!(wizard.engine_mut().set_active_dialog("RichDlg").is_ok());
        wizard.focused_index = 999;
        assert_eq!(
            wizard.handle_key(TuiKey::Enter)?,
            Some(DialogReturnCode::Return)
        );

        // Reactivate RichDlg and test Space on OkBtn (EndDialog Return)
        assert!(wizard.engine_mut().set_active_dialog("RichDlg").is_ok());
        wizard.focused_index = 6;
        assert_eq!(
            wizard.handle_key(TuiKey::Space)?,
            Some(DialogReturnCode::Return)
        );

        // Reactivate RichDlg for remaining tests
        assert!(wizard.engine_mut().set_active_dialog("RichDlg").is_ok());

        // Test empty port rendering (!text.is_empty() is false) and backspace when cursor_pos > 0 but text is empty
        wizard
            .engine_mut()
            .update_control_value("RichDlg", "PortInput", "")?;
        wizard.focused_index = 0;
        wizard.cursor_pos = 5;
        wizard.handle_key(TuiKey::Backspace)?; // cursor_pos > 0 but new_text.is_empty() is true!
        let empty_port_buf = wizard.render_frame(80, 50).render_to_string();
        assert!(!empty_port_buf.contains("[!] Invalid port"));

        // Out of bounds focused index
        wizard.focused_index = 999;
        assert_eq!(wizard.handle_key(TuiKey::Backspace)?, None);
        assert_eq!(wizard.handle_key(TuiKey::End)?, None);

        // Test typing and backspace when control states are cleared
        wizard.engine_mut().clear_control_states();
        wizard.focused_index = 0; // PortInput
        wizard.cursor_pos = 0;
        wizard.handle_key(TuiKey::Right)?;
        wizard.handle_key(TuiKey::End)?;
        wizard.handle_key(TuiKey::Char('X'))?;
        wizard.handle_key(TuiKey::Backspace)?;

        // Test NewDialog event updating active_dialog and focused_index
        let next_dlg = DialogDefinition {
            name: "NextDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 200,
            height: 150,
            attributes: DIALOG_ATTR_VISIBLE,
            title: Some("[Unclosed".to_string()),
            control_first: "NextFirst".to_string(),
            control_default: None,
            control_cancel: None,
        };
        wizard.engine_mut().add_dialog(next_dlg);
        let next_first_btn = ControlDefinition::new(
            "NextDlg",
            "NextFirst",
            ControlType::PushButton,
            DluRect::new(10, 10, 50, 15),
            3,
        );
        wizard.engine_mut().add_control(next_first_btn);

        let nav_btn = ControlDefinition::new(
            "RichDlg",
            "NavBtn",
            ControlType::PushButton,
            DluRect::new(10, 180, 50, 15),
            3,
        );
        wizard.engine_mut().add_control(nav_btn);
        wizard.engine_mut().add_event(ControlEvent::new(
            "RichDlg",
            "NavBtn",
            ControlEventType::NewDialog("NextDlg".to_string()),
            None,
            1,
        ));

        // Focus NavBtn (index 9) and press Enter
        wizard.focused_index = 9;
        wizard.handle_key(TuiKey::Enter)?;
        assert_eq!(
            wizard.engine().active_dialog().map(|d| d.name.as_str()),
            Some("NextDlg")
        );
        assert_eq!(wizard.focused_index, 0);

        // Render frame with unclosed title template
        let next_buf = wizard.render_frame(80, 24).render_to_string();
        assert!(next_buf.contains("[Unclosed"));

        // Test typing on non-Edit control (NextFirst is PushButton at index 0)
        wizard.focused_index = 0;
        assert_eq!(wizard.handle_key(TuiKey::Char('A'))?, None);

        // Test small terminal rendering
        let small_buf = wizard.render_frame(10, 5);
        assert_eq!(small_buf.width, 10);
        assert_eq!(small_buf.height, 5);

        // Test rendering when active dialog is None
        let mut no_dlg_wizard = TerminalWizard::new(UiEngine::new(EvaluationContext::new()));
        let no_dlg_buf = no_dlg_wizard.render_frame(80, 24);
        assert_eq!(no_dlg_buf.width, 80);
        assert_eq!(no_dlg_buf.height, 24);
        assert_eq!(no_dlg_wizard.handle_key(TuiKey::Enter)?, None);
        assert_eq!(no_dlg_wizard.handle_key(TuiKey::Escape)?, None);

        // Test TerminalBuffer out-of-bounds safety
        let mut tbuf = TerminalBuffer::new(10, 10);
        tbuf.draw_box(0, 0, 1, 1, None);
        tbuf.draw_string(20, 20, "out of bounds");
        tbuf.set_char(20, 20, 'X');

        // Action text and progress
        wizard.set_action_text("Step 1 of 5");
        assert_eq!(wizard.action_text(), Some("Step 1 of 5"));

        // Diagnostics log toggle
        assert!(!wizard.is_diagnostics_log_open());
        wizard
            .engine_mut()
            .append_action_log("Action log diagnostic entry");
        wizard.toggle_diagnostics_log();
        assert!(wizard.is_diagnostics_log_open());
        let diag_buf = wizard.render_frame(80, 24).render_to_string();
        assert!(diag_buf.contains("Diagnostics Log [F2: Close]"));
        assert!(diag_buf.contains("Action log diagnostic entry"));
        wizard.toggle_diagnostics_log();
        assert!(!wizard.is_diagnostics_log_open());

        // Exit summary
        let exit_buf = wizard.render_exit_summary(
            80,
            24,
            "MyProduct",
            &[("Web".to_string(), 8080)],
            &["service1".to_string()],
        );
        let exit_str = exit_buf.render_to_string();
        assert!(exit_str.contains("MyProduct - Setup Complete"));
        assert!(exit_str.contains("service1 (running)"));
        assert!(exit_str.contains("Web: http://localhost:8080"));

        Ok(())
    }
}
