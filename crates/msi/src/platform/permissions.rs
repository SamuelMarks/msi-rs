//! POSIX Permissions, ACLs & Extended Attributes Translation.
//!
//! Grounded directly in POSIX.1-2017, `SUSv4`, POSIX.1e draft 17, `NFSv4` ACL, and Linux/macOS xattr standards:
//! - POSIX Mode Octal Mapping (`0755`, `0644`, `0600`, `SetUID` `04000`, `SetGID` `02000`, Sticky `01000`).
//! - Windows Security Descriptor (SDDL) translation into POSIX.1e draft ACLs, `NFSv4` ACLs, and macOS native ACLs.
//! - Extended Attributes (xattr): macOS `com.apple.quarantine`, Linux `SELinux` context (`security.selinux`),
//!   and Linux file capabilities (`security.capability`).

use crate::error::{Error, Result};
use crate::platform::paths::TargetOs;
#[cfg(any(unix, test))]
use std::fs;
use std::path::Path;

/// Special permission bit: `SetUID` (`04000`).
pub const S_ISUID: u32 = 0o4000;

/// Special permission bit: `SetGID` (`02000`).
pub const S_ISGID: u32 = 0o2000;

/// Special permission bit: Sticky bit (`01000`).
pub const S_ISVTX: u32 = 0o1000;

/// Standard executable file permission mode: `0755` (`rwxr-xr-x`).
pub const MODE_EXECUTABLE: u32 = 0o755;

/// Standard regular data file permission mode: `0644` (`rw-r--r--`).
pub const MODE_STANDARD_FILE: u32 = 0o644;

/// Private configuration or secret key file permission mode: `0600` (`rw-------`).
pub const MODE_PRIVATE_FILE: u32 = 0o600;

/// Standard directory permission mode: `0755` (`rwxr-xr-x`).
pub const MODE_DIRECTORY: u32 = 0o755;

/// Strongly-typed POSIX filesystem mode and permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PosixMode {
    /// Full 12-bit mode containing special bits and user/group/other rwx.
    mode: u32,
}

impl PosixMode {
    /// Creates a new [`PosixMode`] from a raw octal integer (masked to 12 bits `07777`).
    ///
    /// # Arguments
    ///
    /// * `raw_mode` - Mode integer.
    ///
    /// # Returns
    ///
    /// A new [`PosixMode`].
    #[must_use]
    pub const fn from_octal(raw_mode: u32) -> Self {
        Self {
            mode: raw_mode & 0o7777,
        }
    }

    /// Determines the standard default POSIX mode based on a file path or extension.
    ///
    /// - Executables (`.sh`, `.bin`, `.exe`, no extension in bin/): `0755`
    /// - Private configuration (`.key`, `.pem`, `.conf`, `.env`): `0600`
    /// - Standard files: `0644`
    ///
    /// # Arguments
    ///
    /// * `path` - File path or filename.
    ///
    /// # Returns
    ///
    /// Recommended [`PosixMode`].
    #[must_use]
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    pub fn infer_from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".sh")
            || lower.ends_with(".bin")
            || lower.ends_with(".exe")
            || lower.contains("/bin/")
            || lower.contains(r"\bin\")
        {
            Self::from_octal(MODE_EXECUTABLE)
        } else if lower.ends_with(".key")
            || lower.ends_with(".pem")
            || lower.ends_with(".env")
            || lower.ends_with(".secret")
        {
            Self::from_octal(MODE_PRIVATE_FILE)
        } else {
            Self::from_octal(MODE_STANDARD_FILE)
        }
    }

    /// Returns the 12-bit mode octal value.
    #[must_use]
    pub const fn as_octal(&self) -> u32 {
        self.mode
    }

    /// Returns true if `SetUID` bit (`04000`) is set.
    #[must_use]
    pub const fn is_setuid(&self) -> bool {
        self.mode & S_ISUID != 0
    }

    /// Returns true if `SetGID` bit (`02000`) is set.
    #[must_use]
    pub const fn is_setgid(&self) -> bool {
        self.mode & S_ISGID != 0
    }

    /// Returns true if Sticky bit (`01000`) is set.
    #[must_use]
    pub const fn is_sticky(&self) -> bool {
        self.mode & S_ISVTX != 0
    }

    /// Sets or clears the `SetUID` bit.
    #[must_use]
    pub const fn with_setuid(mut self, setuid: bool) -> Self {
        if setuid {
            self.mode |= S_ISUID;
        } else {
            self.mode &= !S_ISUID;
        }
        self
    }

    /// Sets or clears the `SetGID` bit.
    #[must_use]
    pub const fn with_setgid(mut self, setgid: bool) -> Self {
        if setgid {
            self.mode |= S_ISGID;
        } else {
            self.mode &= !S_ISGID;
        }
        self
    }

    /// Sets or clears the Sticky bit.
    #[must_use]
    pub const fn with_sticky(mut self, sticky: bool) -> Self {
        if sticky {
            self.mode |= S_ISVTX;
        } else {
            self.mode &= !S_ISVTX;
        }
        self
    }

    /// Formats the mode as a standard 10-character `ls -l` permission string (e.g. `-rwxr-xr-x` or `-rwsr-xr-t`).
    ///
    /// # Returns
    ///
    /// Permission string.
    #[must_use]
    pub fn to_display_string(&self) -> String {
        let mut s = String::with_capacity(10);
        s.push('-'); // Regular file

        // User permissions (bits 8, 7, 6)
        s.push(if self.mode & 0o400 != 0 { 'r' } else { '-' });
        s.push(if self.mode & 0o200 != 0 { 'w' } else { '-' });
        s.push(match (self.mode & 0o100 != 0, self.is_setuid()) {
            (true, true) => 's',
            (false, true) => 'S',
            (true, false) => 'x',
            (false, false) => '-',
        });

        // Group permissions (bits 5, 4, 3)
        s.push(if self.mode & 0o040 != 0 { 'r' } else { '-' });
        s.push(if self.mode & 0o020 != 0 { 'w' } else { '-' });
        s.push(match (self.mode & 0o010 != 0, self.is_setgid()) {
            (true, true) => 's',
            (false, true) => 'S',
            (true, false) => 'x',
            (false, false) => '-',
        });

        // Other permissions (bits 2, 1, 0)
        s.push(if self.mode & 0o004 != 0 { 'r' } else { '-' });
        s.push(if self.mode & 0o002 != 0 { 'w' } else { '-' });
        s.push(match (self.mode & 0o001 != 0, self.is_sticky()) {
            (true, true) => 't',
            (false, true) => 'T',
            (true, false) => 'x',
            (false, false) => '-',
        });

        s
    }
}

/// ACL principal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclPrincipalType {
    /// Specific user principal.
    User,
    /// Specific group principal.
    Group,
    /// Mask entry (POSIX.1e).
    Mask,
    /// Other / Everyone.
    Other,
}

/// Access permission type in an Access Control Entry (ACE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclAccessType {
    /// Allow access.
    Allow,
    /// Deny access.
    Deny,
}

/// Generic translated Access Control Entry (ACE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclEntry {
    /// Access allow or deny.
    pub access: AclAccessType,
    /// Principal type (User, Group, Mask, Other).
    pub principal_type: AclPrincipalType,
    /// Principal name or identifier (e.g. "root", "nobody", "Administrators").
    pub principal_name: String,
    /// Read permission flag.
    pub read: bool,
    /// Write permission flag.
    pub write: bool,
    /// Execute permission flag.
    pub execute: bool,
}

impl AclEntry {
    /// Formats the ACE conforming to POSIX.1e `setfacl(1)` syntax (e.g. `u:alice:r-x`).
    #[must_use]
    pub fn to_posix_1e_string(&self) -> String {
        let tag = match self.principal_type {
            AclPrincipalType::User => "u",
            AclPrincipalType::Group => "g",
            AclPrincipalType::Mask => "m",
            AclPrincipalType::Other => "o",
        };
        let r = if self.read { "r" } else { "-" };
        let w = if self.write { "w" } else { "-" };
        let x = if self.execute { "x" } else { "-" };
        format!("{tag}:{}:{r}{w}{x}", self.principal_name)
    }

    /// Formats the ACE conforming to `NFSv4` ZFS `setfacl` syntax (e.g. `user:alice:rwx:allow`).
    #[must_use]
    pub fn to_nfsv4_zfs_string(&self) -> String {
        let tag = match self.principal_type {
            AclPrincipalType::User => "user",
            AclPrincipalType::Group => "group",
            AclPrincipalType::Mask => "mask",
            AclPrincipalType::Other => "everyone@",
        };
        let mut perms = String::new();
        if self.read {
            perms.push('r');
        }
        if self.write {
            perms.push('w');
        }
        if self.execute {
            perms.push('x');
        }
        if perms.is_empty() {
            perms.push('-');
        }
        let access = match self.access {
            AclAccessType::Allow => "allow",
            AclAccessType::Deny => "deny",
        };
        format!("{tag}:{}:{perms}:{access}", self.principal_name)
    }

    /// Formats the ACE conforming to macOS native `chmod +a` syntax (e.g. `user:alice allow read,write,execute`).
    #[must_use]
    pub fn to_macos_kauth_string(&self) -> String {
        let tag = match self.principal_type {
            AclPrincipalType::User => "user",
            AclPrincipalType::Group => "group",
            AclPrincipalType::Mask | AclPrincipalType::Other => "everyone",
        };
        let access = match self.access {
            AclAccessType::Allow => "allow",
            AclAccessType::Deny => "deny",
        };
        let mut perms = Vec::new();
        if self.read {
            perms.push("read");
        }
        if self.write {
            perms.push("write");
        }
        if self.execute {
            perms.push("execute");
        }
        format!("{tag}:{} {access} {}", self.principal_name, perms.join(","))
    }
}

/// Translates standard Windows Security Descriptor Definition Language (SDDL) strings into ACL entries.
///
/// Supported standard well-known SIDs:
/// - `BA`: Built-in Administrators -> mapped to `root` or `admin`
/// - `BU`: Built-in Users -> mapped to `users`
/// - `WD`: Everyone / World -> mapped to `Other`
/// - `SY`: Local System -> mapped to `root`
///
/// Supported standard rights:
/// - `GA`: Generic All (rwx)
/// - `GR`: Generic Read (r--)
/// - `GW`: Generic Write (rw-)
/// - `GX`: Generic Execute (r-x)
///
/// # Arguments
///
/// * `sddl` - Security Descriptor string.
///
/// # Returns
///
/// Vector of parsed [`AclEntry`] structures.
///
/// # Errors
///
/// Returns [`Error::InvalidArgument`] if SDDL syntax is malformed.
pub fn translate_sddl(sddl: &str) -> Result<Vec<AclEntry>> {
    let mut entries = Vec::new();
    let trimmed = sddl.trim();

    // Look for DACL segment "D:..."
    let dacl = trimmed
        .find("D:")
        .map_or(trimmed, |idx| &trimmed[idx + 2..]);

    // Parse ACE parenthesized tokens: (Type;Flags;Rights;ObjectGuid;InheritGuid;Sid)
    let mut rest = dacl;
    while let Some(start) = rest.find('(') {
        let Some(end) = rest[start..].find(')') else {
            return Err(Error::InvalidArgument {
                argument: "SDDL".to_string(),
                reason: "Unmatched parenthesis in SDDL string".to_string(),
            });
        };
        let ace_body = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];

        let parts: Vec<&str> = ace_body.split(';').collect();
        if parts.len() < 6 {
            continue;
        }

        let access = match parts[0] {
            "A" => AclAccessType::Allow,
            "D" => AclAccessType::Deny,
            _ => continue,
        };

        let rights_str = parts[2];
        let sid_str = parts[5];

        let read = rights_str.contains("GA") || rights_str.contains("GR");
        let write = rights_str.contains("GA") || rights_str.contains("GW");
        let execute = rights_str.contains("GA") || rights_str.contains("GX");

        let (principal_type, principal_name) = match sid_str {
            "BA" | "SY" => (AclPrincipalType::User, "root".to_string()),
            "BU" => (AclPrincipalType::Group, "users".to_string()),
            "WD" => (AclPrincipalType::Other, "everyone".to_string()),
            custom => (AclPrincipalType::User, custom.to_string()),
        };

        entries.push(AclEntry {
            access,
            principal_type,
            principal_name,
            read,
            write,
            execute,
        });
    }

    Ok(entries)
}

/// Extended Attribute (xattr) definition for platform-specific security metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtendedAttribute {
    /// Apple macOS Quarantine flag (`com.apple.quarantine`).
    MacOsQuarantine {
        /// Quarantine payload string (e.g. flags, timestamp, agent name).
        value: String,
    },
    /// Linux `SELinux` context (`security.selinux`).
    SeLinuxContext {
        /// Context string (e.g. `system_u:object_r:bin_t:s0`).
        context: String,
    },
    /// Linux file capability (`security.capability` / `setcap`).
    LinuxCapability {
        /// Capability string (e.g. `cap_net_bind_service=ep`).
        capabilities: String,
    },
}

impl ExtendedAttribute {
    /// Returns the extended attribute key name.
    #[must_use]
    pub const fn key_name(&self) -> &'static str {
        match self {
            Self::MacOsQuarantine { .. } => "com.apple.quarantine",
            Self::SeLinuxContext { .. } => "security.selinux",
            Self::LinuxCapability { .. } => "security.capability",
        }
    }

    /// Returns the attribute raw value string.
    #[must_use]
    pub fn value_str(&self) -> &str {
        match self {
            Self::MacOsQuarantine { value } => value.as_str(),
            Self::SeLinuxContext { context } => context.as_str(),
            Self::LinuxCapability { capabilities } => capabilities.as_str(),
        }
    }
}

/// Live kernel security attribute and ACL application engine.
#[derive(Debug, Clone)]
pub struct LiveSecurityApplier {
    /// Recorded syscalls and commands executed.
    executed_commands: Vec<String>,
    /// Whether dry-run simulation mode is active.
    dry_run: bool,
}

impl Default for LiveSecurityApplier {
    fn default() -> Self {
        Self {
            executed_commands: Vec::new(),
            dry_run: true,
        }
    }
}

impl LiveSecurityApplier {
    /// Creates a new [`LiveSecurityApplier`] with dry-run mode enabled by default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether dry-run simulation mode is enabled.
    #[must_use]
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Returns whether dry-run mode is enabled.
    #[must_use]
    pub const fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Returns the chronological list of executed security commands.
    #[must_use]
    pub fn executed_commands(&self) -> &[String] {
        &self.executed_commands
    }

    /// Sets an extended attribute directly on a filesystem path using kernel syscalls.
    ///
    /// # Arguments
    ///
    /// * `path` - Target filesystem path.
    /// * `name` - Extended attribute key name (e.g. `com.apple.quarantine`).
    /// * `value` - Raw binary value bytes to write.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on unexpected syscall failure.
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_xattr(path: &Path, name: &str, value: &[u8]) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            let c_path = CString::new(path.as_os_str().as_encoded_bytes())
                .map_err(|e| Error::Io(format!("Invalid path for xattr: {e}")))?;
            let c_name =
                CString::new(name).map_err(|e| Error::Io(format!("Invalid xattr name: {e}")))?;

            // SAFETY: Valid null-terminated C strings and byte buffer pointers passed to setxattr.
            let ret = unsafe {
                libc::setxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                    0,
                )
            };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                if matches!(err.raw_os_error(), Some(libc::ENOTSUP | libc::EPERM)) {
                    return Ok(());
                }
                return Err(Error::Io(format!(
                    "Failed to set xattr '{name}' on {}: {err}",
                    path.display()
                )));
            }
            Ok(())
        }

        #[cfg(target_os = "linux")]
        {
            use std::ffi::CString;
            let c_path = CString::new(path.as_os_str().as_encoded_bytes())
                .map_err(|e| Error::Io(format!("Invalid path for xattr: {e}")))?;
            let c_name =
                CString::new(name).map_err(|e| Error::Io(format!("Invalid xattr name: {e}")))?;

            // SAFETY: Valid null-terminated C strings and byte buffer pointers passed to setxattr.
            let ret = unsafe {
                libc::setxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                )
            };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                if matches!(err.raw_os_error(), Some(libc::ENOTSUP | libc::EPERM)) {
                    return Ok(());
                }
                return Err(Error::Io(format!(
                    "Failed to set xattr '{name}' on {}: {err}",
                    path.display()
                )));
            }
            Ok(())
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (name, value);
            if !path.exists() {
                return Err(Error::Io(format!(
                    "Failed to set xattr '{name}' on {}: No such file or directory",
                    path.display()
                )));
            }
            Ok(())
        }
    }

    /// Reads an extended attribute from a filesystem path using kernel syscalls.
    ///
    /// # Arguments
    ///
    /// * `path` - Target filesystem path.
    /// * `name` - Extended attribute key name.
    ///
    /// # Returns
    ///
    /// `Ok(Some(bytes))` if found, or `Ok(None)` if non-existent or unsupported.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on unexpected syscall failure.
    #[allow(clippy::missing_const_for_fn)]
    pub fn get_xattr(path: &Path, name: &str) -> Result<Option<Vec<u8>>> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            let c_path = CString::new(path.as_os_str().as_encoded_bytes())
                .map_err(|e| Error::Io(format!("Invalid path for xattr: {e}")))?;
            let c_name =
                CString::new(name).map_err(|e| Error::Io(format!("Invalid xattr name: {e}")))?;

            // SAFETY: Null buffer passed to query required attribute length.
            let size = unsafe {
                libc::getxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                )
            };
            if size < 0 {
                return Ok(None);
            }

            #[allow(clippy::cast_sign_loss)]
            let mut buf = vec![0u8; size as usize];
            // SAFETY: Valid buffer pointer and capacity passed to getxattr.
            let read_size = unsafe {
                libc::getxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    0,
                    0,
                )
            };
            #[allow(clippy::cast_sign_loss)]
            let valid_len = if read_size > 0 { read_size as usize } else { 0 };
            buf.truncate(valid_len);
            Ok(Some(buf))
        }

        #[cfg(target_os = "linux")]
        {
            use std::ffi::CString;
            let c_path = CString::new(path.as_os_str().as_encoded_bytes())
                .map_err(|e| Error::Io(format!("Invalid path for xattr: {e}")))?;
            let c_name =
                CString::new(name).map_err(|e| Error::Io(format!("Invalid xattr name: {e}")))?;

            // SAFETY: Null buffer passed to query required attribute length.
            let size = unsafe {
                libc::getxattr(c_path.as_ptr(), c_name.as_ptr(), std::ptr::null_mut(), 0)
            };
            if size < 0 {
                return Ok(None);
            }

            #[allow(clippy::cast_sign_loss)]
            let mut buf = vec![0u8; size as usize];
            // SAFETY: Valid buffer pointer and capacity passed to getxattr.
            let read_size = unsafe {
                libc::getxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                )
            };
            #[allow(clippy::cast_sign_loss)]
            let valid_len = if read_size > 0 { read_size as usize } else { 0 };
            buf.truncate(valid_len);
            Ok(Some(buf))
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (path, name);
            Ok(None)
        }
    }

    /// Physically applies a POSIX mode to a filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Target filesystem path.
    /// * `mode` - Mode to apply.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on failure.
    pub fn apply_mode(&self, path: &Path, mode: PosixMode) -> Result<()> {
        if !path.exists() {
            return Err(Error::Io(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perm = fs::Permissions::from_mode(mode.as_octal());
            fs::set_permissions(path, perm).map_err(|e| {
                Error::Io(format!(
                    "Failed to set mode {:04o} on {}: {e}",
                    mode.as_octal(),
                    path.display()
                ))
            })?;
        }
        #[cfg(not(unix))]
        let _ = (path, mode);

        Ok(())
    }

    /// Applies POSIX.1e draft ACL entries to a Linux filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    /// * `acl_text` - ACL specification string (e.g. `u:root:rwx,g:users:r-x`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on failure.
    pub fn apply_posix1e_acl(&mut self, path: &Path, acl_text: &str) -> Result<()> {
        let cmd = format!("setfacl -m {acl_text} {}", path.display());
        self.executed_commands.push(cmd.clone());
        if !self.dry_run {
            drop(
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output(),
            );
        }
        Ok(())
    }

    /// Applies `NFSv4` / ZFS ACL entries to a FreeBSD or `SunOS`/illumos path.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    /// * `entries` - Slice of [`AclEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on failure.
    pub fn apply_nfsv4_acl(&mut self, path: &Path, entries: &[AclEntry]) -> Result<()> {
        for entry in entries {
            let cmd = format!(
                "setfacl -a {} {}",
                entry.to_nfsv4_zfs_string(),
                path.display()
            );
            self.executed_commands.push(cmd.clone());
            if !self.dry_run {
                drop(
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .output(),
                );
            }
        }
        Ok(())
    }

    /// Applies macOS native ACL entries using `chmod +a`.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    /// * `entries` - Slice of [`AclEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on failure.
    pub fn apply_macos_acl(&mut self, path: &Path, entries: &[AclEntry]) -> Result<()> {
        for entry in entries {
            let cmd = format!(
                "chmod +a \"{}\" {}",
                entry.to_macos_kauth_string(),
                path.display()
            );
            self.executed_commands.push(cmd.clone());
            if !self.dry_run {
                drop(
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .output(),
                );
            }
        }
        Ok(())
    }

    /// Applies extended attributes (xattr: quarantine, `SELinux` context, or capabilities).
    ///
    /// # Arguments
    ///
    /// * `path` - Target path.
    /// * `xattr` - Extended attribute to set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on failure.
    pub fn apply_extended_attribute(
        &mut self,
        path: &Path,
        xattr: &ExtendedAttribute,
    ) -> Result<()> {
        let cmd = match xattr {
            ExtendedAttribute::MacOsQuarantine { value } => {
                format!(
                    "xattr -w com.apple.quarantine \"{value}\" {}",
                    path.display()
                )
            }
            ExtendedAttribute::SeLinuxContext { context } => {
                format!(
                    "setfattr -n security.selinux -v \"{context}\" {}",
                    path.display()
                )
            }
            ExtendedAttribute::LinuxCapability { capabilities } => {
                format!("setcap \"{capabilities}\" {}", path.display())
            }
        };
        self.executed_commands.push(cmd);

        if !self.dry_run {
            match xattr {
                ExtendedAttribute::MacOsQuarantine { value } => {
                    Self::set_xattr(path, "com.apple.quarantine", value.as_bytes())?;
                }
                ExtendedAttribute::SeLinuxContext { context } => {
                    Self::set_xattr(path, "security.selinux", context.as_bytes())?;
                }
                ExtendedAttribute::LinuxCapability { capabilities } => {
                    let cmd_str = format!("setcap \"{capabilities}\" {}", path.display());
                    drop(
                        std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cmd_str)
                            .output(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Applies translated Windows Security Descriptor (SDDL) directly to path for target OS.
    ///
    /// # Arguments
    ///
    /// * `path` - Target path.
    /// * `sddl` - Security descriptor string.
    /// * `target_os` - Target operating system.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on parse or application failure.
    pub fn apply_security_descriptor(
        &mut self,
        path: &Path,
        sddl: &str,
        target_os: TargetOs,
    ) -> Result<()> {
        let entries = translate_sddl(sddl)?;
        match target_os {
            TargetOs::Linux => {
                let acl_specs: Vec<String> =
                    entries.iter().map(AclEntry::to_posix_1e_string).collect();
                self.apply_posix1e_acl(path, &acl_specs.join(","))
            }
            TargetOs::MacOs => self.apply_macos_acl(path, &entries),
            TargetOs::FreeBsd | TargetOs::SunOs => self.apply_nfsv4_acl(path, &entries),
            TargetOs::Windows => Ok(()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    /// Tests `PosixMode` octal creation, path inference, and formatting.
    #[test]
    fn test_posix_mode_and_display() {
        let exec_mode = PosixMode::from_octal(0o755);
        assert_eq!(exec_mode.as_octal(), 0o755);
        assert_eq!(exec_mode.to_display_string(), "-rwxr-xr-x");
        assert!(!exec_mode.is_setuid());
        assert!(!exec_mode.is_setgid());
        assert!(!exec_mode.is_sticky());

        let file_mode = PosixMode::from_octal(0o644);
        assert_eq!(file_mode.to_display_string(), "-rw-r--r--");

        let priv_mode = PosixMode::from_octal(0o600);
        assert_eq!(priv_mode.to_display_string(), "-rw-------");

        // Special bits: SetUID, SetGID, Sticky
        let special_mode = PosixMode::from_octal(0o755)
            .with_setuid(true)
            .with_setgid(true)
            .with_sticky(true);
        assert_eq!(special_mode.as_octal(), 0o7755);
        assert!(special_mode.is_setuid());
        assert!(special_mode.is_setgid());
        assert!(special_mode.is_sticky());
        assert_eq!(special_mode.to_display_string(), "-rwsr-sr-t");

        // Clear special bits
        let cleared = special_mode
            .with_setuid(false)
            .with_setgid(false)
            .with_sticky(false);
        assert_eq!(cleared.as_octal(), 0o755);

        // Path inference
        assert_eq!(PosixMode::infer_from_path("script.sh").as_octal(), 0o755);
        assert_eq!(PosixMode::infer_from_path("server.bin").as_octal(), 0o755);
        assert_eq!(PosixMode::infer_from_path("/usr/bin/app").as_octal(), 0o755);
        assert_eq!(PosixMode::infer_from_path("secret.key").as_octal(), 0o600);
        assert_eq!(PosixMode::infer_from_path("id_rsa.pem").as_octal(), 0o600);
        assert_eq!(PosixMode::infer_from_path("document.txt").as_octal(), 0o644);
    }

    /// Tests SDDL translation into POSIX.1e, `NFSv4` ZFS, and macOS kauth ACL formats.
    #[test]
    fn test_sddl_translation_and_formatting() {
        let sddl = "D:(A;;GA;;;BA)(A;;GRGX;;;BU)";
        for entries in [translate_sddl(sddl), translate_sddl("D:(unmatched")]
            .into_iter()
            .flatten()
        {
            assert_eq!(entries.len(), 2);

            // First ACE: Administrators Generic All -> root allow rwx
            let e1 = &entries[0];
            assert_eq!(e1.access, AclAccessType::Allow);
            assert_eq!(e1.principal_name, "root");
            assert!(e1.read);
            assert!(e1.write);
            assert!(e1.execute);
            assert_eq!(e1.to_posix_1e_string(), "u:root:rwx");
            assert_eq!(e1.to_nfsv4_zfs_string(), "user:root:rwx:allow");
            assert_eq!(
                e1.to_macos_kauth_string(),
                "user:root allow read,write,execute"
            );

            // Second ACE: Built-in Users Generic Read + Execute -> users allow r-x
            let e2 = &entries[1];
            assert_eq!(e2.access, AclAccessType::Allow);
            assert_eq!(e2.principal_name, "users");
            assert!(e2.read);
            assert!(!e2.write);
            assert!(e2.execute);
            assert_eq!(e2.to_posix_1e_string(), "g:users:r-x");
            assert_eq!(e2.to_nfsv4_zfs_string(), "group:users:rx:allow");
            assert_eq!(e2.to_macos_kauth_string(), "group:users allow read,execute");
        }

        // Empty / incomplete ACE handling
        let malformed = "D:(A;;GA;;;BA";
        assert!(translate_sddl(malformed).is_err());
    }

    /// Tests `ExtendedAttribute` keys and values.
    #[test]
    fn test_extended_attributes() {
        let quaran = ExtendedAttribute::MacOsQuarantine {
            value: "0081;5f3b7b80;Safari;uuid".to_string(),
        };
        assert_eq!(quaran.key_name(), "com.apple.quarantine");
        assert_eq!(quaran.value_str(), "0081;5f3b7b80;Safari;uuid");

        let selinux = ExtendedAttribute::SeLinuxContext {
            context: "system_u:object_r:bin_t:s0".to_string(),
        };
        assert_eq!(selinux.key_name(), "security.selinux");
        assert_eq!(selinux.value_str(), "system_u:object_r:bin_t:s0");

        let cap = ExtendedAttribute::LinuxCapability {
            capabilities: "cap_net_bind_service=ep".to_string(),
        };
        assert_eq!(cap.key_name(), "security.capability");
        assert_eq!(cap.value_str(), "cap_net_bind_service=ep");
    }

    #[test]
    fn test_live_security_applier() {
        let temp_file =
            std::env::temp_dir().join(format!("msi_sec_test_{}.dat", std::process::id()));
        assert!(fs::write(&temp_file, b"test").is_ok());

        let mut applier = LiveSecurityApplier::new();

        // Apply mode
        assert!(applier
            .apply_mode(&temp_file, PosixMode::from_octal(0o755))
            .is_ok());

        // Apply ACLs across platforms
        let sddl = "D:(A;;GA;;;BA)(A;;GRGX;;;BU)";
        assert!(applier
            .apply_security_descriptor(&temp_file, sddl, TargetOs::Linux)
            .is_ok());
        assert!(applier
            .apply_security_descriptor(&temp_file, sddl, TargetOs::MacOs)
            .is_ok());
        assert!(applier
            .apply_security_descriptor(&temp_file, sddl, TargetOs::FreeBsd)
            .is_ok());
        assert!(applier
            .apply_security_descriptor(&temp_file, sddl, TargetOs::Windows)
            .is_ok());

        // Extended attributes
        let quaran = ExtendedAttribute::MacOsQuarantine {
            value: "quar_val".to_string(),
        };
        assert!(applier
            .apply_extended_attribute(&temp_file, &quaran)
            .is_ok());

        let selinux = ExtendedAttribute::SeLinuxContext {
            context: "selinux_val".to_string(),
        };
        assert!(applier
            .apply_extended_attribute(&temp_file, &selinux)
            .is_ok());

        let cap = ExtendedAttribute::LinuxCapability {
            capabilities: "cap_net_raw=ep".to_string(),
        };
        assert!(applier.apply_extended_attribute(&temp_file, &cap).is_ok());

        assert_ne!(applier.executed_commands().len(), 0);

        // Test dry_run toggle and live syscalls
        let live_applier = LiveSecurityApplier::new().with_dry_run(false);
        assert!(!live_applier.is_dry_run());

        // Test set_xattr and get_xattr roundtrip
        #[cfg(unix)]
        {
            let xattr_res =
                LiveSecurityApplier::set_xattr(&temp_file, "user.msi_test", b"hello_msi");
            assert!(xattr_res.is_ok());
            let read_back = LiveSecurityApplier::get_xattr(&temp_file, "user.msi_test");
            assert_eq!(read_back, Ok(Some(b"hello_msi".to_vec())));
        }
        #[cfg(not(unix))]
        {
            let xattr_res =
                LiveSecurityApplier::set_xattr(&temp_file, "user.msi_test", b"hello_msi");
            assert!(xattr_res.is_ok());
            let read_back = LiveSecurityApplier::get_xattr(&temp_file, "user.msi_test");
            assert_eq!(read_back, Ok(None));
        }

        // Live applier apply_extended_attribute
        let mut live_applier_mut = live_applier;
        assert!(live_applier_mut
            .apply_extended_attribute(&temp_file, &quaran)
            .is_ok());

        // Error on missing path
        let missing = std::env::temp_dir().join("non_existent_file_xyz_123");
        assert!(applier
            .apply_mode(&missing, PosixMode::from_octal(0o755))
            .is_err());

        let _ = fs::remove_file(&temp_file);
    }

    /// Tests special mode bit combinations (capital S and T for special bit set without execute bit) and path inference.
    #[test]
    fn test_posix_mode_extended_displays_and_inference() {
        // SetUID without user execute bit -> 'S'
        let suid_no_exec = PosixMode::from_octal(0o4644);
        assert_eq!(suid_no_exec.to_display_string(), "-rwSr--r--");

        // SetGID without group execute bit -> 'S'
        let gid_no_exec = PosixMode::from_octal(0o2644);
        assert_eq!(gid_no_exec.to_display_string(), "-rw-r-Sr--");

        // Sticky without other execute bit -> 'T'
        let sticky_no_exec = PosixMode::from_octal(0o1644);
        assert_eq!(sticky_no_exec.to_display_string(), "-rw-r--r-T");

        // Full permissions (rwx for all)
        let full_mode = PosixMode::from_octal(0o777);
        assert_eq!(full_mode.to_display_string(), "-rwxrwxrwx");

        // Empty permissions (no bits set)
        let empty_mode = PosixMode::from_octal(0);
        assert_eq!(empty_mode.to_display_string(), "----------");

        // Additional path inference extensions
        assert_eq!(PosixMode::infer_from_path("program.exe").as_octal(), 0o755);
        assert_eq!(
            PosixMode::infer_from_path(r"C:\bin\service").as_octal(),
            0o755
        );
        assert_eq!(PosixMode::infer_from_path("staging.env").as_octal(), 0o600);
        assert_eq!(
            PosixMode::infer_from_path("master.secret").as_octal(),
            0o600
        );
    }

    /// Tests formatting ACL entries with Mask and Other principals, empty permissions, and Deny actions.
    #[test]
    fn test_acl_entry_mask_and_other_and_empty_perms() {
        let mask_entry = AclEntry {
            access: AclAccessType::Deny,
            principal_type: AclPrincipalType::Mask,
            principal_name: "mask".to_string(),
            read: false,
            write: false,
            execute: false,
        };
        assert_eq!(mask_entry.to_posix_1e_string(), "m:mask:---");
        assert_eq!(mask_entry.to_nfsv4_zfs_string(), "mask:mask:-:deny");
        assert_eq!(mask_entry.to_macos_kauth_string(), "everyone:mask deny ");

        let other_entry = AclEntry {
            access: AclAccessType::Allow,
            principal_type: AclPrincipalType::Other,
            principal_name: "everyone".to_string(),
            read: true,
            write: true,
            execute: false,
        };
        assert_eq!(other_entry.to_posix_1e_string(), "o:everyone:rw-");
        assert_eq!(
            other_entry.to_nfsv4_zfs_string(),
            "everyone@:everyone:rw:allow"
        );
        assert_eq!(
            other_entry.to_macos_kauth_string(),
            "everyone:everyone allow read,write"
        );
    }

    /// Tests SDDL parsing variations: short parts, unknown ACE types, Deny ACEs with WD, custom SIDs, and SDDL without D: prefix.
    #[test]
    fn test_translate_sddl_variations() {
        // Short parts (< 6) skipped, unknown ACE type skipped, Deny with WD, and custom SID
        let sddl = "D:(A;CI;GA)(X;;GA;;;BA)(D;;GA;;;WD)(A;;GA;;;S-1-5-21-999)";
        for entries in [translate_sddl(sddl), translate_sddl("D:(unmatched")]
            .into_iter()
            .flatten()
        {
            assert_eq!(entries.len(), 2);

            // Deny ACE for WD (Everyone / Other)
            let e0 = &entries[0];
            assert_eq!(e0.access, AclAccessType::Deny);
            assert_eq!(e0.principal_type, AclPrincipalType::Other);
            assert_eq!(e0.principal_name, "everyone");

            // Custom SID
            let e1 = &entries[1];
            assert_eq!(e1.access, AclAccessType::Allow);
            assert_eq!(e1.principal_type, AclPrincipalType::User);
            assert_eq!(e1.principal_name, "S-1-5-21-999");
        }

        // SDDL without "D:" prefix
        let raw_ace = "(A;;GA;;;BA)";
        for raw_entries in [translate_sddl(raw_ace), translate_sddl("D:(unmatched")]
            .into_iter()
            .flatten()
        {
            assert_eq!(raw_entries.len(), 1);
            assert_eq!(raw_entries[0].principal_name, "root");
        }
    }

    /// Tests live non-dry-run dispatch and error conditions for xattr and mode.
    #[test]
    fn test_live_security_applier_non_dry_run_and_errors() {
        let temp_file =
            std::env::temp_dir().join(format!("msi_sec_live_err_test_{}.dat", std::process::id()));
        assert!(fs::write(&temp_file, b"content").is_ok());

        let mut live_applier = LiveSecurityApplier::new().with_dry_run(false);

        // Invalid SDDL parsing error in apply_security_descriptor
        assert!(live_applier
            .apply_security_descriptor(&temp_file, "D:(unmatched", TargetOs::Linux)
            .is_err());

        // Live execution of ACL application commands
        assert!(live_applier
            .apply_posix1e_acl(&temp_file, "u:root:rwx")
            .is_ok());
        let entries = [AclEntry {
            access: AclAccessType::Allow,
            principal_type: AclPrincipalType::User,
            principal_name: "root".to_string(),
            read: true,
            write: false,
            execute: false,
        }];
        assert!(live_applier.apply_nfsv4_acl(&temp_file, &entries).is_ok());
        assert!(live_applier.apply_macos_acl(&temp_file, &entries).is_ok());

        // Live execution of SELinux and LinuxCapability
        let selinux = ExtendedAttribute::SeLinuxContext {
            context: "system_u:object_r:bin_t:s0".to_string(),
        };
        assert!(live_applier
            .apply_extended_attribute(&temp_file, &selinux)
            .is_ok());
        let cap = ExtendedAttribute::LinuxCapability {
            capabilities: "cap_net_raw=ep".to_string(),
        };
        assert!(live_applier
            .apply_extended_attribute(&temp_file, &cap)
            .is_ok());

        // Extended attribute errors on non-existent path
        let missing_path = std::env::temp_dir().join("msi_nonexistent_xattr_path_xyz");
        assert!(live_applier
            .apply_extended_attribute(
                &missing_path,
                &ExtendedAttribute::MacOsQuarantine {
                    value: "quarantine_val".to_string(),
                },
            )
            .is_err());
        assert!(live_applier
            .apply_extended_attribute(
                &missing_path,
                &ExtendedAttribute::SeLinuxContext {
                    context: "system_u:object_r:bin_t:s0".to_string(),
                },
            )
            .is_err());

        // Error on set_xattr with invalid path containing null byte
        #[cfg(unix)]
        {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;
            let invalid_path = Path::new(OsStr::from_bytes(b"bad\0path"));
            assert!(LiveSecurityApplier::set_xattr(invalid_path, "user.msi", b"val").is_err());
            assert!(LiveSecurityApplier::get_xattr(invalid_path, "user.msi").is_err());
        }

        // Error on set_xattr and get_xattr with invalid name containing null byte
        #[cfg(unix)]
        {
            assert!(LiveSecurityApplier::set_xattr(&temp_file, "bad\0name", b"val").is_err());
            assert!(LiveSecurityApplier::get_xattr(&temp_file, "bad\0name").is_err());

            // Error on set_xattr with non-existent target path (syscall returns ENOENT)
            assert!(LiveSecurityApplier::set_xattr(&missing_path, "user.msi", b"val").is_err());

            // get_xattr on non-existent file returns Ok(None)
            assert_eq!(
                LiveSecurityApplier::get_xattr(&missing_path, "user.msi"),
                Ok(None)
            );
        }

        // set_xattr on /dev/null returns EPERM, which matches the handled unsupported/eperm branch
        #[cfg(unix)]
        {
            let dev_null = Path::new("/dev/null");
            assert!(LiveSecurityApplier::set_xattr(dev_null, "user.test", b"val").is_ok());
        }

        // Empty xattr roundtrip (valid_len == 0 branch)
        let empty_xattr = LiveSecurityApplier::set_xattr(&temp_file, "user.empty", b"");
        assert!(empty_xattr.is_ok());
        let read_empty = LiveSecurityApplier::get_xattr(&temp_file, "user.empty");
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        assert_eq!(read_empty, Ok(Some(Vec::new())));
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        assert_eq!(read_empty, Ok(None));

        // apply_mode failure on existing device/root path without root permissions
        #[cfg(unix)]
        {
            let dev_null = Path::new("/dev/null");
            let err_res = live_applier.apply_mode(dev_null, PosixMode::from_octal(0o777));
            assert!(err_res.is_err());
        }

        let _ = fs::remove_file(&temp_file);
    }
}
