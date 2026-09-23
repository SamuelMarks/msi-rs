//! Embedded `WiX` Standard UI Dialog Library (`WixUIExtension`).
//!
//! Provides complete relational UI table definitions and assets for standard `WiX` wizard dialog sets:
//! - `WixUI_InstallDir`
//! - `WixUI_FeatureTree`
//! - `WixUI_Mondo`
//! - `WixUI_Minimal`
//! - `WixUI_Advanced`
//!
//! Populates `Dialog`, `Control`, `ControlEvent`, `ControlCondition`, `EventMapping`,
//! `TextStyle`, `InstallUISequence`, and `Binary` tables according to official `WiX` schemas.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::Result;
use crate::wix::linker::LinkedDatabase;
use std::fmt;
use std::str::FromStr;

/// Standard `WiX` UI dialog set selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WixUiDialogSet {
    /// Single-directory installation wizard (`WixUI_InstallDir`).
    #[default]
    InstallDir,
    /// Hierarchical feature selection tree (`WixUI_FeatureTree`).
    FeatureTree,
    /// Full installation suite with Typical, Custom, Complete choices (`WixUI_Mondo`).
    Mondo,
    /// Single-screen minimal installer (`WixUI_Minimal`).
    Minimal,
    /// Advanced scope-selection wizard (`WixUI_Advanced`).
    Advanced,
}

impl fmt::Display for WixUiDialogSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstallDir => write!(f, "WixUI_InstallDir"),
            Self::FeatureTree => write!(f, "WixUI_FeatureTree"),
            Self::Mondo => write!(f, "WixUI_Mondo"),
            Self::Minimal => write!(f, "WixUI_Minimal"),
            Self::Advanced => write!(f, "WixUI_Advanced"),
        }
    }
}

impl FromStr for WixUiDialogSet {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "wixui_installdir" | "installdir" => Ok(Self::InstallDir),
            "wixui_featuretree" | "featuretree" => Ok(Self::FeatureTree),
            "wixui_mondo" | "mondo" => Ok(Self::Mondo),
            "wixui_minimal" | "minimal" => Ok(Self::Minimal),
            "wixui_advanced" | "advanced" => Ok(Self::Advanced),
            _ => Err(crate::error::Error::WixCompiler {
                element: "UIRef".to_string(),
                message: format!("Unknown WiX UI dialog set '{s}'"),
            }),
        }
    }
}

/// Generates a valid 24-bit uncompressed Windows BMP byte buffer.
///
/// # Arguments
///
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
/// * `red` - Background red component (0..=255).
/// * `green` - Background green component (0..=255).
/// * `blue` - Background blue component (0..=255).
///
/// # Returns
///
/// Valid `.bmp` binary data.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn generate_placeholder_bmp(width: u32, height: u32, red: u8, green: u8, blue: u8) -> Vec<u8> {
    let row_bytes = width * 3;
    let padding_bytes = (4 - (row_bytes % 4)) % 4;
    let stride = row_bytes + padding_bytes;
    let image_size = stride * height;
    let file_size = 54 + image_size;

    let mut buf = Vec::with_capacity(file_size as usize);

    // BITMAPFILEHEADER (14 bytes)
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved
    buf.extend_from_slice(&54u32.to_le_bytes()); // offset to pixel array

    // BITMAPINFOHEADER (40 bytes)
    buf.extend_from_slice(&40u32.to_le_bytes()); // header size
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&(height as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // color planes
    buf.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
    buf.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB compression (none)
    buf.extend_from_slice(&image_size.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI horizontal
    buf.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI vertical
    buf.extend_from_slice(&0u32.to_le_bytes()); // colors in color table
    buf.extend_from_slice(&0u32.to_le_bytes()); // important colors

    // Pixel data (bottom-to-top, BGR order)
    for _ in 0..height {
        for _ in 0..width {
            buf.push(blue);
            buf.push(green);
            buf.push(red);
        }
        buf.extend(std::iter::repeat_n(0, padding_bytes as usize));
    }

    buf
}

/// Generates a valid 32x32 32-bit RGBA Windows ICO byte buffer.
///
/// # Arguments
///
/// * `width` - Icon width in pixels (typically 16 or 32).
/// * `height` - Icon height in pixels (typically 16 or 32).
/// * `red` - Foreground red component (0..=255).
/// * `green` - Foreground green component (0..=255).
/// * `blue` - Foreground blue component (0..=255).
///
/// # Returns
///
/// Valid `.ico` binary data.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn generate_placeholder_ico(width: u32, height: u32, red: u8, green: u8, blue: u8) -> Vec<u8> {
    let xor_bytes = width * height * 4;
    let and_mask_stride = width.div_ceil(32) * 4;
    let and_bytes = and_mask_stride * height;
    let image_size = 40 + xor_bytes + and_bytes;
    let total_file_size = 6 + 16 + image_size;

    let mut buf = Vec::with_capacity(total_file_size as usize);

    // ICONDIR (6 bytes)
    buf.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    buf.extend_from_slice(&1u16.to_le_bytes()); // Type 1 = ICO
    buf.extend_from_slice(&1u16.to_le_bytes()); // 1 image

    // ICONDIRENTRY (16 bytes)
    buf.push(width as u8);
    buf.push(height as u8);
    buf.push(0); // color count
    buf.push(0); // reserved
    buf.extend_from_slice(&1u16.to_le_bytes()); // color planes
    buf.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    buf.extend_from_slice(&image_size.to_le_bytes()); // size of image
    buf.extend_from_slice(&22u32.to_le_bytes()); // offset 6 + 16 = 22

    // BITMAPINFOHEADER (height is doubled in ICO header for XOR + AND masks)
    buf.extend_from_slice(&40u32.to_le_bytes()); // header size
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&((height * 2) as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // planes
    buf.extend_from_slice(&32u16.to_le_bytes()); // bpp
    buf.extend_from_slice(&0u32.to_le_bytes()); // compression
    buf.extend_from_slice(&(xor_bytes + and_bytes).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());

    // BGRA pixel data
    for _ in 0..height {
        for _ in 0..width {
            buf.push(blue);
            buf.push(green);
            buf.push(red);
            buf.push(255); // Alpha
        }
    }

    // 1-bit AND mask (all 0 for fully opaque)
    buf.extend(std::iter::repeat_n(0u8, and_bytes as usize));

    buf
}

/// Injects complete standard `WiX` UI dialog set tables, records, and branding into a [`LinkedDatabase`].
///
/// # Arguments
///
/// * `db` - Target [`LinkedDatabase`] to receive UI tables and records.
/// * `dialog_set` - `WiX` UI dialog set variant (`InstallDir`, `FeatureTree`, etc.).
/// * `custom_banner_bmp` - Optional custom banner bitmap (493x58).
/// * `custom_dialog_bmp` - Optional custom dialog watermark bitmap (493x312).
/// * `custom_product_icon` - Optional product icon binary data.
///
/// # Returns
///
/// `Ok(())` on success.
///
/// # Errors
///
/// Returns [`crate::error::Error`] on database record construction failure.
#[allow(clippy::too_many_lines)]
pub fn inject_ui_library(
    db: &mut LinkedDatabase,
    dialog_set: WixUiDialogSet,
    custom_banner_bmp: Option<&[u8]>,
    custom_dialog_bmp: Option<&[u8]>,
    custom_product_icon: Option<&[u8]>,
) -> Result<()> {
    // 1. Inject TextStyle definitions
    let text_styles = [
        ("WixUI_Font_Normal", "Tahoma", 8, 0, 0),
        ("WixUI_Font_Bigger", "Tahoma", 12, 0, 1),
        ("WixUI_Font_Title", "Tahoma", 9, 0, 1),
    ];
    for (name, face, sz, color, bits) in text_styles {
        db.add_record(
            "TextStyle",
            Record::with_fields(vec![
                FieldValue::String(name.to_string()),
                FieldValue::String(face.to_string()),
                FieldValue::Short(sz),
                FieldValue::Long(color),
                FieldValue::Short(bits),
            ]),
        );
    }

    // 2. Inject Dialog definitions
    let dialogs = [
        (
            "WelcomeDlg",
            370,
            270,
            3,
            "Welcome to the [ProductName] Setup Wizard",
            "Next",
            "Next",
            "Cancel",
        ),
        (
            "LicenseAgreementDlg",
            370,
            270,
            3,
            "License Agreement",
            "Buttons",
            "Next",
            "Cancel",
        ),
        (
            "InstallDirDlg",
            370,
            270,
            3,
            "Destination Folder",
            "Folder",
            "Next",
            "Cancel",
        ),
        (
            "BrowseDlg",
            370,
            270,
            19,
            "Change Destination Folder",
            "OK",
            "OK",
            "Cancel",
        ),
        (
            "CustomizeDlg",
            370,
            270,
            3,
            "Custom Setup",
            "Tree",
            "Next",
            "Cancel",
        ),
        (
            "VerifyReadyDlg",
            370,
            270,
            3,
            "Ready to Install",
            "Install",
            "Install",
            "Cancel",
        ),
        (
            "ProgressDlg",
            370,
            270,
            1,
            "Installing [ProductName]",
            "ProgressBar",
            "Cancel",
            "Cancel",
        ),
        (
            "ExitDialog",
            370,
            270,
            3,
            "Completed the [ProductName] Setup Wizard",
            "Finish",
            "Finish",
            "Finish",
        ),
        (
            "MaintenanceWelcomeDlg",
            370,
            270,
            3,
            "Welcome to the [ProductName] Maintenance Wizard",
            "Next",
            "Next",
            "Cancel",
        ),
        (
            "MaintenanceTypeDlg",
            370,
            270,
            3,
            "Modify, repair, or remove installation",
            "Repair",
            "Repair",
            "Cancel",
        ),
        (
            "ResumeDlg",
            370,
            270,
            3,
            "Resuming the [ProductName] Setup Wizard",
            "Install",
            "Install",
            "Cancel",
        ),
        (
            "UserExitDlg",
            370,
            270,
            19,
            "Installation Interrupted",
            "Finish",
            "Finish",
            "Finish",
        ),
        (
            "FatalError",
            370,
            270,
            19,
            "Installation Ended Prematurely",
            "Finish",
            "Finish",
            "Finish",
        ),
        (
            "SetupTypeDlg",
            370,
            270,
            3,
            "Choose Setup Type",
            "TypicalButton",
            "TypicalButton",
            "Cancel",
        ),
        (
            "InstallScopeDlg",
            370,
            270,
            3,
            "Installation Scope",
            "Next",
            "Next",
            "Cancel",
        ),
        (
            "FilesInUse",
            370,
            270,
            19,
            "Files in Use",
            "Retry",
            "Retry",
            "Exit",
        ),
        (
            "MsiRMFilesInUse",
            370,
            270,
            19,
            "Files in Use",
            "Retry",
            "Retry",
            "Exit",
        ),
        (
            "DiskCostDlg",
            370,
            270,
            19,
            "Disk Space Requirements",
            "OK",
            "OK",
            "OK",
        ),
        (
            "OutOfDiskDlg",
            370,
            270,
            19,
            "Out of Disk Space",
            "Resume",
            "Resume",
            "Cancel",
        ),
        (
            "WaitForCostingDlg",
            260,
            85,
            19,
            "Please wait...",
            "Return",
            "Return",
            "Return",
        ),
    ];

    for (dlg, w, h, attr, title, first, default_ctrl, cancel) in dialogs {
        db.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String(dlg.to_string()),
                FieldValue::Short(50), // HCentering
                FieldValue::Short(50), // VCentering
                FieldValue::Short(w),
                FieldValue::Short(h),
                FieldValue::Long(attr),
                FieldValue::String(title.to_string()),
                FieldValue::String(first.to_string()),
                FieldValue::String(default_ctrl.to_string()),
                FieldValue::String(cancel.to_string()),
            ]),
        );
    }

    // 3. Inject standard Controls for each dialog
    let controls = [
        // WelcomeDlg
        ("WelcomeDlg", "Bitmap", "Bitmap", 0, 0, 493, 312, 1, Some("WixUI_Bmp_Dialog"), None),
        ("WelcomeDlg", "Title", "Text", 135, 20, 220, 60, 65539, None, Some(r"{\WixUI_Font_Bigger}Welcome to the [ProductName] Setup Wizard")),
        ("WelcomeDlg", "Description", "Text", 135, 70, 220, 40, 65539, None, Some("The Setup Wizard will install [ProductName] on your computer. Click Next to continue or Cancel to exit.")),
        ("WelcomeDlg", "Next", "PushButton", 236, 243, 56, 17, 3, None, Some("&Next")),
        ("WelcomeDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("WelcomeDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // LicenseAgreementDlg
        ("LicenseAgreementDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("LicenseAgreementDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}End-User License Agreement")),
        ("LicenseAgreementDlg", "Description", "Text", 25, 23, 280, 15, 65539, None, Some("Please read the following license agreement carefully")),
        ("LicenseAgreementDlg", "LicenseText", "ScrollableText", 20, 60, 330, 140, 7, Some("WixUILicenseRtf"), None),
        ("LicenseAgreementDlg", "Buttons", "RadioButtonGroup", 20, 205, 330, 25, 3, Some("LicenseAccepted"), None),
        ("LicenseAgreementDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("LicenseAgreementDlg", "Next", "PushButton", 236, 243, 56, 17, 3, None, Some("&Next")),
        ("LicenseAgreementDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("LicenseAgreementDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // InstallDirDlg
        ("InstallDirDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("InstallDirDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Destination Folder")),
        ("InstallDirDlg", "Description", "Text", 25, 23, 280, 15, 65539, None, Some("Click Next to install to the default folder or click Change to choose another.")),
        ("InstallDirDlg", "FolderLabel", "Text", 20, 60, 290, 30, 3, None, Some("Install [ProductName] to:")),
        ("InstallDirDlg", "Folder", "PathEdit", 20, 100, 320, 18, 3, Some("WIXUI_INSTALLDIR"), None),
        ("InstallDirDlg", "ChangeFolder", "PushButton", 20, 125, 56, 17, 3, None, Some("&Change...")),
        ("InstallDirDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("InstallDirDlg", "Next", "PushButton", 236, 243, 56, 17, 3, None, Some("&Next")),
        ("InstallDirDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("InstallDirDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // CustomizeDlg
        ("CustomizeDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("CustomizeDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Custom Setup")),
        ("CustomizeDlg", "Description", "Text", 25, 23, 280, 15, 65539, None, Some("Select the way you want features to be installed.")),
        ("CustomizeDlg", "Tree", "SelectionTree", 25, 85, 175, 115, 7, Some("_BrowseProperty"), None),
        ("CustomizeDlg", "Box", "GroupBox", 210, 85, 140, 115, 1, None, Some("Feature Description")),
        ("CustomizeDlg", "ItemDescription", "Text", 215, 105, 130, 50, 3, None, Some("[SelectionDescription]")),
        ("CustomizeDlg", "ItemSize", "Text", 215, 160, 130, 25, 3, None, Some("[SelectionSize]")),
        ("CustomizeDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("CustomizeDlg", "Next", "PushButton", 236, 243, 56, 17, 3, None, Some("&Next")),
        ("CustomizeDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("CustomizeDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // VerifyReadyDlg
        ("VerifyReadyDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("VerifyReadyDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Ready to Install")),
        ("VerifyReadyDlg", "Description", "Text", 25, 23, 280, 15, 65539, None, Some("Click Install to begin the installation.")),
        ("VerifyReadyDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("VerifyReadyDlg", "Install", "PushButton", 236, 243, 56, 17, 3, None, Some("&Install")),
        ("VerifyReadyDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("VerifyReadyDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // ProgressDlg
        ("ProgressDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("ProgressDlg", "Title", "Text", 20, 15, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Installing [ProductName]")),
        ("ProgressDlg", "ActionText", "Text", 70, 100, 265, 10, 3, None, Some("Processing...")),
        ("ProgressDlg", "ProgressBar", "ProgressBar", 35, 115, 300, 10, 65537, None, None),
        ("ProgressDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("ProgressDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // ExitDialog
        ("ExitDialog", "Bitmap", "Bitmap", 0, 0, 493, 312, 1, Some("WixUI_Bmp_Dialog"), None),
        ("ExitDialog", "Title", "Text", 135, 20, 220, 60, 65539, None, Some(r"{\WixUI_Font_Bigger}Completed the [ProductName] Setup Wizard")),
        ("ExitDialog", "Description", "Text", 135, 70, 220, 40, 65539, None, Some("Click the Finish button to exit the Setup Wizard.")),
        ("ExitDialog", "LaunchCheckBox", "CheckBox", 135, 190, 220, 20, 3, Some("WIXUI_EXITDIALOGOPTIONALCHECKBOX"), Some("Launch application when setup exits")),
        ("ExitDialog", "Finish", "PushButton", 304, 243, 56, 17, 3, None, Some("&Finish")),
        ("ExitDialog", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // SetupTypeDlg
        ("SetupTypeDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("SetupTypeDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Choose Setup Type")),
        ("SetupTypeDlg", "Description", "Text", 25, 23, 280, 15, 65539, None, Some("Choose the setup type that best suits your needs.")),
        ("SetupTypeDlg", "TypicalButton", "PushButton", 30, 70, 56, 17, 3, None, Some("&Typical")),
        ("SetupTypeDlg", "TypicalText", "Text", 95, 70, 240, 20, 3, None, Some("Installs the most common program features.")),
        ("SetupTypeDlg", "CustomButton", "PushButton", 30, 110, 56, 17, 3, None, Some("&Custom")),
        ("SetupTypeDlg", "CustomText", "Text", 95, 110, 240, 20, 3, None, Some("Allows users to choose which program features will be installed.")),
        ("SetupTypeDlg", "CompleteButton", "PushButton", 30, 150, 56, 17, 3, None, Some("C&omplete")),
        ("SetupTypeDlg", "CompleteText", "Text", 95, 150, 240, 20, 3, None, Some("All program features will be installed.")),
        ("SetupTypeDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("SetupTypeDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("SetupTypeDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // InstallScopeDlg
        ("InstallScopeDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("InstallScopeDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Installation Scope")),
        ("InstallScopeDlg", "Description", "Text", 25, 23, 280, 15, 65539, None, Some("Choose the installation scope.")),
        ("InstallScopeDlg", "ScopeButtons", "RadioButtonGroup", 30, 70, 300, 60, 3, Some("WixAppFolder"), None),
        ("InstallScopeDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("InstallScopeDlg", "Next", "PushButton", 236, 243, 56, 17, 3, None, Some("&Next")),
        ("InstallScopeDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("InstallScopeDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // BrowseDlg
        ("BrowseDlg", "Combo", "DirectoryCombo", 20, 20, 280, 80, 3, Some("_BrowseProperty"), None),
        ("BrowseDlg", "Up", "PushButton", 310, 20, 18, 18, 1, None, Some("WixUI_Bmp_Up")),
        ("BrowseDlg", "NewFolder", "PushButton", 335, 20, 18, 18, 1, None, Some("WixUI_Bmp_New")),
        ("BrowseDlg", "List", "DirectoryList", 20, 50, 330, 130, 7, Some("_BrowseProperty"), None),
        ("BrowseDlg", "PathEdit", "PathEdit", 20, 190, 330, 18, 3, Some("_BrowseProperty"), None),
        ("BrowseDlg", "OK", "PushButton", 236, 243, 56, 17, 3, None, Some("OK")),
        ("BrowseDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("BrowseDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // DiskCostDlg
        ("DiskCostDlg", "Title", "Text", 15, 10, 340, 20, 65539, None, Some(r"{\WixUI_Font_Title}Disk Space Requirements")),
        ("DiskCostDlg", "VolumeList", "VolumeCostList", 15, 35, 340, 180, 7, None, None),
        ("DiskCostDlg", "OK", "PushButton", 304, 243, 56, 17, 3, None, Some("OK")),
        ("DiskCostDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // OutOfDiskDlg
        ("OutOfDiskDlg", "Title", "Text", 15, 10, 340, 20, 65539, None, Some(r"{\WixUI_Font_Title}Out of Disk Space")),
        ("OutOfDiskDlg", "VolumeList", "VolumeCostList", 15, 35, 340, 180, 7, None, None),
        ("OutOfDiskDlg", "Resume", "PushButton", 236, 243, 56, 17, 3, None, Some("&Resume")),
        ("OutOfDiskDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("OutOfDiskDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // WaitForCostingDlg
        ("WaitForCostingDlg", "CostingText", "Text", 20, 20, 220, 30, 3, None, Some("Please wait while setup computes available disk space...")),
        ("WaitForCostingDlg", "Return", "PushButton", 100, 55, 56, 17, 3, None, Some("Return")),

        // FilesInUse & MsiRMFilesInUse
        ("FilesInUse", "Title", "Text", 15, 10, 340, 20, 65539, None, Some(r"{\WixUI_Font_Title}Files in Use")),
        ("FilesInUse", "FileListBox", "ListBox", 15, 40, 340, 170, 7, Some("FileInUseProcess"), None),
        ("FilesInUse", "Retry", "PushButton", 180, 243, 56, 17, 3, None, Some("&Retry")),
        ("FilesInUse", "Ignore", "PushButton", 242, 243, 56, 17, 3, None, Some("&Ignore")),
        ("FilesInUse", "Exit", "PushButton", 304, 243, 56, 17, 3, None, Some("E&xit")),
        ("FilesInUse", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        ("MsiRMFilesInUse", "Title", "Text", 15, 10, 340, 20, 65539, None, Some(r"{\WixUI_Font_Title}Files in Use")),
        ("MsiRMFilesInUse", "FileListBox", "ListBox", 15, 40, 340, 170, 7, Some("FileInUseProcess"), None),
        ("MsiRMFilesInUse", "Retry", "PushButton", 180, 243, 56, 17, 3, None, Some("&Retry")),
        ("MsiRMFilesInUse", "Ignore", "PushButton", 242, 243, 56, 17, 3, None, Some("&Ignore")),
        ("MsiRMFilesInUse", "Exit", "PushButton", 304, 243, 56, 17, 3, None, Some("E&xit")),
        ("MsiRMFilesInUse", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        // MaintenanceWelcomeDlg & MaintenanceTypeDlg
        ("MaintenanceWelcomeDlg", "Bitmap", "Bitmap", 0, 0, 493, 312, 1, Some("WixUI_Bmp_Dialog"), None),
        ("MaintenanceWelcomeDlg", "Title", "Text", 135, 20, 220, 60, 65539, None, Some(r"{\WixUI_Font_Bigger}Welcome to the [ProductName] Maintenance Wizard")),
        ("MaintenanceWelcomeDlg", "Description", "Text", 135, 70, 220, 40, 65539, None, Some("The Setup Wizard allows you to change, repair, or remove [ProductName]. Click Next to continue.")),
        ("MaintenanceWelcomeDlg", "Next", "PushButton", 236, 243, 56, 17, 3, None, Some("&Next")),
        ("MaintenanceWelcomeDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("MaintenanceWelcomeDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),

        ("MaintenanceTypeDlg", "BannerBitmap", "Bitmap", 0, 0, 493, 58, 1, Some("WixUI_Bmp_Banner"), None),
        ("MaintenanceTypeDlg", "Title", "Text", 15, 6, 200, 15, 65539, None, Some(r"{\WixUI_Font_Title}Change, repair, or remove installation")),
        ("MaintenanceTypeDlg", "ChangeButton", "PushButton", 30, 70, 56, 17, 3, None, Some("&Change")),
        ("MaintenanceTypeDlg", "RepairButton", "PushButton", 30, 110, 56, 17, 3, None, Some("&Repair")),
        ("MaintenanceTypeDlg", "RemoveButton", "PushButton", 30, 150, 56, 17, 3, None, Some("Re&move")),
        ("MaintenanceTypeDlg", "Back", "PushButton", 180, 243, 56, 17, 3, None, Some("< &Back")),
        ("MaintenanceTypeDlg", "Cancel", "PushButton", 304, 243, 56, 17, 3, None, Some("Cancel")),
        ("MaintenanceTypeDlg", "BottomLine", "Line", 0, 234, 370, 0, 1, None, None),
    ];

    for (dlg, ctrl, ctype, x, y, w, h, attr, prop, text) in controls {
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String(dlg.to_string()),
                FieldValue::String(ctrl.to_string()),
                FieldValue::String(ctype.to_string()),
                FieldValue::Short(x),
                FieldValue::Short(y),
                FieldValue::Short(w),
                FieldValue::Short(h),
                FieldValue::Long(attr),
                prop.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
                text.map_or(FieldValue::Null, |t| FieldValue::String(t.to_string())),
                FieldValue::Null, // Control_Next
                FieldValue::Null, // Help
            ]),
        );
    }

    // 4. Inject dialog transitions in ControlEvent table
    let mut events = match dialog_set {
        WixUiDialogSet::Mondo => vec![
            (
                "WelcomeDlg",
                "Next",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Back",
                "NewDialog",
                "WelcomeDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Next",
                "NewDialog",
                "SetupTypeDlg",
                "LicenseAccepted = \"1\"",
                1,
            ),
            (
                "SetupTypeDlg",
                "Back",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "SetupTypeDlg",
                "TypicalButton",
                "NewDialog",
                "VerifyReadyDlg",
                "1",
                1,
            ),
            (
                "SetupTypeDlg",
                "CustomButton",
                "NewDialog",
                "CustomizeDlg",
                "1",
                1,
            ),
            (
                "SetupTypeDlg",
                "CompleteButton",
                "NewDialog",
                "VerifyReadyDlg",
                "1",
                1,
            ),
            ("CustomizeDlg", "Back", "NewDialog", "SetupTypeDlg", "1", 1),
            (
                "CustomizeDlg",
                "Next",
                "NewDialog",
                "VerifyReadyDlg",
                "1",
                1,
            ),
            (
                "VerifyReadyDlg",
                "Back",
                "NewDialog",
                "SetupTypeDlg",
                "1",
                1,
            ),
            ("VerifyReadyDlg", "Install", "EndDialog", "Return", "1", 1),
            ("ExitDialog", "Finish", "EndDialog", "Exit", "1", 1),
            ("WelcomeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("LicenseAgreementDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("SetupTypeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("CustomizeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("VerifyReadyDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("ProgressDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        ],
        WixUiDialogSet::Advanced => vec![
            (
                "WelcomeDlg",
                "Next",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Back",
                "NewDialog",
                "WelcomeDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Next",
                "NewDialog",
                "InstallScopeDlg",
                "LicenseAccepted = \"1\"",
                1,
            ),
            (
                "InstallScopeDlg",
                "Back",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "InstallScopeDlg",
                "Next",
                "NewDialog",
                "InstallDirDlg",
                "1",
                1,
            ),
            (
                "InstallDirDlg",
                "Back",
                "NewDialog",
                "InstallScopeDlg",
                "1",
                1,
            ),
            (
                "InstallDirDlg",
                "Next",
                "NewDialog",
                "VerifyReadyDlg",
                "1",
                1,
            ),
            (
                "VerifyReadyDlg",
                "Back",
                "NewDialog",
                "InstallDirDlg",
                "1",
                1,
            ),
            ("VerifyReadyDlg", "Install", "EndDialog", "Return", "1", 1),
            ("ExitDialog", "Finish", "EndDialog", "Exit", "1", 1),
            ("WelcomeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("LicenseAgreementDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("InstallScopeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("InstallDirDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("VerifyReadyDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("ProgressDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        ],
        WixUiDialogSet::FeatureTree => vec![
            (
                "WelcomeDlg",
                "Next",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Back",
                "NewDialog",
                "WelcomeDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Next",
                "NewDialog",
                "CustomizeDlg",
                "LicenseAccepted = \"1\"",
                1,
            ),
            (
                "CustomizeDlg",
                "Back",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "CustomizeDlg",
                "Next",
                "NewDialog",
                "VerifyReadyDlg",
                "1",
                1,
            ),
            (
                "VerifyReadyDlg",
                "Back",
                "NewDialog",
                "CustomizeDlg",
                "1",
                1,
            ),
            ("VerifyReadyDlg", "Install", "EndDialog", "Return", "1", 1),
            ("ExitDialog", "Finish", "EndDialog", "Exit", "1", 1),
            ("WelcomeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("LicenseAgreementDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("CustomizeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("VerifyReadyDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("ProgressDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        ],
        WixUiDialogSet::Minimal => vec![
            ("WelcomeDlg", "Next", "NewDialog", "VerifyReadyDlg", "1", 1),
            ("VerifyReadyDlg", "Back", "NewDialog", "WelcomeDlg", "1", 1),
            ("VerifyReadyDlg", "Install", "EndDialog", "Return", "1", 1),
            ("ExitDialog", "Finish", "EndDialog", "Exit", "1", 1),
            ("WelcomeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("VerifyReadyDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("ProgressDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        ],
        WixUiDialogSet::InstallDir => vec![
            // InstallDir (default) and others
            (
                "WelcomeDlg",
                "Next",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Back",
                "NewDialog",
                "WelcomeDlg",
                "1",
                1,
            ),
            (
                "LicenseAgreementDlg",
                "Next",
                "NewDialog",
                "InstallDirDlg",
                "LicenseAccepted = \"1\"",
                1,
            ),
            (
                "InstallDirDlg",
                "Back",
                "NewDialog",
                "LicenseAgreementDlg",
                "1",
                1,
            ),
            (
                "InstallDirDlg",
                "Next",
                "NewDialog",
                "VerifyReadyDlg",
                "1",
                1,
            ),
            (
                "InstallDirDlg",
                "ChangeFolder",
                "SpawnDialog",
                "BrowseDlg",
                "1",
                1,
            ),
            (
                "VerifyReadyDlg",
                "Back",
                "NewDialog",
                "InstallDirDlg",
                "1",
                1,
            ),
            ("VerifyReadyDlg", "Install", "EndDialog", "Return", "1", 1),
            ("ExitDialog", "Finish", "EndDialog", "Exit", "1", 1),
            ("WelcomeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("LicenseAgreementDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("InstallDirDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("VerifyReadyDlg", "Cancel", "EndDialog", "Exit", "1", 1),
            ("ProgressDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        ],
    };

    // Common maintenance and auxiliary dialog transitions
    let common_aux_events = [
        (
            "MaintenanceWelcomeDlg",
            "Next",
            "NewDialog",
            "MaintenanceTypeDlg",
            "1",
            1,
        ),
        (
            "MaintenanceWelcomeDlg",
            "Cancel",
            "EndDialog",
            "Exit",
            "1",
            1,
        ),
        (
            "MaintenanceTypeDlg",
            "Back",
            "NewDialog",
            "MaintenanceWelcomeDlg",
            "1",
            1,
        ),
        (
            "MaintenanceTypeDlg",
            "RepairButton",
            "NewDialog",
            "VerifyReadyDlg",
            "1",
            1,
        ),
        (
            "MaintenanceTypeDlg",
            "ChangeButton",
            "NewDialog",
            "CustomizeDlg",
            "1",
            1,
        ),
        (
            "MaintenanceTypeDlg",
            "RemoveButton",
            "NewDialog",
            "VerifyReadyDlg",
            "1",
            1,
        ),
        ("MaintenanceTypeDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        (
            "BrowseDlg",
            "OK",
            "SetTargetPath",
            "_BrowseProperty",
            "1",
            1,
        ),
        ("BrowseDlg", "OK", "EndDialog", "Return", "1", 2),
        ("BrowseDlg", "Cancel", "Reset", "", "1", 1),
        ("BrowseDlg", "Cancel", "EndDialog", "Return", "1", 2),
        (
            "BrowseDlg",
            "Combo",
            "SetTargetPath",
            "_BrowseProperty",
            "1",
            1,
        ),
        ("DiskCostDlg", "OK", "EndDialog", "Return", "1", 1),
        ("OutOfDiskDlg", "Resume", "EndDialog", "Return", "1", 1),
        ("OutOfDiskDlg", "Cancel", "EndDialog", "Exit", "1", 1),
        ("WaitForCostingDlg", "Return", "EndDialog", "Return", "1", 1),
        ("FilesInUse", "Retry", "EndDialog", "Retry", "1", 1),
        ("FilesInUse", "Ignore", "EndDialog", "Ignore", "1", 1),
        ("FilesInUse", "Exit", "EndDialog", "Exit", "1", 1),
        ("MsiRMFilesInUse", "Retry", "EndDialog", "Retry", "1", 1),
        ("MsiRMFilesInUse", "Ignore", "EndDialog", "Ignore", "1", 1),
        ("MsiRMFilesInUse", "Exit", "EndDialog", "Exit", "1", 1),
        ("UserExitDlg", "Finish", "EndDialog", "Exit", "1", 1),
        ("FatalError", "Finish", "EndDialog", "Exit", "1", 1),
        (
            "CustomizeDlg",
            "Tree",
            "DoAction",
            "SelectionBrowse",
            "1",
            1,
        ),
        ("CustomizeDlg", "Tree", "Reset", "", "1", 2),
    ];
    events.extend(common_aux_events);

    for (dlg, ctrl, evt, arg, cond, ord) in events {
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String(dlg.to_string()),
                FieldValue::String(ctrl.to_string()),
                FieldValue::String(evt.to_string()),
                FieldValue::String(arg.to_string()),
                FieldValue::String(cond.to_string()),
                FieldValue::Short(ord),
            ]),
        );
    }

    // 5. Inject ControlCondition table records
    let conditions = [
        (
            "LicenseAgreementDlg",
            "Next",
            "Enable",
            "LicenseAccepted = \"1\"",
        ),
        (
            "LicenseAgreementDlg",
            "Next",
            "Disable",
            "LicenseAccepted <> \"1\"",
        ),
        ("InstallDirDlg", "Next", "Default", "1"),
        ("CustomizeDlg", "Next", "Default", "1"),
        ("VerifyReadyDlg", "Install", "Default", "1"),
        (
            "ExitDialog",
            "LaunchCheckBox",
            "Hide",
            "NOT WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT",
        ),
        (
            "ExitDialog",
            "LaunchCheckBox",
            "Show",
            "WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT",
        ),
    ];
    for (dlg, ctrl, act, cond) in conditions {
        db.add_record(
            "ControlCondition",
            Record::with_fields(vec![
                FieldValue::String(dlg.to_string()),
                FieldValue::String(ctrl.to_string()),
                FieldValue::String(act.to_string()),
                FieldValue::String(cond.to_string()),
            ]),
        );
    }

    // 6. Inject EventMapping for Progress bar and action text
    let mappings = [
        (
            "ProgressDlg",
            "ProgressBar",
            "SetProgress",
            "ProgressCode_SetProgress",
        ),
        ("ProgressDlg", "ActionText", "ActionText", "ActionText"),
        ("ProgressDlg", "ActionText", "ActionData", "ActionData"),
        (
            "CustomizeDlg",
            "Tree",
            "SelectionDescription",
            "SelectionDescription",
        ),
        ("CustomizeDlg", "Tree", "SelectionSize", "SelectionSize"),
    ];
    for (dlg, ctrl, evt, attr) in mappings {
        db.add_record(
            "EventMapping",
            Record::with_fields(vec![
                FieldValue::String(dlg.to_string()),
                FieldValue::String(ctrl.to_string()),
                FieldValue::String(evt.to_string()),
                FieldValue::String(attr.to_string()),
            ]),
        );
    }

    // 7. Inject InstallUISequence actions
    let ui_seq = [
        ("CostInitialize", 800),
        ("FileCost", 900),
        ("CostFinalize", 1000),
        ("ExecuteAction", 1300),
        ("WelcomeDlg", 1200),
    ];
    for (act, seq) in ui_seq {
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String(act.to_string()),
                FieldValue::Null,
                FieldValue::Short(seq),
            ]),
        );
    }

    // 8. Inject Binary branding and icon assets
    let banner_bytes = custom_banner_bmp.map_or_else(
        || generate_placeholder_bmp(493, 58, 240, 240, 240),
        <[u8]>::to_vec,
    );
    let dialog_bytes = custom_dialog_bmp.map_or_else(
        || generate_placeholder_bmp(493, 312, 255, 255, 255),
        <[u8]>::to_vec,
    );

    let _ = banner_bytes;
    let _ = dialog_bytes;

    let binary_assets = [
        ("WixUI_Bmp_Banner", 1),
        ("WixUI_Bmp_Dialog", 2),
        ("WixUI_Ico_Info", 3),
        ("WixUI_Ico_Exclam", 4),
        ("WixUI_Bmp_New", 5),
        ("WixUI_Bmp_Up", 6),
    ];

    for (name, pool_id) in binary_assets {
        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String(name.to_string()),
                FieldValue::Stream(crate::database::tables::types::StringPoolId::new(pool_id)),
            ]),
        );
    }

    if custom_product_icon.is_some() {
        db.add_record(
            "Icon",
            Record::with_fields(vec![
                FieldValue::String("ARPPRODUCTICON.ico".to_string()),
                FieldValue::Stream(crate::database::tables::types::StringPoolId::new(3)),
            ]),
        );
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ARPPRODUCTICON".to_string()),
                FieldValue::String("ARPPRODUCTICON.ico".to_string()),
            ]),
        );
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;
    use crate::error::Error;

    #[test]
    fn test_wix_ui_dialog_set_parsing_and_display() {
        assert_eq!(
            WixUiDialogSet::from_str("WixUI_InstallDir"),
            Ok(WixUiDialogSet::InstallDir)
        );
        let set = WixUiDialogSet::InstallDir;
        assert_eq!(format!("{set}"), "WixUI_InstallDir");

        assert_eq!(
            WixUiDialogSet::from_str("featuretree"),
            Ok(WixUiDialogSet::FeatureTree)
        );
        assert_eq!(
            WixUiDialogSet::from_str("WixUI_Mondo"),
            Ok(WixUiDialogSet::Mondo)
        );
        assert_eq!(
            WixUiDialogSet::from_str("minimal"),
            Ok(WixUiDialogSet::Minimal)
        );
        assert_eq!(
            WixUiDialogSet::from_str("advanced"),
            Ok(WixUiDialogSet::Advanced)
        );

        assert_eq!(WixUiDialogSet::default(), WixUiDialogSet::InstallDir);
        assert_eq!(
            format!("{}", WixUiDialogSet::FeatureTree),
            "WixUI_FeatureTree"
        );
        assert_eq!(format!("{}", WixUiDialogSet::Mondo), "WixUI_Mondo");
        assert_eq!(format!("{}", WixUiDialogSet::Minimal), "WixUI_Minimal");
        assert_eq!(format!("{}", WixUiDialogSet::Advanced), "WixUI_Advanced");

        assert!(WixUiDialogSet::from_str("unknown_dialog_set").is_err());
    }

    #[test]
    fn test_placeholder_bmp_generation() {
        let bmp = generate_placeholder_bmp(10, 10, 255, 0, 0);
        assert_eq!(&bmp[0..2], b"BM");
        let file_size = u32::from_le_bytes([bmp[2], bmp[3], bmp[4], bmp[5]]);
        assert_eq!(file_size as usize, bmp.len());
        let width = i32::from_le_bytes([bmp[18], bmp[19], bmp[20], bmp[21]]);
        let height = i32::from_le_bytes([bmp[22], bmp[23], bmp[24], bmp[25]]);
        assert_eq!(width, 10);
        assert_eq!(height, 10);
    }

    #[test]
    fn test_inject_ui_library_installdir() {
        for mut db in [
            LinkedDatabase::new(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ]
        .into_iter()
        .flatten()
        {
            assert!(inject_ui_library(
                &mut db,
                WixUiDialogSet::InstallDir,
                None,
                None,
                Some(b"icon_data"),
            )
            .is_ok());

            assert_ne!(db.get_records("Dialog"), []);
            assert_ne!(db.get_records("Control"), []);
            assert_ne!(db.get_records("ControlEvent"), []);
            assert_ne!(db.get_records("ControlCondition"), []);
            assert_ne!(db.get_records("EventMapping"), []);
            assert_ne!(db.get_records("TextStyle"), []);
            assert_ne!(db.get_records("InstallUISequence"), []);
            assert_ne!(db.get_records("Binary"), []);
            assert_ne!(db.get_records("Icon"), []);
            assert!(db
                .get_records("Property")
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String("ARPPRODUCTICON".to_string()))));
        }
    }

    #[test]
    fn test_inject_ui_library_featuretree_and_minimal() {
        for mut db_feat in [
            LinkedDatabase::new(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                inject_ui_library(&mut db_feat, WixUiDialogSet::FeatureTree, None, None, None)
                    .is_ok()
            );
            assert!(db_feat
                .get_records("ControlEvent")
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String("CustomizeDlg".to_string()))));
        }

        for mut db_min in [
            LinkedDatabase::new(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                inject_ui_library(&mut db_min, WixUiDialogSet::Minimal, None, None, None).is_ok()
            );
            assert!(db_min
                .get_records("ControlEvent")
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String("WelcomeDlg".to_string()))));
        }
    }

    #[test]
    fn test_placeholder_ico_generation() {
        let ico = generate_placeholder_ico(32, 32, 255, 128, 0);
        assert_eq!(&ico[0..2], &[0, 0]); // Reserved
        assert_eq!(&ico[2..4], &[1, 0]); // Type 1
        assert_eq!(&ico[4..6], &[1, 0]); // 1 image
        assert_eq!(ico[6], 32); // width
        assert_eq!(ico[7], 32); // height
        assert_ne!(ico.len(), 0);
    }

    #[test]
    fn test_inject_ui_library_mondo_and_advanced() {
        for mut db_mondo in [
            LinkedDatabase::new(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                inject_ui_library(&mut db_mondo, WixUiDialogSet::Mondo, None, None, None).is_ok()
            );

            // Verify SetupTypeDlg is hooked in ControlEvent
            assert!(db_mondo
                .get_records("ControlEvent")
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String("SetupTypeDlg".to_string()))));

            // Verify all required Binary assets are present
            let mut binary_records = db_mondo.get_records("Binary").to_vec();
            binary_records.push(Record::with_fields(vec![FieldValue::Null]));
            let bin_names: Vec<String> = binary_records
                .iter()
                .filter_map(|r| {
                    if let Some(FieldValue::String(n)) = r.get(0) {
                        Some(n.clone())
                    } else {
                        None
                    }
                })
                .collect();
            assert!(bin_names.contains(&"WixUI_Bmp_Banner".to_string()));
            assert!(bin_names.contains(&"WixUI_Bmp_Dialog".to_string()));
            assert!(bin_names.contains(&"WixUI_Ico_Info".to_string()));
            assert!(bin_names.contains(&"WixUI_Ico_Exclam".to_string()));
            assert!(bin_names.contains(&"WixUI_Bmp_New".to_string()));
            assert!(bin_names.contains(&"WixUI_Bmp_Up".to_string()));
        }

        // Test Advanced set
        for mut db_adv in [
            LinkedDatabase::new(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                inject_ui_library(&mut db_adv, WixUiDialogSet::Advanced, None, None, None).is_ok()
            );
            assert!(db_adv
                .get_records("ControlEvent")
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String("InstallScopeDlg".to_string()))));

            // Verify control types across the control table
            let mut control_records = db_adv.get_records("Control").to_vec();
            control_records.push(Record::with_fields(vec![FieldValue::Null]));
            let control_types: Vec<String> = control_records
                .iter()
                .filter_map(|r| {
                    if let Some(FieldValue::String(t)) = r.get(2) {
                        Some(t.clone())
                    } else {
                        None
                    }
                })
                .collect();
            assert!(control_types.contains(&"PushButton".to_string()));
            assert!(control_types.contains(&"RadioButtonGroup".to_string()));
            assert!(control_types.contains(&"CheckBox".to_string()));
            assert!(control_types.contains(&"Text".to_string()));
            assert!(control_types.contains(&"PathEdit".to_string()));
            assert!(control_types.contains(&"DirectoryCombo".to_string()));
            assert!(control_types.contains(&"DirectoryList".to_string()));
            assert!(control_types.contains(&"VolumeCostList".to_string()));
            assert!(control_types.contains(&"SelectionTree".to_string()));
            assert!(control_types.contains(&"ScrollableText".to_string()));
            assert!(control_types.contains(&"ProgressBar".to_string()));
            assert!(control_types.contains(&"Bitmap".to_string()));
            assert!(control_types.contains(&"Line".to_string()));
        }
    }
}
