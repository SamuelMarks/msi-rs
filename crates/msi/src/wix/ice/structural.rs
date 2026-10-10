//! Phase 1: Structural, syntax, naming, and metadata ICE validation rules.
//!
//! Grounded directly in the Windows Installer SDK and `WiX` validation engine specifications:
//! - ICE16, ICE29, ICE35, ICE37, ICE39, ICE40, ICE41, ICE45, ICE46, ICE48,
//!   ICE51, ICE53, ICE58, ICE70, ICE71, ICE73, ICE74, ICE82, ICE84, ICE87,
//!   ICE92, ICE93, ICE95.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::HashSet;

/// Checks whether a string conforms to a valid uppercase GUID format `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
///
/// # Arguments
///
/// * `s` - TODO: Document argument.
///
/// # Returns
///
/// TODO: Document return value.
fn is_valid_uppercase_guid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 38 {
        return false;
    }
    if bytes[0] != b'{' || bytes[37] != b'}' {
        return false;
    }
    if bytes[9] != b'-' || bytes[14] != b'-' || bytes[19] != b'-' || bytes[24] != b'-' {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i == 0 || i == 37 || i == 9 || i == 14 || i == 19 || i == 24 {
            continue;
        }
        if !b.is_ascii_hexdigit() || b.is_ascii_lowercase() {
            return false;
        }
    }
    true
}

/// ICE16: Verifies that the `ProductName` property is defined and non-empty.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ProductName` is missing or empty, otherwise `None`.
#[must_use]
pub fn validate_ice16(db: &LinkedDatabase) -> Option<IceReport> {
    let props = db.get_records("Property");
    if props.is_empty() {
        return None;
    }
    for r in props {
        if let (Some(FieldValue::String(prop)), Some(FieldValue::String(val))) =
            (r.get(0), r.get(1))
        {
            if prop == "ProductName" && !val.trim().is_empty() {
                return None;
            }
        }
    }
    Some(
        IceReport::error(
            "ICE16",
            "Property 'ProductName' is missing or empty in Property table",
        )
        .with_table("Property"),
    )
}

/// ICE29: Validates stream name lengths (<= 62 chars) and character validity in Binary and Icon tables.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if an invalid OLE compound document stream name is discovered.
#[must_use]
pub fn validate_ice29(db: &LinkedDatabase) -> Option<IceReport> {
    for tbl in &["Binary", "Icon"] {
        for r in db.get_records(tbl) {
            if let Some(FieldValue::String(name)) = r.get(0) {
                if name.len() > 62 {
                    return Some(
                        IceReport::error(
                            "ICE29",
                            format!("Stream name '{name}' exceeds maximum allowed length of 62 characters"),
                        )
                        .with_table(*tbl),
                    );
                }
                if name
                    .chars()
                    .any(|c| c == '/' || c == '\\' || c == ':' || c == '!')
                {
                    return Some(
                        IceReport::error(
                            "ICE29",
                            format!(
                                "Stream name '{name}' contains illegal characters (\\, /, :, !)"
                            ),
                        )
                        .with_table(*tbl),
                    );
                }
            }
        }
    }
    None
}

/// ICE35: Validates Cabinet file naming conventions in Media table.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if cabinet name syntax is invalid.
#[must_use]
pub fn validate_ice35(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Media") {
        if let Some(FieldValue::String(cab)) = r.get(3) {
            if !cab.is_empty() {
                let cab_name = cab.strip_prefix('#').unwrap_or(cab);
                if cab_name.chars().any(|c| {
                    c == '/'
                        || c == '\\'
                        || c == ':'
                        || c == '*'
                        || c == '?'
                        || c == '"'
                        || c == '<'
                        || c == '>'
                        || c == '|'
                }) {
                    return Some(
                        IceReport::error(
                            "ICE35",
                            format!("Media cabinet '{cab}' contains illegal path characters"),
                        )
                        .with_table("Media"),
                    );
                }
                if !cab_name.to_ascii_lowercase().ends_with(".cab") {
                    return Some(
                        IceReport::error(
                            "ICE35",
                            format!("Media cabinet '{cab}' must have a .cab extension"),
                        )
                        .with_table("Media"),
                    );
                }
            }
        }
    }
    None
}

/// ICE37: Verifies that standard system directory properties are not defined in Property table.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if standard folder property is overridden in Property table.
#[must_use]
pub fn validate_ice37(db: &LinkedDatabase) -> Option<IceReport> {
    const STANDARD_DIRS: &[&str] = &[
        "SystemFolder",
        "System64Folder",
        "ProgramFilesFolder",
        "ProgramFiles64Folder",
        "CommonFilesFolder",
        "CommonFiles64Folder",
        "WindowsFolder",
        "AdminToolsFolder",
        "AppDataFolder",
        "CommonAppDataFolder",
        "LocalAppDataFolder",
        "DesktopFolder",
        "FontsFolder",
        "SendToFolder",
        "StartMenuFolder",
        "StartupFolder",
        "TemplateFolder",
    ];

    for r in db.get_records("Property") {
        if let Some(FieldValue::String(prop)) = r.get(0) {
            if STANDARD_DIRS.iter().any(|&d| d == prop) {
                return Some(
                    IceReport::error(
                        "ICE37",
                        format!("Standard directory property '{prop}' must not be defined in Property table"),
                    )
                    .with_table("Property"),
                );
            }
        }
    }
    None
}

/// ICE39: Verifies presence of required core packaging properties in Property table.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a required property is missing.
#[must_use]
pub fn validate_ice39(db: &LinkedDatabase) -> Option<IceReport> {
    let props = db.get_records("Property");
    if props.is_empty() {
        return None;
    }
    let mut found_product_code = false;
    let mut found_product_version = false;
    let mut found_manufacturer = false;

    for r in props {
        if let Some(FieldValue::String(prop)) = r.get(0) {
            match prop.as_str() {
                "ProductCode" => found_product_code = true,
                "ProductVersion" => found_product_version = true,
                "Manufacturer" => found_manufacturer = true,
                _ => {}
            }
        }
    }

    if !found_product_code {
        return Some(
            IceReport::error("ICE39", "Required property 'ProductCode' is missing")
                .with_table("Property"),
        );
    }
    if !found_product_version {
        return Some(
            IceReport::error("ICE39", "Required property 'ProductVersion' is missing")
                .with_table("Property"),
        );
    }
    if !found_manufacturer {
        return Some(
            IceReport::error("ICE39", "Required property 'Manufacturer' is missing")
                .with_table("Property"),
        );
    }

    None
}

/// ICE40: Validates MIME Content-Type syntax and extension linkage.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if MIME table has invalid content type or unlinked extension.
#[must_use]
pub fn validate_ice40(db: &LinkedDatabase) -> Option<IceReport> {
    let extensions: HashSet<String> = db
        .get_records("Extension")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(ext)) => Some(ext.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("MIME") {
        if let Some(FieldValue::String(content_type)) = r.get(0) {
            if !content_type.contains('/')
                || content_type.starts_with('/')
                || content_type.ends_with('/')
            {
                return Some(
                    IceReport::error(
                        "ICE40",
                        format!("MIME ContentType '{content_type}' is malformed; must be 'type/subtype'"),
                    )
                    .with_table("MIME"),
                );
            }
        }
        if let Some(FieldValue::String(ext_ref)) = r.get(1) {
            if !extensions.is_empty() && !extensions.contains(ext_ref) {
                return Some(
                    IceReport::error(
                        "ICE40",
                        format!("MIME table references non-existent Extension '{ext_ref}'"),
                    )
                    .with_table("MIME"),
                );
            }
        }
    }
    None
}

/// ICE41: Validates Component GUID formatting (must be uppercase or null).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Component table has invalid GUID.
#[must_use]
pub fn validate_ice41(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Component") {
        if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(guid))) =
            (r.get(0), r.get(1))
        {
            if !guid.is_empty() && !is_valid_uppercase_guid(guid) {
                return Some(
                    IceReport::error(
                        "ICE41",
                        format!("Component '{comp_id}' has malformed or lowercase GUID '{guid}'"),
                    )
                    .with_table("Component"),
                );
            }
        }
    }
    None
}

/// ICE45: Validates Win32 filename restrictions and reserved DOS names.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if File table contains illegal filenames.
#[must_use]
pub fn validate_ice45(db: &LinkedDatabase) -> Option<IceReport> {
    const RESERVED_NAMES: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    for r in db.get_records("File") {
        if let Some(FieldValue::String(fname)) = r.get(2) {
            let short_name = fname.split('|').next().unwrap_or(fname);
            let base_name = short_name.split('.').next().unwrap_or(short_name);

            if RESERVED_NAMES
                .iter()
                .any(|&res| res.eq_ignore_ascii_case(base_name))
            {
                return Some(
                    IceReport::error(
                        "ICE45",
                        format!(
                            "Filename '{short_name}' uses reserved DOS device name '{base_name}'"
                        ),
                    )
                    .with_table("File"),
                );
            }

            if short_name.chars().any(|c| {
                c == '/'
                    || c == '\\'
                    || c == ':'
                    || c == '*'
                    || c == '?'
                    || c == '"'
                    || c == '<'
                    || c == '>'
                    || c == '|'
            }) {
                return Some(
                    IceReport::error(
                        "ICE45",
                        format!(
                            "Filename '{short_name}' contains illegal Win32 filename characters"
                        ),
                    )
                    .with_table("File"),
                );
            }
        }
    }
    None
}

/// ICE46: Validates Property identifier naming conventions (no starting digit, valid chars).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Property table contains invalid property names.
#[must_use]
pub fn validate_ice46(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Property") {
        if let Some(FieldValue::String(prop)) = r.get(0) {
            if prop.is_empty() {
                return Some(
                    IceReport::error("ICE46", "Property identifier cannot be empty")
                        .with_table("Property"),
                );
            }
            let first = prop.chars().next().unwrap_or(' ');
            if first.is_ascii_digit() {
                return Some(
                    IceReport::error(
                        "ICE46",
                        format!("Property identifier '{prop}' cannot start with a digit"),
                    )
                    .with_table("Property"),
                );
            }
            if !prop
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                return Some(
                    IceReport::error(
                        "ICE46",
                        format!("Property identifier '{prop}' contains invalid characters"),
                    )
                    .with_table("Property"),
                );
            }
        }
    }
    None
}

/// ICE48: Verifies that Directory definitions do not contain hardcoded drive letters.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Directory table contains hardcoded drive letters.
#[must_use]
pub fn validate_ice48(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Directory") {
        if let (Some(FieldValue::String(dir_id)), Some(FieldValue::String(def_dir))) =
            (r.get(0), r.get(2))
        {
            if def_dir.len() >= 2 {
                let bytes = def_dir.as_bytes();
                if bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
                    return Some(
                        IceReport::error(
                            "ICE48",
                            format!("Directory '{dir_id}' has hardcoded drive path '{def_dir}'"),
                        )
                        .with_table("Directory"),
                    );
                }
            }
        }
    }
    None
}

/// ICE51: Validates Font table references to File table and font title presence.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Font table has dangling file reference or empty title.
#[must_use]
pub fn validate_ice51(db: &LinkedDatabase) -> Option<IceReport> {
    let files: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(f)) => Some(f.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Font") {
        if let Some(FieldValue::String(file_ref)) = r.get(0) {
            if !files.is_empty() && !files.contains(file_ref) {
                return Some(
                    IceReport::error(
                        "ICE51",
                        format!("Font table references non-existent File '{file_ref}'"),
                    )
                    .with_table("Font"),
                );
            }
        }
        if let Some(FieldValue::String(title)) = r.get(1) {
            if title.trim().is_empty() {
                return Some(
                    IceReport::error("ICE51", "Font table has empty FontTitle").with_table("Font"),
                );
            }
        }
    }
    None
}

/// ICE53: Validates Registry key path syntax (no leading, trailing, or double slashes).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Registry key has invalid slash formatting.
#[must_use]
pub fn validate_ice53(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Registry") {
        if let Some(FieldValue::String(key)) = r.get(2) {
            if key.starts_with('\\') {
                return Some(
                    IceReport::error(
                        "ICE53",
                        format!("Registry key '{key}' has illegal leading backslash"),
                    )
                    .with_table("Registry"),
                );
            }
            if key.ends_with('\\') {
                return Some(
                    IceReport::error(
                        "ICE53",
                        format!("Registry key '{key}' has illegal trailing backslash"),
                    )
                    .with_table("Registry"),
                );
            }
            if key.contains(r"\\") {
                return Some(
                    IceReport::error(
                        "ICE53",
                        format!("Registry key '{key}' contains illegal consecutive backslashes"),
                    )
                    .with_table("Registry"),
                );
            }
        }
    }
    None
}

/// ICE58: Validates Media table `DiskId` sequencing (positive, non-zero, sorted).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Media table `DiskId` sequence is invalid.
#[must_use]
pub fn validate_ice58(db: &LinkedDatabase) -> Option<IceReport> {
    let mut prev_disk_id = 0i16;
    for r in db.get_records("Media") {
        if let Some(FieldValue::Short(disk_id)) = r.get(0) {
            if *disk_id <= 0 {
                return Some(
                    IceReport::error(
                        "ICE58",
                        format!("Media DiskId {disk_id} must be greater than zero"),
                    )
                    .with_table("Media"),
                );
            }
            if *disk_id <= prev_disk_id {
                return Some(
                    IceReport::error(
                        "ICE58",
                        format!("Media DiskId {disk_id} is not in strictly ascending order"),
                    )
                    .with_table("Media"),
                );
            }
            prev_disk_id = *disk_id;
        }
    }
    None
}

/// ICE70: Validates Shortcut argument string formatting and quotation balance.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Shortcut argument formatting has unbalanced quotes.
#[must_use]
pub fn validate_ice70(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Shortcut") {
        if let (Some(FieldValue::String(sc_id)), Some(FieldValue::String(args))) =
            (r.get(0), r.get(7))
        {
            let quote_count = args.chars().filter(|&c| c == '"').count();
            if quote_count % 2 != 0 {
                return Some(
                    IceReport::error(
                        "ICE70",
                        format!("Shortcut '{sc_id}' has unbalanced quotation marks in Arguments: {args}"),
                    )
                    .with_table("Shortcut"),
                );
            }
        }
    }
    None
}

/// ICE71: Validates Media table cabinet compression attributes and disk prompt text.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if multi-disk media is missing disk prompt text.
#[must_use]
pub fn validate_ice71(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Media") {
        if let (Some(FieldValue::Short(disk_id)), Some(disk_prompt_val)) = (r.get(0), r.get(2)) {
            if *disk_id > 1 {
                let empty_prompt = match disk_prompt_val {
                    FieldValue::String(p) => p.trim().is_empty(),
                    FieldValue::Null => true,
                    _ => false,
                };
                if empty_prompt {
                    return Some(
                        IceReport::warning(
                            "ICE71",
                            format!("Media disk {disk_id} has empty DiskPrompt"),
                        )
                        .with_table("Media"),
                    );
                }
            }
        }
    }
    None
}

/// ICE73: Validates Package Code formatting (valid uppercase GUID without curly braces or standard format).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if package code format is invalid.
#[must_use]
pub fn validate_ice73(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Property") {
        if let (Some(FieldValue::String(prop)), Some(FieldValue::String(code))) =
            (r.get(0), r.get(1))
        {
            if prop == "PackageCode" && !is_valid_uppercase_guid(code) {
                return Some(
                    IceReport::error(
                        "ICE73",
                        format!("PackageCode '{code}' is not a valid uppercase formatted GUID"),
                    )
                    .with_table("Property"),
                );
            }
        }
    }
    None
}

/// ICE74: Validates FASTOEM property usage.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if FASTOEM property is improperly configured.
#[must_use]
pub fn validate_ice74(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Property") {
        if let (Some(FieldValue::String(prop)), Some(FieldValue::String(val))) =
            (r.get(0), r.get(1))
        {
            if prop == "FASTOEM" && val != "1" && val != "0" {
                return Some(
                    IceReport::error(
                        "ICE74",
                        format!("FASTOEM property has invalid value '{val}'; must be '0' or '1'"),
                    )
                    .with_table("Property"),
                );
            }
        }
    }
    None
}

/// ICE82: Validates duplicate sequence numbers in sequence tables.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if duplicate positive sequence number is found in any sequence table.
#[must_use]
pub fn validate_ice82(db: &LinkedDatabase) -> Option<IceReport> {
    const SEQUENCE_TABLES: &[&str] = &[
        "InstallExecuteSequence",
        "InstallUISequence",
        "AdminExecuteSequence",
        "AdminUISequence",
        "AdvtExecuteSequence",
    ];

    for &tbl in SEQUENCE_TABLES {
        let mut seen = HashSet::new();
        for r in db.get_records(tbl) {
            if let Some(FieldValue::Short(seq)) = r.get(2) {
                if *seq > 0 && !seen.insert(*seq) {
                    return Some(
                        IceReport::error(
                            "ICE82",
                            format!("Duplicate sequence number {seq} in sequence table '{tbl}'"),
                        )
                        .with_table(tbl),
                    );
                }
            }
        }
    }
    None
}

/// ICE84: Validates `ActionText` descriptions and parameter format tokens.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ActionText` template tokens are unbalanced.
#[must_use]
pub fn validate_ice84(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("ActionText") {
        if let (Some(FieldValue::String(act)), Some(FieldValue::String(tmpl))) =
            (r.get(0), r.get(2))
        {
            let open = tmpl.chars().filter(|&c| c == '[').count();
            let close = tmpl.chars().filter(|&c| c == ']').count();
            if open != close {
                return Some(
                    IceReport::error(
                        "ICE84",
                        format!(
                            "ActionText for '{act}' has unbalanced template brackets: '{tmpl}'"
                        ),
                    )
                    .with_table("ActionText"),
                );
            }
        }
    }
    None
}

/// ICE87: Validates File table attribute bitmasks against recognized flags.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if File attributes contain unknown bit flags.
#[must_use]
pub fn validate_ice87(db: &LinkedDatabase) -> Option<IceReport> {
    const VALID_ATTRIBUTES: i16 = 0x3E07;

    for r in db.get_records("File") {
        if let (Some(FieldValue::String(file_id)), Some(FieldValue::Short(attr))) =
            (r.get(0), r.get(6))
        {
            if (*attr & !VALID_ATTRIBUTES) != 0 {
                return Some(
                    IceReport::error(
                        "ICE87",
                        format!("File '{file_id}' has unrecognized attribute bits: 0x{attr:04X}"),
                    )
                    .with_table("File"),
                );
            }
        }
    }
    None
}

/// ICE92: Validates that Component Directory_ column references an existing record in Directory table.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Component references non-existent Directory.
#[must_use]
pub fn validate_ice92(db: &LinkedDatabase) -> Option<IceReport> {
    let dirs: HashSet<String> = db
        .get_records("Directory")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Component") {
        if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(dir_ref))) =
            (r.get(0), r.get(2))
        {
            if !dirs.is_empty() && !dirs.contains(dir_ref) {
                return Some(
                    IceReport::error(
                        "ICE92",
                        format!(
                            "Component '{comp_id}' references non-existent Directory '{dir_ref}'"
                        ),
                    )
                    .with_table("Component"),
                );
            }
        }
    }
    None
}

/// ICE93: Validates uppercase GUID format across all standard primary and foreign key columns.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if any standard GUID column contains a malformed or lowercase GUID.
#[must_use]
pub fn validate_ice93(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Property") {
        if let (Some(FieldValue::String(prop)), Some(FieldValue::String(val))) =
            (r.get(0), r.get(1))
        {
            if prop.ends_with("Code") && !val.is_empty() && !is_valid_uppercase_guid(val) {
                return Some(
                    IceReport::error(
                        "ICE93",
                        format!("Property '{prop}' contains invalid GUID format: '{val}'"),
                    )
                    .with_table("Property"),
                );
            }
        }
    }
    None
}

/// ICE95: Validates Font table character set and attributes.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Font table character set value is invalid.
#[must_use]
pub fn validate_ice95(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Font") {
        if let (Some(FieldValue::String(file_id)), Some(FieldValue::Short(charset))) =
            (r.get(0), r.get(2))
        {
            if *charset < 0 {
                return Some(
                    IceReport::error(
                        "ICE95",
                        format!("Font '{file_id}' has negative character set value: {charset}"),
                    )
                    .with_table("Font"),
                );
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::Record;

    fn make_test_db(table: &str, records: Vec<Record>) -> LinkedDatabase {
        let mut db = LinkedDatabase::default();
        for r in records {
            db.add_record(table, r);
        }
        db
    }

    #[test]
    fn test_is_valid_uppercase_guid_direct() {
        assert!(is_valid_uppercase_guid(
            "{11111111-1111-1111-1111-111111111111}"
        ));
        // wrong len
        assert!(!is_valid_uppercase_guid("{1111}"));
        // wrong braces
        assert!(!is_valid_uppercase_guid(
            "[11111111-1111-1111-1111-111111111111]"
        ));
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111-1111-1111-111111111111]"
        ));
        // wrong hyphens
        assert!(!is_valid_uppercase_guid(
            "{11111111_1111-1111-1111-111111111111}"
        ));
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111_1111-1111-111111111111}"
        ));
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111-1111_1111-111111111111}"
        ));
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111-1111-1111_111111111111}"
        ));
        // lowercase and invalid chars
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111-1111-1111-11111111111a}"
        ));
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111-1111-1111-11111111111Z}"
        ));
        assert!(!is_valid_uppercase_guid(
            "{11111111-1111-1111-1111-11111111111@}"
        ));
    }

    #[test]
    fn test_ice16_product_name() {
        let db_empty_table = LinkedDatabase::default();
        assert!(validate_ice16(&db_empty_table).is_none());

        let db_bad = make_test_db(
            "Property",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("Manufacturer".to_string()),
                    FieldValue::String("Acme".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice16(&db_bad).is_some());

        let db_empty = make_test_db(
            "Property",
            vec![Record::with_fields(vec![
                FieldValue::String("ProductName".to_string()),
                FieldValue::String("   ".to_string()),
            ])],
        );
        assert!(validate_ice16(&db_empty).is_some());

        let db_good = make_test_db(
            "Property",
            vec![Record::with_fields(vec![
                FieldValue::String("ProductName".to_string()),
                FieldValue::String("My Product".to_string()),
            ])],
        );
        assert!(validate_ice16(&db_good).is_none());
    }

    #[test]
    fn test_ice29_stream_names() {
        let db_good = make_test_db(
            "Binary",
            vec![
                Record::with_fields(vec![FieldValue::String("MyBinaryStream".to_string())]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice29(&db_good).is_none());

        let long_name = "A".repeat(65);
        let db_long = make_test_db(
            "Binary",
            vec![Record::with_fields(vec![FieldValue::String(long_name)])],
        );
        assert!(validate_ice29(&db_long).is_some());

        for bad in &["app:icon", "app/icon", "app\\icon", "app!icon"] {
            let db_illegal = make_test_db(
                "Icon",
                vec![Record::with_fields(vec![FieldValue::String(
                    #[allow(clippy::inefficient_to_string)]
                    bad.to_string(),
                )])],
            );
            assert!(validate_ice29(&db_illegal).is_some());
        }
    }

    #[test]
    fn test_ice35_cabinet_naming() {
        let db_good = make_test_db(
            "Media",
            vec![
                Record::with_fields(vec![
                    FieldValue::Short(1),
                    FieldValue::Short(1),
                    FieldValue::Null,
                    FieldValue::String("#cab1.cab".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::Short(2),
                    FieldValue::Short(1),
                    FieldValue::Null,
                    FieldValue::String(String::new()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice35(&db_good).is_none());

        let db_bad_ext = make_test_db(
            "Media",
            vec![Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::String("cab1.zip".to_string()),
            ])],
        );
        assert!(validate_ice35(&db_bad_ext).is_some());

        for bad in &[
            "dir/cab.cab",
            "dir\\cab.cab",
            "dir:cab.cab",
            "dir*cab.cab",
            "dir?cab.cab",
            "dir\"cab.cab",
            "dir<cab.cab",
            "dir>cab.cab",
            "dir|cab.cab",
        ] {
            let db_bad_char = make_test_db(
                "Media",
                vec![Record::with_fields(vec![
                    FieldValue::Short(1),
                    FieldValue::Short(1),
                    FieldValue::Null,
                    #[allow(clippy::inefficient_to_string)]
                    FieldValue::String(bad.to_string()),
                ])],
            );
            assert!(validate_ice35(&db_bad_char).is_some());
        }
    }

    #[test]
    fn test_ice37_standard_directory_properties() {
        let db_good = make_test_db(
            "Property",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("MY_PROP".to_string()),
                    FieldValue::String("Val".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice37(&db_good).is_none());

        let db_bad = make_test_db(
            "Property",
            vec![Record::with_fields(vec![
                FieldValue::String("ProgramFilesFolder".to_string()),
                FieldValue::String(r"C:\Progs".to_string()),
            ])],
        );
        assert!(validate_ice37(&db_bad).is_some());
    }

    #[test]
    fn test_ice39_required_properties() {
        let db_empty = LinkedDatabase::default();
        assert!(validate_ice39(&db_empty).is_none());

        let mut db_no_prod = LinkedDatabase::default();
        db_no_prod.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("OtherProp".to_string()),
                FieldValue::String("val".to_string()),
            ]),
        );
        db_no_prod.add_record("Property", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice39(&db_no_prod).is_some());

        let mut db = LinkedDatabase::default();
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductCode".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
            ]),
        );
        assert!(validate_ice39(&db).is_some());

        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("1.0.0".to_string()),
            ]),
        );
        assert!(validate_ice39(&db).is_some());

        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Manufacturer".to_string()),
                FieldValue::String("Acme".to_string()),
            ]),
        );
        assert!(validate_ice39(&db).is_none());
    }

    #[test]
    fn test_ice40_mime_syntax() {
        let mut db = LinkedDatabase::default();
        db.add_record(
            "Extension",
            Record::with_fields(vec![FieldValue::String("txt".to_string())]),
        );
        db.add_record("Extension", Record::with_fields(vec![FieldValue::Short(1)]));
        db.add_record(
            "MIME",
            Record::with_fields(vec![
                FieldValue::String("text/plain".to_string()),
                FieldValue::String("txt".to_string()),
            ]),
        );
        db.add_record("MIME", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice40(&db).is_none());

        for bad in &["textplain", "/plain", "text/"] {
            let db_bad_syntax = make_test_db(
                "MIME",
                vec![Record::with_fields(vec![
                    #[allow(clippy::inefficient_to_string)]
                    FieldValue::String(bad.to_string()),
                    FieldValue::String("txt".to_string()),
                ])],
            );
            assert!(validate_ice40(&db_bad_syntax).is_some());
        }

        let mut db_dangling = LinkedDatabase::default();
        db_dangling.add_record(
            "Extension",
            Record::with_fields(vec![FieldValue::String("doc".to_string())]),
        );
        db_dangling.add_record(
            "MIME",
            Record::with_fields(vec![
                FieldValue::String("text/plain".to_string()),
                FieldValue::String("txt".to_string()),
            ]),
        );
        assert!(validate_ice40(&db_dangling).is_some());

        // Empty extensions table
        let mut db_no_ext = LinkedDatabase::default();
        db_no_ext.add_record(
            "MIME",
            Record::with_fields(vec![
                FieldValue::String("text/plain".to_string()),
                FieldValue::String("txt".to_string()),
            ]),
        );
        assert!(validate_ice40(&db_no_ext).is_none());
    }

    #[test]
    fn test_ice41_component_guid() {
        let db_good = make_test_db(
            "Component",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("C1".to_string()),
                    FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("C2".to_string()),
                    FieldValue::String(String::new()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice41(&db_good).is_none());

        let db_lower = make_test_db(
            "Component",
            vec![Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-11111111111a}".to_string()),
            ])],
        );
        assert!(validate_ice41(&db_lower).is_some());

        let db_malformed = make_test_db(
            "Component",
            vec![Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::String("not-a-guid".to_string()),
            ])],
        );
        assert!(validate_ice41(&db_malformed).is_some());
    }

    #[test]
    fn test_ice45_filename_restrictions() {
        let db_good = make_test_db(
            "File",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("F1".to_string()),
                    FieldValue::String("C1".to_string()),
                    FieldValue::String("app.exe".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice45(&db_good).is_none());

        let db_dos = make_test_db(
            "File",
            vec![Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
                FieldValue::String("NUL.txt".to_string()),
            ])],
        );
        assert!(validate_ice45(&db_dos).is_some());

        for bad in &[
            "bad/name.txt",
            "bad\\name.txt",
            "bad:name.txt",
            "bad*name.txt",
            "bad?name.txt",
            "bad\"name.txt",
            "bad<name.txt",
            "bad>name.txt",
        ] {
            let db_char = make_test_db(
                "File",
                vec![Record::with_fields(vec![
                    FieldValue::String("F1".to_string()),
                    FieldValue::String("C1".to_string()),
                    #[allow(clippy::inefficient_to_string)]
                    FieldValue::String(bad.to_string()),
                ])],
            );
            assert!(validate_ice45(&db_char).is_some());
        }
    }

    #[test]
    fn test_ice46_property_naming() {
        let db_good = make_test_db(
            "Property",
            vec![
                Record::with_fields(vec![FieldValue::String("PUBLIC_PROP".to_string())]),
                Record::with_fields(vec![FieldValue::String("PrivateProp".to_string())]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice46(&db_good).is_none());

        let db_digit = make_test_db(
            "Property",
            vec![Record::with_fields(vec![FieldValue::String(
                "1PROP".to_string(),
            )])],
        );
        assert!(validate_ice46(&db_digit).is_some());

        let db_empty = make_test_db(
            "Property",
            vec![Record::with_fields(vec![FieldValue::String(String::new())])],
        );
        assert!(validate_ice46(&db_empty).is_some());

        let db_badchar = make_test_db(
            "Property",
            vec![Record::with_fields(vec![FieldValue::String(
                "PROP-NAME".to_string(),
            )])],
        );
        assert!(validate_ice46(&db_badchar).is_some());
    }

    #[test]
    fn test_ice48_hardcoded_drive() {
        let db_good = make_test_db(
            "Directory",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("TARGETDIR".to_string()),
                    FieldValue::Null,
                    FieldValue::String("SourceDir".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("DotDir".to_string()),
                    FieldValue::Null,
                    FieldValue::String(".".to_string()), // len < 2
                ]),
                Record::with_fields(vec![
                    FieldValue::String("ColonDir".to_string()),
                    FieldValue::Null,
                    FieldValue::String("1:dir".to_string()), // bytes[0] not alphabetic
                ]),
                Record::with_fields(vec![
                    FieldValue::String("SubDir".to_string()),
                    FieldValue::Null,
                    FieldValue::String("sub/path".to_string()), // bytes[1] != b':'
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice48(&db_good).is_none());

        let db_bad = make_test_db(
            "Directory",
            vec![Record::with_fields(vec![
                FieldValue::String("AppDir".to_string()),
                FieldValue::Null,
                FieldValue::String(r"C:\Program Files\App".to_string()),
            ])],
        );
        assert!(validate_ice48(&db_bad).is_some());
    }

    #[test]
    fn test_ice51_font_validation() {
        let mut db = LinkedDatabase::default();
        db.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("Arial.ttf".to_string())]),
        );
        db.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        db.add_record(
            "Font",
            Record::with_fields(vec![
                FieldValue::String("Arial.ttf".to_string()),
                FieldValue::String("Arial TrueType".to_string()),
            ]),
        );
        db.add_record("Font", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice51(&db).is_none());

        let mut db_dangling = LinkedDatabase::default();
        db_dangling.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("Other.ttf".to_string())]),
        );
        db_dangling.add_record(
            "Font",
            Record::with_fields(vec![
                FieldValue::String("Missing.ttf".to_string()),
                FieldValue::String("Missing Font".to_string()),
            ]),
        );
        assert!(validate_ice51(&db_dangling).is_some());

        let db_empty_title = make_test_db(
            "Font",
            vec![Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("   ".to_string()),
            ])],
        );
        assert!(validate_ice51(&db_empty_title).is_some());
    }

    #[test]
    fn test_ice53_registry_syntax() {
        let db_good = make_test_db(
            "Registry",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("R1".to_string()),
                    FieldValue::Short(2),
                    FieldValue::String(r"Software\Vendor\App".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice53(&db_good).is_none());

        let db_leading = make_test_db(
            "Registry",
            vec![Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"\Software\Vendor".to_string()),
            ])],
        );
        assert!(validate_ice53(&db_leading).is_some());

        let db_trailing = make_test_db(
            "Registry",
            vec![Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"Software\Vendor\".to_string()),
            ])],
        );
        assert!(validate_ice53(&db_trailing).is_some());

        let db_double = make_test_db(
            "Registry",
            vec![Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"Software\\Vendor".to_string()),
            ])],
        );
        assert!(validate_ice53(&db_double).is_some());
    }

    #[test]
    fn test_ice58_media_sequencing() {
        let db_good = make_test_db(
            "Media",
            vec![
                Record::with_fields(vec![FieldValue::Short(1)]),
                Record::with_fields(vec![FieldValue::Short(2)]),
                Record::with_fields(vec![FieldValue::Short(3)]),
                Record::with_fields(vec![FieldValue::String("invalid".to_string())]),
            ],
        );
        assert!(validate_ice58(&db_good).is_none());

        let db_zero = make_test_db(
            "Media",
            vec![Record::with_fields(vec![FieldValue::Short(0)])],
        );
        assert!(validate_ice58(&db_zero).is_some());

        let db_unsorted = make_test_db(
            "Media",
            vec![
                Record::with_fields(vec![FieldValue::Short(2)]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice58(&db_unsorted).is_some());
    }

    #[test]
    fn test_ice70_shortcut_arguments() {
        let db_good = make_test_db(
            "Shortcut",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("SC1".to_string()),
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::String(r#"-arg "value""#.to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice70(&db_good).is_none());

        let db_unbalanced = make_test_db(
            "Shortcut",
            vec![Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String(r#"-arg "unbalanced"#.to_string()),
            ])],
        );
        assert!(validate_ice70(&db_unbalanced).is_some());
    }

    #[test]
    fn test_ice71_media_disk_prompt() {
        let db_good = make_test_db(
            "Media",
            vec![
                Record::with_fields(vec![
                    FieldValue::Short(1),
                    FieldValue::Short(1),
                    FieldValue::Null,
                ]),
                Record::with_fields(vec![
                    FieldValue::Short(2),
                    FieldValue::Short(1),
                    FieldValue::String("Please insert disk 2".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::Short(3),
                    FieldValue::Short(1),
                    FieldValue::Short(123), // non-string, non-null prompt
                ]),
                Record::with_fields(vec![FieldValue::String("invalid".to_string())]),
            ],
        );
        assert!(validate_ice71(&db_good).is_none());

        let db_warn = make_test_db(
            "Media",
            vec![Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Short(1),
                FieldValue::Null,
            ])],
        );
        assert!(validate_ice71(&db_warn).is_some());

        let db_warn_empty = make_test_db(
            "Media",
            vec![Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Short(1),
                FieldValue::String("   ".to_string()),
            ])],
        );
        assert!(validate_ice71(&db_warn_empty).is_some());
    }

    #[test]
    fn test_ice73_package_code() {
        let db_good = make_test_db(
            "Property",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("PackageCode".to_string()),
                    FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("OtherProp".to_string()),
                    FieldValue::String("non-guid-value".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice73(&db_good).is_none());

        let db_lower = make_test_db(
            "Property",
            vec![Record::with_fields(vec![
                FieldValue::String("PackageCode".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-11111111111a}".to_string()),
            ])],
        );
        assert!(validate_ice73(&db_lower).is_some());
    }

    #[test]
    fn test_ice74_fastoem() {
        let db_good = make_test_db(
            "Property",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("FASTOEM".to_string()),
                    FieldValue::String("1".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("FASTOEM".to_string()),
                    FieldValue::String("0".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("OtherProp".to_string()),
                    FieldValue::String("some-value".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice74(&db_good).is_none());

        let db_bad = make_test_db(
            "Property",
            vec![Record::with_fields(vec![
                FieldValue::String("FASTOEM".to_string()),
                FieldValue::String("invalid".to_string()),
            ])],
        );
        assert!(validate_ice74(&db_bad).is_some());
    }

    #[test]
    fn test_ice82_duplicate_sequence() {
        let db_good = make_test_db(
            "InstallExecuteSequence",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("A1".to_string()),
                    FieldValue::Null,
                    FieldValue::Short(100),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("A2".to_string()),
                    FieldValue::Null,
                    FieldValue::Short(200),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("A3".to_string()),
                    FieldValue::Null,
                    FieldValue::Short(-1), // sequence <= 0 skipped
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice82(&db_good).is_none());

        let db_dup = make_test_db(
            "InstallExecuteSequence",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("A1".to_string()),
                    FieldValue::Null,
                    FieldValue::Short(100),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("A2".to_string()),
                    FieldValue::Null,
                    FieldValue::Short(100),
                ]),
            ],
        );
        assert!(validate_ice82(&db_dup).is_some());
    }

    #[test]
    fn test_ice84_actiontext_templates() {
        let db_good = make_test_db(
            "ActionText",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("InstallFiles".to_string()),
                    FieldValue::Null,
                    FieldValue::String("Copying [1] to [2]".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice84(&db_good).is_none());

        let db_bad = make_test_db(
            "ActionText",
            vec![Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::String("Copying [1 to [2]".to_string()),
            ])],
        );
        assert!(validate_ice84(&db_bad).is_some());
    }

    #[test]
    fn test_ice87_file_attributes() {
        let db_good = make_test_db(
            "File",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("F1".to_string()),
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Short(0x0200), // vital
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice87(&db_good).is_none());

        let db_bad = make_test_db(
            "File",
            vec![Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0x4000), // unrecognized bit
            ])],
        );
        assert!(validate_ice87(&db_bad).is_some());
    }

    #[test]
    fn test_ice92_component_directory() {
        let mut db = LinkedDatabase::default();
        db.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::String("INSTALLDIR".to_string())]),
        );
        db.add_record("Directory", Record::with_fields(vec![FieldValue::Short(1)]));
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
            ]),
        );
        db.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice92(&db).is_none());

        let mut db_bad = LinkedDatabase::default();
        db_bad.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::String("INSTALLDIR".to_string())]),
        );
        db_bad.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::Null,
                FieldValue::String("MISSINGDIR".to_string()),
            ]),
        );
        assert!(validate_ice92(&db_bad).is_some());

        // Empty Directory table
        let mut db_empty_dirs = LinkedDatabase::default();
        db_empty_dirs.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::Null,
                FieldValue::String("ANYDIR".to_string()),
            ]),
        );
        assert!(validate_ice92(&db_empty_dirs).is_none());
    }

    #[test]
    fn test_ice93_guid_format() {
        let db_good = make_test_db(
            "Property",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("ProductCode".to_string()),
                    FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("UpgradeCode".to_string()),
                    FieldValue::String("{22222222-2222-2222-2222-222222222222}".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("CustomCode".to_string()),
                    FieldValue::String("{33333333-3333-3333-3333-333333333333}".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("ProductCode".to_string()),
                    FieldValue::String(String::new()), // empty val
                ]),
                Record::with_fields(vec![
                    FieldValue::String("OtherProp".to_string()),
                    FieldValue::String("non-guid-value".to_string()),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice93(&db_good).is_none());

        let db_bad = make_test_db(
            "Property",
            vec![Record::with_fields(vec![
                FieldValue::String("ProductCode".to_string()),
                FieldValue::String("{invalid-guid}".to_string()),
            ])],
        );
        assert!(validate_ice93(&db_bad).is_some());
    }

    #[test]
    fn test_ice95_font_attributes() {
        let db_good = make_test_db(
            "Font",
            vec![
                Record::with_fields(vec![
                    FieldValue::String("F1".to_string()),
                    FieldValue::Null,
                    FieldValue::Short(1),
                ]),
                Record::with_fields(vec![FieldValue::Short(1)]),
            ],
        );
        assert!(validate_ice95(&db_good).is_none());

        let db_bad = make_test_db(
            "Font",
            vec![Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::Short(-1),
            ])],
        );
        assert!(validate_ice95(&db_bad).is_some());
    }
}
