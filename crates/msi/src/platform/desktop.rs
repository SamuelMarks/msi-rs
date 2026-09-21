//! Desktop Integration & Shell Links (Freedesktop XDG and macOS Application Bundles).
//!
//! Grounded directly in Freedesktop.org Desktop Entry Specification and Apple Application Bundle guidelines:
//! - Translates Windows Installer `Shortcut` table entries into standards-compliant `.desktop` files.
//! - Generates macOS Application Bundles (`<Product>.app/Contents/Info.plist`) with executable structure.
//! - Provides commands for cache updates (`update-desktop-database`, `gtk-update-icon-cache`, `lsregister`).

use std::path::{Path, PathBuf};

/// Freedesktop XDG Desktop Entry (`.desktop`) representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgDesktopEntry {
    /// Application display name.
    pub name: String,
    /// Executable launch command and parameters.
    pub exec: String,
    /// Icon theme name or icon file path.
    pub icon: Option<String>,
    /// Whether application runs inside a terminal console.
    pub terminal: bool,
    /// Desktop menu categories (e.g. `["Utility", "Development"]`).
    pub categories: Vec<String>,
    /// Supported MIME types (e.g. `["application/x-custom"]`).
    pub mime_types: Vec<String>,
    /// Tooltip or description comment.
    pub comment: Option<String>,
}

impl XdgDesktopEntry {
    /// Creates a new [`XdgDesktopEntry`].
    ///
    /// # Arguments
    ///
    /// * `name` - Display name.
    /// * `exec` - Executable launch command.
    ///
    /// # Returns
    ///
    /// A new [`XdgDesktopEntry`].
    #[must_use]
    pub fn new(name: impl Into<String>, exec: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exec: exec.into(),
            icon: None,
            terminal: false,
            categories: Vec::new(),
            mime_types: Vec::new(),
            comment: None,
        }
    }

    /// Sets the icon name or path.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets whether the application runs in a terminal.
    #[must_use]
    pub const fn terminal(mut self, terminal: bool) -> Self {
        self.terminal = terminal;
        self
    }

    /// Adds a desktop menu category.
    #[must_use]
    pub fn category(mut self, cat: impl Into<String>) -> Self {
        self.categories.push(cat.into());
        self
    }

    /// Adds a supported MIME type.
    #[must_use]
    pub fn mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_types.push(mime.into());
        self
    }

    /// Sets the description comment.
    #[must_use]
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Returns the target `.desktop` installation path.
    ///
    /// # Arguments
    ///
    /// * `is_system` - `true` for `/usr/share/applications`, `false` for `~/.local/share/applications`.
    #[must_use]
    pub fn destination_path(&self, is_system: bool) -> PathBuf {
        let slug = self.name.to_ascii_lowercase().replace(' ', "-");
        let base = if is_system {
            PathBuf::from("/usr/share/applications")
        } else {
            PathBuf::from("~/.local/share/applications")
        };
        base.join(format!("{slug}.desktop"))
    }

    /// Generates complete INI-formatted `.desktop` file content.
    ///
    /// # Returns
    ///
    /// File content string.
    #[must_use]
    #[allow(clippy::format_push_string)]
    pub fn generate_desktop_file(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "[Desktop Entry]
",
        );
        out.push_str(
            "Type=Application
",
        );
        out.push_str(&format!(
            "Name={}
",
            self.name
        ));
        out.push_str(&format!(
            "Exec={}
",
            self.exec
        ));

        if let Some(ref c) = self.comment {
            out.push_str(&format!(
                "Comment={c}
"
            ));
        }
        if let Some(ref ic) = self.icon {
            out.push_str(&format!(
                "Icon={ic}
"
            ));
        }
        out.push_str(&format!(
            "Terminal={}
",
            if self.terminal { "true" } else { "false" }
        ));

        if !self.categories.is_empty() {
            out.push_str(&format!(
                "Categories={};
",
                self.categories.join(";")
            ));
        }
        if !self.mime_types.is_empty() {
            out.push_str(&format!(
                "MimeType={};
",
                self.mime_types.join(";")
            ));
        }

        out
    }

    /// Returns post-installation cache update commands.
    ///
    /// # Returns
    ///
    /// Sequence of shell command strings.
    #[must_use]
    pub fn desktop_database_update_commands() -> Vec<String> {
        vec![
            "update-desktop-database /usr/share/applications".to_string(),
            "gtk-update-icon-cache -f -t /usr/share/icons/hicolor".to_string(),
        ]
    }
}

/// Apple macOS Application Bundle generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsAppBundle {
    /// Application product name.
    pub product_name: String,
    /// Apple Bundle Identifier (e.g. `com.company.product`).
    pub bundle_identifier: String,
    /// Marketing or version string (e.g. `1.0.0`).
    pub version: String,
    /// Main executable binary name inside `Contents/MacOS/`.
    pub executable_name: String,
    /// Main icon file name inside `Contents/Resources/` (e.g. `AppIcon.icns`).
    pub icon_file: Option<String>,
}

impl MacOsAppBundle {
    /// Creates a new [`MacOsAppBundle`] specification.
    ///
    /// # Arguments
    ///
    /// * `product_name` - Application title.
    /// * `bundle_id` - Bundle identifier.
    /// * `version` - Version string.
    /// * `executable` - Main binary name.
    ///
    /// # Returns
    ///
    /// A new [`MacOsAppBundle`].
    #[must_use]
    pub fn new(
        product_name: impl Into<String>,
        bundle_id: impl Into<String>,
        version: impl Into<String>,
        executable: impl Into<String>,
    ) -> Self {
        Self {
            product_name: product_name.into(),
            bundle_identifier: bundle_id.into(),
            version: version.into(),
            executable_name: executable.into(),
            icon_file: None,
        }
    }

    /// Sets the icon filename.
    #[must_use]
    pub fn icon_file(mut self, icon: impl Into<String>) -> Self {
        self.icon_file = Some(icon.into());
        self
    }

    /// Returns the bundle root directory name (e.g. `<Product>.app`).
    #[must_use]
    pub fn bundle_dir_name(&self) -> String {
        format!("{}.app", self.product_name)
    }

    /// Returns relative directory paths within the `.app` bundle structure.
    ///
    /// # Returns
    ///
    /// Vector of relative paths: `Contents/MacOS`, `Contents/Resources`.
    #[must_use]
    pub fn required_directories(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("Contents/MacOS"),
            PathBuf::from("Contents/Resources"),
        ]
    }

    /// Generates XML `Info.plist` content for the application bundle.
    ///
    /// # Returns
    ///
    /// XML string.
    #[must_use]
    #[allow(clippy::format_push_string)]
    pub fn generate_info_plist(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
        out.push_str("<plist version=\"1.0\">\n");
        out.push_str("<dict>\n");
        out.push_str("    <key>CFBundlePackageType</key>\n    <string>APPL</string>\n");
        out.push_str(&format!(
            "    <key>CFBundleName</key>\n    <string>{}</string>\n",
            self.product_name
        ));
        out.push_str(&format!(
            "    <key>CFBundleDisplayName</key>\n    <string>{}</string>\n",
            self.product_name
        ));
        out.push_str(&format!(
            "    <key>CFBundleIdentifier</key>\n    <string>{}</string>\n",
            self.bundle_identifier
        ));
        out.push_str(&format!(
            "    <key>CFBundleVersion</key>\n    <string>{}</string>\n",
            self.version
        ));
        out.push_str(&format!(
            "    <key>CFBundleShortVersionString</key>\n    <string>{}</string>\n",
            self.version
        ));
        out.push_str(&format!(
            "    <key>CFBundleExecutable</key>\n    <string>{}</string>\n",
            self.executable_name
        ));

        if let Some(ref icon) = self.icon_file {
            out.push_str(&format!(
                "    <key>CFBundleIconFile</key>\n    <string>{icon}</string>\n"
            ));
        }

        out.push_str("</dict>\n");
        out.push_str("</plist>\n");

        out
    }

    /// Returns the `LaunchServices` registration command (`lsregister`).
    ///
    /// # Arguments
    ///
    /// * `bundle_path` - Path to the `.app` bundle directory.
    ///
    /// # Returns
    ///
    /// Shell command string.
    #[must_use]
    pub fn launch_services_register_command(bundle_path: &Path) -> String {
        format!(
            "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f {}",
            bundle_path.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests XDG desktop entry generation and properties.
    #[test]
    fn test_xdg_desktop_entry() {
        let entry = XdgDesktopEntry::new("Acme Studio", "/usr/bin/acme-studio %F")
            .comment("Acme Professional Suite")
            .icon("acme-studio")
            .terminal(false)
            .category("Development")
            .category("IDE")
            .mime_type("text/x-acme");

        assert_eq!(
            entry.destination_path(true),
            PathBuf::from("/usr/share/applications/acme-studio.desktop")
        );
        assert_eq!(
            entry.destination_path(false),
            PathBuf::from("~/.local/share/applications/acme-studio.desktop")
        );

        let content = entry.generate_desktop_file();
        assert!(content.contains("[Desktop Entry]"));
        assert!(content.contains("Type=Application"));
        assert!(content.contains("Name=Acme Studio"));
        assert!(content.contains("Exec=/usr/bin/acme-studio %F"));
        assert!(content.contains("Icon=acme-studio"));
        assert!(content.contains("Terminal=false"));
        assert!(content.contains("Categories=Development;IDE;"));
        assert!(content.contains("MimeType=text/x-acme;"));

        let cmds = XdgDesktopEntry::desktop_database_update_commands();
        assert_eq!(cmds.len(), 2);

        // Minimal entry with terminal=true, no comment, icon, categories, or mime types
        let min_entry = XdgDesktopEntry::new("Acme CLI", "/usr/bin/acme-cli").terminal(true);
        let min_content = min_entry.generate_desktop_file();
        assert!(min_content.contains("Terminal=true"));
        assert!(!min_content.contains("Comment="));
        assert!(!min_content.contains("Icon="));
        assert!(!min_content.contains("Categories="));
        assert!(!min_content.contains("MimeType="));
    }

    /// Tests macOS `.app` bundle structure and Info.plist generation.
    #[test]
    fn test_macos_app_bundle() {
        let bundle = MacOsAppBundle::new("Studio", "com.acme.studio", "2.1.0", "studio")
            .icon_file("AppIcon.icns");

        assert_eq!(bundle.bundle_dir_name(), "Studio.app");
        let dirs = bundle.required_directories();
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("Contents/MacOS"),
                PathBuf::from("Contents/Resources"),
            ]
        );

        let plist = bundle.generate_info_plist();
        assert!(plist.contains("<key>CFBundleName</key>\n    <string>Studio</string>"));
        assert!(
            plist.contains("<key>CFBundleIdentifier</key>\n    <string>com.acme.studio</string>")
        );
        assert!(plist.contains("<key>CFBundleVersion</key>\n    <string>2.1.0</string>"));
        assert!(plist.contains("<key>CFBundleExecutable</key>\n    <string>studio</string>"));
        assert!(plist.contains("<key>CFBundleIconFile</key>\n    <string>AppIcon.icns</string>"));

        // Bundle without icon file
        let min_bundle = MacOsAppBundle::new("StudioCLI", "com.acme.cli", "1.0.0", "cli");
        let min_plist = min_bundle.generate_info_plist();
        assert!(!min_plist.contains("CFBundleIconFile"));

        let reg_cmd =
            MacOsAppBundle::launch_services_register_command(Path::new("/Applications/Studio.app"));
        assert!(reg_cmd.contains("lsregister -f /Applications/Studio.app"));
    }
}
