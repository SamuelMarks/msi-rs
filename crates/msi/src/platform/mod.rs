//! Cross-Platform POSIX Translation Engine (`msi-platform`).
//!
//! Grounded directly in POSIX.1-2017 / `SUSv4` standards, Freedesktop XDG specifications,
//! and Apple macOS guidelines:
//! - Standard filesystem hierarchies and path translation (`paths`).
//! - Octal permission modes, SDDL ACL translation, and extended attributes (`permissions`).
//! - Service supervisor integrations: systemd, launchd, rc.d, SMF (`daemon`).
//! - Desktop integration: Freedesktop `.desktop` files and macOS `.app` bundles (`desktop`).
//! - Hierarchical registry emulation and shell profile script generation (`registry_store`).

pub mod daemon;
pub mod desktop;
pub mod paths;
pub mod permissions;
pub mod registry_store;

pub use daemon::{
    HostSupervisorExecutor, InstalledService, ServiceControlAction, ServiceDefinition,
    SupervisorType,
};
pub use desktop::{MacOsAppBundle, XdgDesktopEntry};
pub use paths::{PathResolver, StandardDirectoryId, TargetOs};
pub use permissions::{
    translate_sddl, AclAccessType, AclEntry, AclPrincipalType, ExtendedAttribute,
    LiveSecurityApplier, PosixMode, MODE_DIRECTORY, MODE_EXECUTABLE, MODE_PRIVATE_FILE,
    MODE_STANDARD_FILE, S_ISGID, S_ISUID, S_ISVTX,
};
pub use registry_store::{
    NativeConfigBridge, RegistryRoot, RegistryStore, RegistryValue, SqliteRegistryDriver,
    SqliteTransactionLogEntry, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    HKEY_USERS, SQLITE_REGISTRY_INIT_SQL,
};
