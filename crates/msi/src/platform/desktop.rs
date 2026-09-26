//! Desktop Integration & Shell Links (Freedesktop XDG and macOS Application Bundles).
//!
//! Grounded directly in Freedesktop.org Desktop Entry Specification and Apple Application Bundle guidelines:
//! - Translates Windows Installer `Shortcut` table entries into standards-compliant `.desktop` files.
//! - Generates macOS Application Bundles (`<Product>.app/Contents/Info.plist`) with executable structure.
//! - Provides commands for cache updates (`update-desktop-database`, `gtk-update-icon-cache`, `lsregister`).

#[allow(unused_imports)]
use crate::error::Error;
use crate::error::Result;
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
    pub fn new(name: &str, exec: &str) -> Self {
        Self {
            name: name.to_string(),
            exec: exec.to_string(),
            icon: None,
            terminal: false,
            categories: Vec::new(),
            mime_types: Vec::new(),
            comment: None,
        }
    }

    /// Sets the icon name or path.
    #[must_use]
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
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
    pub fn category(mut self, cat: &str) -> Self {
        self.categories.push(cat.to_string());
        self
    }

    /// Adds a supported MIME type.
    #[must_use]
    pub fn mime_type(mut self, mime: &str) -> Self {
        self.mime_types.push(mime.to_string());
        self
    }

    /// Sets the description comment.
    #[must_use]
    pub fn comment(mut self, comment: &str) -> Self {
        self.comment = Some(comment.to_string());
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

    /// Returns the system applications directory (`/usr/share/applications`).
    #[must_use]
    pub fn system_applications_dir() -> PathBuf {
        PathBuf::from("/usr/share/applications")
    }

    /// Returns the user applications directory (`~/.local/share/applications`).
    #[must_use]
    pub fn user_applications_dir() -> PathBuf {
        PathBuf::from("~/.local/share/applications")
    }

    /// Writes this `.desktop` entry file into the specified directory.
    ///
    /// # Arguments
    ///
    /// * `dir` - Target directory path.
    ///
    /// # Returns
    ///
    /// The written `.desktop` file [`PathBuf`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on filesystem write failure.
    pub fn install_to_directory(&self, dir: &Path) -> Result<PathBuf> {
        let slug = self.name.to_lowercase().replace(' ', "-");
        let file_path = dir.join(format!("{slug}.desktop"));
        std::fs::create_dir_all(dir)?;
        std::fs::write(&file_path, self.generate_desktop_file())?;
        Ok(file_path)
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
    pub fn new(product_name: &str, bundle_id: &str, version: &str, executable: &str) -> Self {
        Self {
            product_name: product_name.to_string(),
            bundle_identifier: bundle_id.to_string(),
            version: version.to_string(),
            executable_name: executable.to_string(),
            icon_file: None,
        }
    }

    /// Sets the icon filename.
    #[must_use]
    pub fn icon_file(mut self, icon: &str) -> Self {
        self.icon_file = Some(icon.to_string());
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

    /// Creates a symbolic link in `/Applications` (or custom path) pointing to the application bundle directory.
    ///
    /// # Arguments
    ///
    /// * `bundle_path` - Path to the actual installed `.app` bundle directory.
    /// * `link_name` - Optional custom name for the symlink (defaults to bundle directory name).
    ///
    /// # Returns
    ///
    /// Path to the created symlink.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem error.
    pub fn create_applications_symlink(
        bundle_path: &Path,
        link_name: Option<&str>,
    ) -> Result<PathBuf> {
        let name = match link_name {
            Some(n)
                if Path::new(n)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("app")) =>
            {
                n.to_string()
            }
            Some(n) => format!("{n}.app"),
            None => bundle_path.file_name().map_or_else(
                || "App.app".to_string(),
                |f| f.to_string_lossy().to_string(),
            ),
        };
        let link_target = PathBuf::from("/Applications").join(name);
        #[cfg(unix)]
        {
            if link_target.exists() || link_target.is_symlink() {
                let _ = std::fs::remove_file(&link_target);
            }
            std::os::unix::fs::symlink(bundle_path, &link_target)?;
        }
        #[cfg(not(unix))]
        {
            let _ = (bundle_path, &link_target);
        }
        Ok(link_target)
    }
}

/// Binary `.lnk` Shell Link generator compliant with MS-SHLLINK (Shell Link Binary File Format).
///
/// Supports icon resource associations, command line arguments, working directories, and window states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Win32ShellLink {
    /// Target application or file path.
    target_path: String,
    /// Optional command-line arguments.
    arguments: Option<String>,
    /// Optional working directory path.
    working_dir: Option<String>,
    /// Optional icon location path.
    icon_location: Option<String>,
    /// Zero-based icon resource index.
    icon_index: i32,
    /// Optional link description / tooltip.
    description: Option<String>,
    /// Window show command (1: `SW_SHOWNORMAL`, 3: `SW_SHOWMAXIMIZED`, 7: `SW_SHOWMINNOACTIVE`).
    show_command: u32,
}

impl Win32ShellLink {
    /// Creates a new [`Win32ShellLink`] pointing to a target path.
    ///
    /// # Arguments
    ///
    /// * `target_path` - Path to the target binary or script.
    ///
    /// # Returns
    ///
    /// A new [`Win32ShellLink`] instance.
    #[must_use]
    pub fn new(target_path: &str) -> Self {
        Self {
            target_path: target_path.to_string(),
            arguments: None,
            working_dir: None,
            icon_location: None,
            icon_index: 0,
            description: None,
            show_command: 1, // SW_SHOWNORMAL
        }
    }

    /// Sets the command-line arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - Argument string.
    ///
    /// # Returns
    ///
    /// Updated [`Win32ShellLink`].
    #[must_use]
    pub fn arguments(mut self, args: &str) -> Self {
        self.arguments = Some(args.to_string());
        self
    }

    /// Sets the working directory.
    ///
    /// # Arguments
    ///
    /// * `dir` - Working directory path.
    ///
    /// # Returns
    ///
    /// Updated [`Win32ShellLink`].
    #[must_use]
    pub fn working_dir(mut self, dir: &str) -> Self {
        self.working_dir = Some(dir.to_string());
        self
    }

    /// Sets the icon resource location and index.
    ///
    /// # Arguments
    ///
    /// * `path` - Icon resource path (e.g. `.ico`, `.exe`, or `.dll`).
    /// * `index` - Icon resource index.
    ///
    /// # Returns
    ///
    /// Updated [`Win32ShellLink`].
    #[must_use]
    pub fn icon(mut self, path: &str, index: i32) -> Self {
        self.icon_location = Some(path.to_string());
        self.icon_index = index;
        self
    }

    /// Sets the link description / tooltip.
    ///
    /// # Arguments
    ///
    /// * `desc` - Description text.
    ///
    /// # Returns
    ///
    /// Updated [`Win32ShellLink`].
    #[must_use]
    pub fn description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Sets the window show command.
    ///
    /// # Arguments
    ///
    /// * `cmd` - Show command (`1` for Normal, `3` for Maximized, `7` for Minimized).
    ///
    /// # Returns
    ///
    /// Updated [`Win32ShellLink`].
    #[must_use]
    pub const fn show_command(mut self, cmd: u32) -> Self {
        self.show_command = cmd;
        self
    }

    /// Serializes the shell link into a standard binary `.lnk` byte stream conforming to MS-SHLLINK.
    ///
    /// # Returns
    ///
    /// Byte vector containing the valid binary `.lnk` file.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(512);

        // 1. ShellLinkHeader (76 bytes = 0x0000004C)
        buf.extend_from_slice(&0x0000_004Cu32.to_le_bytes()); // HeaderSize
                                                              // LinkCLSID: 00021401-0000-0000-C000-000000000046
        buf.extend_from_slice(&[
            0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x46,
        ]);

        let has_name = self.description.is_some();
        let has_rel_path = !self.target_path.is_empty();
        let has_working_dir = self.working_dir.is_some();
        let has_arguments = self.arguments.is_some();
        let has_icon_loc = self.icon_location.is_some();

        let mut flags: u32 = 0x0000_0080; // IsUnicode
        if has_name {
            flags |= 0x0000_0004; // HasName
        }
        if has_rel_path {
            flags |= 0x0000_0008; // HasRelativePath
        }
        if has_working_dir {
            flags |= 0x0000_0010; // HasWorkingDir
        }
        if has_arguments {
            flags |= 0x0000_0020; // HasArguments
        }
        if has_icon_loc {
            flags |= 0x0000_0040; // HasIconLocation
        }
        buf.extend_from_slice(&flags.to_le_bytes()); // LinkFlags

        buf.extend_from_slice(&0x0000_0020u32.to_le_bytes()); // FileAttributes: FILE_ATTRIBUTE_ARCHIVE
        buf.extend_from_slice(&[0u8; 8]); // CreationTime (FILETIME)
        buf.extend_from_slice(&[0u8; 8]); // AccessTime (FILETIME)
        buf.extend_from_slice(&[0u8; 8]); // WriteTime (FILETIME)
        buf.extend_from_slice(&0u32.to_le_bytes()); // FileSize
        buf.extend_from_slice(&self.icon_index.to_le_bytes()); // IconIndex
        buf.extend_from_slice(&self.show_command.to_le_bytes()); // ShowCommand
        buf.extend_from_slice(&0u16.to_le_bytes()); // HotKey
        buf.extend_from_slice(&0u16.to_le_bytes()); // Reserved1
        buf.extend_from_slice(&0u32.to_le_bytes()); // Reserved2
        buf.extend_from_slice(&0u32.to_le_bytes()); // Reserved3

        // 2. String Data Blocks (UTF-16LE with 2-byte character length count)
        if let Some(ref desc) = self.description {
            Self::encode_string_data(desc, &mut buf);
        }
        if has_rel_path {
            Self::encode_string_data(&self.target_path, &mut buf);
        }
        if let Some(ref dir) = self.working_dir {
            Self::encode_string_data(dir, &mut buf);
        }
        if let Some(ref args) = self.arguments {
            Self::encode_string_data(args, &mut buf);
        }
        if let Some(ref icon) = self.icon_location {
            Self::encode_string_data(icon, &mut buf);
        }

        buf
    }

    /// Saves the `.lnk` shell link file to disk.
    ///
    /// # Arguments
    ///
    /// * `dest` - Destination file path (e.g. `C:\Users\Public\Desktop\App.lnk`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on write failure.
    pub fn save_to_disk(&self, dest: &Path) -> Result<()> {
        let path = dest;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_bytes())?;
        Ok(())
    }

    /// Helper encoding a string into UTF-16LE prefixed with its character length.
    fn encode_string_data(s: &str, buf: &mut Vec<u8>) {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let len = u16::try_from(utf16.len()).unwrap_or(u16::MAX);
        buf.extend_from_slice(&len.to_le_bytes());
        for u in &utf16 {
            buf.extend_from_slice(&u.to_le_bytes());
        }
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

        // Trait derives
        assert_eq!(&entry, &entry.clone());
        assert!(!format!("{entry:?}").is_empty());
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

        // Trait derives
        assert_eq!(&bundle, &bundle.clone());
        assert!(!format!("{bundle:?}").is_empty());
    }

    /// Tests `Win32ShellLink` binary serialization, `XdgDesktopEntry` directory installation, and macOS symlinks.
    #[test]
    fn test_win32_shell_link_and_app_symlink() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("desktop_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. XDG desktop entry installation
        assert_eq!(
            XdgDesktopEntry::system_applications_dir(),
            PathBuf::from("/usr/share/applications")
        );
        assert_eq!(
            XdgDesktopEntry::user_applications_dir(),
            PathBuf::from("~/.local/share/applications")
        );

        let xdg = XdgDesktopEntry::new("Test App", "/usr/bin/test-app")
            .comment("Test application description");
        let installed_xdg = xdg.install_to_directory(&temp_dir)?;
        assert!(installed_xdg.exists());
        let xdg_content = std::fs::read_to_string(&installed_xdg)?;
        assert!(xdg_content.contains("Name=Test App"));

        // 2. Win32 Shell Link (.lnk) generation
        let lnk = Win32ShellLink::new(r"C:\Program Files\App\app.exe")
            .arguments("--verbose --mode=gui")
            .working_dir(r"C:\Program Files\App")
            .icon(r"C:\Program Files\App\app.ico", 0)
            .description("Shortcut to App")
            .show_command(3); // SW_SHOWMAXIMIZED

        let bytes = lnk.to_bytes();
        // Header check: HeaderSize = 76 bytes (0x4C)
        assert_eq!(&bytes[0..4], &0x0000_004Cu32.to_le_bytes());
        // LinkCLSID check
        assert_eq!(
            &bytes[4..20],
            &[
                0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x46
            ]
        );
        // ShowCommand check at offset 0x3C (60)
        assert_eq!(&bytes[60..64], &3u32.to_le_bytes());

        // File save check
        let lnk_path = temp_dir.join("App.lnk");
        lnk.save_to_disk(lnk_path.as_path())?;
        assert!(lnk_path.exists());
        assert_eq!(std::fs::read(&lnk_path)?, bytes);

        // Minimal Win32ShellLink without optional flags and empty target path
        let empty_lnk = Win32ShellLink::new("");
        let empty_bytes = empty_lnk.to_bytes();
        assert_eq!(&empty_bytes[0..4], &0x0000_004Cu32.to_le_bytes());
        assert!(empty_lnk.save_to_disk(Path::new("")).is_err());

        // Trait derives
        assert_eq!(&lnk, &lnk.clone());
        assert!(!format!("{lnk:?}").is_empty());

        // 3. macOS App Bundle symlink
        let mock_bundle = temp_dir.join("MyStudio.app");
        let _ = std::fs::create_dir_all(&mock_bundle);

        // Name with explicit .app extension
        let target1 =
            MacOsAppBundle::create_applications_symlink(&mock_bundle, Some("MyStudioLink.app"))?;
        // Second call when target already exists (exercises link_target.exists() == true branch)
        let link_res_repeat =
            MacOsAppBundle::create_applications_symlink(&mock_bundle, Some("MyStudioLink.app"));
        assert!(link_res_repeat.is_ok());

        // Dangling symlink (exercises link_target.exists() == false && link_target.is_symlink() == true)
        let _ = std::fs::remove_file(&target1);
        #[cfg(unix)]
        {
            let dangling_dest = temp_dir.join("nonexistent_dangling.app");
            let _ = std::os::unix::fs::symlink(&dangling_dest, &target1);
            let link_res_dangling =
                MacOsAppBundle::create_applications_symlink(&mock_bundle, Some("MyStudioLink.app"));
            assert!(link_res_dangling.is_ok());
        }
        let _ = std::fs::remove_file(&target1);

        // Name without .app extension
        let target2 =
            MacOsAppBundle::create_applications_symlink(&mock_bundle, Some("MyStudioLink"))?;
        let _ = std::fs::remove_file(&target2);

        // None name with file_name() present on bundle_path
        let target3 = MacOsAppBundle::create_applications_symlink(&mock_bundle, None)?;
        let _ = std::fs::remove_file(&target3);

        // None name with bundle_path without file_name (e.g. root "/")
        let target4 = MacOsAppBundle::create_applications_symlink(Path::new("/"), None)?;
        let _ = std::fs::remove_file(&target4);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
