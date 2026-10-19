//! Windows Shell Links (`.lnk`) binary file synthesizer.
//!
//! Generates binary `.lnk` files compliant with Microsoft Open Specification `[MS-SHLLINK]`
//! (Shell Link Binary File Format, v2.1). Supports target paths, command-line arguments,
//! icon location and index, and `DarwinDataBlock` (`0xA000_0006`) for Windows Installer
//! advertised shortcuts.

use crate::error::{IoContext, MsiError, Result};
use std::fs;
use std::path::Path;

/// `[MS-SHLLINK]` `HeaderSize`: `0x0000_004C` (76 bytes).
pub const LNK_HEADER_SIZE: u32 = 0x0000_004C;

/// `[MS-SHLLINK]` `LinkCLSID`: `00021401-0000-0000-C000-000000000046` in little-endian wire layout.
pub const LNK_CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

/// `LinkFlags`: `HasName` (`0x0000_0004`) - `NAME_STRING` is present in `StringData`.
pub const LINK_FLAG_HAS_NAME: u32 = 0x0000_0004;

/// `LinkFlags`: `HasArguments` (`0x0000_0020`) - `COMMAND_LINE_ARGUMENTS` is present in `StringData`.
pub const LINK_FLAG_HAS_ARGUMENTS: u32 = 0x0000_0020;

/// `LinkFlags`: `HasIconLocation` (`0x0000_0040`) - `ICON_LOCATION` is present in `StringData`.
pub const LINK_FLAG_HAS_ICON_LOCATION: u32 = 0x0000_0040;

/// `LinkFlags`: `IsUnicode` (`0x0000_0080`) - `StringData` strings are encoded in UTF-16LE.
pub const LINK_FLAG_IS_UNICODE: u32 = 0x0000_0080;

/// `LinkFlags`: `HasDarwinID` (`0x0000_1000`) - `DarwinDataBlock` is present in `ExtraData`.
pub const LINK_FLAG_HAS_DARWIN_ID: u32 = 0x0000_1000;

/// `[MS-SHLLINK]` `SW_SHOWNORMAL` (`0x0000_0001`).
pub const SW_SHOWNORMAL: u32 = 0x0000_0001;

/// `[MS-SHLLINK]` `DarwinDataBlock` signature (`0xA000_0006`).
pub const DARWIN_DATA_BLOCK_SIGNATURE: u32 = 0xA000_0006;

/// `[MS-SHLLINK]` `DarwinDataBlock` total byte size (`0x0000_0314` = 788 bytes).
pub const DARWIN_DATA_BLOCK_SIZE: u32 = 0x0000_0314;

/// A builder for Windows Shell Links (`.lnk`) per `[MS-SHLLINK]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellLinkBuilder {
    /// Target filesystem path for the shortcut.
    target: String,
    /// Optional command-line arguments passed to the target executable.
    arguments: Option<String>,
    /// Optional icon file path.
    icon_location: Option<String>,
    /// Zero-based or resource ID icon index within `icon_location`.
    icon_index: i32,
    /// Optional Windows Installer Darwin descriptor for advertised/self-healing shortcuts.
    darwin_descriptor: Option<String>,
}

impl ShellLinkBuilder {
    /// Creates a new default [`ShellLinkBuilder`].
    ///
    /// # Returns
    ///
    /// An empty [`ShellLinkBuilder`] ready for configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the shortcut target path.
    ///
    /// # Arguments
    ///
    /// * `target` - Filesystem path to the target executable or document.
    ///
    /// # Returns
    ///
    /// Mutable reference to `self` for method chaining.
    pub fn set_target(&mut self, target: &str) -> &mut Self {
        self.target = target.to_string();
        self
    }

    /// Sets the command-line arguments for the shortcut target.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument string.
    ///
    /// # Returns
    ///
    /// Mutable reference to `self` for method chaining.
    pub fn set_arguments(&mut self, args: &str) -> &mut Self {
        self.arguments = Some(args.to_string());
        self
    }

    /// Sets the icon location and icon index.
    ///
    /// # Arguments
    ///
    /// * `location` - Path to the `.ico`, `.exe`, or `.dll` containing the icon.
    /// * `index` - Icon index within the resource file.
    ///
    /// # Returns
    ///
    /// Mutable reference to `self` for method chaining.
    pub fn set_icon(&mut self, location: &str, index: i32) -> &mut Self {
        self.icon_location = Some(location.to_string());
        self.icon_index = index;
        self
    }

    /// Sets a Windows Installer Darwin descriptor for an advertised shortcut.
    ///
    /// # Arguments
    ///
    /// * `descriptor` - Base85-encoded Darwin product/feature/component descriptor.
    ///
    /// # Returns
    ///
    /// Mutable reference to `self` for method chaining.
    pub fn set_darwin_descriptor(&mut self, descriptor: &str) -> &mut Self {
        self.darwin_descriptor = Some(descriptor.to_string());
        self
    }

    /// Encodes a string as an `[MS-SHLLINK]` `CountedUnicodeString` (16-bit character count + UTF-16LE code units).
    ///
    /// # Arguments
    ///
    /// * `out` - Destination byte buffer.
    /// * `val` - String to encode.
    fn write_counted_unicode_string(out: &mut Vec<u8>, val: &str) {
        let utf16: Vec<u16> = val.encode_utf16().collect();
        let count = u16::try_from(utf16.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&count.to_le_bytes());
        for unit in utf16.into_iter().take(usize::from(count)) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }

    /// Synthesizes the binary `.lnk` file data according to `[MS-SHLLINK]`.
    ///
    /// Emits the 76-byte `ShellLinkHeader` (`HeaderSize` `0x4C`, `LinkCLSID`, `LinkFlags`,
    /// `FileAttributes`, timestamps, `IconIndex`, `ShowCommand`), followed by `StringData`
    /// (`NAME_STRING`, `COMMAND_LINE_ARGUMENTS`, `ICON_LOCATION`), `DarwinDataBlock` (if set),
    /// and a 4-byte `TerminalBlock` (`0x0000_0000`) when extra blocks or strings are present.
    ///
    /// # Returns
    ///
    /// Synthesized `.lnk` byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if both `target` and `darwin_descriptor` are empty.
    pub fn build(&self) -> Result<Vec<u8>> {
        if self.target.is_empty() && self.darwin_descriptor.is_none() {
            return Err(MsiError::Validation {
                element: "ShellLinkBuilder.target".to_string(),
                reason: "Shortcut must specify a target path or Darwin descriptor".to_string(),
            });
        }

        let mut flags = LINK_FLAG_IS_UNICODE;
        if !self.target.is_empty() {
            flags |= LINK_FLAG_HAS_NAME;
        }
        if self.arguments.is_some() {
            flags |= LINK_FLAG_HAS_ARGUMENTS;
        }
        if self.icon_location.is_some() {
            flags |= LINK_FLAG_HAS_ICON_LOCATION;
        }
        if self.darwin_descriptor.is_some() {
            flags |= LINK_FLAG_HAS_DARWIN_ID;
        }

        let mut data = vec![0u8; 76];
        // 0..4: HeaderSize (0x0000004C)
        data[0..4].copy_from_slice(&LNK_HEADER_SIZE.to_le_bytes());
        // 4..20: LinkCLSID (00021401-0000-0000-C000-000000000046)
        data[4..20].copy_from_slice(&LNK_CLSID);
        // 20..24: LinkFlags
        data[20..24].copy_from_slice(&flags.to_le_bytes());
        // 24..28: FileAttributes (FILE_ATTRIBUTE_NORMAL = 0x80)
        data[24..28].copy_from_slice(&0x0000_0080_u32.to_le_bytes());
        // 56..60: IconIndex
        data[56..60].copy_from_slice(&self.icon_index.to_le_bytes());
        // 60..64: ShowCommand (SW_SHOWNORMAL = 1)
        data[60..64].copy_from_slice(&SW_SHOWNORMAL.to_le_bytes());

        // StringData section
        if !self.target.is_empty() {
            Self::write_counted_unicode_string(&mut data, &self.target);
        }
        if let Some(ref args) = self.arguments {
            Self::write_counted_unicode_string(&mut data, args);
        }
        if let Some(ref icon) = self.icon_location {
            Self::write_counted_unicode_string(&mut data, icon);
        }

        // ExtraData: DarwinDataBlock (788 bytes) + TerminalBlock (4 bytes)
        if let Some(ref darwin) = self.darwin_descriptor {
            let mut block = vec![0u8; DARWIN_DATA_BLOCK_SIZE as usize];
            block[0..4].copy_from_slice(&DARWIN_DATA_BLOCK_SIZE.to_le_bytes());
            block[4..8].copy_from_slice(&DARWIN_DATA_BLOCK_SIGNATURE.to_le_bytes());

            // 8..268: DarwinDataAnsi (260 bytes null-terminated ANSI)
            let ansi_bytes = darwin.as_bytes();
            let copy_len = ansi_bytes.len().min(259);
            block[8..8 + copy_len].copy_from_slice(&ansi_bytes[..copy_len]);

            // 268..788: DarwinDataUnicode (260 UTF-16LE code units = 520 bytes)
            for (idx, unit) in darwin.encode_utf16().take(259).enumerate() {
                let offset = 268 + idx * 2;
                block[offset..offset + 2].copy_from_slice(&unit.to_le_bytes());
            }

            data.extend_from_slice(&block);
        }

        // TerminalBlock (4 zero bytes)
        data.extend_from_slice(&0u32.to_le_bytes());

        Ok(data)
    }

    /// Saves the synthesized `.lnk` binary shortcut to the specified file path on disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination filesystem path where the `.lnk` file will be written.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the `.lnk` file is synthesized and written to disk.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if building the `.lnk` payload fails or writing to `path` fails.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.save_to_path(path.as_ref())
    }

    /// Writes the `.lnk` binary payload to `target_path`, creating parent directories as needed.
    fn save_to_path(&self, target_path: &Path) -> Result<()> {
        let bytes = self.build()?;
        if let Some(parent) = target_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(IoContext)?;
        }
        fs::write(target_path, bytes).map_err(IoContext)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies `[MS-SHLLINK]` binary structure, flags, `DarwinDataBlock`, and file persistence.
    #[test]
    fn test_shell_link_builder_and_save() -> Result<()> {
        let mut builder = ShellLinkBuilder::new();
        assert!(builder.build().is_err());

        builder
            .set_target(r"C:\App.exe")
            .set_arguments("--run")
            .set_icon(r"C:\App.exe", 2)
            .set_darwin_descriptor("DarwinDesc123");

        let cloned = builder.clone();
        assert_eq!(builder, cloned);
        let b_dbg = format!("{builder:?}");
        assert!(b_dbg.contains("ShellLinkBuilder"));

        let data = builder.build()?;
        assert!(data.len() > 76);
        assert_eq!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]), 76);
        assert_eq!(&data[4..20], &LNK_CLSID);

        let flags = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        assert_ne!(flags & LINK_FLAG_IS_UNICODE, 0);
        assert_ne!(flags & LINK_FLAG_HAS_NAME, 0);
        assert_ne!(flags & LINK_FLAG_HAS_ARGUMENTS, 0);
        assert_ne!(flags & LINK_FLAG_HAS_ICON_LOCATION, 0);
        assert_ne!(flags & LINK_FLAG_HAS_DARWIN_ID, 0);

        let icon_idx = i32::from_le_bytes([data[56], data[57], data[58], data[59]]);
        assert_eq!(icon_idx, 2);

        let temp_dir = tempfile::tempdir().map_err(IoContext)?;
        let lnk_path = temp_dir.path().join("nested").join("test.lnk");
        builder.save(&lnk_path)?;

        let read_back = fs::read(&lnk_path).map_err(IoContext)?;
        assert_eq!(read_back, data);

        // Save to relative filename without parent directory creation
        let local_lnk = temp_dir.path().join("direct.lnk");
        let mut minimal = ShellLinkBuilder::new();
        minimal.set_target(r"C:\Minimal.exe");
        minimal.save(&local_lnk)?;
        assert!(local_lnk.exists());

        // Save with empty parent path branch
        let mut darwin_only = ShellLinkBuilder::new();
        darwin_only.set_darwin_descriptor("OnlyDarwin");
        let d_bytes = darwin_only.build()?;
        assert!(d_bytes.len() > 76 + 788);

        // Save to an invalid path (directory as file) triggers error
        assert!(minimal.save(temp_dir.path()).is_err());

        // Save where parent is an existing file triggers create_dir_all error
        let invalid_nested = local_lnk.join("child.lnk");
        assert!(minimal.save(&invalid_nested).is_err());

        // Test save with a relative filename that has an empty parent component
        let prev_dir = std::env::current_dir().map_err(IoContext)?;
        std::env::set_current_dir(temp_dir.path()).map_err(IoContext)?;
        let rel_res = minimal.save("rel_only.lnk");
        let _ = std::env::set_current_dir(prev_dir);
        rel_res?;

        Ok(())
    }
}
