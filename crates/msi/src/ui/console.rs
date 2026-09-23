//! Console & Framebuffer TUI Runtime for Bare-Metal Environments.
//!
//! Provides terminal fallbacks when terminfo is absent, Linux VT keyboard and scancode
//! decoding, serial console auto-detection, and direct UEFI Graphics Output Protocol (GOP)
//! framebuffer rasterization.

use crate::error::{Error, Result};
use crate::ui::theme::Color32;
use crate::ui::tui::TuiKey;
use std::fmt::Write as _;

/// Console environment operational modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConsoleKind {
    /// Standard ANSI terminal with terminfo support.
    AnsiTerminal,
    /// Linux virtual terminal (/dev/tty0, /dev/tty1).
    LinuxVirtualTerminal,
    /// Serial port console (/dev/ttyS0, COM1).
    SerialPort,
    /// Direct UEFI Graphics Output Protocol framebuffer.
    UefiGop,
    /// Headless fallback without cursor positioning or ANSI escape support.
    HeadlessFallback,
}

/// Fallback text console renderer for environments lacking terminfo or terminal capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeadlessConsole {
    /// Staged output log lines.
    lines: Vec<String>,
}

impl HeadlessConsole {
    /// Creates a new [`HeadlessConsole`].
    ///
    /// # Returns
    ///
    /// A fresh [`HeadlessConsole`] instance.
    #[must_use]
    pub const fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Appends a log line to the console buffer.
    ///
    /// # Arguments
    ///
    /// * `line` - Text message line.
    pub fn append_line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    /// Formats a plain-text prompt dialog suitable for dumb terminals.
    ///
    /// # Arguments
    ///
    /// * `title` - Dialog header text.
    /// * `body` - Dialog explanation body.
    /// * `options` - List of selectable choices.
    ///
    /// # Returns
    ///
    /// Formatted ASCII box string representation.
    #[must_use]
    pub fn render_dialog_plain(title: &str, body: &str, options: &[&str]) -> String {
        let mut out =
            String::from("============================================================\n");
        out.push_str(title);
        out.push('\n');
        out.push_str("------------------------------------------------------------\n");
        out.push_str(body);
        out.push('\n');
        out.push_str("------------------------------------------------------------\n");
        for (idx, opt) in options.iter().enumerate() {
            let _ = writeln!(out, "  [{}] {}", idx + 1, opt);
        }
        out.push_str("============================================================\n");
        out
    }

    /// Formats a progress update line for dumb terminal output.
    ///
    /// # Arguments
    ///
    /// * `step_name` - Descriptive step title.
    /// * `percent` - Current percentage (0 to 100).
    ///
    /// # Returns
    ///
    /// Formatted ASCII progress line.
    #[must_use]
    pub fn render_progress_line(step_name: &str, percent: u8) -> String {
        let bounded = percent.min(100);
        let hashes = usize::from(bounded / 5);
        let spaces = 20 - hashes;
        format!(
            "[{}{}] {:3}% - {}",
            "#".repeat(hashes),
            " ".repeat(spaces),
            bounded,
            step_name
        )
    }
}

/// Controller and escape sequence decoder for Linux Virtual Terminals (`/dev/tty0`, `/dev/tty1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinuxVtDriver;

impl LinuxVtDriver {
    /// Decodes raw escape sequence bytes from a Linux VT into a [`TuiKey`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw byte stream slice from the virtual terminal input.
    ///
    /// # Returns
    ///
    /// Tuple of `(TuiKey, consumed_byte_count)` if matched, or `None`.
    #[must_use]
    pub fn decode_vt_sequence(bytes: &[u8]) -> Option<(TuiKey, usize)> {
        if bytes.is_empty() {
            return None;
        }

        // Check for VT escape sequences: ESC [ or ESC O or ESC [ [
        if bytes[0] == 0x1B {
            if bytes.len() == 1 {
                return Some((TuiKey::Escape, 1));
            }

            if bytes.len() >= 3 && bytes[1] == b'[' {
                // Function keys with double bracket: ESC [ [ A -> F1
                if bytes.len() >= 4 && bytes[2] == b'[' {
                    return match bytes[3] {
                        b'A' => Some((TuiKey::F(1), 4)),
                        b'B' => Some((TuiKey::F(2), 4)),
                        b'C' => Some((TuiKey::F(3), 4)),
                        b'D' => Some((TuiKey::F(4), 4)),
                        b'E' => Some((TuiKey::F(5), 4)),
                        _ => None,
                    };
                }

                // Arrow keys: ESC [ A / B / C / D
                match bytes[2] {
                    b'A' => return Some((TuiKey::Up, 3)),
                    b'B' => return Some((TuiKey::Down, 3)),
                    b'C' => return Some((TuiKey::Right, 3)),
                    b'D' => return Some((TuiKey::Left, 3)),
                    b'H' => return Some((TuiKey::Home, 3)),
                    b'F' => return Some((TuiKey::End, 3)),
                    _ => {}
                }

                // Tilde extended sequences: ESC [ <num> ~
                if bytes.len() >= 4 && bytes[bytes.len() - 1] == b'~' {
                    let num_str = std::str::from_utf8(&bytes[2..bytes.len() - 1]).ok()?;
                    return match num_str {
                        "1" | "7" => Some((TuiKey::Home, bytes.len())),
                        "4" | "8" => Some((TuiKey::End, bytes.len())),
                        "2" => Some((TuiKey::Char(' '), bytes.len())),
                        "3" => Some((TuiKey::Backspace, bytes.len())),
                        "5" => Some((TuiKey::PageUp, bytes.len())),
                        "6" => Some((TuiKey::PageDown, bytes.len())),
                        "11" => Some((TuiKey::F(1), bytes.len())),
                        "12" => Some((TuiKey::F(2), bytes.len())),
                        "13" => Some((TuiKey::F(3), bytes.len())),
                        "14" => Some((TuiKey::F(4), bytes.len())),
                        "15" => Some((TuiKey::F(5), bytes.len())),
                        "17" => Some((TuiKey::F(6), bytes.len())),
                        "18" => Some((TuiKey::F(7), bytes.len())),
                        "19" => Some((TuiKey::F(8), bytes.len())),
                        "20" => Some((TuiKey::F(9), bytes.len())),
                        "21" => Some((TuiKey::F(10), bytes.len())),
                        "23" => Some((TuiKey::F(11), bytes.len())),
                        "24" => Some((TuiKey::F(12), bytes.len())),
                        _ => None,
                    };
                }
            }
        }

        match bytes[0] {
            b'\t' => Some((TuiKey::Tab, 1)),
            b'\r' | b'\n' => Some((TuiKey::Enter, 1)),
            0x7F | 0x08 => Some((TuiKey::Backspace, 1)),
            b' ' => Some((TuiKey::Space, 1)),
            b if b.is_ascii_graphic() => Some((TuiKey::Char(b as char), 1)),
            _ => None,
        }
    }
}

/// Serial communication parity configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SerialParity {
    /// No parity bit (8N1 standard).
    #[default]
    None,
    /// Even parity bit.
    Even,
    /// Odd parity bit.
    Odd,
}

/// Hardware serial console configuration descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialConsoleConfig {
    /// Device path (e.g. `/dev/ttyS0` or `COM1`).
    pub port_name: String,
    /// Baud rate (e.g. 115200).
    pub baud_rate: u32,
    /// Data bits count (typically 8).
    pub data_bits: u8,
    /// Parity mode.
    pub parity: SerialParity,
    /// Stop bits count (typically 1).
    pub stop_bits: u8,
}

impl Default for SerialConsoleConfig {
    /// Default serial configuration: 115200 8N1 on `/dev/ttyS0`.
    fn default() -> Self {
        Self {
            port_name: "/dev/ttyS0".to_string(),
            baud_rate: 115_200,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
        }
    }
}

impl SerialConsoleConfig {
    /// Detects active serial console configuration from Linux cmdline or active tty identifiers.
    ///
    /// # Arguments
    ///
    /// * `proc_cmdline` - Contents of `/proc/cmdline`.
    /// * `sys_active` - Contents of `/sys/class/tty/console/active`.
    ///
    /// # Returns
    ///
    /// Detected [`SerialConsoleConfig`], or `None` if no serial console was specified.
    #[must_use]
    pub fn detect_active(proc_cmdline: &str, sys_active: &str) -> Option<Self> {
        // First check /sys/class/tty/console/active (e.g. "ttyS0 tty0")
        for word in sys_active.split_whitespace().rev() {
            if word.starts_with("ttyS") || word.starts_with("ttyUSB") || word.starts_with("ttyAMA")
            {
                return Some(Self {
                    port_name: format!("/dev/{word}"),
                    baud_rate: 115_200,
                    data_bits: 8,
                    parity: SerialParity::None,
                    stop_bits: 1,
                });
            }
        }

        // Then inspect /proc/cmdline: console=ttyS0,115200n8
        for param in proc_cmdline.split_whitespace() {
            if let Some(val) = param.strip_prefix("console=") {
                let parts: Vec<&str> = val.split(',').collect();
                let port = parts[0];
                if port.starts_with("ttyS")
                    || port.starts_with("ttyUSB")
                    || port.starts_with("ttyAMA")
                {
                    let baud = if parts.len() > 1 {
                        let raw_digits: String =
                            parts[1].chars().take_while(char::is_ascii_digit).collect();
                        raw_digits.parse::<u32>().unwrap_or(115_200)
                    } else {
                        115_200
                    };
                    return Some(Self {
                        port_name: format!("/dev/{port}"),
                        baud_rate: baud,
                        data_bits: 8,
                        parity: SerialParity::None,
                        stop_bits: 1,
                    });
                }
            }
        }

        None
    }
}

/// Pixel format layout for UEFI Graphics Output Protocol framebuffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GopPixelFormat {
    /// 32-bit Red-Green-Blue (RGBX/RGBA).
    Rgb8,
    /// 32-bit Blue-Green-Red (BGRX/BGRA).
    Bgr8,
    /// Custom bitmask color packing.
    Bitmask {
        /// Red channel bitmask.
        red: u32,
        /// Green channel bitmask.
        green: u32,
        /// Blue channel bitmask.
        blue: u32,
    },
}

/// Direct UEFI Graphics Output Protocol (GOP) framebuffer rendering pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UefiGopFramebuffer {
    /// Screen width in horizontal pixels.
    pub width: u32,
    /// Screen height in vertical pixels.
    pub height: u32,
    /// Framebuffer stride in pixels per scanline.
    pub stride: u32,
    /// Color pixel encoding format.
    pub pixel_format: GopPixelFormat,
}

impl UefiGopFramebuffer {
    /// Creates a new [`UefiGopFramebuffer`] descriptor.
    ///
    /// # Arguments
    ///
    /// * `width` - Display width.
    /// * `height` - Display height.
    /// * `stride` - Scanline stride in pixels.
    /// * `pixel_format` - GOP pixel color format.
    ///
    /// # Returns
    ///
    /// A new [`UefiGopFramebuffer`] instance.
    #[must_use]
    pub const fn new(width: u32, height: u32, stride: u32, pixel_format: GopPixelFormat) -> Self {
        Self {
            width,
            height,
            stride,
            pixel_format,
        }
    }

    /// Plots a single colored pixel directly to the GOP memory slice.
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal pixel coordinate.
    /// * `y` - Vertical pixel coordinate.
    /// * `color` - RGBA color value.
    /// * `buffer` - Mutable byte buffer of the linear framebuffer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConsoleInitError`] if pixel is out of screen bounds or buffer is truncated.
    pub fn put_pixel(&self, x: u32, y: u32, color: Color32, buffer: &mut [u8]) -> Result<()> {
        if x >= self.width || y >= self.height {
            return Err(Error::ConsoleInitError {
                device: "uefi-gop".to_string(),
                reason: format!(
                    "coordinates ({}, {}) out of bounds ({}x{})",
                    x, y, self.width, self.height
                ),
            });
        }

        let offset = ((y * self.stride + x) * 4) as usize;
        if offset + 4 > buffer.len() {
            return Err(Error::ConsoleInitError {
                device: "uefi-gop".to_string(),
                reason: "framebuffer slice too small".to_string(),
            });
        }

        self.write_pixel_unchecked(offset, color, buffer);
        Ok(())
    }

    /// Internal helper writing a formatted pixel to a pre-validated buffer offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte index within `buffer`.
    /// * `color` - RGBA color value.
    /// * `buffer` - Mutable byte slice of the linear framebuffer.
    fn write_pixel_unchecked(&self, offset: usize, color: Color32, buffer: &mut [u8]) {
        match self.pixel_format {
            GopPixelFormat::Rgb8 => {
                buffer[offset] = color.r;
                buffer[offset + 1] = color.g;
                buffer[offset + 2] = color.b;
                buffer[offset + 3] = color.a;
            }
            GopPixelFormat::Bgr8 => {
                buffer[offset] = color.b;
                buffer[offset + 1] = color.g;
                buffer[offset + 2] = color.r;
                buffer[offset + 3] = color.a;
            }
            GopPixelFormat::Bitmask { red, green, blue } => {
                let r_val = (u32::from(color.r) * red) / 255;
                let g_val = (u32::from(color.g) * green) / 255;
                let b_val = (u32::from(color.b) * blue) / 255;
                let packed = (r_val & red) | (g_val & green) | (b_val & blue);
                buffer[offset..offset + 4].copy_from_slice(&packed.to_le_bytes());
            }
        }
    }

    /// Clears the entire framebuffer surface to a single solid color.
    ///
    /// # Arguments
    ///
    /// * `color` - Background fill color.
    /// * `buffer` - Mutable framebuffer byte slice.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConsoleInitError`] if buffer length is inadequate.
    pub fn clear(&self, color: Color32, buffer: &mut [u8]) -> Result<()> {
        let total_bytes = (self.height * self.stride * 4) as usize;
        if buffer.len() < total_bytes {
            return Err(Error::ConsoleInitError {
                device: "uefi-gop".to_string(),
                reason: "buffer shorter than required framebuffer size".to_string(),
            });
        }

        for y in 0..self.height {
            for x in 0..self.width {
                let offset = ((y * self.stride + x) * 4) as usize;
                self.write_pixel_unchecked(offset, color, buffer);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests headless console fallback line rendering and dialog generation.
    #[test]
    fn test_headless_console_rendering() {
        let mut console = HeadlessConsole::new();
        console.append_line("Boot initialized");
        assert_eq!(console.lines.len(), 1);

        let dialog = HeadlessConsole::render_dialog_plain(
            "Select Target Disk",
            "Choose destination device:",
            &["/dev/nvme0n1", "/dev/sda"],
        );
        assert!(dialog.contains("Select Target Disk"));
        assert!(dialog.contains("[1] /dev/nvme0n1"));
        assert!(dialog.contains("[2] /dev/sda"));

        let progress = HeadlessConsole::render_progress_line("Copying files", 45);
        assert!(progress.contains("45%"));
        assert!(progress.contains("Copying files"));

        let progress_max = HeadlessConsole::render_progress_line("Complete", 150);
        assert!(progress_max.contains("100%"));
    }

    /// Tests Linux VT escape sequence decoding for function and navigation keys.
    #[test]
    fn test_linux_vt_decoding() {
        test_linux_vt_navigation();
        test_linux_vt_bracket_fkeys();
        test_linux_vt_tilde_fkeys();
        test_linux_vt_ascii_and_control();
    }

    /// Tests Linux VT navigation keys decoding.
    fn test_linux_vt_navigation() {
        // Arrow keys
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[A"),
            Some((TuiKey::Up, 3))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[B"),
            Some((TuiKey::Down, 3))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[C"),
            Some((TuiKey::Right, 3))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[D"),
            Some((TuiKey::Left, 3))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[H"),
            Some((TuiKey::Home, 3))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[F"),
            Some((TuiKey::End, 3))
        );
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1b[Z"), None);

        // Escape standalone and incomplete sequences
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b"),
            Some((TuiKey::Escape, 1))
        );
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1bO"), None);
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1babc"), None);
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1b[99x"), None);
    }

    /// Tests Linux VT double-bracket function keys decoding.
    fn test_linux_vt_bracket_fkeys() {
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[[A"),
            Some((TuiKey::F(1), 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[[B"),
            Some((TuiKey::F(2), 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[[C"),
            Some((TuiKey::F(3), 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[[D"),
            Some((TuiKey::F(4), 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[[E"),
            Some((TuiKey::F(5), 4))
        );
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1b[[Z"), None);
    }

    /// Tests Linux VT tilde-extended navigation and function keys decoding.
    fn test_linux_vt_tilde_fkeys() {
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[1~"),
            Some((TuiKey::Home, 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[7~"),
            Some((TuiKey::Home, 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[4~"),
            Some((TuiKey::End, 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[8~"),
            Some((TuiKey::End, 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[2~"),
            Some((TuiKey::Char(' '), 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[3~"),
            Some((TuiKey::Backspace, 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[5~"),
            Some((TuiKey::PageUp, 4))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[6~"),
            Some((TuiKey::PageDown, 4))
        );

        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[11~"),
            Some((TuiKey::F(1), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[12~"),
            Some((TuiKey::F(2), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[13~"),
            Some((TuiKey::F(3), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[14~"),
            Some((TuiKey::F(4), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[15~"),
            Some((TuiKey::F(5), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[17~"),
            Some((TuiKey::F(6), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[18~"),
            Some((TuiKey::F(7), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[19~"),
            Some((TuiKey::F(8), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[20~"),
            Some((TuiKey::F(9), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[21~"),
            Some((TuiKey::F(10), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[23~"),
            Some((TuiKey::F(11), 5))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x1b[24~"),
            Some((TuiKey::F(12), 5))
        );
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1b[99~"), None);
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x1b[\xFF~"), None);
    }

    /// Tests Linux VT ASCII and control keys decoding.
    fn test_linux_vt_ascii_and_control() {
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\t"),
            Some((TuiKey::Tab, 1))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\r"),
            Some((TuiKey::Enter, 1))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\n"),
            Some((TuiKey::Enter, 1))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x7F"),
            Some((TuiKey::Backspace, 1))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"\x08"),
            Some((TuiKey::Backspace, 1))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b" "),
            Some((TuiKey::Space, 1))
        );
        assert_eq!(
            LinuxVtDriver::decode_vt_sequence(b"q"),
            Some((TuiKey::Char('q'), 1))
        );
        assert_eq!(LinuxVtDriver::decode_vt_sequence(b"\x00"), None);
        assert_eq!(LinuxVtDriver::decode_vt_sequence(&[]), None);
    }

    /// Tests serial console detection from active tty list and kernel commandline.
    #[test]
    fn test_serial_console_detection() {
        let detected_active = SerialConsoleConfig::detect_active("", "tty0 ttyS0");
        assert!(detected_active.is_some());
        let conf1 = detected_active.unwrap_or_default();
        assert_eq!(conf1.port_name, "/dev/ttyS0");

        let detected_usb = SerialConsoleConfig::detect_active("", "ttyUSB0");
        assert!(detected_usb.is_some());
        assert_eq!(detected_usb.unwrap_or_default().port_name, "/dev/ttyUSB0");

        let detected_ama = SerialConsoleConfig::detect_active("", "ttyAMA0");
        assert!(detected_ama.is_some());
        assert_eq!(detected_ama.unwrap_or_default().port_name, "/dev/ttyAMA0");

        let detected_cmdline = SerialConsoleConfig::detect_active(
            "BOOT_IMAGE=/vmlinuz console=ttyS1,57600n8 quiet",
            "",
        );
        assert!(detected_cmdline.is_some());
        let conf2 = detected_cmdline.unwrap_or_default();
        assert_eq!(conf2.port_name, "/dev/ttyS1");
        assert_eq!(conf2.baud_rate, 57_600);

        let detected_cmdline_usb = SerialConsoleConfig::detect_active("console=ttyUSB1 quiet", "");
        assert_eq!(
            detected_cmdline_usb,
            Some(SerialConsoleConfig {
                port_name: "/dev/ttyUSB1".to_string(),
                baud_rate: 115_200,
                data_bits: 8,
                parity: SerialParity::None,
                stop_bits: 1,
            })
        );

        let detected_cmdline_ama = SerialConsoleConfig::detect_active("console=ttyAMA1,9600", "");
        assert_eq!(
            detected_cmdline_ama,
            Some(SerialConsoleConfig {
                port_name: "/dev/ttyAMA1".to_string(),
                baud_rate: 9_600,
                data_bits: 8,
                parity: SerialParity::None,
                stop_bits: 1,
            })
        );

        let detected_cmdline_none = SerialConsoleConfig::detect_active("quiet splash", "tty0");
        assert!(detected_cmdline_none.is_none());

        let detected_cmdline_other = SerialConsoleConfig::detect_active("console=tty1", "");
        assert!(detected_cmdline_other.is_none());

        let default_conf = SerialConsoleConfig::default();
        assert_eq!(default_conf.port_name, "/dev/ttyS0");
        assert_eq!(default_conf.baud_rate, 115_200);
        assert_eq!(default_conf.parity, SerialParity::None);
    }

    /// Tests UEFI GOP framebuffer pixel plotting and clear operations.
    #[test]
    fn test_uefi_gop_framebuffer() {
        let gop_rgb = UefiGopFramebuffer::new(64, 64, 64, GopPixelFormat::Rgb8);
        let mut buffer = vec![0u8; 64 * 64 * 4];

        let red = Color32::from_rgba(255, 0, 0, 255);
        assert!(gop_rgb.put_pixel(10, 10, red, &mut buffer).is_ok());
        let offset = (10 * 64 + 10) * 4;
        assert_eq!(buffer[offset], 255);
        assert_eq!(buffer[offset + 1], 0);

        let gop_bgr = UefiGopFramebuffer::new(64, 64, 64, GopPixelFormat::Bgr8);
        assert!(gop_bgr.put_pixel(10, 10, red, &mut buffer).is_ok());
        assert_eq!(buffer[offset], 0); // blue
        assert_eq!(buffer[offset + 2], 255); // red

        let mask = GopPixelFormat::Bitmask {
            red: 0x00FF_0000,
            green: 0x0000_FF00,
            blue: 0x0000_00FF,
        };
        let gop_mask = UefiGopFramebuffer::new(64, 64, 64, mask);
        assert!(gop_mask.put_pixel(5, 5, red, &mut buffer).is_ok());

        // Test clear on valid buffer
        assert!(gop_rgb.clear(red, &mut buffer).is_ok());

        // Out of bounds checks
        assert!(gop_rgb.put_pixel(100, 10, red, &mut buffer).is_err());
        assert!(gop_rgb.put_pixel(10, 100, red, &mut buffer).is_err());
        let mut small_buf = vec![0u8; 2];
        assert!(gop_rgb.put_pixel(0, 0, red, &mut small_buf).is_err());
        assert!(gop_rgb.clear(red, &mut small_buf).is_err());
    }

    /// Tests `ConsoleKind` enum derives and equality.
    #[test]
    fn test_console_kind_derives() {
        let kind = ConsoleKind::AnsiTerminal;
        let kind_clone = kind;
        assert_eq!(kind, kind_clone);
        assert_ne!(ConsoleKind::LinuxVirtualTerminal, ConsoleKind::SerialPort);
        assert_ne!(ConsoleKind::UefiGop, ConsoleKind::HeadlessFallback);
        assert!(format!("{kind:?}").contains("AnsiTerminal"));
    }

    /// Tests `SerialParity` enum variants and defaults.
    #[test]
    fn test_serial_parity_variants() {
        assert_eq!(SerialParity::default(), SerialParity::None);
        assert_ne!(SerialParity::Even, SerialParity::Odd);
        assert!(format!("{:?}", SerialParity::Even).contains("Even"));
    }
}
