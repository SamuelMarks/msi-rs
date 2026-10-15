//! Standard Windows Installer directories and action sequence definitions.

/// Standard directory mapping definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardDirectory {
    /// Directory identifier.
    pub id: &'static str,
    /// Parent directory identifier (None if root `TARGETDIR`).
    pub parent_id: Option<&'static str>,
    /// Default directory specification.
    pub default_dir: &'static str,
}

/// Known standard directory identifiers in Windows Installer.
pub const STANDARD_DIRECTORIES: &[StandardDirectory] = &[
    StandardDirectory {
        id: "TARGETDIR",
        parent_id: None,
        default_dir: "SourceDir",
    },
    StandardDirectory {
        id: "ProgramFilesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "PFiles",
    },
    StandardDirectory {
        id: "ProgramFiles64Folder",
        parent_id: Some("TARGETDIR"),
        default_dir: "PFiles64",
    },
    StandardDirectory {
        id: "CommonFilesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Common",
    },
    StandardDirectory {
        id: "CommonFiles64Folder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Common64",
    },
    StandardDirectory {
        id: "SystemFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "System",
    },
    StandardDirectory {
        id: "System64Folder",
        parent_id: Some("TARGETDIR"),
        default_dir: "System64",
    },
    StandardDirectory {
        id: "AppDataFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "AppData",
    },
    StandardDirectory {
        id: "LocalAppDataFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "LocalApp",
    },
    StandardDirectory {
        id: "CommonAppDataFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "CommonAppData",
    },
    StandardDirectory {
        id: "DesktopFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Desktop",
    },
    StandardDirectory {
        id: "ProgramMenuFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "ProgMenu",
    },
    StandardDirectory {
        id: "WindowsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Windows",
    },
    StandardDirectory {
        id: "StartMenuFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "StartMenu",
    },
    StandardDirectory {
        id: "StartupFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Startup",
    },
    StandardDirectory {
        id: "SendToFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "SendTo",
    },
    StandardDirectory {
        id: "RecentFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Recent",
    },
    StandardDirectory {
        id: "FavoritesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Favorites",
    },
    StandardDirectory {
        id: "NetHoodFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "NetHood",
    },
    StandardDirectory {
        id: "PrintHoodFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "PrintHood",
    },
    StandardDirectory {
        id: "TemplatesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Templates",
    },
    StandardDirectory {
        id: "FontsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Fonts",
    },
    StandardDirectory {
        id: "AdminToolsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "AdminTools",
    },
    StandardDirectory {
        id: "TempFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Temp",
    },
    // Standard POSIX directories
    StandardDirectory {
        id: "/usr",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr",
    },
    StandardDirectory {
        id: "UsrFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr",
    },
    StandardDirectory {
        id: "/usr/local",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_local",
    },
    StandardDirectory {
        id: "UsrLocalFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_local",
    },
    StandardDirectory {
        id: "/usr/bin",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_bin",
    },
    StandardDirectory {
        id: "UsrBinFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_bin",
    },
    StandardDirectory {
        id: "/usr/share",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_share",
    },
    StandardDirectory {
        id: "UsrShareFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_share",
    },
    StandardDirectory {
        id: "/etc",
        parent_id: Some("TARGETDIR"),
        default_dir: "etc",
    },
    StandardDirectory {
        id: "EtcFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "etc",
    },
    StandardDirectory {
        id: "/var",
        parent_id: Some("TARGETDIR"),
        default_dir: "var",
    },
    StandardDirectory {
        id: "VarFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "var",
    },
    StandardDirectory {
        id: "/opt",
        parent_id: Some("TARGETDIR"),
        default_dir: "opt",
    },
    StandardDirectory {
        id: "OptFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "opt",
    },
    StandardDirectory {
        id: "/Applications",
        parent_id: Some("TARGETDIR"),
        default_dir: "Applications",
    },
    StandardDirectory {
        id: "ApplicationsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Applications",
    },
    StandardDirectory {
        id: "~/Library/Application Support",
        parent_id: Some("TARGETDIR"),
        default_dir: "UserAppSupport",
    },
    StandardDirectory {
        id: "UserApplicationSupportFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "UserAppSupport",
    },
];

/// Standard action ordering entry in execution sequence tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardActionOrder {
    /// Action name identifier.
    pub name: &'static str,
    /// Default sequence number.
    pub sequence: i16,
    /// Default condition expression (None for unconditional).
    pub condition: Option<&'static str>,
}

/// Standard MSI action sequence in `InstallExecuteSequence` table per MSI SDK.
pub const STANDARD_INSTALL_EXECUTE_ACTIONS: &[StandardActionOrder] = &[
    StandardActionOrder {
        name: "AppSearch",
        sequence: 400,
        condition: None,
    },
    StandardActionOrder {
        name: "FindRelatedProducts",
        sequence: 500,
        condition: None,
    },
    StandardActionOrder {
        name: "LaunchConditions",
        sequence: 600,
        condition: None,
    },
    StandardActionOrder {
        name: "ValidateProductID",
        sequence: 700,
        condition: None,
    },
    StandardActionOrder {
        name: "CostInitialize",
        sequence: 800,
        condition: None,
    },
    StandardActionOrder {
        name: "FileCost",
        sequence: 900,
        condition: None,
    },
    StandardActionOrder {
        name: "CostFinalize",
        sequence: 1000,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallValidate",
        sequence: 1400,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallInitialize",
        sequence: 1500,
        condition: None,
    },
    StandardActionOrder {
        name: "ProcessComponents",
        sequence: 1600,
        condition: None,
    },
    StandardActionOrder {
        name: "UnpublishComponents",
        sequence: 1700,
        condition: Some("Installed"),
    },
    StandardActionOrder {
        name: "UnpublishFeatures",
        sequence: 1800,
        condition: Some("Installed"),
    },
    StandardActionOrder {
        name: "StopServices",
        sequence: 1900,
        condition: Some("VersionNT"),
    },
    StandardActionOrder {
        name: "DeleteServices",
        sequence: 2000,
        condition: Some("VersionNT"),
    },
    StandardActionOrder {
        name: "RemoveFiles",
        sequence: 3500,
        condition: None,
    },
    StandardActionOrder {
        name: "RemoveFolders",
        sequence: 3600,
        condition: None,
    },
    StandardActionOrder {
        name: "CreateFolders",
        sequence: 3700,
        condition: None,
    },
    StandardActionOrder {
        name: "MoveFiles",
        sequence: 3800,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallFiles",
        sequence: 4000,
        condition: None,
    },
    StandardActionOrder {
        name: "CreateShortcuts",
        sequence: 4500,
        condition: None,
    },
    StandardActionOrder {
        name: "WriteRegistryValues",
        sequence: 5000,
        condition: None,
    },
    StandardActionOrder {
        name: "WriteIniValues",
        sequence: 5100,
        condition: None,
    },
    StandardActionOrder {
        name: "WriteEnvironmentStrings",
        sequence: 5200,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallServices",
        sequence: 5800,
        condition: Some("VersionNT"),
    },
    StandardActionOrder {
        name: "StartServices",
        sequence: 5900,
        condition: Some("VersionNT"),
    },
    StandardActionOrder {
        name: "RegisterUser",
        sequence: 6000,
        condition: None,
    },
    StandardActionOrder {
        name: "RegisterProduct",
        sequence: 6100,
        condition: None,
    },
    StandardActionOrder {
        name: "PublishComponents",
        sequence: 6200,
        condition: None,
    },
    StandardActionOrder {
        name: "PublishFeatures",
        sequence: 6300,
        condition: None,
    },
    StandardActionOrder {
        name: "PublishProduct",
        sequence: 6400,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallFinalize",
        sequence: 6600,
        condition: None,
    },
];

/// Standard MSI action sequence in `InstallUISequence` table per MSI SDK.
pub const STANDARD_INSTALL_UI_ACTIONS: &[StandardActionOrder] = &[
    StandardActionOrder {
        name: "AppSearch",
        sequence: 400,
        condition: None,
    },
    StandardActionOrder {
        name: "FindRelatedProducts",
        sequence: 500,
        condition: None,
    },
    StandardActionOrder {
        name: "LaunchConditions",
        sequence: 600,
        condition: None,
    },
    StandardActionOrder {
        name: "ValidateProductID",
        sequence: 700,
        condition: None,
    },
    StandardActionOrder {
        name: "CostInitialize",
        sequence: 800,
        condition: None,
    },
    StandardActionOrder {
        name: "FileCost",
        sequence: 900,
        condition: None,
    },
    StandardActionOrder {
        name: "CostFinalize",
        sequence: 1000,
        condition: None,
    },
    StandardActionOrder {
        name: "ExecuteAction",
        sequence: 1300,
        condition: None,
    },
];
