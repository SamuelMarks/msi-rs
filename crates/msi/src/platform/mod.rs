//! Cross-Platform POSIX Translation Engine (`msi-platform`).
//!
//! Grounded directly in POSIX.1-2017 / `SUSv4` standards, Freedesktop XDG specifications,
//! and Apple macOS guidelines:
//! - Standard filesystem hierarchies and path translation (`paths`).
//! - Octal permission modes, SDDL ACL translation, and extended attributes (`permissions`).
//! - Service supervisor integrations: systemd, launchd, rc.d, SMF (`daemon`).
//! - Desktop integration: Freedesktop `.desktop` files and macOS `.app` bundles (`desktop`).
//! - Hierarchical registry emulation and shell profile script generation (`registry_store`).

pub mod boot_harness;
pub mod bootloader;
pub mod daemon;
pub mod desktop;
pub mod disk;
pub mod driver;
pub mod firewall;
pub mod font;
pub mod format;
pub mod hive;
pub mod iis;
pub mod linux_config;
pub mod odbc;
pub mod partition;
pub mod paths;
pub mod permissions;
pub mod provider;
pub mod registry_store;
pub mod sysroot;
pub mod unattend;
pub mod users;

pub use boot_harness::{
    InitScriptBuilder, InitTargetMode, KernelConfig, KernelDriverKind, LiveMediaFormat,
    LiveMediaGenerator, UkiPackager, UserlandBundle, UserlandUtilityKind, WinPeHarness,
};
pub use bootloader::{
    BootloaderKind, EfiNvramManager, EspLayoutManager, LinuxBootloaderConfig, WindowsBcdStore,
};
pub use daemon::{
    HostSupervisorExecutor, InstalledService, ServiceControlAction, ServiceDefinition,
    SupervisorType,
};
pub use desktop::{MacOsAppBundle, XdgDesktopEntry};
pub use disk::{
    BlockDevice, BlockDevicePath, BlockDeviceScanner, BusType, DeviceKind, SmartHealthStatus,
};
pub use driver::{
    DriverInf, KernelModule, LinuxKernelModuleServicing, WindowsDriverStoreServicing,
};
pub use firewall::{
    FirewallManager, FirewallRuleName, PacketFilterManager, PortNumber, Protocol, UfwManager,
    WindowsFirewallManager,
};
pub use format::{
    Ext4Formatter, Fat32FormatOptions, Fat32Formatter, FileSystemKind, FileSystemVerifier,
    FormatCommandBuilder, NtfsFormatter,
};
pub use hive::{OfflineHiveStore, OfflineRegistryData, OfflineRegistryHive, RegistryValueType};
pub use iis::{
    CertificateHash, NginxGenerator, VirtualDirectoryPath, WebAppPoolName,
    WebServerConfigGenerator, WebSitePort, WindowsIisExecutor,
};
pub use linux_config::{
    FstabEntry, FstabGenerator, LinuxIdentityConfig, ProvisionUserAccount, UserProvisioningEngine,
};
pub use partition::{
    compute_crc32, GptPartitionEntry, GptTable, Lba, MbrPartitionEntry, MbrTable,
    PartitionTypeGuid, PartitionUuid, StandardPartitionScheme,
};
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
pub use sysroot::SysrootMountGuard;
pub use unattend::{LinuxCloudInitConfig, WindowsUnattendConfig};
pub use users::{
    GroupName, LocalAccountManager, PosixAccountManager, SecurityIdentifier, UserName,
    WindowsAccountManager,
};
