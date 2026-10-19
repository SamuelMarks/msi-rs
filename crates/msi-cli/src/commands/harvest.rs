//! Filesystem and registry harvesting (`harvest`) subcommand handler.

use clap::Args;
use msi::MsiError;
use std::path::{Path, PathBuf};

/// Arguments for harvesting files or registries into `WiX` XML fragments.
#[derive(Debug, Args)]
pub struct HarvestArgs {
    /// Target filesystem path, registry file path, or harvest type (e.g. `dir`, `reg`).
    #[arg(value_name = "TARGET_OR_TYPE", default_value = "")]
    pub target: String,

    /// Target path when harvest type is given as first positional argument (e.g. `msi harvest dir /path`).
    #[arg(value_name = "TARGET_PATH")]
    pub extra_target: Option<String>,

    /// Source directory to harvest directly (alternative to positional argument).
    #[arg(long = "source-dir", alias = "source")]
    pub source_dir: Option<String>,

    /// Path to generated `WiX` fragment output (alias for --output / -o).
    #[arg(long = "wix-fragment", alias = "fragment")]
    pub wix_fragment: Option<String>,

    /// Component group identifier.
    #[arg(
        long,
        default_value = "HarvestedComponents",
        alias = "component-group",
        short = 'c'
    )]
    pub group: String,

    /// Target directory identifier.
    #[arg(
        long,
        default_value = "INSTALLFOLDER",
        alias = "directory-id",
        short = 'd'
    )]
    pub dir_id: String,

    /// Destination output .wxs file path (defaults to stdout).
    #[arg(short, long, alias = "out")]
    pub output: Option<String>,

    /// Mode: "dir" for directory harvesting, "reg" for registry file.
    #[arg(long, default_value = "dir")]
    pub mode: String,

    /// Path to a .gitignore file to respect during harvesting.
    #[arg(long)]
    pub gitignore: Option<String>,

    /// Disk ID assignment rules in `PATTERN=DISK_ID` format (e.g. `cache/runtimes/*=2`).
    #[arg(long = "disk-rule")]
    pub disk_rules: Vec<String>,

    /// Default Disk ID for non-matching files.
    #[arg(long = "default-disk-id", default_value = "1")]
    pub default_disk_id: i16,

    /// Automatic multi-cabinet split size in bytes.
    #[arg(long = "split-size")]
    pub split_size: Option<u64>,

    /// Secondary component groups in `PATTERN=GROUP_ID` format (e.g. `cache/*=LibscriptOfflineCacheComponents`).
    #[arg(long = "secondary-group")]
    pub secondary_groups: Vec<String>,

    /// Specific file extensions to exclude from harvesting (e.g. `tmp`, `pdb`).
    #[arg(long = "exclude-ext")]
    pub exclude_extensions: Vec<String>,

    /// Specific path or glob patterns to exclude from harvesting.
    #[arg(long = "exclude-pattern", alias = "filter", alias = "exclude")]
    pub exclude_patterns: Vec<String>,

    /// Path to output newline-delimited manifest file of harvested relative paths.
    #[arg(long = "manifest-file")]
    pub manifest_file: Option<String>,

    /// Target directory where harvested files will be staged/copied.
    #[arg(long = "output-dir")]
    pub output_dir: Option<String>,

    /// Offline cache directory to inject into payload under `cache/`.
    #[arg(long = "include-cache")]
    pub include_cache: Option<String>,

    /// Whether to include .msi files in harvested payload (matches `LibScript` `--include-msi`).
    #[arg(long = "include-msi")]
    pub include_msi: bool,
}

/// Handles the `harvest` command.
///
/// # Arguments
///
/// * `args` - Command-line or invocation arguments slice.
///
/// # Errors
///
/// Returns an [`MsiError`] if harvesting or output file writing fails.
///
/// # Returns
///
/// `Ok` containing the formatted or resolved string, or an error on failure.
pub fn handle_harvest(args: &HarvestArgs) -> Result<String, MsiError> {
    let raw_target = if args.target.is_empty() {
        args.source_dir.as_deref().unwrap_or(".")
    } else {
        args.target.as_str()
    };

    let (effective_mode, target_path) = args.extra_target.as_ref().map_or_else(
        || {
            let p = Path::new(raw_target);
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("reg")) {
                ("reg".to_string(), raw_target)
            } else {
                (args.mode.to_lowercase(), raw_target)
            }
        },
        |second| (raw_target.to_lowercase(), second.as_str()),
    );

    let effective_output = args.output.as_ref().or(args.wix_fragment.as_ref());

    let mut harvester = msi::wix::Harvester::new();
    harvester.set_default_disk_id(args.default_disk_id);
    if let Some(split) = args.split_size {
        harvester.set_split_size(split);
    }
    if let Some(ref gitignore) = args.gitignore {
        harvester.load_gitignore(Path::new(gitignore))?;
    }
    for rule in &args.disk_rules {
        if let Some((pat, disk_str)) = rule.split_once('=') {
            if let Ok(did) = disk_str.parse::<i16>() {
                harvester.add_disk_rule(pat, did);
            }
        }
    }
    for sec in &args.secondary_groups {
        if let Some((pat, grp)) = sec.split_once('=') {
            harvester.add_secondary_group(grp, pat);
        }
    }
    for ext in &args.exclude_extensions {
        harvester.exclude_extension(ext);
    }
    for pat in &args.exclude_patterns {
        harvester.add_exclude_pattern(pat);
    }

    if let Some(ref _cache_dir) = args.include_cache {
        harvester.add_secondary_group("LibscriptOfflineCacheComponents", "cache/**");
        harvester.add_disk_rule("cache/runtimes/*", 2);
        harvester.add_disk_rule("cache/databases/*", 3);
        harvester.add_disk_rule("cache/codebase/*", 4);
        harvester.add_disk_rule("cache/wheels/*", 4);
        harvester.add_disk_rule("cache/npm/*", 4);
    }

    if args.include_msi {
        harvester.set_include_msi(true);
    }

    let payload_opts = msi::wix::HarvestPayloadOptions {
        component_group: args.group.clone(),
        directory_ref: args.dir_id.clone(),
        wix_fragment: effective_output.map(PathBuf::from),
        manifest_file: args.manifest_file.as_ref().map(PathBuf::from),
        output_dir: args.output_dir.as_ref().map(PathBuf::from),
        include_cache: args.include_cache.as_ref().map(PathBuf::from),
        include_msi: args.include_msi,
    };

    let xml = if effective_mode == "reg" {
        let content = std::fs::read_to_string(target_path).map_err(|e| {
            MsiError::CommandLineError(format!("Failed to read registry file '{target_path}': {e}"))
        })?;
        let res = harvester.harvest_registry(&content, &args.group)?;
        if let Some(out_file) = effective_output {
            std::fs::write(out_file, res.as_bytes()).map_err(|e| {
                MsiError::CommandLineError(format!("Failed to write output to '{out_file}': {e}"))
            })?;
        }
        res
    } else {
        let result = harvester.harvest_payload(Path::new(target_path), &payload_opts)?;
        result.wix_fragment
    };

    effective_output.map_or(Ok(xml), |out_file| {
        Ok(format!("Harvested WiX source written to '{out_file}'"))
    })
}
