//! `WiX` source and manifest compilation (`pack`) subcommand handler.

use clap::Args;
use msi::MsiError;
use std::path::{Path, PathBuf};

/// Arguments for compiling and linking `WiX` source manifests into an MSI package.
#[derive(Debug, Args)]
pub struct PackArgs {
    /// Output path for the generated .msi package.
    #[arg(short, long, value_name = "PATH")]
    pub output: String,

    /// One or more .wxs source files or .wixobj objects.
    #[arg(value_name = "SOURCES")]
    pub sources: Vec<String>,

    /// Optional path to packaging.json manifest for direct schema synthesis.
    #[arg(long = "manifest", value_name = "PATH")]
    pub manifest: Option<String>,

    /// Optional path to vars.schema.json schema for direct property synthesis.
    #[arg(long = "schema", value_name = "PATH")]
    pub schema: Option<String>,

    /// Preprocessor variable definitions (e.g. -d NAME=VALUE).
    #[arg(short, long = "define", value_name = "NAME=VALUE")]
    pub defines: Vec<String>,

    /// Target platform architecture (e.g. x86, x64, arm64).
    #[arg(long, default_value = "x64")]
    pub arch: String,

    /// Suppress internal consistency evaluators (ICE) validation.
    #[arg(short = 's', long = "suppress-validation", alias = "sval")]
    pub suppress_validation: bool,

    /// Specific ICE validation rules to suppress (-sice:ICE01).
    #[arg(long = "suppress-ice", alias = "sice")]
    pub suppress_ice: Vec<String>,

    /// Specific ICE validation rules to selectively run (-ice:ICE01).
    #[arg(long = "select-ice", alias = "ice")]
    pub select_ice: Vec<String>,

    /// `WiX` extension identifiers (e.g. `WixUIExtension`, `WixToolset.UI.wixext`).
    #[arg(short = 'e', long = "extension", alias = "ext")]
    pub extensions: Vec<String>,

    /// Additional bind paths for resolving source files and media (-b).
    #[arg(short = 'b', long = "bind-path")]
    pub bind_paths: Vec<String>,

    /// Enable verbose progress and binding diagnostics.
    #[arg(short, long)]
    pub verbose: bool,
}

/// Automatically discovers and links companion payload `.wxs` fragments in the same directory.
///
/// # Arguments
///
/// * `sources` - The `sources` argument.
///
/// # Returns
///
/// The return value.
#[must_use]
pub fn discover_companion_fragments(sources: &[String]) -> Vec<String> {
    let mut effective_sources = sources.to_vec();
    if effective_sources.len() == 1 {
        let primary_path = PathBuf::from(&effective_sources[0]);
        if let Ok(content) = std::fs::read_to_string(&primary_path) {
            let search_dir = match primary_path.parent() {
                Some(p) if !p.as_os_str().is_empty() => p,
                _ => Path::new("."),
            };
            if let Ok(entries) = std::fs::read_dir(search_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let is_wxs_file = path.is_file()
                        && path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("wxs"))
                        && path != primary_path;
                    if !is_wxs_file {
                        continue;
                    }
                    let path_str = path.to_string_lossy().to_string();
                    if let Ok(sibling_content) = std::fs::read_to_string(&path) {
                        let is_pure_fragment = sibling_content.contains("<Fragment")
                            && !sibling_content.contains("<Product")
                            && !sibling_content.contains("<Module");
                        if !is_pure_fragment {
                            continue;
                        }
                        let is_companion_by_name = primary_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .is_some_and(|stem| {
                                path.file_name()
                                    .and_then(|n| n.to_str())
                                    .is_some_and(|name| {
                                        name.starts_with(stem) && name.contains("payload")
                                    })
                            });
                        let resolves_group = content.contains("<ComponentGroupRef")
                            && sibling_content.contains("<ComponentGroup");
                        if is_companion_by_name || resolves_group {
                            effective_sources.push(path_str);
                        }
                    }
                }
            }
        }
    }
    effective_sources
}

/// Handles the `pack` command by compiling and linking `WiX` source manifests into an MSI package.
///
/// # Arguments
///
/// * `args` - The command arguments containing source manifests and build configuration.
///
/// # Returns
///
/// A message string with the generated package path on success.
///
/// # Errors
///
/// Returns an [`MsiError`] if source files are missing, compilation fails, or linking fails.
#[allow(clippy::too_many_lines)]
pub fn handle_pack(args: &PackArgs) -> Result<String, MsiError> {
    if args.output.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Output path cannot be empty".to_string(),
        ));
    }

    if let Some(ref manifest_path) = args.manifest {
        let manifest_content = std::fs::read_to_string(manifest_path).map_err(|e| {
            MsiError::CommandLineError(format!(
                "Failed to read packaging manifest '{manifest_path}': {e}"
            ))
        })?;

        let mut synth =
            msi::wix::ManifestMsiSynthesizer::new(manifest_content).with_arch(&args.arch);

        if let Some(ref schema_path) = args.schema {
            let schema_content = std::fs::read_to_string(schema_path).map_err(|e| {
                MsiError::CommandLineError(format!(
                    "Failed to read variable schema '{schema_path}': {e}"
                ))
            })?;
            synth = synth.with_schema(schema_content);
        }

        if let Some(first_src) = args.sources.first() {
            synth = synth.with_payload_fragment(PathBuf::from(first_src));
        }

        let out_p = Path::new(&args.output);
        let out_path = synth.build_msi(out_p)?;

        return Ok(format!(
            "Successfully synthesized and linked MSI package from manifest: '{}'",
            out_path.display()
        ));
    }

    if args.sources.is_empty() {
        return Err(MsiError::CommandLineError(
            "No source files specified".to_string(),
        ));
    }

    let effective_sources = discover_companion_fragments(&args.sources);

    let mut raw_args = vec!["-o".to_string(), args.output.clone()];
    for src in &effective_sources {
        raw_args.push(src.clone());
    }
    for def in &args.defines {
        raw_args.push(format!("-d{def}"));
    }
    if args.suppress_validation {
        raw_args.push("-sval".to_string());
    }
    for sice in &args.suppress_ice {
        raw_args.push(format!("-sice:{sice}"));
    }
    for ice in &args.select_ice {
        raw_args.push(format!("-ice:{ice}"));
    }
    for ext in &args.extensions {
        raw_args.push("-ext".to_string());
        raw_args.push(ext.clone());
    }
    if !args.arch.is_empty() {
        raw_args.push("-arch".to_string());
        raw_args.push(args.arch.clone());
    }
    for b in &args.bind_paths {
        raw_args.push("-b".to_string());
        raw_args.push(b.clone());
    }

    let opts = msi::wix::toolchain::WixBuildOptions::parse(&raw_args)?;
    let out_path = opts.execute()?;

    let mut summary = format!(
        "Successfully compiled and linked MSI package: '{}'",
        out_path.display()
    );
    if args.verbose {
        let pkg = msi::Package::open(&out_path)?;
        let media_recs = pkg.database().get_records("Media");
        let file_count = pkg.database().get_records("File").len();
        let chainer_count = pkg.database().get_records("MsiEmbeddedChainer").len();
        let _ = std::fmt::Write::write_fmt(
            &mut summary,
            format_args!(
                "
[INFO] Media cabinets: {}, Files: {file_count}, EmbeddedChainers: {chainer_count}",
                media_recs.len()
            ),
        );
        for rec in media_recs {
            if let (
                Some(msi::database::tables::record::FieldValue::Short(disk_id)),
                Some(msi::database::tables::record::FieldValue::Long(last_seq)),
                Some(msi::database::tables::record::FieldValue::String(cab_name)),
            ) = (rec.get(0), rec.get(1), rec.get(3))
            {
                let _ = std::fmt::Write::write_fmt(
                    &mut summary,
                    format_args!(
                        "
  * DiskId {disk_id}: {cab_name} (LastSequence: {last_seq})"
                    ),
                );
            }
        }
    }

    Ok(summary)
}
