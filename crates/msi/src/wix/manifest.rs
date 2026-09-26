//! Direct JSON packaging manifest and schema-to-MSI synthesis.
//!
//! Provides parsing and synthesis pipelines for:
//! - Reading `packaging.json` component specifications.
//! - Reading `vars.schema.json` configuration property definitions.
//! - Auto-generating dynamic configuration dialogs (`Dlg_<component>`) and properties (`PROP_<component>_<var>`).
//! - Automatically detecting sensitive schema properties (`*PASSWORD*`, `*SECRET*`, `*KEY*`, `*TOKEN*`)
//!   and configuring them with `Password="yes"` in UI controls and registering them in `MsiHiddenProperties`.
//! - Synthesizing `WiX` XML manifests and binary `.msi` packages without external templating scripts.

use crate::error::{Error, Result};
use crate::wix::toolchain::WixBuildOptions;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Simple, zero-dependency JSON parser AST for packaging manifests and variable schemas.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// Null literal.
    Null,
    /// Boolean literal (`true` / `false`).
    Bool(bool),
    /// Numeric value represented as 64-bit float.
    Number(f64),
    /// UTF-8 string literal.
    String(String),
    /// Array of values.
    Array(Vec<Self>),
    /// Object mapping string keys to values.
    Object(Vec<(String, Self)>),
}

impl JsonValue {
    /// Parses a JSON text string into a [`JsonValue`].
    ///
    /// # Arguments
    ///
    /// * `input` - Input JSON string.
    ///
    /// # Returns
    ///
    /// Parsed [`JsonValue`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on JSON syntax or parsing failure.
    pub fn parse(input: &str) -> Result<Self> {
        let chars: Vec<char> = input.chars().collect();
        let mut idx = 0;
        skip_ws(&chars, &mut idx);
        let val = parse_value(&chars, &mut idx)?;
        skip_ws(&chars, &mut idx);
        if idx < chars.len() {
            return Err(Error::Validation {
                element: "JsonValue".to_string(),
                reason: format!(
                    "unexpected trailing character '{}' at offset {idx}",
                    chars[idx]
                ),
            });
        }
        Ok(val)
    }

    /// Retrieves an object property by key.
    ///
    /// # Arguments
    ///
    /// * `key` - Key name to search.
    ///
    /// # Returns
    ///
    /// Optional reference to the matching [`JsonValue`].
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Object(pairs) => {
                for (k, v) in pairs {
                    if k == key {
                        return Some(v);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Returns string slice if value is [`JsonValue::String`].
    ///
    /// # Returns
    ///
    /// Optional inner string slice.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns array slice if value is [`JsonValue::Array`].
    ///
    /// # Returns
    ///
    /// Optional slice of [`JsonValue`] elements.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    /// Returns object key-value slice if value is [`JsonValue::Object`].
    ///
    /// # Returns
    ///
    /// Optional slice of key-value tuples.
    #[must_use]
    pub fn as_object(&self) -> Option<&[(String, Self)]> {
        match self {
            Self::Object(pairs) => Some(pairs.as_slice()),
            _ => None,
        }
    }
}

/// Skips whitespace characters in a character buffer.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
fn skip_ws(chars: &[char], idx: &mut usize) {
    while *idx < chars.len() && chars[*idx].is_whitespace() {
        *idx += 1;
    }
}

/// Parses any valid JSON value from character buffer.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed [`JsonValue`].
///
/// # Errors
///
/// Returns [`Error::Validation`] on unexpected end of input or invalid character.
fn parse_value(chars: &[char], idx: &mut usize) -> Result<JsonValue> {
    skip_ws(chars, idx);
    if *idx >= chars.len() {
        return Err(Error::Validation {
            element: "JsonValue".to_string(),
            reason: "unexpected end of input".to_string(),
        });
    }

    match chars[*idx] {
        '{' => parse_object(chars, idx),
        '[' => parse_array(chars, idx),
        '"' => parse_string(chars, idx).map(JsonValue::String),
        't' | 'f' => parse_bool(chars, idx),
        'n' => parse_null(chars, idx),
        c if c == '-' || c.is_ascii_digit() => parse_number(chars, idx),
        other => Err(Error::Validation {
            element: "JsonValue".to_string(),
            reason: format!("unexpected character '{other}' at index {idx}"),
        }),
    }
}

/// Parses a JSON object `{ ... }`.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed [`JsonValue::Object`].
///
/// # Errors
///
/// Returns [`Error::Validation`] on invalid object syntax.
fn parse_object(chars: &[char], idx: &mut usize) -> Result<JsonValue> {
    *idx += 1; // skip '{'
    skip_ws(chars, idx);
    let mut pairs = Vec::new();

    if *idx < chars.len() && chars[*idx] == '}' {
        *idx += 1;
        return Ok(JsonValue::Object(pairs));
    }

    loop {
        skip_ws(chars, idx);
        if *idx >= chars.len() || chars[*idx] != '"' {
            return Err(Error::Validation {
                element: "JsonValue::Object".to_string(),
                reason: format!("expected string key at index {idx}"),
            });
        }
        let key = parse_string(chars, idx)?;
        skip_ws(chars, idx);
        if *idx >= chars.len() || chars[*idx] != ':' {
            return Err(Error::Validation {
                element: "JsonValue::Object".to_string(),
                reason: format!("expected ':' after key '{key}' at index {idx}"),
            });
        }
        *idx += 1; // skip ':'
        let val = parse_value(chars, idx)?;
        pairs.push((key, val));
        skip_ws(chars, idx);

        if *idx >= chars.len() {
            return Err(Error::Validation {
                element: "JsonValue::Object".to_string(),
                reason: "unclosed object".to_string(),
            });
        }

        if chars[*idx] == '}' {
            *idx += 1;
            break;
        } else if chars[*idx] == ',' {
            *idx += 1;
        } else {
            return Err(Error::Validation {
                element: "JsonValue::Object".to_string(),
                reason: format!("expected ',' or '}}' at index {idx}"),
            });
        }
    }

    Ok(JsonValue::Object(pairs))
}

/// Parses a JSON array `[ ... ]`.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed [`JsonValue::Array`].
///
/// # Errors
///
/// Returns [`Error::Validation`] on invalid array syntax.
fn parse_array(chars: &[char], idx: &mut usize) -> Result<JsonValue> {
    *idx += 1; // skip '['
    skip_ws(chars, idx);
    let mut items = Vec::new();

    if *idx < chars.len() && chars[*idx] == ']' {
        *idx += 1;
        return Ok(JsonValue::Array(items));
    }

    loop {
        let val = parse_value(chars, idx)?;
        items.push(val);
        skip_ws(chars, idx);

        if *idx >= chars.len() {
            return Err(Error::Validation {
                element: "JsonValue::Array".to_string(),
                reason: "unclosed array".to_string(),
            });
        }

        if chars[*idx] == ']' {
            *idx += 1;
            break;
        } else if chars[*idx] == ',' {
            *idx += 1;
        } else {
            return Err(Error::Validation {
                element: "JsonValue::Array".to_string(),
                reason: format!("expected ',' or ']' at index {idx}"),
            });
        }
    }

    Ok(JsonValue::Array(items))
}

/// Parses a JSON string `"..."`.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed string value.
///
/// # Errors
///
/// Returns [`Error::Validation`] on invalid string or escape syntax.
fn parse_string(chars: &[char], idx: &mut usize) -> Result<String> {
    *idx += 1; // skip '"'
    let mut s = String::new();
    while *idx < chars.len() {
        let ch = chars[*idx];
        *idx += 1;
        if ch == '"' {
            return Ok(s);
        }
        if ch == '\\' {
            if *idx >= chars.len() {
                return Err(Error::Validation {
                    element: "JsonValue::String".to_string(),
                    reason: "unclosed escape sequence".to_string(),
                });
            }
            let esc = chars[*idx];
            *idx += 1;
            match esc {
                '"' => s.push('"'),
                '\\' => s.push('\\'),
                '/' => s.push('/'),
                'b' => s.push('\x08'),
                'f' => s.push('\x0c'),
                'n' => s.push('\n'),
                'r' => s.push('\r'),
                't' => s.push('\t'),
                'u' => {
                    if *idx + 4 > chars.len() {
                        return Err(Error::Validation {
                            element: "JsonValue::String".to_string(),
                            reason: "invalid unicode escape".to_string(),
                        });
                    }
                    let hex_str: String = chars[*idx..*idx + 4].iter().collect();
                    *idx += 4;
                    let cp = u32::from_str_radix(&hex_str, 16).map_err(|e| Error::Validation {
                        element: "JsonValue::String".to_string(),
                        reason: format!("invalid unicode hex '{hex_str}': {e}"),
                    })?;
                    let c = char::from_u32(cp).ok_or_else(|| Error::Validation {
                        element: "JsonValue::String".to_string(),
                        reason: format!("invalid unicode code point '{hex_str}'"),
                    })?;
                    s.push(c);
                }
                other => s.push(other),
            }
        } else {
            s.push(ch);
        }
    }
    Err(Error::Validation {
        element: "JsonValue::String".to_string(),
        reason: "unclosed string literal".to_string(),
    })
}

/// Parses boolean literal.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed [`JsonValue::Bool`].
///
/// # Errors
///
/// Returns [`Error::Validation`] if token is neither `true` nor `false`.
fn parse_bool(chars: &[char], idx: &mut usize) -> Result<JsonValue> {
    if chars[*idx..].starts_with(&['t', 'r', 'u', 'e']) {
        *idx += 4;
        Ok(JsonValue::Bool(true))
    } else if chars[*idx..].starts_with(&['f', 'a', 'l', 's', 'e']) {
        *idx += 5;
        Ok(JsonValue::Bool(false))
    } else {
        Err(Error::Validation {
            element: "JsonValue::Bool".to_string(),
            reason: format!("invalid boolean token at index {idx}"),
        })
    }
}

/// Parses null literal.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed [`JsonValue::Null`].
///
/// # Errors
///
/// Returns [`Error::Validation`] if token is not `null`.
fn parse_null(chars: &[char], idx: &mut usize) -> Result<JsonValue> {
    if chars[*idx..].starts_with(&['n', 'u', 'l', 'l']) {
        *idx += 4;
        Ok(JsonValue::Null)
    } else {
        Err(Error::Validation {
            element: "JsonValue::Null".to_string(),
            reason: format!("invalid null token at index {idx}"),
        })
    }
}

/// Parses number literal.
///
/// # Arguments
///
/// * `chars` - Character slice.
/// * `idx` - Current parsing index.
///
/// # Returns
///
/// Parsed [`JsonValue::Number`].
///
/// # Errors
///
/// Returns [`Error::Validation`] on invalid numeric format.
fn parse_number(chars: &[char], idx: &mut usize) -> Result<JsonValue> {
    let start = *idx;
    if *idx < chars.len() && chars[*idx] == '-' {
        *idx += 1;
    }
    while *idx < chars.len() && chars[*idx].is_ascii_digit() {
        *idx += 1;
    }
    if *idx < chars.len() && chars[*idx] == '.' {
        *idx += 1;
        while *idx < chars.len() && chars[*idx].is_ascii_digit() {
            *idx += 1;
        }
    }
    if *idx < chars.len() && (chars[*idx] == 'e' || chars[*idx] == 'E') {
        *idx += 1;
        if *idx < chars.len() && (chars[*idx] == '+' || chars[*idx] == '-') {
            *idx += 1;
        }
        while *idx < chars.len() && chars[*idx].is_ascii_digit() {
            *idx += 1;
        }
    }
    let num_str: String = chars[start..*idx].iter().collect();
    let num = num_str.parse::<f64>().map_err(|e| Error::Validation {
        element: "JsonValue::Number".to_string(),
        reason: format!("invalid number format '{num_str}': {e}"),
    })?;
    Ok(JsonValue::Number(num))
}

/// Checks if a property or variable name is considered sensitive and requires password masking.
///
/// # Arguments
///
/// * `name` - Variable or property name to evaluate.
///
/// # Returns
///
/// Returns true if name matches `*PASSWORD*`, `*SECRET*`, `*KEY*`, or `*TOKEN*` (case-insensitive).
#[must_use]
pub fn is_sensitive_property_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.contains("PASSWORD")
        || upper.contains("SECRET")
        || upper.contains("KEY")
        || upper.contains("TOKEN")
}

/// Property descriptor synthesized from a JSON schema variable entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaProperty {
    /// Target variable name (e.g. `admin_password` or `port`).
    pub name: String,
    /// Generated MSI property identifier (e.g. `PROP_MYSQL_PORT`).
    pub property_id: String,
    /// Human-readable label or title.
    pub title: String,
    /// Detailed description or tooltip text.
    pub description: String,
    /// Default string value.
    pub default_value: String,
    /// Property data type (e.g. `string`, `integer`, `boolean`).
    pub data_type: String,
    /// Whether this property contains sensitive credentials and must be masked.
    pub is_sensitive: bool,
}

/// Packaging component metadata loaded from `packaging.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagingManifest {
    /// Component or package name (e.g. `mysql`, `openedx`).
    pub name: String,
    /// Human-friendly display title (e.g. `MySQL Database Server`).
    pub title: String,
    /// Version string (e.g. `8.0.36`).
    pub version: String,
    /// Manufacturer or vendor string.
    pub manufacturer: String,
    /// Upgrade code GUID.
    pub upgrade_code: String,
    /// Installation target folder name.
    pub target_folder: String,
    /// Optional executable or custom action command string.
    pub command: Option<String>,
}

impl PackagingManifest {
    /// Parses a `PackagingManifest` from raw JSON text.
    ///
    /// # Arguments
    ///
    /// * `json_text` - Content of `packaging.json`.
    ///
    /// # Returns
    ///
    /// Parsed [`PackagingManifest`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on missing required fields.
    pub fn parse(json_text: &str) -> Result<Self> {
        let json = JsonValue::parse(json_text)?;

        let name = json
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| Error::Validation {
                element: "PackagingManifest".to_string(),
                reason: "missing required 'name' field".to_string(),
            })?
            .to_string();

        let title = json
            .get("title")
            .and_then(JsonValue::as_str)
            .unwrap_or(&name)
            .to_string();

        let version = json
            .get("version")
            .and_then(JsonValue::as_str)
            .unwrap_or("1.0.0")
            .to_string();

        let manufacturer = json
            .get("manufacturer")
            .and_then(JsonValue::as_str)
            .unwrap_or("LibScript")
            .to_string();

        let upgrade_code = json
            .get("upgrade_code")
            .and_then(JsonValue::as_str)
            .unwrap_or("{00000000-0000-0000-0000-000000000000}")
            .to_string();

        let target_folder = json
            .get("target_folder")
            .and_then(JsonValue::as_str)
            .unwrap_or(&name)
            .to_string();

        let command = json
            .get("command")
            .and_then(JsonValue::as_str)
            .map(ToString::to_string);

        Ok(Self {
            name,
            title,
            version,
            manufacturer,
            upgrade_code,
            target_folder,
            command,
        })
    }
}

/// Parses schema properties from a `vars.schema.json` document.
///
/// # Arguments
///
/// * `component_name` - Name of the component used to qualify property identifiers (e.g. `mysql`).
/// * `json_text` - Content of `vars.schema.json`.
///
/// # Returns
///
/// Vector of parsed [`SchemaProperty`] items.
///
/// # Errors
///
/// Returns [`Error::Validation`] on JSON syntax failure.
pub fn parse_vars_schema(component_name: &str, json_text: &str) -> Result<Vec<SchemaProperty>> {
    let json = JsonValue::parse(json_text)?;
    let mut properties = Vec::new();

    let props_obj = json
        .get("properties")
        .and_then(JsonValue::as_object)
        .unwrap_or(&[]);

    for (var_name, var_def) in props_obj {
        let title = var_def
            .get("title")
            .and_then(JsonValue::as_str)
            .unwrap_or(var_name)
            .to_string();

        let desc = var_def
            .get("description")
            .and_then(JsonValue::as_str)
            .unwrap_or("")
            .to_string();

        let data_type = var_def
            .get("type")
            .and_then(JsonValue::as_str)
            .unwrap_or("string")
            .to_string();

        let default_val = match var_def.get("default") {
            Some(JsonValue::String(s)) => s.clone(),
            Some(JsonValue::Number(n)) => {
                if n.fract() == 0.0 {
                    #[allow(clippy::cast_possible_truncation)]
                    let int_val = *n as i64;
                    format!("{int_val}")
                } else {
                    format!("{n}")
                }
            }
            Some(JsonValue::Bool(b)) => {
                if *b {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            _ => String::new(),
        };

        let prop_id = format!(
            "PROP_{}_{}",
            component_name.to_ascii_uppercase(),
            var_name.to_ascii_uppercase()
        );
        let sensitive = is_sensitive_property_name(var_name);

        properties.push(SchemaProperty {
            name: var_name.clone(),
            property_id: prop_id,
            title,
            description: desc,
            default_value: default_val,
            data_type,
            is_sensitive: sensitive,
        });
    }

    Ok(properties)
}

/// Synthesizes a complete `WiX` XML product document from packaging manifest and variable schemas.
///
/// # Arguments
///
/// * `manifest` - Manifest metadata from `packaging.json`.
/// * `properties` - Schema properties parsed from `vars.schema.json`.
/// * `payload_fragment_path` - Path to harvested payload fragment or payload files.
///
/// # Returns
///
/// Well-formed `WiX` XML string ready for compilation.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn synthesize_wix_xml(
    manifest: &PackagingManifest,
    properties: &[SchemaProperty],
    payload_fragment_path: Option<&Path>,
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\">\n");

    let clean_upg = manifest.upgrade_code.trim();
    let upg_guid = if clean_upg.starts_with('{') {
        clean_upg.to_string()
    } else {
        format!("{{{clean_upg}}}")
    };

    let _ = writeln!(xml, "  <Product Id=\"*\" Name=\"{0}\" Language=\"1033\" Version=\"{1}\" Manufacturer=\"{2}\" UpgradeCode=\"{3}\">",
        manifest.title, manifest.version, manifest.manufacturer, upg_guid
    );
    let _ = writeln!(xml, "    <Package Description=\"{0} Setup\" Manufacturer=\"{1}\" Compressed=\"yes\" InstallScope=\"perMachine\" />",
        manifest.title, manifest.manufacturer
    );
    xml.push_str("    <Media Id=\"1\" Cabinet=\"#app.cab\" EmbedCab=\"yes\" />\n");
    xml.push_str("    <MajorUpgrade DowngradeErrorMessage=\"A newer version is already installed.\" Schedule=\"afterInstallInitialize\" />\n\n");

    // Collect sensitive properties for MsiHiddenProperties
    let sensitive_props: Vec<String> = properties
        .iter()
        .filter(|p| p.is_sensitive)
        .map(|p| p.property_id.clone())
        .collect();

    if !sensitive_props.is_empty() {
        let hidden_val = sensitive_props.join(";");
        let _ = writeln!(
            xml,
            "    <Property Id=\"MsiHiddenProperties\" Value=\"{hidden_val}\" Secure=\"yes\" />"
        );
    }

    // Register all schema properties
    for prop in properties {
        let _ = writeln!(
            xml,
            "    <Property Id=\"{0}\" Value=\"{1}\" Secure=\"yes\" />",
            prop.property_id, prop.default_value
        );
    }
    xml.push('\n');

    // Directory Structure
    xml.push_str("    <Directory Id=\"TARGETDIR\" Name=\"SourceDir\">\n");
    xml.push_str("      <Directory Id=\"ProgramFilesFolder\">\n");
    let _ = writeln!(
        xml,
        "        <Directory Id=\"INSTALLFOLDER\" Name=\"{0}\">",
        manifest.target_folder
    );

    if payload_fragment_path.is_none() {
        xml.push_str("          <Component Id=\"DefaultComponent\" Guid=\"{11111111-2222-3333-4444-555555555555}\">\n");
        xml.push_str("            <CreateFolder />\n");
        xml.push_str("          </Component>\n");
    }

    xml.push_str("        </Directory>\n");
    xml.push_str("      </Directory>\n");
    xml.push_str("    </Directory>\n\n");

    // Features
    let _ = writeln!(
        xml,
        "    <Feature Id=\"MainFeature\" Title=\"{0}\" Level=\"1\">",
        manifest.title
    );
    if payload_fragment_path.is_none() {
        xml.push_str("      <ComponentRef Id=\"DefaultComponent\" />\n");
    } else {
        xml.push_str("      <ComponentGroupRef Id=\"HarvestedPayloadComponents\" />\n");
    }
    xml.push_str("    </Feature>\n\n");

    // Dynamic UI Dialog Generation
    let dlg_id = format!("Dlg_{0}", manifest.name);
    xml.push_str("    <UI>\n");
    let _ = writeln!(
        xml,
        "      <Dialog Id=\"{dlg_id}\" Width=\"370\" Height=\"270\" Title=\"{0} Configuration\">",
        manifest.title
    );

    let mut current_y = 30;
    for (i, prop) in properties.iter().enumerate() {
        let label_id = format!("Lbl_{0}_{i}", prop.name);
        let edit_id = format!("Edt_{0}_{i}", prop.name);

        let _ = writeln!(xml, "        <Control Id=\"{label_id}\" Type=\"Text\" X=\"20\" Y=\"{current_y}\" Width=\"120\" Height=\"15\" Text=\"{0}:\" />",
            prop.title
        );

        let password_attr = if prop.is_sensitive {
            " Password=\"yes\""
        } else {
            ""
        };
        let _ = writeln!(xml, "        <Control Id=\"{edit_id}\" Type=\"Edit\" X=\"145\" Y=\"{current_y}\" Width=\"190\" Height=\"18\" Property=\"{0}\"{password_attr} />",
            prop.property_id
        );

        current_y += 28;
    }

    // Dialog navigation buttons
    let next_y = 240;
    let _ = writeln!(xml, "        <Control Id=\"Next\" Type=\"PushButton\" X=\"236\" Y=\"{next_y}\" Width=\"56\" Height=\"17\" Default=\"yes\" Text=\"Next\">"
    );
    xml.push_str("          <Publish Event=\"EndDialog\" Value=\"Return\">1</Publish>\n");
    xml.push_str("        </Control>\n");
    xml.push_str("      </Dialog>\n");
    xml.push_str("    </UI>\n\n");

    xml.push_str("    <InstallUISequence>\n");
    let _ = writeln!(
        xml,
        "      <Show Dialog=\"{dlg_id}\" After=\"CostFinalize\">NOT Installed</Show>"
    );
    xml.push_str("    </InstallUISequence>\n");

    // Optional Custom Action if manifest contains command
    if let Some(ref cmd) = manifest.command {
        xml.push_str("\n    <CustomAction Id=\"RunManifestCommand\" Directory=\"INSTALLFOLDER\" ExeCommand=\"");
        for ch in cmd.chars() {
            match ch {
                '&' => xml.push_str("&amp;"),
                '<' => xml.push_str("&lt;"),
                '>' => xml.push_str("&gt;"),
                '\"' => xml.push_str("&quot;"),
                c => xml.push(c),
            }
        }
        xml.push_str("\" Execute=\"deferred\" Return=\"ignore\" Impersonate=\"no\" />\n");
        xml.push_str("    <InstallExecuteSequence>\n");
        xml.push_str("      <Custom Action=\"RunManifestCommand\" Before=\"InstallFinalize\">NOT Installed</Custom>\n");
        xml.push_str("    </InstallExecuteSequence>\n");
    }

    xml.push_str("  </Product>\n");
    xml.push_str("</Wix>\n");

    xml
}

/// High-level builder that compiles a `packaging.json` and `vars.schema.json` directly into an MSI package.
#[derive(Debug, Clone)]
pub struct ManifestMsiSynthesizer {
    /// Manifest path or content.
    manifest_content: String,
    /// Schema path or content.
    schema_content: Option<String>,
    /// Optional harvested payload fragment path.
    payload_wxs: Option<PathBuf>,
    /// Target architecture (`x64`, `x86`, `arm64`).
    arch: String,
}

impl ManifestMsiSynthesizer {
    /// Creates a new [`ManifestMsiSynthesizer`] from raw JSON content.
    ///
    /// # Arguments
    ///
    /// * `manifest_json` - JSON string of `packaging.json`.
    ///
    /// # Returns
    ///
    /// Newly constructed [`ManifestMsiSynthesizer`].
    #[must_use]
    pub fn new(manifest_json: impl Into<String>) -> Self {
        Self {
            manifest_content: manifest_json.into(),
            schema_content: None,
            payload_wxs: None,
            arch: "x64".to_string(),
        }
    }

    /// Sets the variable JSON schema content.
    ///
    /// # Arguments
    ///
    /// * `schema_json` - JSON string of `vars.schema.json`.
    ///
    /// # Returns
    ///
    /// Updated [`ManifestMsiSynthesizer`].
    #[must_use]
    pub fn with_schema(mut self, schema_json: impl Into<String>) -> Self {
        self.schema_content = Some(schema_json.into());
        self
    }

    /// Sets the harvested payload `.wxs` source fragment path.
    ///
    /// # Arguments
    ///
    /// * `payload_path` - Path to harvested fragment.
    ///
    /// # Returns
    ///
    /// Updated [`ManifestMsiSynthesizer`].
    #[must_use]
    pub fn with_payload_fragment(mut self, payload_path: impl Into<PathBuf>) -> Self {
        self.payload_wxs = Some(payload_path.into());
        self
    }

    /// Sets the target architecture.
    ///
    /// # Arguments
    ///
    /// * `arch` - Target architecture string (e.g. `x64`).
    ///
    /// # Returns
    ///
    /// Updated [`ManifestMsiSynthesizer`].
    #[must_use]
    pub fn with_arch(mut self, arch: impl Into<String>) -> Self {
        self.arch = arch.into();
        self
    }

    /// Synthesizes the `WiX` XML product source string.
    ///
    /// # Returns
    ///
    /// Well-formed `WiX` XML source document.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on JSON manifest or schema parse failure.
    pub fn generate_wix_xml(&self) -> Result<String> {
        let manifest = PackagingManifest::parse(&self.manifest_content)?;
        let properties = if let Some(ref schema) = self.schema_content {
            parse_vars_schema(&manifest.name, schema)?
        } else {
            Vec::new()
        };
        Ok(synthesize_wix_xml(
            &manifest,
            &properties,
            self.payload_wxs.as_deref(),
        ))
    }

    /// Synthesizes and builds the binary `.msi` package at `out_path`.
    ///
    /// # Arguments
    ///
    /// * `out_path` - Target destination `.msi` file path.
    ///
    /// # Returns
    ///
    /// `PathBuf` to generated `.msi` package.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on compilation, linking, or filesystem error.
    pub fn build_msi(&self, out_path: &Path) -> Result<PathBuf> {
        static BUILD_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let xml_str = self.generate_wix_xml()?;
        let count = BUILD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp_dir =
            std::env::temp_dir().join(format!("msi_syn_{}_{}", std::process::id(), count));
        let _ = std::fs::create_dir_all(&temp_dir);

        let main_wxs = temp_dir.join("SynthesizedProduct.wxs");
        std::fs::write(&main_wxs, xml_str.as_bytes())?;

        let mut sources = vec![main_wxs];
        if let Some(ref p) = self.payload_wxs {
            sources.push(p.clone());
        }

        let build_opts = WixBuildOptions {
            sources,
            output: Some(out_path.to_path_buf()),
            arch: Some(self.arch.clone()),
            suppress_ice: true,
            ..WixBuildOptions::new()
        };

        let res = build_opts.execute();
        let _ = std::fs::remove_dir_all(&temp_dir);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::FieldValue;
    use crate::package::Package;

    /// Tests JSON value accessors and fallbacks.
    #[test]
    fn test_json_value_accessors() {
        let val_null = JsonValue::Null;
        assert_eq!(val_null.get("key"), None);
        assert_eq!(val_null.as_str(), None);
        assert_eq!(val_null.as_array(), None);
        assert_eq!(val_null.as_object(), None);

        let val_bool = JsonValue::Bool(true);
        assert_eq!(val_bool.get("key"), None);
        assert_eq!(val_bool.as_str(), None);
        assert_eq!(val_bool.as_array(), None);
        assert_eq!(val_bool.as_object(), None);

        let val_num = JsonValue::Number(42.0);
        assert_eq!(val_num.get("key"), None);

        let val_str = JsonValue::String("hello".to_string());
        assert_eq!(val_str.get("key"), None);
        assert_eq!(val_str.as_str(), Some("hello"));

        let val_arr = JsonValue::Array(vec![JsonValue::Bool(true)]);
        assert_eq!(val_arr.get("key"), None);
        assert_eq!(val_arr.as_array(), Some([JsonValue::Bool(true)].as_slice()));

        let val_obj = JsonValue::Object(vec![(
            "name".to_string(),
            JsonValue::String("val".to_string()),
        )]);
        assert_eq!(val_obj.get("missing"), None);
        assert_eq!(
            val_obj.get("name"),
            Some(&JsonValue::String("val".to_string()))
        );
        assert!(val_obj.as_object().is_some());
    }

    /// Tests JSON parsing edge cases, literals, and all escape sequences.
    #[test]
    fn test_json_parser_comprehensive() {
        // Empty object and array
        assert_eq!(
            JsonValue::parse("{}").as_ref(),
            Ok(&JsonValue::Object(Vec::new()))
        );

        assert_eq!(
            JsonValue::parse("[]").as_ref(),
            Ok(&JsonValue::Array(Vec::new()))
        );

        // All string escapes: \", \\, \/, \b, \f, \n, \r, \t, \u0041, and other char \q
        let escapes_json = r#""\" \\ \/ \b \f \n \r \t \u0041 \q""#;
        assert_eq!(
            JsonValue::parse(escapes_json).as_ref(),
            Ok(&JsonValue::String(
                "\" \\ / \x08 \x0c \n \r \t A q".to_string()
            ))
        );

        // Numbers: negative, float, scientific notations
        assert_eq!(
            JsonValue::parse("-42").as_ref(),
            Ok(&JsonValue::Number(-42.0))
        );
        assert_eq!(
            JsonValue::parse("4.5678").as_ref(),
            Ok(&JsonValue::Number(4.5678))
        );
        assert_eq!(
            JsonValue::parse("-0.5").as_ref(),
            Ok(&JsonValue::Number(-0.5))
        );
        assert_eq!(
            JsonValue::parse("1e2").as_ref(),
            Ok(&JsonValue::Number(100.0))
        );
        assert_eq!(
            JsonValue::parse("1.5e+3").as_ref(),
            Ok(&JsonValue::Number(1500.0))
        );
        assert_eq!(
            JsonValue::parse("2e-2").as_ref(),
            Ok(&JsonValue::Number(0.02))
        );
        assert_eq!(
            JsonValue::parse("4E2").as_ref(),
            Ok(&JsonValue::Number(400.0))
        );
        assert_eq!(
            JsonValue::parse("3.5E+1").as_ref(),
            Ok(&JsonValue::Number(35.0))
        );
        assert_eq!(
            JsonValue::parse("5E-1").as_ref(),
            Ok(&JsonValue::Number(0.5))
        );
        assert_eq!(
            JsonValue::parse("[1e2, 3]").as_ref(),
            Ok(&JsonValue::Array(vec![
                JsonValue::Number(100.0),
                JsonValue::Number(3.0)
            ]))
        );

        // Booleans and null
        assert_eq!(
            JsonValue::parse("true").as_ref(),
            Ok(&JsonValue::Bool(true))
        );
        assert_eq!(
            JsonValue::parse("false").as_ref(),
            Ok(&JsonValue::Bool(false))
        );
        assert_eq!(JsonValue::parse("null").as_ref(), Ok(&JsonValue::Null));

        // Whitespace handling
        let ws_json = "   \t\r\n {   \"a\"  :   [ 1 ,  2 ]  ,  \"b\" : true   }   \n";
        let parsed_ws = JsonValue::parse(ws_json);
        assert!(parsed_ws.as_ref().is_ok_and(|v| v.get("a").is_some()));
        assert_eq!(
            parsed_ws.as_ref().map(|v| v.get("b")),
            Ok(Some(&JsonValue::Bool(true)))
        );
    }

    /// Tests JSON parser error cases across all syntax elements.
    #[test]
    fn test_json_parser_error_cases() {
        assert!(JsonValue::parse("").is_err());
        assert!(JsonValue::parse("   \t  ").is_err());
        assert!(JsonValue::parse("@").is_err());
        assert!(JsonValue::parse(r#""unclosed"#).is_err());
        assert!(JsonValue::parse(r#""escape at end \"#).is_err());
        assert!(JsonValue::parse(r#""\u12""#).is_err());
        assert!(JsonValue::parse(r#""\uZZZZ""#).is_err());
        assert!(JsonValue::parse(r#""\uD800""#).is_err());
        assert!(JsonValue::parse("truth").is_err());
        assert!(JsonValue::parse("fake").is_err());
        assert!(JsonValue::parse("nope").is_err());
        assert!(JsonValue::parse("-").is_err());
        assert!(JsonValue::parse("1e").is_err());
        assert!(JsonValue::parse("1e+").is_err());
        assert!(parse_number(&[], &mut 0).is_err());
        assert!(JsonValue::parse("{").is_err());
        assert!(JsonValue::parse("{\n  ").is_err());
        assert!(JsonValue::parse(r#"{"key""#).is_err());
        assert!(JsonValue::parse("{123: 1}").is_err());
        assert!(JsonValue::parse(r#"{"key" 1}"#).is_err());
        assert!(JsonValue::parse(r#"{"key": 1"#).is_err());
        assert!(JsonValue::parse(r#"{"key": 1 "key2": 2}"#).is_err());
        assert!(JsonValue::parse("[").is_err());
        assert!(JsonValue::parse("[\n  ").is_err());
        assert!(JsonValue::parse("[1, 2").is_err());
        assert!(JsonValue::parse("[1 2]").is_err());
        assert!(JsonValue::parse(r#"{"a": 1} trailing"#).is_err());
        // String parse error inside object key
        assert!(JsonValue::parse(r#"{"\uZZZZ": 1}"#).is_err());
        // Value parse error inside object value
        assert!(JsonValue::parse(r#"{"key": @}"#).is_err());
    }

    /// Tests sensitive property detection.
    #[test]
    fn test_sensitive_property_detection() {
        assert!(is_sensitive_property_name("admin_password"));
        assert!(is_sensitive_property_name("PROP_MYSQL_SECRET"));
        assert!(is_sensitive_property_name("api_key"));
        assert!(is_sensitive_property_name("auth_token"));
        assert!(!is_sensitive_property_name("admin_user"));
        assert!(!is_sensitive_property_name("port"));
        assert!(!is_sensitive_property_name("host"));
    }

    /// Tests packaging manifest parsing with defaults and errors.
    #[test]
    fn test_packaging_manifest_parsing() {
        // Missing required 'name' field
        assert!(PackagingManifest::parse("{}").is_err());
        assert!(PackagingManifest::parse("not json").is_err());

        // Minimal manifest with defaults
        let minimal_json = r#"{"name": "minimalapp"}"#;
        assert_eq!(
            PackagingManifest::parse(minimal_json).as_ref(),
            Ok(&PackagingManifest {
                name: "minimalapp".to_string(),
                title: "minimalapp".to_string(),
                version: "1.0.0".to_string(),
                manufacturer: "LibScript".to_string(),
                upgrade_code: "{00000000-0000-0000-0000-000000000000}".to_string(),
                target_folder: "minimalapp".to_string(),
                command: None,
            })
        );

        // Full manifest
        let full_json = r#"{
            "name": "fullapp",
            "title": "Full Application",
            "version": "2.0.0",
            "manufacturer": "Acme Corp",
            "upgrade_code": "{11111111-2222-3333-4444-555555555555}",
            "target_folder": "AcmeApp",
            "command": "cmd.exe /c start & run < in > out \"quoted\""
        }"#;
        assert_eq!(
            PackagingManifest::parse(full_json).as_ref(),
            Ok(&PackagingManifest {
                name: "fullapp".to_string(),
                title: "Full Application".to_string(),
                version: "2.0.0".to_string(),
                manufacturer: "Acme Corp".to_string(),
                upgrade_code: "{11111111-2222-3333-4444-555555555555}".to_string(),
                target_folder: "AcmeApp".to_string(),
                command: Some("cmd.exe /c start & run < in > out \"quoted\"".to_string()),
            })
        );
    }

    /// Tests variables schema parsing across all supported data types and defaults.
    #[test]
    fn test_parse_vars_schema_types() {
        // Empty schema
        assert_eq!(
            parse_vars_schema("test", "{}").as_ref(),
            Ok(&Vec::<SchemaProperty>::new())
        );
        assert!(parse_vars_schema("test", "not json").is_err());

        let schema_json = r#"{
            "properties": {
                "port": {
                    "title": "Port Number",
                    "description": "Network port",
                    "type": "integer",
                    "default": 8080.0
                },
                "ratio": {
                    "type": "number",
                    "default": 0.75
                },
                "debug_enabled": {
                    "type": "boolean",
                    "default": true
                },
                "ssl_enabled": {
                    "type": "boolean",
                    "default": false
                },
                "host_name": {
                    "type": "string",
                    "default": "localhost"
                },
                "null_prop": {
                    "default": null
                },
                "user_password": {
                    "type": "string",
                    "default": "hunter2"
                }
            }
        }"#;

        let props_res = parse_vars_schema("myapp", schema_json);
        assert_eq!(props_res.as_ref().map(Vec::len), Ok(7));

        assert_eq!(
            props_res.as_ref().map(|props| {
                props
                    .iter()
                    .find(|p| p.name == "port")
                    .map(|p| (p.default_value.as_str(), p.is_sensitive))
            }),
            Ok(Some(("8080", false)))
        );

        assert_eq!(
            props_res.as_ref().map(|props| {
                props
                    .iter()
                    .find(|p| p.name == "ratio")
                    .map(|p| (p.default_value.as_str(), p.title.as_str()))
            }),
            Ok(Some(("0.75", "ratio")))
        );

        assert_eq!(
            props_res.as_ref().map(|props| {
                props
                    .iter()
                    .find(|p| p.name == "debug_enabled")
                    .map(|p| p.default_value.as_str())
            }),
            Ok(Some("1"))
        );

        assert_eq!(
            props_res.as_ref().map(|props| {
                props
                    .iter()
                    .find(|p| p.name == "ssl_enabled")
                    .map(|p| p.default_value.as_str())
            }),
            Ok(Some("0"))
        );

        assert_eq!(
            props_res.as_ref().map(|props| {
                props
                    .iter()
                    .find(|p| p.name == "null_prop")
                    .map(|p| p.default_value.as_str())
            }),
            Ok(Some(""))
        );

        assert_eq!(
            props_res.as_ref().map(|props| {
                props
                    .iter()
                    .find(|p| p.name == "user_password")
                    .map(|p| (p.default_value.as_str(), p.is_sensitive))
            }),
            Ok(Some(("hunter2", true)))
        );
    }

    /// Tests `WiX` XML synthesis permutations (sensitive properties present/absent, payload fragment present/absent).
    #[test]
    fn test_synthesize_wix_xml_permutations() {
        let manifest_no_cmd = PackagingManifest {
            name: "simple".to_string(),
            title: "Simple App".to_string(),
            version: "1.0.0".to_string(),
            manufacturer: "LibScript".to_string(),
            upgrade_code: "12345678-1234-1234-1234-123456789abc".to_string(),
            target_folder: "Simple".to_string(),
            command: None,
        };

        let non_sensitive_props = vec![SchemaProperty {
            name: "port".to_string(),
            property_id: "PROP_SIMPLE_PORT".to_string(),
            title: "Port".to_string(),
            description: "Listen port".to_string(),
            default_value: "80".to_string(),
            data_type: "integer".to_string(),
            is_sensitive: false,
        }];

        // Without sensitive props and with payload fragment
        let payload_path = Path::new("fragment.wxs");
        let xml_payload =
            synthesize_wix_xml(&manifest_no_cmd, &non_sensitive_props, Some(payload_path));

        assert!(!xml_payload.contains("MsiHiddenProperties"));
        assert!(xml_payload.contains("ComponentGroupRef Id=\"HarvestedPayloadComponents\""));
        assert!(!xml_payload.contains("Component Id=\"DefaultComponent\""));
        assert!(!xml_payload.contains("CustomAction Id=\"RunManifestCommand\""));
        // UpgradeCode should have braces added
        assert!(xml_payload.contains("UpgradeCode=\"{12345678-1234-1234-1234-123456789abc}\""));

        // Without payload fragment (emits DefaultComponent)
        let xml_no_payload = synthesize_wix_xml(&manifest_no_cmd, &non_sensitive_props, None);
        assert!(xml_no_payload.contains("Component Id=\"DefaultComponent\""));
        assert!(xml_no_payload.contains("ComponentRef Id=\"DefaultComponent\""));

        // With command containing XML special characters (&, <, >, ")
        let manifest_xml_cmd = PackagingManifest {
            name: "cmdapp".to_string(),
            title: "Command App".to_string(),
            version: "1.0.0".to_string(),
            manufacturer: "LibScript".to_string(),
            upgrade_code: "{12345678-1234-1234-1234-123456789abc}".to_string(),
            target_folder: "CmdApp".to_string(),
            command: Some("cmd.exe /c \"test & <redirect> \"\"quoted\"\"\"".to_string()),
        };
        let xml_cmd = synthesize_wix_xml(&manifest_xml_cmd, &[], None);
        assert!(xml_cmd.contains("&amp;"));
        assert!(xml_cmd.contains("&lt;"));
        assert!(xml_cmd.contains("&gt;"));
        assert!(xml_cmd.contains("&quot;"));
    }

    /// Tests `ManifestMsiSynthesizer` without schema and with payload fragment in MSI build.
    #[test]
    fn test_manifest_msi_synthesizer_builder_and_payload_wxs() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_syn_payload_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let pkg_json = r#"{
            "name": "payloadapp",
            "title": "Payload App",
            "version": "1.0.0",
            "manufacturer": "Acme",
            "upgrade_code": "{99999999-9999-9999-9999-999999999999}",
            "target_folder": "PayloadApp"
        }"#;

        // Build without schema
        let synth_no_schema = ManifestMsiSynthesizer::new(pkg_json);
        assert_eq!(
            synth_no_schema
                .generate_wix_xml()
                .as_ref()
                .map(|xml| xml.contains("Product Id=\"*\" Name=\"Payload App\"")),
            Ok(true)
        );

        // Build with payload fragment file
        let frag_wxs = temp_dir.join("PayloadFragment.wxs");
        let frag_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <ComponentGroup Id="HarvestedPayloadComponents">
      <Component Id="FragmentComponent" Directory="INSTALLFOLDER" Guid="{44444444-5555-6666-7777-888888888888}">
        <CreateFolder />
      </Component>
    </ComponentGroup>
  </Fragment>
</Wix>
"#;
        assert!(std::fs::write(&frag_wxs, frag_content.as_bytes()).is_ok());

        let synth_with_frag = ManifestMsiSynthesizer::new(pkg_json)
            .with_payload_fragment(&frag_wxs)
            .with_arch("x86");

        let msi_out = temp_dir.join("payloadapp.msi");
        assert_eq!(synth_with_frag.build_msi(&msi_out).as_ref(), Ok(&msi_out));
        assert!(msi_out.exists());

        assert_eq!(
            Package::open(&msi_out)
                .as_ref()
                .map(|pkg| pkg.metadata().product_name()),
            Ok("Payload App")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests full end-to-end synthesis and MSI compilation from packaging and schema JSON.
    #[test]
    fn test_manifest_msi_synthesizer_e2e() {
        let temp_dir = std::env::temp_dir().join(format!("msi_syn_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let pkg_json = r#"{
            "name": "redis",
            "title": "Redis Cache Server",
            "version": "7.2.4",
            "manufacturer": "LibScript Team",
            "upgrade_code": "88888888-4444-4444-4444-123456789abc",
            "target_folder": "RedisStack",
            "command": "cmd.exe /c start redis-server.exe"
        }"#;

        let schema_json = r#"{
            "properties": {
                "port": {
                    "title": "Listening Port",
                    "type": "integer",
                    "default": 6379
                },
                "auth_password": {
                    "title": "Authentication Password",
                    "type": "string",
                    "default": "SecretRedisPass123"
                },
                "protected_mode": {
                    "title": "Enable Protected Mode",
                    "type": "boolean",
                    "default": true
                }
            }
        }"#;

        let synth = ManifestMsiSynthesizer::new(pkg_json)
            .with_schema(schema_json)
            .with_arch("x64");

        let xml_res = synth.generate_wix_xml();
        assert_eq!(
            xml_res.as_ref().map(|xml| xml.contains("Dlg_redis")),
            Ok(true)
        );
        assert_eq!(
            xml_res.as_ref().map(|xml| xml.contains("PROP_REDIS_PORT")),
            Ok(true)
        );
        assert_eq!(
            xml_res
                .as_ref()
                .map(|xml| xml.contains("PROP_REDIS_AUTH_PASSWORD")),
            Ok(true)
        );
        assert_eq!(
            xml_res
                .as_ref()
                .map(|xml| xml.contains("PROP_REDIS_PROTECTED_MODE")),
            Ok(true)
        );
        assert_eq!(
            xml_res
                .as_ref()
                .map(|xml| xml.contains(r#"Password="yes""#)),
            Ok(true)
        );
        assert_eq!(
            xml_res
                .as_ref()
                .map(|xml| xml.contains("MsiHiddenProperties")),
            Ok(true)
        );

        let msi_out = temp_dir.join("redis.msi");
        assert_eq!(synth.build_msi(&msi_out).as_ref(), Ok(&msi_out));
        assert!(msi_out.exists());

        let pkg_res = Package::open(&msi_out);
        assert_eq!(
            pkg_res.as_ref().map(|pkg| pkg.metadata().product_name()),
            Ok("Redis Cache Server")
        );

        let pkg_verified = pkg_res.as_ref().map(|pkg| {
            let db = pkg.database();
            let prop_records = db.get_records("Property");
            let port_found = prop_records.iter().any(|r| {
                r.get(0) == Some(&FieldValue::String("PROP_REDIS_PORT".to_string()))
                    && r.get(1) == Some(&FieldValue::String("6379".to_string()))
            });
            let hidden_prop = prop_records
                .iter()
                .find(|r| r.get(0) == Some(&FieldValue::String("MsiHiddenProperties".to_string())));
            let hidden_val = hidden_prop.and_then(|r| r.get(1))
                == Some(&FieldValue::String("PROP_REDIS_AUTH_PASSWORD".to_string()));
            let dlg_records = db.get_records("Dialog");
            let dlg_found = dlg_records
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String("Dlg_redis".to_string())));
            (port_found, hidden_val, dlg_found)
        });
        assert_eq!(pkg_verified, Ok((true, true, true)));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests error propagation in `ManifestMsiSynthesizer` when manifest or schema fails.
    #[test]
    fn test_manifest_synthesizer_error_paths() {
        // Invalid manifest JSON in generate_wix_xml (Line 936)
        let synth_bad_manifest = ManifestMsiSynthesizer::new("not json");
        assert!(synth_bad_manifest.generate_wix_xml().is_err());

        // Invalid schema JSON in generate_wix_xml (Line 938)
        let valid_pkg = r#"{"name": "testapp"}"#;
        let synth_bad_schema = ManifestMsiSynthesizer::new(valid_pkg).with_schema("not json");
        assert!(synth_bad_schema.generate_wix_xml().is_err());

        // Invalid manifest in build_msi (Line 964)
        assert!(synth_bad_manifest.build_msi(Path::new("out.msi")).is_err());
    }
}
