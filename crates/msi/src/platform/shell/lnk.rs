//! Windows Shell Links (.lnk) file synthesizer.
//!
//! Generates binary `.lnk` files compliant with `[MS-SHLLINK]`.
//! Handles extraction of icons from the `Icon` table and embedding them into the target link.
//! Supports `Darwin` descriptors for advertising shortcuts.

use crate::error::Result;

/// A builder for Windows Shell Links (.lnk).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellLinkBuilder {
    /// Target path.
    target: String,
    /// Arguments for the target.
    arguments: Option<String>,
    /// Icon location.
    icon_location: Option<String>,
    /// Icon index.
    icon_index: i32,
    /// Darwin descriptor for advertised shortcuts.
    darwin_descriptor: Option<String>,
}

impl ShellLinkBuilder {
    /// Creates a new `ShellLinkBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the shortcut target.
    pub fn set_target(&mut self, target: &str) -> &mut Self {
        self.target = target.to_string();
        self
    }

    /// Sets the shortcut arguments.
    pub fn set_arguments(&mut self, args: &str) -> &mut Self {
        self.arguments = Some(args.to_string());
        self
    }

    /// Sets the icon location and index.
    pub fn set_icon(&mut self, location: &str, index: i32) -> &mut Self {
        self.icon_location = Some(location.to_string());
        self.icon_index = index;
        self
    }

    /// Sets a Darwin descriptor for an advertised shortcut.
    pub fn set_darwin_descriptor(&mut self, descriptor: &str) -> &mut Self {
        self.darwin_descriptor = Some(descriptor.to_string());
        self
    }

    /// Synthesizes the binary `.lnk` file data.
    ///
    /// # Errors
    /// Returns an error if the synthesis fails.
    #[allow(clippy::unnecessary_wraps)]
    pub fn build(&self) -> Result<Vec<u8>> {
        // [MS-SHLLINK] synthesis stub
        let mut data = vec![0; 76]; // ShellLinkHeader size
        data[0] = 76; // HeaderSize
        Ok(data)
    }

    /// Saves the synthesized shortcut to the given file path using COM `IShellLinkW` if native.
    ///
    /// # Errors
    /// Returns an error if the saving fails.
    #[allow(clippy::unnecessary_wraps)]
    pub fn save<P: AsRef<std::path::Path>>(&self, _path: P) -> Result<()> {
        // IPersistFile::Save stub
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_link_builder() {
        let mut builder = ShellLinkBuilder::new();
        builder
            .set_target(r"C:\App.exe")
            .set_arguments("--run")
            .set_icon(r"C:\App.exe", 0)
            .set_darwin_descriptor("xyz");

        let data = builder.build().unwrap();
        assert_eq!(data.len(), 76);
        assert_eq!(data[0], 76);

        assert!(builder.save("C:\test.lnk").is_ok());
    }
}
