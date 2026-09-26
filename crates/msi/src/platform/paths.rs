//! Filesystem Hierarchy & Standard Paths Translation (FHS, XDG, Apple File System).
//!
//! Grounded directly in POSIX.1-2017, Freedesktop XDG, and macOS Apple File System specifications:
//! - Standard path resolution mapping Windows Installer directories to target operating system paths.
//! - Target platforms: `Linux`, `MacOs`, `FreeBsd`, `SunOs`, `Windows`.
//! - Support for `$XDG_DATA_HOME`, `$XDG_CONFIG_HOME`, `$XDG_DESKTOP_DIR`, and `$TMPDIR` overrides.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Target operating system platform for path resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOs {
    /// Linux conforming to FHS and XDG Base Directory Specification.
    Linux,
    /// Apple macOS conforming to Apple File System hierarchy (`/Applications`, `~/Library`).
    MacOs,
    /// FreeBSD conforming to `hier(7)` (`/usr/local`).
    FreeBsd,
    /// `SunOS` / illumos / Solaris filesystem layout.
    SunOs,
    /// Native Windows layout.
    Windows,
}

impl TargetOs {
    /// Returns the active host operating system.
    #[must_use]
    pub const fn host() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(target_os = "linux")]
        {
            Self::Linux
        }
        #[cfg(target_os = "freebsd")]
        {
            Self::FreeBsd
        }
        #[cfg(target_os = "solaris")]
        {
            Self::SunOs
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "freebsd",
            target_os = "solaris",
            target_os = "windows"
        )))]
        {
            Self::Linux
        }
    }
}

/// Standard Windows Installer directory identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardDirectoryId {
    /// Root installation prefix (`[TARGETDIR]`).
    TargetDir,
    /// 32-bit Program Files folder (`[ProgramFilesFolder]`).
    ProgramFilesFolder,
    /// 64-bit Program Files folder (`[ProgramFiles64Folder]`).
    ProgramFiles64Folder,
    /// Shared files folder (`[CommonFilesFolder]`).
    CommonFilesFolder,
    /// System binaries directory (`[SystemFolder]`).
    SystemFolder,
    /// 64-bit System binaries directory (`[System64Folder]`).
    System64Folder,
    /// Windows system directory (`[WindowsFolder]`).
    WindowsFolder,
    /// Shared machine application data (`[CommonAppDataFolder]`).
    CommonAppDataFolder,
    /// Per-user local application data (`[LocalAppDataFolder]`).
    LocalAppDataFolder,
    /// Per-user roaming application data (`[AppDataFolder]`).
    AppDataFolder,
    /// User desktop directory (`[DesktopFolder]`).
    DesktopFolder,
    /// User or system Start Menu programs folder (`[ProgramMenuFolder]`).
    ProgramMenuFolder,
    /// User profiles root directory (`[ProfilesFolder]`).
    ProfilesFolder,
    /// Temporary files directory (`[TempFolder]`).
    TempFolder,
}

impl StandardDirectoryId {
    /// Parses a directory identifier string from MSI database records.
    ///
    /// # Arguments
    ///
    /// * `name` - Directory name.
    ///
    /// # Returns
    ///
    /// Optional [`StandardDirectoryId`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "TARGETDIR" => Some(Self::TargetDir),
            "ProgramFilesFolder" => Some(Self::ProgramFilesFolder),
            "ProgramFiles64Folder" => Some(Self::ProgramFiles64Folder),
            "CommonFilesFolder" => Some(Self::CommonFilesFolder),
            "SystemFolder" => Some(Self::SystemFolder),
            "System64Folder" => Some(Self::System64Folder),
            "WindowsFolder" => Some(Self::WindowsFolder),
            "CommonAppDataFolder" => Some(Self::CommonAppDataFolder),
            "LocalAppDataFolder" => Some(Self::LocalAppDataFolder),
            "AppDataFolder" => Some(Self::AppDataFolder),
            "DesktopFolder" => Some(Self::DesktopFolder),
            "ProgramMenuFolder" => Some(Self::ProgramMenuFolder),
            "ProfilesFolder" => Some(Self::ProfilesFolder),
            "TempFolder" => Some(Self::TempFolder),
            _ => None,
        }
    }

    /// Returns the standard MSI identifier name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TargetDir => "TARGETDIR",
            Self::ProgramFilesFolder => "ProgramFilesFolder",
            Self::ProgramFiles64Folder => "ProgramFiles64Folder",
            Self::CommonFilesFolder => "CommonFilesFolder",
            Self::SystemFolder => "SystemFolder",
            Self::System64Folder => "System64Folder",
            Self::WindowsFolder => "WindowsFolder",
            Self::CommonAppDataFolder => "CommonAppDataFolder",
            Self::LocalAppDataFolder => "LocalAppDataFolder",
            Self::AppDataFolder => "AppDataFolder",
            Self::DesktopFolder => "DesktopFolder",
            Self::ProgramMenuFolder => "ProgramMenuFolder",
            Self::ProfilesFolder => "ProfilesFolder",
            Self::TempFolder => "TempFolder",
        }
    }
}

/// Cross-platform standard directory path resolver.
#[derive(Debug, Clone)]
pub struct PathResolver {
    /// Target operating system.
    target_os: TargetOs,
    /// Vendor or manufacturer name for directory naming (e.g. "Acme").
    vendor: Option<String>,
    /// Product or application name (e.g. "`MyProduct`").
    product: String,
    /// User home directory path (defaults to `/home/user` or `/Users/user`).
    home_dir: PathBuf,
    /// Target root prefix path (defaults to `/`).
    root_prefix: PathBuf,
    /// Optional offline sysroot path for bare-metal OS installation.
    sysroot: Option<PathBuf>,
    /// Environment variable overrides (e.g. `XDG_DATA_HOME`, `TMPDIR`).
    env_vars: HashMap<String, String>,
}

impl PathResolver {
    /// Creates a new [`PathResolver`].
    ///
    /// # Arguments
    ///
    /// * `target_os` - Target operating system.
    /// * `product` - Product name.
    ///
    /// # Returns
    ///
    /// A new [`PathResolver`].
    #[must_use]
    pub fn new(target_os: TargetOs, product: impl Into<String>) -> Self {
        let home = match target_os {
            TargetOs::MacOs => PathBuf::from("/Users/user"),
            TargetOs::Windows => PathBuf::from(r"C:\Users\user"),
            TargetOs::Linux | TargetOs::FreeBsd | TargetOs::SunOs => PathBuf::from("/home/user"),
        };
        Self {
            target_os,
            vendor: None,
            product: product.into(),
            home_dir: home,
            root_prefix: PathBuf::from("/"),
            sysroot: None,
            env_vars: HashMap::new(),
        }
    }

    /// Sets the vendor or manufacturer name.
    #[must_use]
    pub fn vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into());
        self
    }

    /// Sets the home directory path.
    #[must_use]
    pub fn home_dir(mut self, home: impl Into<PathBuf>) -> Self {
        self.home_dir = home.into();
        self
    }

    /// Sets the root prefix path.
    #[must_use]
    pub fn root_prefix(mut self, prefix: impl Into<PathBuf>) -> Self {
        self.root_prefix = prefix.into();
        self
    }

    /// Sets an explicit sysroot target directory for offline OS deployment.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Root prefix path of mounted target OS partition.
    ///
    /// # Returns
    ///
    /// Updated [`PathResolver`].
    #[must_use]
    pub fn sysroot(mut self, sysroot: impl Into<PathBuf>) -> Self {
        self.sysroot = Some(sysroot.into());
        self
    }

    /// Returns the configured sysroot path, if any.
    ///
    /// # Returns
    ///
    /// Optional borrowed [`Path`] reference.
    #[must_use]
    pub fn get_sysroot(&self) -> Option<&Path> {
        self.sysroot.as_deref()
    }

    /// Prevents directory traversal attacks outside the mounted sysroot target.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Root prefix path.
    /// * `subpath` - Requested path within sysroot.
    ///
    /// # Returns
    ///
    /// Sanitized absolute [`PathBuf`] within sysroot.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if path escapes the sysroot hierarchy.
    pub fn sanitize_sysroot_path(sysroot: &Path, subpath: &Path) -> Result<PathBuf> {
        let mut clean = sysroot.to_path_buf();
        for component in subpath.components() {
            match component {
                std::path::Component::ParentDir => {
                    if clean == sysroot {
                        return Err(Error::SysrootMountError {
                            path: subpath.display().to_string(),
                            reason:
                                "path attempts to escape sysroot via parent directory traversal"
                                    .to_string(),
                        });
                    }
                    clean.pop();
                }
                std::path::Component::Normal(c) => {
                    clean.push(c);
                }
                std::path::Component::RootDir
                | std::path::Component::Prefix(_)
                | std::path::Component::CurDir => {}
            }
        }
        Ok(clean)
    }

    /// Sets an environment variable override for testing or custom environments.
    pub fn set_env(&mut self, key: impl Into<String>, val: impl Into<String>) {
        self.env_vars.insert(key.into(), val.into());
    }

    /// Resolves an MSI standard directory identifier into its target platform filesystem path.
    ///
    /// # Arguments
    ///
    /// * `dir_id` - Standard directory identifier.
    ///
    /// # Returns
    ///
    /// Absolute target platform [`PathBuf`].
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn resolve(&self, dir_id: StandardDirectoryId) -> PathBuf {
        if let Some(ref sysroot) = self.sysroot {
            return self.resolve_offline_sysroot(dir_id, sysroot);
        }

        match (self.target_os, dir_id) {
            (_, StandardDirectoryId::TargetDir) => self.root_prefix.clone(),

            (TargetOs::Windows, StandardDirectoryId::WindowsFolder) => PathBuf::from(r"C:\Windows"),
            (
                TargetOs::Windows,
                StandardDirectoryId::SystemFolder | StandardDirectoryId::System64Folder,
            ) => PathBuf::from(r"C:\Windows\System32"),
            (TargetOs::Windows, StandardDirectoryId::ProgramFilesFolder) => {
                PathBuf::from(r"C:\Program Files (x86)")
            }
            (TargetOs::Windows, StandardDirectoryId::ProgramFiles64Folder) => {
                PathBuf::from(r"C:\Program Files")
            }
            (TargetOs::Windows, StandardDirectoryId::CommonFilesFolder) => {
                PathBuf::from(r"C:\Program Files\Common Files")
            }
            (TargetOs::Windows, StandardDirectoryId::CommonAppDataFolder) => {
                PathBuf::from(r"C:\ProgramData").join(&self.product)
            }
            (TargetOs::Windows, StandardDirectoryId::LocalAppDataFolder) => {
                self.home_dir.join(r"AppData\Local").join(&self.product)
            }
            (TargetOs::Windows, StandardDirectoryId::AppDataFolder) => {
                self.home_dir.join(r"AppData\Roaming").join(&self.product)
            }
            (TargetOs::Windows, StandardDirectoryId::ProfilesFolder) => PathBuf::from(r"C:\Users"),
            (TargetOs::Windows | TargetOs::MacOs, StandardDirectoryId::DesktopFolder) => {
                self.home_dir.join("Desktop")
            }
            (TargetOs::Windows, StandardDirectoryId::ProgramMenuFolder) => self
                .home_dir
                .join(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs"),
            (TargetOs::MacOs, StandardDirectoryId::ProgramMenuFolder) => {
                self.home_dir.join("Applications")
            }
            (TargetOs::Windows, StandardDirectoryId::TempFolder) => self
                .env_vars
                .get("TMPDIR")
                .or_else(|| self.env_vars.get("TEMP"))
                .map_or_else(
                    || PathBuf::from(r"C:\Users\user\AppData\Local\Temp"),
                    PathBuf::from,
                ),

            (
                TargetOs::MacOs,
                StandardDirectoryId::ProgramFilesFolder | StandardDirectoryId::ProgramFiles64Folder,
            ) => PathBuf::from("/Applications"),
            (TargetOs::MacOs, StandardDirectoryId::CommonFilesFolder) => {
                PathBuf::from("/Library/Application Support")
            }
            (
                TargetOs::MacOs | TargetOs::FreeBsd,
                StandardDirectoryId::SystemFolder | StandardDirectoryId::System64Folder,
            ) => PathBuf::from("/usr/local/bin"),
            (TargetOs::MacOs, StandardDirectoryId::WindowsFolder) => PathBuf::from("/Library"),
            (TargetOs::MacOs, StandardDirectoryId::ProfilesFolder) => PathBuf::from("/Users"),
            (TargetOs::MacOs, StandardDirectoryId::CommonAppDataFolder) => {
                PathBuf::from("/Library/Application Support").join(&self.product)
            }
            (TargetOs::MacOs, StandardDirectoryId::LocalAppDataFolder) => self
                .home_dir
                .join("Library/Application Support")
                .join(&self.product),
            (TargetOs::MacOs, StandardDirectoryId::AppDataFolder) => self
                .home_dir
                .join("Library/Preferences")
                .join(&self.product),
            (TargetOs::MacOs, StandardDirectoryId::TempFolder) => self
                .env_vars
                .get("TMPDIR")
                .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from),

            (
                TargetOs::FreeBsd,
                StandardDirectoryId::ProgramFilesFolder | StandardDirectoryId::ProgramFiles64Folder,
            ) => self.vendor.as_ref().map_or_else(
                || PathBuf::from("/usr/local"),
                |v| PathBuf::from("/usr/local").join(v),
            ),
            (TargetOs::FreeBsd, StandardDirectoryId::CommonFilesFolder) => {
                PathBuf::from("/usr/local/share")
            }
            (TargetOs::FreeBsd, StandardDirectoryId::CommonAppDataFolder) => {
                PathBuf::from("/var/db").join(&self.product)
            }

            (
                TargetOs::SunOs | TargetOs::Linux,
                StandardDirectoryId::ProgramFilesFolder | StandardDirectoryId::ProgramFiles64Folder,
            ) => self
                .vendor
                .as_ref()
                .map_or_else(|| PathBuf::from("/opt"), |v| PathBuf::from("/opt").join(v)),
            (TargetOs::SunOs | TargetOs::Linux, StandardDirectoryId::CommonFilesFolder) => {
                PathBuf::from("/usr/share")
            }
            (
                TargetOs::SunOs | TargetOs::Linux,
                StandardDirectoryId::SystemFolder | StandardDirectoryId::System64Folder,
            ) => PathBuf::from("/usr/bin"),
            (TargetOs::SunOs, StandardDirectoryId::CommonAppDataFolder) => {
                PathBuf::from("/etc/opt").join(&self.product)
            }

            // Linux CommonAppDataFolder
            (TargetOs::Linux, StandardDirectoryId::CommonAppDataFolder) => {
                PathBuf::from("/var/lib").join(&self.product)
            }

            // Linux/FreeBSD/SunOS WindowsFolder and ProfilesFolder
            (_, StandardDirectoryId::WindowsFolder) => PathBuf::from("/etc"),
            (_, StandardDirectoryId::ProfilesFolder) => PathBuf::from("/home"),

            // Shared XDG user directories for Linux, FreeBSD, SunOS
            (_, StandardDirectoryId::LocalAppDataFolder) => {
                self.env_vars.get("XDG_DATA_HOME").map_or_else(
                    || self.home_dir.join(".local/share").join(&self.product),
                    |xdg_data| PathBuf::from(xdg_data).join(&self.product),
                )
            }
            (_, StandardDirectoryId::AppDataFolder) => {
                self.env_vars.get("XDG_CONFIG_HOME").map_or_else(
                    || self.home_dir.join(".config").join(&self.product),
                    |xdg_config| PathBuf::from(xdg_config).join(&self.product),
                )
            }
            (_, StandardDirectoryId::DesktopFolder) => self
                .env_vars
                .get("XDG_DESKTOP_DIR")
                .map_or_else(|| self.home_dir.join("Desktop"), PathBuf::from),
            (_, StandardDirectoryId::ProgramMenuFolder) => {
                self.env_vars.get("XDG_DATA_HOME").map_or_else(
                    || self.home_dir.join(".local/share/applications"),
                    |xdg_data| PathBuf::from(xdg_data).join("applications"),
                )
            }
            (_, StandardDirectoryId::TempFolder) => self
                .env_vars
                .get("TMPDIR")
                .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from),
        }
    }

    /// Resolves directories relative to an explicit offline sysroot.
    fn resolve_offline_sysroot(&self, dir_id: StandardDirectoryId, sysroot: &Path) -> PathBuf {
        match (self.target_os, dir_id) {
            (_, StandardDirectoryId::TargetDir) => sysroot.to_path_buf(),
            (TargetOs::Windows, StandardDirectoryId::WindowsFolder) => sysroot.join("Windows"),
            (
                TargetOs::Windows,
                StandardDirectoryId::SystemFolder | StandardDirectoryId::System64Folder,
            ) => sysroot.join("Windows").join("System32"),
            (TargetOs::Windows, StandardDirectoryId::ProgramFilesFolder) => {
                sysroot.join("Program Files (x86)")
            }
            (TargetOs::Windows, StandardDirectoryId::ProgramFiles64Folder) => {
                sysroot.join("Program Files")
            }
            (TargetOs::Windows, StandardDirectoryId::CommonFilesFolder) => {
                sysroot.join("Program Files").join("Common Files")
            }
            (
                TargetOs::Windows,
                StandardDirectoryId::AppDataFolder | StandardDirectoryId::ProfilesFolder,
            ) => sysroot.join("Users"),
            (TargetOs::Windows, StandardDirectoryId::CommonAppDataFolder) => {
                sysroot.join("ProgramData").join(&self.product)
            }
            (TargetOs::Windows, StandardDirectoryId::DesktopFolder) => {
                sysroot.join("Users").join("Default").join("Desktop")
            }
            (TargetOs::Windows, StandardDirectoryId::ProgramMenuFolder) => {
                sysroot.join(r"ProgramData\Microsoft\Windows\Start Menu\Programs")
            }
            (TargetOs::Windows, StandardDirectoryId::TempFolder) => {
                sysroot.join("Windows").join("Temp")
            }
            (TargetOs::Windows, StandardDirectoryId::LocalAppDataFolder) => sysroot
                .join("Users")
                .join("Default")
                .join("AppData")
                .join("Local")
                .join(&self.product),
            (_, StandardDirectoryId::WindowsFolder) => sysroot.join("etc"),
            (_, StandardDirectoryId::SystemFolder | StandardDirectoryId::System64Folder) => {
                sysroot.join("usr/bin")
            }
            (
                _,
                StandardDirectoryId::ProgramFilesFolder | StandardDirectoryId::ProgramFiles64Folder,
            ) => self
                .vendor
                .as_ref()
                .map_or_else(|| sysroot.join("opt"), |v| sysroot.join("opt").join(v)),
            (_, StandardDirectoryId::CommonFilesFolder) => sysroot.join("usr/share"),
            (_, StandardDirectoryId::CommonAppDataFolder) => {
                sysroot.join("var/lib").join(&self.product)
            }
            (_, StandardDirectoryId::AppDataFolder | StandardDirectoryId::ProfilesFolder) => {
                sysroot.join("home")
            }
            (_, StandardDirectoryId::LocalAppDataFolder) => {
                sysroot.join("var/cache").join(&self.product)
            }
            (_, StandardDirectoryId::DesktopFolder) => sysroot.join("etc/skel/Desktop"),
            (_, StandardDirectoryId::ProgramMenuFolder) => sysroot.join("usr/share/applications"),
            (_, StandardDirectoryId::TempFolder) => sysroot.join("tmp"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `StandardDirectoryId` parsing and string representation.
    #[test]
    fn test_standard_directory_id() {
        let ids = [
            ("TARGETDIR", StandardDirectoryId::TargetDir),
            (
                "ProgramFilesFolder",
                StandardDirectoryId::ProgramFilesFolder,
            ),
            (
                "ProgramFiles64Folder",
                StandardDirectoryId::ProgramFiles64Folder,
            ),
            ("CommonFilesFolder", StandardDirectoryId::CommonFilesFolder),
            ("SystemFolder", StandardDirectoryId::SystemFolder),
            ("System64Folder", StandardDirectoryId::System64Folder),
            ("WindowsFolder", StandardDirectoryId::WindowsFolder),
            ("ProfilesFolder", StandardDirectoryId::ProfilesFolder),
            (
                "CommonAppDataFolder",
                StandardDirectoryId::CommonAppDataFolder,
            ),
            (
                "LocalAppDataFolder",
                StandardDirectoryId::LocalAppDataFolder,
            ),
            ("AppDataFolder", StandardDirectoryId::AppDataFolder),
            ("DesktopFolder", StandardDirectoryId::DesktopFolder),
            ("ProgramMenuFolder", StandardDirectoryId::ProgramMenuFolder),
            ("TempFolder", StandardDirectoryId::TempFolder),
        ];

        for (name, expected) in ids {
            assert_eq!(StandardDirectoryId::from_name(name), Some(expected));
            assert_eq!(expected.as_str(), name);
        }

        assert_eq!(StandardDirectoryId::from_name("CustomDir"), None);
    }

    /// Tests path resolution on Linux with XDG environment variable overrides.
    #[test]
    fn test_path_resolution_linux_xdg() {
        let mut resolver = PathResolver::new(TargetOs::Linux, "SuperApp")
            .vendor("Acme")
            .home_dir("/home/alice");

        assert_eq!(
            resolver.resolve(StandardDirectoryId::TargetDir),
            PathBuf::from("/")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from("/opt/Acme")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::SystemFolder),
            PathBuf::from("/usr/bin")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::CommonAppDataFolder),
            PathBuf::from("/var/lib/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::LocalAppDataFolder),
            PathBuf::from("/home/alice/.local/share/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::AppDataFolder),
            PathBuf::from("/home/alice/.config/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::DesktopFolder),
            PathBuf::from("/home/alice/Desktop")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::ProgramMenuFolder),
            PathBuf::from("/home/alice/.local/share/applications")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from("/tmp")
        );

        // With XDG overrides
        resolver.set_env("XDG_DATA_HOME", "/custom/data");
        resolver.set_env("XDG_CONFIG_HOME", "/custom/config");
        resolver.set_env("XDG_DESKTOP_DIR", "/custom/desktop");
        resolver.set_env("TMPDIR", "/var/tmp");

        assert_eq!(
            resolver.resolve(StandardDirectoryId::ProgramMenuFolder),
            PathBuf::from("/custom/data/applications")
        );

        assert_eq!(
            resolver.resolve(StandardDirectoryId::LocalAppDataFolder),
            PathBuf::from("/custom/data/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::AppDataFolder),
            PathBuf::from("/custom/config/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::DesktopFolder),
            PathBuf::from("/custom/desktop")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from("/var/tmp")
        );
    }

    /// Tests path resolution on macOS.
    #[test]
    fn test_path_resolution_macos() {
        let resolver = PathResolver::new(TargetOs::MacOs, "SuperApp").home_dir("/Users/bob");

        assert_eq!(
            resolver.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from("/Applications")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::CommonFilesFolder),
            PathBuf::from("/Library/Application Support")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::SystemFolder),
            PathBuf::from("/usr/local/bin")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::CommonAppDataFolder),
            PathBuf::from("/Library/Application Support/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::LocalAppDataFolder),
            PathBuf::from("/Users/bob/Library/Application Support/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::AppDataFolder),
            PathBuf::from("/Users/bob/Library/Preferences/SuperApp")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::DesktopFolder),
            PathBuf::from("/Users/bob/Desktop")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::ProgramMenuFolder),
            PathBuf::from("/Users/bob/Applications")
        );
    }

    /// Tests path resolution on FreeBSD and `SunOS`.
    #[test]
    fn test_path_resolution_freebsd_and_sunos() {
        let fbsd = PathResolver::new(TargetOs::FreeBsd, "DaemonApp").vendor("FreeCorp");
        assert_eq!(
            fbsd.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from("/usr/local/FreeCorp")
        );
        assert_eq!(
            fbsd.resolve(StandardDirectoryId::CommonFilesFolder),
            PathBuf::from("/usr/local/share")
        );
        assert_eq!(
            fbsd.resolve(StandardDirectoryId::CommonAppDataFolder),
            PathBuf::from("/var/db/DaemonApp")
        );
        assert_eq!(
            fbsd.resolve(StandardDirectoryId::ProgramMenuFolder),
            PathBuf::from("/home/user/.local/share/applications")
        );

        let sunos = PathResolver::new(TargetOs::SunOs, "SolarisApp");
        assert_eq!(
            sunos.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from("/opt")
        );
        assert_eq!(
            sunos.resolve(StandardDirectoryId::CommonAppDataFolder),
            PathBuf::from("/etc/opt/SolarisApp")
        );

        // Host OS detection
        let _ = TargetOs::host();
    }

    /// Tests path resolution on Windows across all standard directory identifiers and temp overrides.
    #[test]
    fn test_path_resolution_windows() {
        let win = PathResolver::new(TargetOs::Windows, "WinApp");

        assert_eq!(
            win.resolve(StandardDirectoryId::TargetDir),
            PathBuf::from("/")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from(r"C:\Program Files (x86)")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::ProgramFiles64Folder),
            PathBuf::from(r"C:\Program Files")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::CommonFilesFolder),
            PathBuf::from(r"C:\Program Files\Common Files")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::SystemFolder),
            PathBuf::from(r"C:\Windows\System32")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::CommonAppDataFolder),
            PathBuf::from(r"C:\ProgramData").join("WinApp")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::LocalAppDataFolder),
            PathBuf::from(r"C:\Users\user")
                .join(r"AppData\Local")
                .join("WinApp")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::AppDataFolder),
            PathBuf::from(r"C:\Users\user")
                .join(r"AppData\Roaming")
                .join("WinApp")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::DesktopFolder),
            PathBuf::from(r"C:\Users\user").join("Desktop")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::ProgramMenuFolder),
            PathBuf::from(r"C:\Users\user")
                .join(r"AppData\Roaming\Microsoft\Windows\Start Menu\Programs")
        );
        assert_eq!(
            win.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from(r"C:\Users\user\AppData\Local\Temp")
        );

        // TempFolder with TEMP environment variable override
        let mut win_temp = win;
        win_temp.set_env("TEMP", r"D:\CustomTemp");
        assert_eq!(
            win_temp.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from(r"D:\CustomTemp")
        );

        // TempFolder with TMPDIR environment variable override taking precedence over TEMP
        win_temp.set_env("TMPDIR", r"E:\FastTemp");
        assert_eq!(
            win_temp.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from(r"E:\FastTemp")
        );
    }

    /// Tests additional path resolution branches: `MacOS` temp overrides, FreeBSD without vendor, `SunOS` common dirs, and builder `root_prefix`.
    #[test]
    fn test_path_resolution_edge_cases_and_traits() {
        // MacOS TempFolder default and with TMPDIR
        let mut mac = PathResolver::new(TargetOs::MacOs, "MacApp");
        assert_eq!(
            mac.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from("/tmp")
        );
        mac.set_env("TMPDIR", "/var/tmp/custom");
        assert_eq!(
            mac.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from("/var/tmp/custom")
        );

        // FreeBSD without vendor
        let fbsd_novendor = PathResolver::new(TargetOs::FreeBsd, "DaemonApp");
        assert_eq!(
            fbsd_novendor.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from("/usr/local")
        );
        assert_eq!(
            fbsd_novendor.resolve(StandardDirectoryId::ProgramFiles64Folder),
            PathBuf::from("/usr/local")
        );
        assert_eq!(
            fbsd_novendor.resolve(StandardDirectoryId::SystemFolder),
            PathBuf::from("/usr/local/bin")
        );

        // SunOS and Linux CommonFilesFolder
        let sunos = PathResolver::new(TargetOs::SunOs, "SolApp").vendor("OracleCorp");
        assert_eq!(
            sunos.resolve(StandardDirectoryId::ProgramFilesFolder),
            PathBuf::from("/opt/OracleCorp")
        );
        assert_eq!(
            sunos.resolve(StandardDirectoryId::CommonFilesFolder),
            PathBuf::from("/usr/share")
        );
        assert_eq!(
            sunos.resolve(StandardDirectoryId::SystemFolder),
            PathBuf::from("/usr/bin")
        );

        let linux = PathResolver::new(TargetOs::Linux, "LinApp");
        assert_eq!(
            linux.resolve(StandardDirectoryId::CommonFilesFolder),
            PathBuf::from("/usr/share")
        );

        // Custom root_prefix builder method
        let custom_root = PathResolver::new(TargetOs::Linux, "App").root_prefix("/opt/custom_root");
        assert_eq!(
            custom_root.resolve(StandardDirectoryId::TargetDir),
            PathBuf::from("/opt/custom_root")
        );

        // Debug formatting for types
        assert!(format!("{:?}", TargetOs::Windows).contains("Windows"));
        assert!(
            format!("{:?}", StandardDirectoryId::ProgramFilesFolder).contains("ProgramFilesFolder")
        );
        assert!(format!("{custom_root:?}").contains("PathResolver"));
    }

    /// Tests offline sysroot path resolution on Windows.
    #[test]
    fn test_path_resolution_offline_sysroot_windows() {
        let sysroot = Path::new("/mnt/target");
        let win_resolver = PathResolver::new(TargetOs::Windows, "MyApp").sysroot(sysroot);

        assert_eq!(win_resolver.get_sysroot(), Some(sysroot));
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::TargetDir),
            PathBuf::from("/mnt/target")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::WindowsFolder),
            PathBuf::from("/mnt/target/Windows")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::SystemFolder),
            sysroot.join("Windows").join("System32")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::System64Folder),
            sysroot.join("Windows").join("System32")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::ProgramFilesFolder),
            sysroot.join("Program Files (x86)")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::ProgramFiles64Folder),
            sysroot.join("Program Files")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::CommonFilesFolder),
            sysroot.join("Program Files").join("Common Files")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::ProfilesFolder),
            sysroot.join("Users")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::AppDataFolder),
            sysroot.join("Users")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::DesktopFolder),
            sysroot.join("Users").join("Default").join("Desktop")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::ProgramMenuFolder),
            sysroot.join(r"ProgramData\Microsoft\Windows\Start Menu\Programs")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::TempFolder),
            sysroot.join("Windows").join("Temp")
        );

        // Test CommonAppDataFolder and LocalAppDataFolder with sysroot on Windows
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::CommonAppDataFolder),
            sysroot.join("ProgramData").join("MyApp")
        );
        assert_eq!(
            win_resolver.resolve(StandardDirectoryId::LocalAppDataFolder),
            sysroot
                .join("Users")
                .join("Default")
                .join("AppData")
                .join("Local")
                .join("MyApp")
        );
    }

    /// Tests offline sysroot path resolution on Linux.
    #[test]
    fn test_path_resolution_offline_sysroot_linux() {
        let sysroot = Path::new("/mnt/target");
        let linux_resolver = PathResolver::new(TargetOs::Linux, "MyApp").sysroot(sysroot);
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::TargetDir),
            PathBuf::from("/mnt/target")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::WindowsFolder),
            PathBuf::from("/mnt/target/etc")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::SystemFolder),
            PathBuf::from("/mnt/target/usr/bin")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::CommonFilesFolder),
            PathBuf::from("/mnt/target/usr/share")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::DesktopFolder),
            PathBuf::from("/mnt/target/etc/skel/Desktop")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::ProgramMenuFolder),
            PathBuf::from("/mnt/target/usr/share/applications")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::TempFolder),
            PathBuf::from("/mnt/target/tmp")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::AppDataFolder),
            sysroot.join("home")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::ProfilesFolder),
            sysroot.join("home")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::ProgramFilesFolder),
            sysroot.join("opt")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::CommonAppDataFolder),
            sysroot.join("var/lib").join("MyApp")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::LocalAppDataFolder),
            sysroot.join("var/cache").join("MyApp")
        );
        assert_eq!(
            linux_resolver.resolve(StandardDirectoryId::DesktopFolder),
            sysroot.join("etc/skel/Desktop")
        );

        // Linux resolver with vendor specified
        let linux_vendor_resolver = PathResolver::new(TargetOs::Linux, "MyApp")
            .vendor("AcmeCorp")
            .sysroot(sysroot);
        assert_eq!(
            linux_vendor_resolver.resolve(StandardDirectoryId::ProgramFilesFolder),
            sysroot.join("opt").join("AcmeCorp")
        );
        assert_eq!(
            linux_vendor_resolver.resolve(StandardDirectoryId::ProgramFiles64Folder),
            sysroot.join("opt").join("AcmeCorp")
        );
    }

    /// Tests directory traversal sanitization within sysroot boundary.
    #[test]
    fn test_path_sanitization_and_traversal() {
        let sysroot = Path::new("/mnt/target");

        // Path sanitization checks
        assert_eq!(
            PathResolver::sanitize_sysroot_path(sysroot, Path::new("Windows/System32")),
            Ok(PathBuf::from("/mnt/target/Windows/System32"))
        );

        // CurDir and RootDir components
        assert_eq!(
            PathResolver::sanitize_sysroot_path(sysroot, Path::new("./Windows/./System32")),
            Ok(PathBuf::from("/mnt/target/Windows/System32"))
        );
        assert_eq!(
            PathResolver::sanitize_sysroot_path(sysroot, Path::new("/Windows/System32")),
            Ok(PathBuf::from("/mnt/target/Windows/System32"))
        );

        // Parent dir within sysroot is allowed
        assert_eq!(
            PathResolver::sanitize_sysroot_path(sysroot, Path::new("Windows/../Program Files")),
            Ok(PathBuf::from("/mnt/target/Program Files"))
        );

        // Traversal escape outside sysroot is rejected
        let escape = PathResolver::sanitize_sysroot_path(sysroot, Path::new("../../etc/shadow"));
        assert!(escape.is_err());
    }

    /// Tests non-sysroot `WindowsFolder` and `ProfilesFolder` resolution across OSes and `set_env`.
    #[test]
    fn test_path_resolution_extra_branches_and_env() {
        let mut resolver = PathResolver::new(TargetOs::Windows, "MyApp");
        resolver.set_env("CUSTOM_ENV", "CUSTOM_VAL");
        assert_eq!(
            resolver.resolve(StandardDirectoryId::WindowsFolder),
            PathBuf::from(r"C:\Windows")
        );
        assert_eq!(
            resolver.resolve(StandardDirectoryId::ProfilesFolder),
            PathBuf::from(r"C:\Users")
        );

        let mac_resolver = PathResolver::new(TargetOs::MacOs, "MyApp");
        assert_eq!(
            mac_resolver.resolve(StandardDirectoryId::WindowsFolder),
            PathBuf::from("/Library")
        );
        assert_eq!(
            mac_resolver.resolve(StandardDirectoryId::ProfilesFolder),
            PathBuf::from("/Users")
        );

        let freebsd_resolver = PathResolver::new(TargetOs::FreeBsd, "MyApp");
        assert_eq!(
            freebsd_resolver.resolve(StandardDirectoryId::WindowsFolder),
            PathBuf::from("/etc")
        );
        assert_eq!(
            freebsd_resolver.resolve(StandardDirectoryId::ProfilesFolder),
            PathBuf::from("/home")
        );
    }
}
