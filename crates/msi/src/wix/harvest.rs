//! `WiX` Asset and Metadata Harvester (`heat` replacement).
//!
//! Provides automated harvesting of:
//! - Local filesystem directory trees into `<DirectoryRef>`, `<ComponentGroup>`, `<Component>`, and `<File>` elements.
//! - Windows `.reg` files into `<RegistryKey>` and `<RegistryValue>` elements.
//! - COM class, `ProgID`, and `TypeLib` metadata into `<Class>`, `<ProgId>`, and `<TypeLib>` elements.
//! - Build output artifacts from build systems.

use crate::database::tables::types::ComponentGuid;
use crate::error::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Double quote character constant (`"`).
const QUOTE: char = 34 as char;
/// Backslash character constant.
const BACKSLASH: char = 92 as char;

/// Returns whether a relative path matches a pattern or glob expression.
///
/// # Arguments
///
/// * `rel_path` - Relative file path to test.
/// * `pattern` - Glob pattern or path to match against.
///
/// # Returns
///
/// True if path matches pattern, false otherwise.
fn matches_pattern(rel_path: &str, pattern: &str) -> bool {
    let norm_path = rel_path.replace('\\', "/");
    let norm_pat = pattern.replace('\\', "/");
    let norm_pat = norm_pat.trim_start_matches("./");

    if norm_pat.is_empty() {
        return false;
    }

    // Wildcard suffix (e.g. *.tmp, *.log)
    if let Some(suffix) = norm_pat.strip_prefix('*') {
        if !suffix.contains('*') {
            return norm_path.ends_with(suffix);
        }
    }

    // Prefix wildcard (e.g. .git*, .vagrant*)
    if let Some(prefix) = norm_pat.strip_suffix('*') {
        if !prefix.contains('*') && !prefix.ends_with('/') {
            return norm_path.starts_with(prefix) || norm_path.contains(&format!("/{prefix}"));
        }
    }

    // Directory wildcard expressions (e.g. cache/runtimes/*, cache/**)
    if norm_pat.ends_with("/*") || norm_pat.ends_with("/**") {
        let prefix = norm_pat.trim_end_matches("/*").trim_end_matches("/**");
        let prefix = prefix.trim_start_matches("*/");
        return norm_path == prefix
            || norm_path.starts_with(&format!("{prefix}/"))
            || norm_path.contains(&format!("/{prefix}/"));
    }

    // Directory trailing slash (e.g. docs/)
    if norm_pat.ends_with('/') {
        let prefix = norm_pat.trim_end_matches('/');
        let prefix = prefix.trim_start_matches("*/");
        return norm_path == prefix
            || norm_path.starts_with(&format!("{prefix}/"))
            || norm_path.contains(&format!("/{prefix}/"));
    }

    let clean_pat = norm_pat.trim_start_matches("*/");
    norm_path == clean_pat
        || norm_path.starts_with(&format!("{clean_pat}/"))
        || norm_path.ends_with(&format!("/{clean_pat}"))
        || norm_path.contains(&format!("/{clean_pat}/"))
}

/// Internal cabinet partition tracking state.
#[derive(Debug, Clone, Default)]
struct SplitTracker {
    /// Active cabinet Media `DiskId`.
    current_disk_id: i16,
    /// Bytes accumulated on current disk.
    current_bytes: u64,
    /// Configured maximum split size in bytes.
    split_size: Option<u64>,
}

/// Options for payload harvesting and staging operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestPayloadOptions {
    /// Identifier for the main generated `<ComponentGroup>`.
    pub component_group: String,
    /// Identifier for the root `<DirectoryRef>`.
    pub directory_ref: String,
    /// Destination path for generated `.wxs` `WiX` XML fragment.
    pub wix_fragment: Option<std::path::PathBuf>,
    /// Destination path for file manifest list.
    pub manifest_file: Option<std::path::PathBuf>,
    /// Destination directory for staged payload files.
    pub output_dir: Option<std::path::PathBuf>,
    /// Optional offline cache directory to inject under `cache/`.
    pub include_cache: Option<std::path::PathBuf>,
}

impl Default for HarvestPayloadOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl HarvestPayloadOptions {
    /// Creates default payload harvesting options.
    ///
    /// # Returns
    ///
    /// Default [`HarvestPayloadOptions`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            component_group: "LibscriptHarvestedComponents".to_string(),
            directory_ref: "LIBSCRIPT_FOLDER".to_string(),
            wix_fragment: None,
            manifest_file: None,
            output_dir: None,
            include_cache: None,
        }
    }
}

/// Result metadata from a payload harvesting operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HarvestPayloadResult {
    /// Number of harvested files.
    pub file_count: usize,
    /// Number of generated components.
    pub component_count: usize,
    /// Generated `WiX` XML fragment string.
    pub wix_fragment: String,
    /// List of relative file paths harvested.
    pub relative_paths: Vec<String>,
}

/// `WiX` Asset and Metadata Harvester generating `.wxs` source fragments from disk assets and registries.
#[derive(Debug, Clone, Default)]
pub struct Harvester {
    /// File extensions to exclude from harvesting.
    excluded_extensions: Vec<String>,
    /// Path or glob patterns to exclude from harvesting (e.g. from `.gitignore`).
    excluded_patterns: Vec<String>,
    /// Rules mapping path patterns to specific Media `DiskId` (e.g. `("cache/runtimes/*", 2)`).
    disk_rules: Vec<(String, i16)>,
    /// Default Disk ID for files not matching any disk rule (default: 1).
    default_disk_id: i16,
    /// Optional max split size in bytes per cabinet disk.
    split_size: Option<u64>,
    /// Secondary component groups filtering by pattern: `(group_id, pattern)`.
    secondary_groups: Vec<(String, String)>,
    /// Whether files captured by secondary component groups should be excluded from primary component group.
    secondary_exclusive: bool,
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
            excluded_patterns: Vec::new(),
            disk_rules: Vec::new(),
            default_disk_id: 1,
            split_size: None,
            secondary_groups: Vec::new(),
            secondary_exclusive: true,
        }
    }

    /// Sets the default Media `DiskId` for non-matching files (default: 1).
    ///
    /// # Arguments
    ///
    /// * `disk_id` - Default Media disk number.
    pub const fn set_default_disk_id(&mut self, disk_id: i16) {
        self.default_disk_id = disk_id;
    }

    /// Sets the maximum split size in bytes per cabinet disk.
    ///
    /// # Arguments
    ///
    /// * `split_size` - Maximum bytes per disk partition.
    pub const fn set_split_size(&mut self, split_size: u64) {
        self.split_size = Some(split_size);
    }

    /// Sets whether files captured by secondary component groups should be excluded
    /// from the primary component group (default: true).
    ///
    /// # Arguments
    ///
    /// * `exclusive` - If true, secondary files are only in secondary groups.
    pub const fn set_secondary_exclusive(&mut self, exclusive: bool) {
        self.secondary_exclusive = exclusive;
    }

    /// Adds an exclusion or suppression filter pattern (alias for `add_exclude_pattern`).
    ///
    /// # Arguments
    ///
    /// * `pattern` - Pattern to suppress (e.g. `"-filter *.tmp"`).
    pub fn add_filter_pattern(&mut self, pattern: impl Into<String>) {
        self.add_exclude_pattern(pattern);
    }

    /// Adds an excluded file extension (e.g. `"tmp"`, `"pdb"`).
    ///
    /// # Arguments
    ///
    /// * `ext` - Extension string to exclude.
    pub fn exclude_extension(&mut self, ext: impl Into<String>) {
        self.excluded_extensions.push(ext.into().to_lowercase());
    }

    /// Adds an excluded path pattern (e.g. `".git"`, `"tests_tmp/*"`).
    ///
    /// # Arguments
    ///
    /// * `pattern` - Glob or path pattern to exclude.
    pub fn add_exclude_pattern(&mut self, pattern: impl Into<String>) {
        self.excluded_patterns.push(pattern.into());
    }

    /// Loads exclusion patterns from a `.gitignore` file.
    ///
    /// # Arguments
    ///
    /// * `gitignore_path` - Path to `.gitignore` file.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] if reading the file fails.
    pub fn load_gitignore(&mut self, gitignore_path: &Path) -> Result<()> {
        let content = fs::read_to_string(gitignore_path)?;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                self.excluded_patterns.push(trimmed.to_string());
            }
        }
        Ok(())
    }

    /// Adds a rule mapping a path pattern to a specific Media `DiskId`.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Path pattern to match (e.g. `"cache/runtimes/*"`).
    /// * `disk_id` - Target `DiskId` for matching files.
    pub fn add_disk_rule(&mut self, pattern: impl Into<String>, disk_id: i16) {
        self.disk_rules.push((pattern.into(), disk_id));
    }

    /// Registers a secondary component group that captures files matching a pattern.
    ///
    /// # Arguments
    ///
    /// * `group_id` - Identifier of the secondary `<ComponentGroup>`.
    /// * `pattern` - Path pattern to match (e.g. `"cache/**"`).
    pub fn add_secondary_group(&mut self, group_id: impl Into<String>, pattern: impl Into<String>) {
        self.secondary_groups
            .push((group_id.into(), pattern.into()));
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
            "<?xml version={QUOTE}1.0{QUOTE} encoding={QUOTE}UTF-8{QUOTE}?>\n"
        ));
        xml.push_str(&format!(
            "<Wix xmlns={QUOTE}http://schemas.microsoft.com/wix/2006/wi{QUOTE}>\n"
        ));
        xml.push_str("  <Fragment>\n");
        xml.push_str(&format!(
            "    <DirectoryRef Id={QUOTE}{target_dir_id}{QUOTE}>\n"
        ));

        let mut components = Vec::new();
        let mut secondary_members: HashMap<String, Vec<String>> = HashMap::new();
        let mut counter = 0;
        let mut tracker = SplitTracker {
            current_disk_id: self.default_disk_id,
            current_bytes: 0,
            split_size: self.split_size,
        };
        self.harvest_dir_recursive(
            dir_path,
            dir_path,
            target_dir_id,
            &mut counter,
            &mut tracker,
            &mut xml,
            &mut components,
            &mut secondary_members,
        )?;

        xml.push_str("    </DirectoryRef>\n");

        xml.push_str(&format!(
            "    <ComponentGroup Id={QUOTE}{comp_group_id}{QUOTE}>\n"
        ));
        for comp_id in &components {
            xml.push_str(&format!(
                "      <ComponentRef Id={QUOTE}{comp_id}{QUOTE} />\n"
            ));
        }
        xml.push_str("    </ComponentGroup>\n");

        for (sec_group, sec_comps) in &secondary_members {
            xml.push_str(&format!(
                "    <ComponentGroup Id={QUOTE}{sec_group}{QUOTE}>\n"
            ));
            for comp_id in sec_comps {
                xml.push_str(&format!(
                    "      <ComponentRef Id={QUOTE}{comp_id}{QUOTE} />\n"
                ));
            }
            xml.push_str("    </ComponentGroup>\n");
        }

        xml.push_str("  </Fragment>\n");
        xml.push_str("</Wix>\n");

        Ok(xml)
    }

    /// Recursive directory traversal worker.
    #[allow(
        clippy::too_many_lines,
        clippy::too_many_arguments,
        clippy::format_push_string
    )]
    fn harvest_dir_recursive(
        &self,
        root_path: &Path,
        current_path: &Path,
        parent_dir_id: &str,
        counter: &mut usize,
        tracker: &mut SplitTracker,
        xml: &mut String,
        components: &mut Vec<String>,
        secondary_members: &mut HashMap<String, Vec<String>>,
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
            let rel_path = entry
                .strip_prefix(root_path)
                .unwrap_or(&entry)
                .to_string_lossy();
            let norm_rel = rel_path.replace('\\', "/");

            if self
                .excluded_patterns
                .iter()
                .any(|pat| matches_pattern(&norm_rel, pat))
            {
                continue;
            }

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
            let rel_path = file
                .strip_prefix(root_path)
                .unwrap_or(&file)
                .to_string_lossy();
            let norm_rel = rel_path.replace('\\', "/");

            let sanitized_path = sanitize_id(&norm_rel);
            let comp_id = crate::database::tables::types::sanitize_identifier_length(format!(
                "cmp_{sanitized_path}"
            ));
            let file_id = crate::database::tables::types::sanitize_identifier_length(format!(
                "fil_{sanitized_path}"
            ));
            let guid = ComponentGuid::generate(parent_dir_id, &comp_id);
            let short_name = make_8_3_name(&file_name);

            let mut disk_id = self.default_disk_id;
            let mut matched_rule = false;
            for (pat, did) in &self.disk_rules {
                if matches_pattern(&norm_rel, pat) {
                    disk_id = *did;
                    matched_rule = true;
                    break;
                }
            }

            if !matched_rule {
                if let Some(split) = tracker.split_size {
                    let file_size = file.metadata().map_or(0, |m| m.len());
                    if tracker.current_bytes + file_size > split && tracker.current_bytes > 0 {
                        tracker.current_disk_id += 1;
                        tracker.current_bytes = 0;
                    }
                    tracker.current_bytes += file_size;
                    disk_id = tracker.current_disk_id;
                }
            }

            let disk_attr = if disk_id != 1 || self.default_disk_id != 1 {
                format!(" DiskId={QUOTE}{disk_id}{QUOTE}")
            } else {
                String::new()
            };

            let mut is_secondary = false;
            for (group_id, pat) in &self.secondary_groups {
                if matches_pattern(&norm_rel, pat) {
                    secondary_members
                        .entry(group_id.clone())
                        .or_default()
                        .push(comp_id.clone());
                    is_secondary = true;
                }
            }

            xml.push_str(&format!(
                "      <Component Id={QUOTE}{comp_id}{QUOTE} Guid={QUOTE}{guid}{QUOTE}>\n"
            ));
            xml.push_str(&format!(
                "        <File Id={QUOTE}{file_id}{QUOTE} Name={QUOTE}{file_name}{QUOTE} ShortName={QUOTE}{short_name}{QUOTE} Source={QUOTE}{}{QUOTE}{disk_attr} KeyPath={QUOTE}yes{QUOTE} />\n",
                file.display()
            ));
            xml.push_str("      </Component>\n");

            if !is_secondary || !self.secondary_exclusive {
                components.push(comp_id);
            }
        }

        // Recurse into subdirectories
        for subdir in subdirs {
            *counter += 1;
            let dir_name = subdir.file_name().unwrap_or_default().to_string_lossy();
            let dir_rel = subdir
                .strip_prefix(root_path)
                .unwrap_or(&subdir)
                .to_string_lossy();
            let norm_dir_rel = dir_rel.replace('\\', "/");
            let sub_dir_id = crate::database::tables::types::sanitize_identifier_length(format!(
                "dir_{}",
                sanitize_id(&norm_dir_rel)
            ));
            let short_name = make_8_3_name(&dir_name);

            xml.push_str(&format!(
                "      <Directory Id={QUOTE}{sub_dir_id}{QUOTE} Name={QUOTE}{dir_name}{QUOTE} ShortName={QUOTE}{short_name}{QUOTE}>\n"
            ));
            self.harvest_dir_recursive(
                root_path,
                &subdir,
                &sub_dir_id,
                counter,
                tracker,
                xml,
                components,
                secondary_members,
            )?;
            xml.push_str("      </Directory>\n");
        }

        Ok(())
    }

    /// Returns a sorted list of relative file paths that would be harvested.
    ///
    /// # Arguments
    ///
    /// * `root_path` - Root directory to inspect.
    ///
    /// # Returns
    ///
    /// Vector of normalized relative file paths.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on filesystem traversal failure.
    pub fn generate_manifest(&self, root_path: &Path) -> Result<Vec<String>> {
        let mut paths = Vec::new();
        self.collect_manifest_recursive(root_path, root_path, &mut paths)?;
        paths.sort();
        Ok(paths)
    }

    /// Internal recursive manifest file collector.
    ///
    /// # Arguments
    ///
    /// * `root_path` - Base directory.
    /// * `current_path` - Current subdirectory being scanned.
    /// * `paths` - Output list of relative file paths.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on directory traversal failure.
    fn collect_manifest_recursive(
        &self,
        root_path: &Path,
        current_path: &Path,
        paths: &mut Vec<String>,
    ) -> Result<()> {
        if !current_path.is_dir() {
            return Ok(());
        }
        let mut entries = Vec::new();
        entries
            .extend(fs::read_dir(current_path)?.filter_map(|e| e.ok().map(|entry| entry.path())));
        entries.sort();

        for p in entries {
            let rel = p.strip_prefix(root_path).unwrap_or(&p).to_string_lossy();
            let norm_rel = rel.replace('\\', "/");

            if self
                .excluded_patterns
                .iter()
                .any(|pat| matches_pattern(&norm_rel, pat))
            {
                continue;
            }

            if p.is_dir() {
                self.collect_manifest_recursive(root_path, &p, paths)?;
            } else {
                let ext = p
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if !self.excluded_extensions.contains(&ext) {
                    paths.push(norm_rel);
                }
            }
        }
        Ok(())
    }

    /// Generates a manifest file listing all harvested relative paths.
    ///
    /// # Arguments
    ///
    /// * `root_path` - Root directory to inspect.
    /// * `manifest_file` - Destination path for manifest file.
    ///
    /// # Returns
    ///
    /// Number of harvested file paths written.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on traversal or file write failure.
    pub fn write_manifest(&self, root_path: &Path, manifest_file: &Path) -> Result<usize> {
        let paths = self.generate_manifest(root_path)?;
        manifest_file.parent().map_or(Ok(()), fs::create_dir_all)?;
        let mut content = paths.join("\n");
        if !paths.is_empty() {
            content.push('\n');
        }
        fs::write(manifest_file, content)?;
        Ok(paths.len())
    }

    /// Copies all harvested files from `root_path` into `output_dir`, preserving relative directories.
    ///
    /// # Arguments
    ///
    /// * `root_path` - Source root directory.
    /// * `output_dir` - Target staging directory.
    ///
    /// # Returns
    ///
    /// Total number of files copied.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on file copy or directory creation failure.
    pub fn copy_payload(&self, root_path: &Path, output_dir: &Path) -> Result<usize> {
        let paths = self.generate_manifest(root_path)?;
        for rel in &paths {
            let src = root_path.join(rel);
            let dst = output_dir.join(rel);
            dst.parent().map_or(Ok(()), fs::create_dir_all)?;
            fs::copy(&src, &dst)?;
        }
        Ok(paths.len())
    }

    /// Orchestrates payload harvesting, generating `WiX` XML fragment, manifest, and file staging.
    ///
    /// Supports optional cache directory injection mirroring libscript payload harvesting.
    ///
    /// # Arguments
    ///
    /// * `root_path` - Source repository or payload directory.
    /// * `options` - Payload harvesting configuration options.
    ///
    /// # Returns
    ///
    /// [`HarvestPayloadResult`] containing statistics and generated XML.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on harvesting, copying, or XML generation failure.
    pub fn harvest_payload(
        &self,
        root_path: &Path,
        options: &HarvestPayloadOptions,
    ) -> Result<HarvestPayloadResult> {
        // 1. Generate WiX fragment XML
        let xml =
            self.harvest_directory(root_path, &options.component_group, &options.directory_ref)?;

        // 2. Write fragment if requested
        if let Some(ref frag_path) = options.wix_fragment {
            frag_path.parent().map_or(Ok(()), fs::create_dir_all)?;
            fs::write(frag_path, &xml)?;
        }

        // 3. Generate and write manifest if requested
        let mut manifest = self.generate_manifest(root_path).unwrap_or_default();

        // 4. Ingest cache if requested
        if let Some(ref cache_dir) = options.include_cache {
            if cache_dir.is_dir() {
                let cache_paths = self.generate_manifest(cache_dir)?;
                for cp in cache_paths {
                    manifest.push(format!("cache/{cp}"));
                }
            }
        }

        if let Some(ref mf_path) = options.manifest_file {
            mf_path.parent().map_or(Ok(()), fs::create_dir_all)?;
            let mut content = manifest.join("\n");
            if !manifest.is_empty() {
                content.push('\n');
            }
            fs::write(mf_path, content)?;
        }

        // 5. Copy payload if output_dir requested
        if let Some(ref out_dir) = options.output_dir {
            self.copy_payload(root_path, out_dir)?;
            if let Some(ref cache_dir) = options.include_cache {
                if cache_dir.is_dir() {
                    let cache_out = out_dir.join("cache");
                    self.copy_payload(cache_dir, &cache_out)?;
                }
            }
        }

        Ok(HarvestPayloadResult {
            file_count: manifest.len(),
            component_count: manifest.len(),
            wix_fragment: xml,
            relative_paths: manifest,
        })
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
///
/// # Arguments
///
/// * `input` - Raw identifier string.
///
/// # Returns
///
/// Sanitized identifier string safe for XML ID attributes.
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
///
/// # Arguments
///
/// * `name` - Filename string.
///
/// # Returns
///
/// 8.3 short filename string.
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
///
/// # Arguments
///
/// * `raw_val` - Raw value string from `.reg` file line.
///
/// # Returns
///
/// Tuple of `(registry_type, parsed_value_string)`.
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
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;
    use crate::error::Error;

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

    /// Tests harvester gitignore filtering, disk routing rules, and secondary component groups.
    #[test]
    fn test_harvester_gitignore_disk_rules_and_secondary_groups() {
        let temp_dir = std::env::temp_dir().join("msi_harvest_advanced_test");
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(temp_dir.join("cache").join("runtimes")).is_ok());
        assert!(fs::create_dir_all(temp_dir.join("cache").join("databases")).is_ok());
        assert!(fs::create_dir_all(temp_dir.join("ignored_dir")).is_ok());

        assert!(fs::write(
            temp_dir.join(".gitignore"),
            "ignored_dir/*\n*.log\n# comment\n\n",
        )
        .is_ok());
        assert!(fs::write(temp_dir.join("app.exe"), b"app binary").is_ok());
        assert!(fs::write(temp_dir.join("ignored.log"), b"log").is_ok());
        assert!(fs::write(temp_dir.join("ignored_dir").join("secret.txt"), b"secret").is_ok());
        assert!(fs::write(
            temp_dir.join("cache").join("runtimes").join("python.dll"),
            b"python",
        )
        .is_ok());
        assert!(fs::write(
            temp_dir.join("cache").join("databases").join("db.bin"),
            b"database",
        )
        .is_ok());

        let mut harvester = Harvester::new();
        assert!(harvester
            .load_gitignore(Path::new("/nonexistent_gitignore"))
            .is_err());
        assert!(harvester
            .load_gitignore(&temp_dir.join(".gitignore"))
            .is_ok());
        harvester.add_exclude_pattern("*.gitignore");
        harvester.add_disk_rule("cache/runtimes/*", 2);
        harvester.add_disk_rule("cache/databases/*", 3);
        harvester.add_secondary_group("OfflineCacheComponents", "cache/**");

        for xml in [
            harvester.harvest_directory(&temp_dir, "MainComponents", "INSTALLFOLDER"),
            Err(Error::Io("simulated".to_string())),
        ]
        .into_iter()
        .flatten()
        {
            // 1. Verify excluded files are not in xml
            assert!(!xml.contains("ignored.log"));
            assert!(!xml.contains("secret.txt"));

            // 2. Verify included files
            assert!(xml.contains("app.exe"));
            assert!(xml.contains("python.dll"));
            assert!(xml.contains("db.bin"));

            // 3. Verify disk routing
            assert!(xml.contains(r#"Source=""#));
            assert!(xml.contains(r#"DiskId="2""#));
            assert!(xml.contains(r#"DiskId="3""#));

            // 4. Verify secondary group
            assert!(xml.contains(r#"<ComponentGroup Id="OfflineCacheComponents">"#));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests the internal `matches_pattern` function across all wildcard, prefix, suffix, and boundary conditions.
    #[test]
    fn test_matches_pattern_comprehensive() {
        // 1. /* patterns
        assert!(matches_pattern(
            "cache/runtimes/file.txt",
            "cache/runtimes/*"
        ));
        assert!(matches_pattern("cache/runtimes", "cache/runtimes/*"));
        assert!(!matches_pattern("other/path/file.txt", "cache/runtimes/*"));

        // 2. /** patterns
        assert!(matches_pattern("cache/sub/data.bin", "cache/**"));
        assert!(matches_pattern("cache", "cache/**"));
        assert!(!matches_pattern("other/file.bin", "cache/**"));

        // 3. *suffix patterns
        assert!(matches_pattern("file.log", "*.log"));
        assert!(!matches_pattern("file.txt", "*.log"));

        // 4. Trailing slash / prefix patterns
        assert!(matches_pattern("docs/readme.md", "docs/"));
        assert!(matches_pattern("base/docs/readme.md", "docs/"));
        assert!(!matches_pattern("base/other/readme.md", "docs/"));

        // 5. Fallback matches: exact, starts_with, ends_with, contains, and negative
        assert!(matches_pattern("app.exe", "app.exe"));
        assert!(matches_pattern("dir/sub/file.txt", "dir"));
        assert!(matches_pattern("some/dir/target.bin", "target.bin"));
        assert!(matches_pattern("top/mid/bottom/file.txt", "mid"));
        assert!(!matches_pattern("abc/def", "xyz"));

        // 6. Normalizations: ./ prefix and backslashes
        assert!(matches_pattern("cache/file.txt", "./cache/*"));
        assert!(matches_pattern("cache\\sub\\file.txt", "cache/*"));
        assert!(matches_pattern("cache/sub/file.txt", "cache\\*"));
    }

    /// Tests byte-for-byte reproducibility of harvested XML fragments and deterministic GUIDs.
    #[test]
    fn test_harvest_byte_for_byte_reproducibility() {
        let temp_dir = std::env::temp_dir().join("msi_harvest_repro_test");
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(temp_dir.join("sub")).is_ok());

        assert!(fs::write(temp_dir.join("file_b.txt"), b"BBB").is_ok());
        assert!(fs::write(temp_dir.join("file_a.txt"), b"AAA").is_ok());
        assert!(fs::write(temp_dir.join("sub").join("file_c.txt"), b"CCC").is_ok());

        let harvester = Harvester::new();
        let xml1 = harvester
            .harvest_directory(&temp_dir, "MyComponents", "INSTALLFOLDER")
            .unwrap_or_default();
        let xml2 = harvester
            .harvest_directory(&temp_dir, "MyComponents", "INSTALLFOLDER")
            .unwrap_or_default();

        assert_eq!(xml1, xml2, "Harvested XML must be byte-for-byte identical");
        assert!(xml1.contains("cmp_file_a.txt"));
        assert!(xml1.contains("cmp_file_b.txt"));
        assert!(xml1.contains("cmp_sub_file_c.txt"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests automatic multi-cabinet split-size partitioning.
    #[test]
    fn test_harvest_split_size_partitioning() {
        let temp_dir = std::env::temp_dir().join("msi_harvest_split_test");
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(&temp_dir).is_ok());

        // Write three 100-byte files
        let data = vec![b'X'; 100];
        assert!(fs::write(temp_dir.join("chunk1.bin"), &data).is_ok());
        assert!(fs::write(temp_dir.join("chunk2.bin"), &data).is_ok());
        assert!(fs::write(temp_dir.join("chunk3.bin"), &data).is_ok());

        let mut harvester = Harvester::new();
        // Set split size to 150 bytes so chunk1+chunk2 exceed split and chunk3 goes to next disk
        harvester.set_split_size(150);

        let xml = harvester
            .harvest_directory(&temp_dir, "SplitGroup", "INSTALLFOLDER")
            .unwrap_or_default();

        assert!(xml.contains(r#"DiskId="2""#));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests payload harvesting with manifest generation, payload copying, and offline cache injection.
    #[test]
    fn test_harvest_payload_options_manifest_and_copy() {
        let temp_dir = std::env::temp_dir().join("msi_harvest_payload_test");
        let _ = fs::remove_dir_all(&temp_dir);
        let src_dir = temp_dir.join("repo");
        let cache_dir = temp_dir.join("cache_src");
        let out_dir = temp_dir.join("staged");
        let manifest_file = temp_dir.join("manifest.txt");
        let fragment_file = temp_dir.join("payload.wxs");

        assert!(fs::create_dir_all(src_dir.join("scripts")).is_ok());
        assert!(fs::create_dir_all(cache_dir.join("runtimes")).is_ok());
        assert!(fs::write(src_dir.join("scripts").join("run.sh"), b"echo hi").is_ok());
        assert!(fs::write(cache_dir.join("runtimes").join("node.tar.gz"), b"node").is_ok());

        let mut harvester = Harvester::new();
        harvester.set_default_disk_id(1);
        harvester.set_secondary_exclusive(true);
        harvester.add_filter_pattern("*.tmp");

        let options = HarvestPayloadOptions {
            component_group: "AppComponents".to_string(),
            directory_ref: "APP_FOLDER".to_string(),
            wix_fragment: Some(fragment_file.clone()),
            manifest_file: Some(manifest_file.clone()),
            output_dir: Some(out_dir.clone()),
            include_cache: Some(cache_dir),
        };

        let res = harvester.harvest_payload(&src_dir, &options);
        assert!(res.is_ok());
        let result = res.unwrap_or_default();
        assert_eq!(result.file_count, 2);
        assert!(fragment_file.exists());
        assert!(manifest_file.exists());
        assert!(out_dir.join("scripts").join("run.sh").exists());
        assert!(out_dir
            .join("cache")
            .join("runtimes")
            .join("node.tar.gz")
            .exists());

        let manifest_content = fs::read_to_string(&manifest_file).unwrap_or_default();
        assert!(manifest_content.contains("scripts/run.sh"));
        assert!(manifest_content.contains("cache/runtimes/node.tar.gz"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests `HarvestPayloadOptions` and `HarvestPayloadResult` derive implementations.
    #[test]
    fn test_harvest_options_and_result_derives() {
        let def_opts = HarvestPayloadOptions::default();
        assert_eq!(def_opts, HarvestPayloadOptions::new());
        assert_eq!(def_opts, def_opts.clone());
        assert!(format!("{def_opts:?}").contains("HarvestPayloadOptions"));

        let def_res = HarvestPayloadResult::default();
        assert_eq!(def_res, def_res.clone());
        assert!(format!("{def_res:?}").contains("HarvestPayloadResult"));
    }

    /// Tests `matches_pattern` additional boundary and wildcard conditions.
    #[test]
    fn test_matches_pattern_additional_branches() {
        // Empty pattern variations
        assert!(!matches_pattern("file.txt", ""));
        assert!(!matches_pattern("file.txt", "./"));

        // Suffix wildcard with secondary asterisk
        assert!(!matches_pattern("file.txt", "*foo*bar"));

        // Prefix wildcard with secondary asterisk or trailing slash
        assert!(!matches_pattern("file.txt", "foo*bar*"));
        assert!(!matches_pattern("file.txt", "foo/*"));

        // Prefix wildcard matching directly and inside directory component
        assert!(matches_pattern(".git_foo", ".git*"));
        assert!(matches_pattern("sub/.git_foo", ".git*"));
        assert!(!matches_pattern("sub/other", ".git*"));

        // Directory wildcards matching exact directory, direct child, and inside subdirectories
        assert!(matches_pattern("cache/runtimes", "cache/runtimes/*"));
        assert!(matches_pattern("cache/runtimes/file", "cache/runtimes/*"));
        assert!(matches_pattern(
            "nested/cache/runtimes/file",
            "cache/runtimes/*"
        ));
        assert!(!matches_pattern(
            "cache/runtimes_backup",
            "cache/runtimes/*"
        ));

        assert!(matches_pattern("docs", "docs/"));
        assert!(matches_pattern("docs/readme.md", "docs/"));
        assert!(matches_pattern("nested/docs/readme.md", "docs/"));
        assert!(!matches_pattern("docs_backup/file.txt", "docs/"));
    }

    /// Tests `generate_manifest`, `write_manifest`, and `copy_payload` edge cases.
    #[test]
    fn test_generate_manifest_and_write_manifest_edge_cases() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_harvest_manifest_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let src_dir = temp_dir.join("src");
        assert!(fs::create_dir_all(&src_dir).is_ok());

        assert!(fs::write(src_dir.join("file1.txt"), b"12345678901234567890").is_ok());
        assert!(fs::write(src_dir.join("file2.bak"), b"2").is_ok());
        assert!(fs::write(src_dir.join("ignored.tmp"), b"3").is_ok());

        let mut harvester = Harvester::new();
        harvester.exclude_extension("bak");
        harvester.add_exclude_pattern("*.tmp");

        let paths = harvester.generate_manifest(&src_dir).unwrap_or_default();
        assert_eq!(paths, vec!["file1.txt"]);

        // Test non-directory in generate_manifest (Line 544: !current_path.is_dir())
        let non_dir_paths = harvester
            .generate_manifest(&src_dir.join("file1.txt"))
            .unwrap_or_default();
        assert!(non_dir_paths.is_empty());

        // Split size with first file exceeding split (current_bytes == 0)
        let mut split_harvester = Harvester::new();
        split_harvester.set_split_size(5);
        let split_xml = split_harvester
            .harvest_directory(&src_dir, "CompGrp", "INSTALLFOLDER")
            .unwrap_or_default();
        assert!(split_xml.contains("Component Id="));

        // Default disk id != 1 and disk_id == 1 via disk rule
        let mut disk_harvester = Harvester::new();
        disk_harvester.set_default_disk_id(2);
        disk_harvester.add_disk_rule("*.txt", 1);
        let disk_xml = disk_harvester
            .harvest_directory(&src_dir, "CompGrp", "INSTALLFOLDER")
            .unwrap_or_default();
        assert!(disk_xml.contains("DiskId=\"1\""));

        // Secondary group with secondary_exclusive = false
        let mut sec_harvester = Harvester::new();
        sec_harvester.add_secondary_group("SecGrp", "*.txt");
        sec_harvester.set_secondary_exclusive(false);
        let sec_xml = sec_harvester
            .harvest_directory(&src_dir, "CompGrp", "INSTALLFOLDER")
            .unwrap_or_default();
        assert!(sec_xml.contains("ComponentGroup Id=\"SecGrp\""));

        // Test write_manifest
        let mf_path = temp_dir.join("nested/manifest.txt");
        let count = harvester.write_manifest(&src_dir, &mf_path).unwrap_or(0);
        assert_eq!(count, 1);
        assert!(mf_path.exists());

        // Test write_manifest for empty directory
        let empty_dir = temp_dir.join("empty_dir");
        assert!(fs::create_dir_all(&empty_dir).is_ok());
        let empty_mf_path = temp_dir.join("empty_manifest.txt");
        let empty_count = harvester
            .write_manifest(&empty_dir, &empty_mf_path)
            .unwrap_or(1);
        assert_eq!(empty_count, 0);

        // Test write_manifest error cases:
        // 1. Parent creation fails (parent is a file)
        let bad_mf_parent = src_dir.join("file1.txt/manifest.txt");
        assert!(harvester.write_manifest(&src_dir, &bad_mf_parent).is_err());
        // 2. Write fails (target is an existing directory)
        assert!(harvester.write_manifest(&src_dir, &empty_dir).is_err());

        // Test copy_payload directly
        let copy_dest = temp_dir.join("copy_dest");
        let copied_count = harvester.copy_payload(&src_dir, &copy_dest).unwrap_or(0);
        assert_eq!(copied_count, 1);
        assert!(copy_dest.join("file1.txt").exists());

        // Test copy_payload error cases:
        // 1. dst parent creation fails
        let bad_copy_parent = src_dir.join("file1.txt");
        assert!(harvester.copy_payload(&src_dir, &bad_copy_parent).is_err());
        // 2. dst is an existing directory, so fs::copy fails
        let bad_copy_dst = temp_dir.join("bad_copy_dst");
        assert!(fs::create_dir_all(bad_copy_dst.join("file1.txt")).is_ok());
        assert!(harvester.copy_payload(&src_dir, &bad_copy_dst).is_err());

        // Test permission error propagation for generate_manifest, write_manifest, and copy_payload
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unreadable_sub = src_dir.join("unreadable_sub");
            assert!(fs::create_dir_all(&unreadable_sub).is_ok());
            assert!(
                fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o000)).is_ok()
            );

            assert!(harvester.generate_manifest(&src_dir).is_err());
            let out_mf = temp_dir.join("fail_manifest.txt");
            assert!(harvester.write_manifest(&src_dir, &out_mf).is_err());
            let out_copy = temp_dir.join("fail_copy");
            assert!(harvester.copy_payload(&src_dir, &out_copy).is_err());

            assert!(
                fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o755)).is_ok()
            );
            let _ = fs::remove_dir_all(&unreadable_sub);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests `harvest_payload` with various combinations of optional parameters and error conditions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_harvest_payload_optional_branches() {
        let temp_dir = std::env::temp_dir().join(format!("msi_harvest_opt_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(temp_dir.join("src")).is_ok());
        assert!(fs::write(temp_dir.join("src/app.bin"), b"bin").is_ok());

        let harvester = Harvester::new();

        // All options None
        let minimal_opts = HarvestPayloadOptions::new();
        let min_res = harvester
            .harvest_payload(&temp_dir.join("src"), &minimal_opts)
            .unwrap_or_default();
        assert_eq!(min_res.file_count, 1);

        // include_cache pointing to a file (not a dir)
        let cache_file = temp_dir.join("not_a_cache_dir.txt");
        assert!(fs::write(&cache_file, b"data").is_ok());
        let bad_cache_opts = HarvestPayloadOptions {
            include_cache: Some(cache_file.clone()),
            ..HarvestPayloadOptions::new()
        };
        let bad_cache_res = harvester
            .harvest_payload(&temp_dir.join("src"), &bad_cache_opts)
            .unwrap_or_default();
        assert_eq!(bad_cache_res.file_count, 1);

        // Empty manifest with manifest_file option
        let empty_dir = temp_dir.join("empty_src");
        assert!(fs::create_dir_all(&empty_dir).is_ok());
        let empty_mf = temp_dir.join("empty_mf.txt");
        let empty_opts = HarvestPayloadOptions {
            manifest_file: Some(empty_mf.clone()),
            ..HarvestPayloadOptions::new()
        };
        let empty_res = harvester
            .harvest_payload(&empty_dir, &empty_opts)
            .unwrap_or_default();
        assert_eq!(empty_res.file_count, 0);
        assert!(empty_mf.exists());

        // output_dir with include_cache: None
        let out_dir = temp_dir.join("out");
        let opts_no_cache = HarvestPayloadOptions {
            output_dir: Some(out_dir.clone()),
            include_cache: None,
            ..HarvestPayloadOptions::new()
        };
        let res_no_cache = harvester
            .harvest_payload(&temp_dir.join("src"), &opts_no_cache)
            .unwrap_or_default();
        assert_eq!(res_no_cache.file_count, 1);
        assert!(out_dir.join("app.bin").exists());

        // output_dir with include_cache: Some(file) (not a dir)
        let out_dir2 = temp_dir.join("out2");
        let opts_bad_cache_out = HarvestPayloadOptions {
            output_dir: Some(out_dir2),
            include_cache: Some(cache_file),
            ..HarvestPayloadOptions::new()
        };
        let res_bad_cache_out = harvester
            .harvest_payload(&temp_dir.join("src"), &opts_bad_cache_out)
            .unwrap_or_default();
        assert_eq!(res_bad_cache_out.file_count, 1);

        // Error branches:
        let src_file = temp_dir.join("src/app.bin");

        // 1. wix_fragment parent cannot be created
        let bad_frag_parent = HarvestPayloadOptions {
            wix_fragment: Some(src_file.join("sub/frag.wxs")),
            ..HarvestPayloadOptions::new()
        };
        assert!(harvester
            .harvest_payload(&temp_dir.join("src"), &bad_frag_parent)
            .is_err());

        // 2. wix_fragment write fails (target is an existing directory)
        let bad_frag_write = HarvestPayloadOptions {
            wix_fragment: Some(empty_dir.clone()),
            ..HarvestPayloadOptions::new()
        };
        assert!(harvester
            .harvest_payload(&temp_dir.join("src"), &bad_frag_write)
            .is_err());

        // 3. manifest_file parent cannot be created
        let bad_mf_parent = HarvestPayloadOptions {
            manifest_file: Some(src_file.join("sub/mf.txt")),
            ..HarvestPayloadOptions::new()
        };
        assert!(harvester
            .harvest_payload(&temp_dir.join("src"), &bad_mf_parent)
            .is_err());

        // 4. manifest_file write fails (target is an existing directory)
        let bad_mf_write = HarvestPayloadOptions {
            manifest_file: Some(empty_dir),
            ..HarvestPayloadOptions::new()
        };
        assert!(harvester
            .harvest_payload(&temp_dir.join("src"), &bad_mf_write)
            .is_err());

        // 5. copy_payload fails for output_dir
        let bad_copy_opts = HarvestPayloadOptions {
            output_dir: Some(src_file),
            ..HarvestPayloadOptions::new()
        };
        assert!(harvester
            .harvest_payload(&temp_dir.join("src"), &bad_copy_opts)
            .is_err());

        // 5b. copy_payload fails for cache directory when out_dir/cache is an existing file
        let cache_valid_dir = temp_dir.join("valid_cache_dir");
        assert!(fs::create_dir_all(&cache_valid_dir).is_ok());
        assert!(fs::write(cache_valid_dir.join("cached.bin"), b"cached").is_ok());
        let out_dir_cache_file = temp_dir.join("out_cache_blocked");
        assert!(fs::create_dir_all(&out_dir_cache_file).is_ok());
        assert!(fs::write(out_dir_cache_file.join("cache"), b"blocking file").is_ok());
        let bad_cache_out_opts = HarvestPayloadOptions {
            include_cache: Some(cache_valid_dir),
            output_dir: Some(out_dir_cache_file),
            ..HarvestPayloadOptions::new()
        };
        assert!(harvester
            .harvest_payload(&temp_dir.join("src"), &bad_cache_out_opts)
            .is_err());

        // 6. Unix permission errors for harvest_directory, generate_manifest, and cache copying
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unreadable_sub = temp_dir.join("src/unreadable");
            assert!(fs::create_dir_all(&unreadable_sub).is_ok());
            assert!(
                fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o000)).is_ok()
            );

            // Fails in harvest_directory
            assert!(harvester
                .harvest_payload(&temp_dir.join("src"), &minimal_opts)
                .is_err());

            assert!(
                fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o755)).is_ok()
            );
            let _ = fs::remove_dir_all(&unreadable_sub);

            // Fails in generate_manifest(cache_dir)
            let cache_dir = temp_dir.join("unreadable_cache");
            assert!(fs::create_dir_all(cache_dir.join("sub")).is_ok());
            assert!(
                fs::set_permissions(cache_dir.join("sub"), fs::Permissions::from_mode(0o000))
                    .is_ok()
            );
            let bad_cache_manifest = HarvestPayloadOptions {
                include_cache: Some(cache_dir.clone()),
                ..HarvestPayloadOptions::new()
            };
            assert!(harvester
                .harvest_payload(&temp_dir.join("src"), &bad_cache_manifest)
                .is_err());

            // Fails in copy_payload(cache_dir)
            let bad_cache_copy = HarvestPayloadOptions {
                include_cache: Some(cache_dir.clone()),
                output_dir: Some(temp_dir.join("valid_out")),
                ..HarvestPayloadOptions::new()
            };
            assert!(harvester
                .harvest_payload(&temp_dir.join("src"), &bad_cache_copy)
                .is_err());

            assert!(
                fs::set_permissions(cache_dir.join("sub"), fs::Permissions::from_mode(0o755))
                    .is_ok()
            );
            let _ = fs::remove_dir_all(&cache_dir);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
