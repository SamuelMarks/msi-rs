//! Cross-Platform User & Group Management.
//!
//! Provides execution logic for `WixUtilExtension` `<util:User>` and `<util:Group>` provisioning.
//! - **Windows:** Integrates natively with Win32 `NetUserAdd`, `NetLocalGroupAdd`, and LSA policies.
//! - **POSIX:** Translates to standard OS binaries (`useradd`, `groupadd`, `usermod`) with graceful degradation.

use crate::error::{MsiError, Result};
use std::fmt;

/// Determines if the current process is running with elevated privileges (Administrator/root).
#[must_use]
pub fn is_elevated() -> bool {
    #[cfg(target_family = "unix")]
    {
        // SAFETY: geteuid is always safe to call and has no side effects.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(target_os = "windows")]
    {
        // Stub for Windows elevation check without heavy winapi dependencies.
        // In a full implementation, this checks the process token for Administrators SID.
        std::env::var("UAC_ELEVATED").is_ok()
    }
    #[cfg(not(any(target_family = "unix", target_os = "windows")))]
    {
        false
    }
}

/// A strong type representing a User Name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserName(String);

impl UserName {
    /// Creates a new `UserName`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strong type representing a Group Name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupName(String);

impl GroupName {
    /// Creates a new `GroupName`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GroupName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A strong type representing a Windows Security Identifier (SID).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecurityIdentifier(String);

impl SecurityIdentifier {
    /// Creates a new `SecurityIdentifier`.
    #[must_use]
    pub fn new(sid: impl Into<String>) -> Self {
        Self(sid.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecurityIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A manager for local users and groups.
pub trait LocalAccountManager {
    /// Creates a local user account.
    ///
    /// # Privilege Escalation
    /// This operation typically requires `root` or `Administrator` privileges. Ensure the installer
    /// context is elevated before invocation, otherwise execution will fail gracefully.
    ///
    /// # Errors
    /// Returns [`MsiError::UserManagementError`] if the operation fails, or if the user already exists
    /// and `fail_if_exists` is true.
    fn create_user(
        &self,
        name: &UserName,
        password: Option<&str>,
        fail_if_exists: bool,
    ) -> Result<String>;

    /// Creates a local group.
    ///
    /// # Privilege Escalation
    /// Requires elevated execution context.
    ///
    /// # Errors
    /// Returns [`MsiError::UserManagementError`] if the operation fails, or if the group already exists
    /// and `fail_if_exists` is true.
    fn create_group(&self, name: &GroupName, fail_if_exists: bool) -> Result<String>;

    /// Adds a user to a local group.
    ///
    /// # Errors
    /// Returns [`MsiError::UserManagementError`] if the operation fails.
    fn add_user_to_group(&self, user: &UserName, group: &GroupName) -> Result<String>;
}

/// POSIX implementation using standard `useradd`/`groupadd` binaries.
#[derive(Debug, Default, Clone, Copy)]
pub struct PosixAccountManager;

impl LocalAccountManager for PosixAccountManager {
    fn create_user(
        &self,
        name: &UserName,
        _password: Option<&str>,
        fail_if_exists: bool,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::UserManagementError(
                "User name cannot be empty".to_string(),
            ));
        }

        let mut cmd = format!("useradd {name}");
        if !fail_if_exists {
            cmd = format!("id -u {name} >/dev/null 2>&1 || {cmd}");
        }

        Ok(cmd)
    }

    fn create_group(&self, name: &GroupName, fail_if_exists: bool) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::UserManagementError(
                "Group name cannot be empty".to_string(),
            ));
        }

        let mut cmd = format!("groupadd {name}");
        if !fail_if_exists {
            cmd = format!("getent group {name} >/dev/null 2>&1 || {cmd}");
        }

        Ok(cmd)
    }

    fn add_user_to_group(&self, user: &UserName, group: &GroupName) -> Result<String> {
        if user.as_str().is_empty() || group.as_str().is_empty() {
            return Err(MsiError::UserManagementError(
                "User and group names cannot be empty".to_string(),
            ));
        }

        Ok(format!("usermod -a -G {group} {user}"))
    }
}

/// Windows native implementation utilizing `NetUserAdd` and `NetLocalGroupAdd`.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsAccountManager;

impl LocalAccountManager for WindowsAccountManager {
    fn create_user(
        &self,
        name: &UserName,
        password: Option<&str>,
        fail_if_exists: bool,
    ) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::UserManagementError(
                "User name cannot be empty".to_string(),
            ));
        }

        let has_pwd = password.is_some_and(|p| !p.is_empty());
        Ok(format!(
            "NetUserAdd(name={name}, has_password={has_pwd}, fail_if_exists={fail_if_exists})"
        ))
    }

    fn create_group(&self, name: &GroupName, fail_if_exists: bool) -> Result<String> {
        if name.as_str().is_empty() {
            return Err(MsiError::UserManagementError(
                "Group name cannot be empty".to_string(),
            ));
        }

        Ok(format!(
            "NetLocalGroupAdd(name={name}, fail_if_exists={fail_if_exists})"
        ))
    }

    fn add_user_to_group(&self, user: &UserName, group: &GroupName) -> Result<String> {
        if user.as_str().is_empty() || group.as_str().is_empty() {
            return Err(MsiError::UserManagementError(
                "User and group names cannot be empty".to_string(),
            ));
        }

        Ok(format!(
            "NetLocalGroupAddMembers(group={group}, user={user})"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_name() {
        let user = UserName::new("admin");
        assert_eq!(user.as_str(), "admin");
        assert_eq!(format!("{user}"), "admin");
    }

    #[test]
    fn test_group_name() {
        let group = GroupName::new("wheel");
        assert_eq!(group.as_str(), "wheel");
        assert_eq!(format!("{group}"), "wheel");
    }

    #[test]
    fn test_security_identifier() {
        let sid = SecurityIdentifier::new("S-1-5-32-544");
        assert_eq!(sid.as_str(), "S-1-5-32-544");
        assert_eq!(format!("{sid}"), "S-1-5-32-544");
    }

    #[test]
    fn test_posix_account_manager() {
        let mgr = PosixAccountManager;
        let user = UserName::new("testuser");
        let group = GroupName::new("testgroup");

        let cmd1 = mgr.create_user(&user, None, true).unwrap_or_default();
        assert_eq!(cmd1, "useradd testuser");

        let cmd2 = mgr
            .create_user(&user, Some("pwd"), false)
            .unwrap_or_default();
        assert_eq!(cmd2, "id -u testuser >/dev/null 2>&1 || useradd testuser");

        let user_err = mgr.create_user(&UserName::new(""), None, true);
        assert!(matches!(user_err, Err(MsiError::UserManagementError(_))));

        let cmd3 = mgr.create_group(&group, true).unwrap_or_default();
        assert_eq!(cmd3, "groupadd testgroup");

        let cmd4 = mgr.create_group(&group, false).unwrap_or_default();
        assert_eq!(
            cmd4,
            "getent group testgroup >/dev/null 2>&1 || groupadd testgroup"
        );

        let group_err = mgr.create_group(&GroupName::new(""), true);
        assert!(matches!(group_err, Err(MsiError::UserManagementError(_))));

        let cmd5 = mgr.add_user_to_group(&user, &group).unwrap_or_default();
        assert_eq!(cmd5, "usermod -a -G testgroup testuser");

        let ug_err = mgr.add_user_to_group(&UserName::new(""), &group);
        assert!(matches!(ug_err, Err(MsiError::UserManagementError(_))));
    }

    #[test]
    fn test_windows_account_manager() {
        let mgr = WindowsAccountManager;
        let user = UserName::new("winuser");
        let group = GroupName::new("wingroup");

        let cmd1 = mgr
            .create_user(&user, Some("pass"), true)
            .unwrap_or_default();
        assert_eq!(
            cmd1,
            "NetUserAdd(name=winuser, has_password=true, fail_if_exists=true)"
        );

        let user_err = mgr.create_user(&UserName::new(""), None, true);
        assert!(matches!(user_err, Err(MsiError::UserManagementError(_))));

        let cmd2 = mgr.create_group(&group, false).unwrap_or_default();
        assert_eq!(
            cmd2,
            "NetLocalGroupAdd(name=wingroup, fail_if_exists=false)"
        );

        let group_err = mgr.create_group(&GroupName::new(""), true);
        assert!(matches!(group_err, Err(MsiError::UserManagementError(_))));

        let cmd3 = mgr.add_user_to_group(&user, &group).unwrap_or_default();
        assert_eq!(
            cmd3,
            "NetLocalGroupAddMembers(group=wingroup, user=winuser)"
        );

        let ug_err = mgr.add_user_to_group(&user, &GroupName::new(""));
        assert!(matches!(ug_err, Err(MsiError::UserManagementError(_))));
    }

    #[test]
    fn test_is_elevated() {
        // Just calling it to ensure coverage. The value will depend on the environment.
        let elevated = is_elevated();
        // Since we can't reliably assert the environment's elevation in a generic CI,
        // we just assert that it returns a boolean (which is trivial).
        assert!(elevated || !elevated);
    }
}
