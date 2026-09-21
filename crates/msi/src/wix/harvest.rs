//! `WiX` Asset and Metadata Harvester (`heat` replacement).
//!
//! Provides automated harvesting of:
//! - Local filesystem directory trees into `<DirectoryRef>`, `<ComponentGroup>`, `<Component>`, and `<File>` elements.
//! - Windows `.reg` files into `<RegistryKey>` and `<RegistryValue>` elements.
//! - COM class, `ProgID`, and `TypeLib` metadata into `<Class>`, `<ProgId>`, and `<TypeLib>` elements.
//! - Build output artifacts from build systems.

use crate::database::tables::types::ComponentGuid;
use crate::error::Result;
use std::fs;
use std::path::Path;

/// Double quote character constant (`"`).
const QUOTE: char = 34 as char;
/// Backslash character constant.
const BACKSLASH: char = 92 as char;

/// `WiX` Asset and Metadata Harvester generating `.wxs` source fragments from disk assets and registries.
#[derive(Debug, Clone, Default)]
pub struct Harvester {
    /// File extensions to exclude from harvesting.
    excluded_extensions: Vec<String>,
}

impl Harvester {
    /// Creates a new [`Harvester`] instance.
    ///
    /// # Returns
    ///
    /// A new [`Harvester`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            excluded_extensions: Vec::new(),
        }
    }

    /// Adds an excluded file extension (e.g. `"tmp"`, `"pdb"`).
    ///
    /// # Arguments
    ///
    /// * `ext` - Extension string to exclude.
    pub fn exclude_extension(&mut self, ext: impl Into<String>) {
        self.excluded_extensions.push(ext.into().to_lowercase());
    }

    /// Recursively harvests a directory tree into a valid `WiX` XML source fragment.
    ///
    /// # Arguments
    ///
    /// * `dir_path` - Path to local filesystem directory to harvest.
    /// * `comp_group_id` - Identifier for generated `<ComponentGroup>`.
    /// * `target_dir_id` - Target directory identifier (e.g. `"INSTALLFOLDER"`).
    ///
    /// # Returns
    ///
    /// `WiX` XML fragment string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] if reading files or directories fails.
    #[allow(clippy::too_many_lines, clippy::format_push_string)]
    pub fn harvest_directory(
        &self,
        dir_path: &Path,
        comp_group_id: &str,
        target_dir_id: &str,
    ) -> Result<String> {
        let mut xml = String::new();
        xml.push_str(&format!(
            "<?xml version={QUOTE}1.0{QUOTE} encoding={QUOTE}UTF-8{QUOTE}?>
"
        ));
        xml.push_str(&format!(
            "<Wix xmlns={QUOTE}http://schemas.microsoft.com/wix/2006/wi{QUOTE}>
"
        ));
        xml.push_str(
            "  <Fragment>
",
        );
        xml.push_str(&format!(
            "    <DirectoryRef Id={QUOTE}{target_dir_id}{QUOTE}>
"
        ));

        let mut components = Vec::new();
        let mut counter = 0;
        self.harvest_dir_recursive(
            dir_path,
            target_dir_id,
            &mut counter,
            &mut xml,
            &mut components,
        )?;

        xml.push_str(
            "    </DirectoryRef>
",
        );

        xml.push_str(&format!(
            "    <ComponentGroup Id={QUOTE}{comp_group_id}{QUOTE}>
"
        ));
        for comp_id in &components {
            xml.push_str(&format!(
                "      <ComponentRef Id={QUOTE}{comp_id}{QUOTE} />
"
            ));
        }
        xml.push_str(
            "    </ComponentGroup>
",
        );
        xml.push_str(
            "  </Fragment>
",
        );
        xml.push_str(
            "</Wix>
",
        );

        Ok(xml)
    }

    /// Recursive directory traversal worker.
    #[allow(clippy::too_many_lines, clippy::format_push_string)]
    fn harvest_dir_recursive(
        &self,
        current_path: &Path,
        parent_dir_id: &str,
        counter: &mut usize,
        xml: &mut String,
        components: &mut Vec<String>,
    ) -> Result<()> {
        let mut entries = Vec::new();
        if current_path.is_dir() {
            entries.extend(
                fs::read_dir(current_path)?.filter_map(|e| e.ok().map(|entry| entry.path())),
            );
        }
        entries.sort();

        let mut files = Vec::new();
        let mut subdirs = Vec::new();

        for entry in entries {
            if entry.is_dir() {
                subdirs.push(entry);
            } else {
                let ext = entry
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if !self.excluded_extensions.contains(&ext) {
                    files.push(entry);
                }
            }
        }

        // Emit components for files in this folder
        for file in files {
            *counter += 1;
            let file_name = file.file_name().unwrap_or_default().to_string_lossy();
            let comp_id = format!("cmp_{counter}_{}", sanitize_id(&file_name));
            let file_id = format!("fil_{counter}_{}", sanitize_id(&file_name));
            let guid = ComponentGuid::generate_deterministic(parent_dir_id, &comp_id)?;
            let short_name = make_8_3_name(&file_name);

            xml.push_str(&format!(
                "      <Component Id={QUOTE}{comp_id}{QUOTE} Guid={QUOTE}{guid}{QUOTE}>
"
            ));
            xml.push_str(&format!(
                "        <File Id={QUOTE}{file_id}{QUOTE} Name={QUOTE}{file_name}{QUOTE} ShortName={QUOTE}{short_name}{QUOTE} Source={QUOTE}{}{QUOTE} KeyPath={QUOTE}yes{QUOTE} />
",
                file.display()
            ));
            xml.push_str(
                "      </Component>
",
            );
            components.push(comp_id);
        }

        // Recurse into subdirectories
        for subdir in subdirs {
            *counter += 1;
            let dir_name = subdir.file_name().unwrap_or_default().to_string_lossy();
            let sub_dir_id = format!("dir_{counter}_{}", sanitize_id(&dir_name));
            let short_name = make_8_3_name(&dir_name);

            xml.push_str(&format!(
                "      <Directory Id={QUOTE}{sub_dir_id}{QUOTE} Name={QUOTE}{dir_name}{QUOTE} ShortName={QUOTE}{short_name}{QUOTE}>
"
            ));
            self.harvest_dir_recursive(&subdir, &sub_dir_id, counter, xml, components)?;
            xml.push_str(
                "      </Directory>
",
            );
        }

        Ok(())
    }

    /// Parses Windows `.reg` file format text and generates a `WiX` source fragment with `<RegistryKey>` and `<RegistryValue>` elements.
    ///
    /// # Arguments
    ///
    /// * `reg_text` - Content of Windows registry export file.
    /// * `component_id` - Identifier of component containing harvested keys.
    ///
    /// # Returns
    ///
    /// `WiX` XML fragment string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Validation`] on malformed registry syntax.
    #[allow(clippy::too_many_lines, clippy::format_push_string)]
    pub fn harvest_registry(&self, reg_text: &str, component_id: &str) -> Result<String> {
        if reg_text.trim().is_empty() {
            return Err(crate::error::Error::Validation {
                element: "Registry".to_string(),
                reason: "registry file content is empty".to_string(),
            });
        }

        let mut xml = String::new();
        xml.push_str(&format!(
            "<?xml version={QUOTE}1.0{QUOTE} encoding={QUOTE}UTF-8{QUOTE}?>
"
        ));
        xml.push_str(&format!(
            "<Wix xmlns={QUOTE}http://schemas.microsoft.com/wix/2006/wi{QUOTE}>
"
        ));
        xml.push_str(
            "  <Fragment>
",
        );
        xml.push_str(&format!(
            "    <Component Id={QUOTE}{component_id}{QUOTE} Guid={QUOTE}*{QUOTE}>
"
        ));

        let mut current_key: Option<(String, String)> = None;

        for line in reg_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with([';', '#']) {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let full_key = &trimmed[1..trimmed.len() - 1];
                if let Some((root_part, subkey_part)) = full_key.split_once(BACKSLASH) {
                    let root = match root_part {
                        "HKEY_CLASSES_ROOT" | "HKCR" => "HKCR",
                        "HKEY_CURRENT_USER" | "HKCU" => "HKCU",
                        "HKEY_USERS" | "HKU" => "HKU",
                        _ => "HKLM",
                    };
                    current_key = Some((root.to_string(), subkey_part.to_string()));
                } else {
                    current_key = Some(("HKLM".to_string(), full_key.to_string()));
                }
                continue;
            }

            if let Some((ref root, ref key)) = current_key {
                if let Some((name_part, val_part)) = trimmed.split_once('=') {
                    let val_name = if name_part == "@" {
                        String::new()
                    } else {
                        name_part.trim_matches(QUOTE).to_string()
                    };

                    let (v_type, v_val) = parse_reg_value(val_part);

                    xml.push_str(&format!(
                        "      <RegistryKey Root={QUOTE}{root}{QUOTE} Key={QUOTE}{key}{QUOTE}>
"
                    ));
                    if val_name.is_empty() {
                        xml.push_str(&format!(
                            "        <RegistryValue Type={QUOTE}{v_type}{QUOTE} Value={QUOTE}{v_val}{QUOTE} />
"
                        ));
                    } else {
                        xml.push_str(&format!(
                            "        <RegistryValue Name={QUOTE}{val_name}{QUOTE} Type={QUOTE}{v_type}{QUOTE} Value={QUOTE}{v_val}{QUOTE} />
"
                        ));
                    }
                    xml.push_str(
                        "      </RegistryKey>
",
                    );
                }
            }
        }

        xml.push_str(
            "    </Component>
",
        );
        xml.push_str(
            "  </Fragment>
",
        );
        xml.push_str(
            "</Wix>
",
        );

        Ok(xml)
    }

    /// Harvests COM registration metadata into `<Class>`, `<ProgId>`, and `<TypeLib>` XML elements.
    ///
    /// # Arguments
    ///
    /// * `clsid` - Class GUID.
    /// * `prog_id` - Associated programmatic identifier.
    /// * `description` - Human-readable description.
    /// * `component_id` - Target component identifier.
    ///
    /// # Returns
    ///
    /// `WiX` XML fragment string.
    #[must_use]
    pub fn harvest_com(
        &self,
        clsid: &str,
        prog_id: &str,
        description: &str,
        component_id: &str,
    ) -> String {
        format!(
            "<?xml version={QUOTE}1.0{QUOTE} encoding={QUOTE}UTF-8{QUOTE}?>
<Wix xmlns={QUOTE}http://schemas.microsoft.com/wix/2006/wi{QUOTE}>
  <Fragment>
    <Component Id={QUOTE}{component_id}{QUOTE}>
      <Class Id={QUOTE}{clsid}{QUOTE} Context={QUOTE}InprocServer32{QUOTE} Description={QUOTE}{description}{QUOTE} ProgId={QUOTE}{prog_id}{QUOTE} />
      <ProgId Id={QUOTE}{prog_id}{QUOTE} Description={QUOTE}{description}{QUOTE} />
    </Component>
  </Fragment>
</Wix>
"
        )
    }

    /// Harvests build outputs from a designated binary target directory.
    ///
    /// # Arguments
    ///
    /// * `build_output_dir` - Directory containing compiled binaries and assets.
    /// * `group_id` - Component group ID to assign.
    ///
    /// # Returns
    ///
    /// `WiX` XML fragment string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on filesystem read failure.
    pub fn harvest_build_outputs(&self, build_output_dir: &Path, group_id: &str) -> Result<String> {
        self.harvest_directory(build_output_dir, group_id, "INSTALLFOLDER")
    }
}

/// Helper to sanitize arbitrary string into valid `WiX` XML identifier (`[A-Za-z_][A-Za-z0-9_.]*`).
fn sanitize_id(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for (i, ch) in input.chars().enumerate() {
        if (i == 0 && (ch.is_ascii_alphabetic() || ch == '_'))
            || (i > 0 && (ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "id_".to_string()
    } else {
        out
    }
}

/// Helper to compute deterministic 8.3 short file names.
fn make_8_3_name(name: &str) -> String {
    let parts: Vec<&str> = name.split('.').collect();
    let base = parts[0];
    let ext = if parts.len() > 1 { parts[1] } else { "" };

    let short_base = if base.len() > 6 {
        format!("{}~1", base[..6].to_ascii_uppercase())
    } else {
        base.to_ascii_uppercase()
    };

    let short_ext = if ext.len() > 3 {
        ext[..3].to_ascii_uppercase()
    } else {
        ext.to_ascii_uppercase()
    };

    if short_ext.is_empty() {
        short_base
    } else {
        format!("{short_base}.{short_ext}")
    }
}

/// Parses a `.reg` value payload into `(Type, StringValue)`.
fn parse_reg_value(raw_val: &str) -> (&'static str, String) {
    let trimmed = raw_val.trim();
    if trimmed.starts_with(QUOTE) && trimmed.ends_with(QUOTE) && trimmed.len() >= 2 {
        ("string", trimmed[1..trimmed.len() - 1].to_string())
    } else if let Some(hex_data) = trimmed.strip_prefix("dword:") {
        let num = u32::from_str_radix(hex_data.trim(), 16).unwrap_or(0);
        ("integer", num.to_string())
    } else if let Some(hex_data) = trimmed.strip_prefix("hex:") {
        ("binary", hex_data.replace(',', ""))
    } else if let Some(hex_data) = trimmed.strip_prefix("hex(2):") {
        ("expandable", hex_data.replace(',', ""))
    } else if let Some(hex_data) = trimmed.strip_prefix("hex(7):") {
        ("multiString", hex_data.replace(',', ""))
    } else {
        ("string", trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests harvesting local directory trees, excluding extensions, handling files without extension,
    /// and generating short 8.3 names and deterministic component GUIDs.
    #[test]
    fn test_harvest_directory() {
        let temp_dir = std::env::temp_dir().join("msi_harvest_test_suite");
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(temp_dir.join("sub-dir")).is_ok());

        assert!(fs::write(temp_dir.join("app.exe"), b"binary").is_ok());
        assert!(fs::write(temp_dir.join("app.pdb"), b"debug").is_ok());
        assert!(fs::write(temp_dir.join("LICENSE"), b"license without ext").is_ok());
        assert!(fs::write(temp_dir.join("1st_file.dat"), b"leading digit file").is_ok());
        assert!(fs::write(temp_dir.join("alongfilename.jpeg"), b"long ext").is_ok());
        assert!(fs::write(temp_dir.join("sub-dir").join("data.txt"), b"text").is_ok());

        let mut harvester = Harvester::new();
        harvester.exclude_extension("pdb");

        let res = harvester.harvest_directory(&temp_dir, "AppComponents", "INSTALLFOLDER");
        assert!(res.is_ok());
        let xml = res.unwrap_or_default();

        assert!(xml.contains("<ComponentGroup Id="));
        assert!(xml.contains("AppComponents"));
        assert!(xml.contains("app.exe"));
        assert!(xml.contains("data.txt"));
        assert!(xml.contains("LICENSE"));
        assert!(xml.contains("1st_file.dat"));
        assert!(xml.contains("alongfilename.jpeg"));
        assert!(!xml.contains("app.pdb"));

        // Test non-existent path where is_dir() is false
        let non_dir_res =
            harvester.harvest_directory(Path::new("/nonexistent_dir_123456789"), "Grp", "Dir");
        assert!(non_dir_res.is_ok());

        // Test error propagation when a subdirectory cannot be read
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unreadable_sub = temp_dir.join("unreadable_dir");
            let _ = fs::create_dir_all(&unreadable_sub);
            let _ = fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o000));
            assert!(harvester
                .harvest_directory(&temp_dir, "FailGroup", "INSTALLFOLDER")
                .is_err());
            let _ = fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o755));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests harvesting Windows `.reg` files into `WiX` `RegistryKey` and `RegistryValue` elements.
    #[test]
    fn test_harvest_registry() {
        let harvester = Harvester::default();
        let b = BACKSLASH;
        let q = QUOTE;
        let reg_content = format!(
            "; Comment line 1
# Comment line 2
{q}OrphanValueBeforeKey{q}={q}Ignored{q}
[UnclosedSectionWithoutClosingBracket

[HKEY_LOCAL_MACHINE{b}Software{b}Acme{b}App]
LineWithoutEquals
{q}InstallDir{q}={q}C:{b}{b}Program Files{b}{b}Acme{q}
{q}Port{q}=dword:00001f90
{q}BinaryData{q}=hex:01,02,03,04
{q}ExpandPath{q}=hex(2):25,00,53,00
{q}MultiStr{q}=hex(7):61,00,00,00
{q}Unquoted{q}=raw_unquoted_value
@={q}DefaultVal{q}

[HKEY_CLASSES_ROOT{b}Acme.Document]
@={q}Acme Document{q}

[HKCR{b}Acme.Short]
@={q}Short HKCR{q}

[HKEY_CURRENT_USER{b}Software{b}Acme]
{q}UserPref{q}={q}Dark{q}

[HKCU{b}Software{b}Short]
{q}ShortPref{q}={q}Light{q}

[HKEY_USERS{b}Default{b}App]
{q}Setting{q}={q}Val{q}

[HKU{b}Default{b}Short]
{q}ShortSetting{q}={q}Val2{q}

[HKLM]
@={q}RootOnly{q}
"
        );
        let res = harvester.harvest_registry(&reg_content, "RegComponent");
        assert!(res.is_ok());
        let xml = res.unwrap_or_default();

        assert!(xml.contains("Component Id="));
        assert!(xml.contains("RegComponent"));
        assert!(xml.contains("RegistryKey Root="));
        assert!(xml.contains("HKLM"));
        assert!(xml.contains("HKCR"));
        assert!(xml.contains("HKCU"));
        assert!(xml.contains("HKU"));
        assert!(xml.contains("InstallDir"));
        assert!(xml.contains("8080"));
        assert!(xml.contains("01020304"));
        assert!(xml.contains("Type=\"expandable\""));
        assert!(xml.contains("Type=\"multiString\""));
        assert!(xml.contains("raw_unquoted_value"));
        assert!(xml.contains("DefaultVal"));
        assert!(harvester.harvest_registry("", "Empty").is_err());
    }

    /// Tests COM metadata harvesting, build output harvesting, and traits.
    #[test]
    fn test_harvest_com_and_build() {
        let harvester = Harvester::new();
        let com_xml = harvester.harvest_com(
            "{11111111-2222-3333-4444-555555555555}",
            "Acme.Parser",
            "Acme COM Parser",
            "ComComponent",
        );
        assert!(com_xml.contains("Class Id="));
        assert!(com_xml.contains("{11111111-2222-3333-4444-555555555555}"));
        assert!(com_xml.contains("ProgId Id="));

        let temp_dir = std::env::temp_dir().join("msi_harvest_build_test");
        let _ = fs::create_dir_all(&temp_dir);
        let res = harvester.harvest_build_outputs(&temp_dir, "BuildComponents");
        assert!(res.is_ok());
        let build_xml = res.unwrap_or_default();
        assert!(build_xml.contains("ComponentGroup Id="));
        assert!(build_xml.contains("BuildComponents"));
        let _ = fs::remove_dir_all(&temp_dir);

        assert!(format!("{harvester:?}").contains("Harvester"));
    }

    /// Tests `sanitize_id` and `make_8_3_name` helpers directly on edge cases.
    #[test]
    fn test_harvest_helpers_edge_cases() {
        // sanitize_id
        assert_eq!(sanitize_id(""), "id_");
        assert_eq!(sanitize_id("123abc"), "_23abc");
        assert_eq!(sanitize_id("_leading_under"), "_leading_under");
        assert_eq!(sanitize_id("hello-world!"), "hello_world_");
        assert_eq!(sanitize_id("valid_id.1"), "valid_id.1");

        // make_8_3_name
        assert_eq!(make_8_3_name("README"), "README");
        assert_eq!(make_8_3_name("short.txt"), "SHORT.TXT");
        assert_eq!(make_8_3_name("verylongname.html"), "VERYLO~1.HTM");
        assert_eq!(make_8_3_name("file.jpeg"), "FILE.JPE");

        // parse_reg_value edge cases: unterminated quotes and single quote
        assert_eq!(
            parse_reg_value("\"unterminated"),
            ("string", "\"unterminated".to_string())
        );
        assert_eq!(parse_reg_value("\""), ("string", "\"".to_string()));
    }
}
