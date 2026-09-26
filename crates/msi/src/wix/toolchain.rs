//! `WiX` toolchain CLI command abstractions and executors (`candle`, `light`, `wix`).
//!
//! Provides parameter parsers and execution pipelines replicating the official `WiX` CLI toolchain:
//! - [`CandleOptions`]: Replicates `WiX` v3 `candle.exe` compiler command-line interface.
//! - [`LightOptions`]: Replicates `WiX` v3 `light.exe` linker/binder command-line interface.
//! - [`WixBuildOptions`]: Replicates `WiX` .NET Tools v4/v5 `wix build` command-line interface.

use crate::error::{Error, Result};
use crate::package::Package;
use crate::wix::linker::Linker;
use crate::wix::localization::WixLocalization;
use crate::wix::preprocessor::PreprocessorContext;
use crate::wix::ui_library::{inject_ui_library, WixUiDialogSet};
use crate::wix::wixlib::WixLibrary;
use crate::wix::wixobj::WixObject;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Helper function to strip leading flag prefix (`--`, `-`, or `/`).
///
/// Returns the flag without prefix if it represents a command flag,
/// or `None` if it represents a path or non-flag argument.
#[allow(clippy::option_if_let_else)]
fn strip_flag_prefix(arg: &str) -> Option<&str> {
    if let Some(rest) = arg.strip_prefix("--") {
        Some(rest)
    } else if let Some(rest) = arg.strip_prefix('-') {
        Some(rest)
    } else if let Some(rest) = arg.strip_prefix('/') {
        if rest.contains('/') {
            None
        } else {
            Some(rest)
        }
    } else {
        None
    }
}

/// Returns whether an argument is a CLI flag rather than a filesystem path.
#[must_use]
pub fn is_flag(arg: &str) -> bool {
    arg.starts_with('@') || strip_flag_prefix(arg).is_some()
}

/// Creates parent directory for a target path if one is specified and non-empty.
fn ensure_parent_dir_exists(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
}

/// Command-line options for the `candle` compiler.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct CandleOptions {
    /// Suppress copyright banner output (`-nologo`).
    pub nologo: bool,
    /// Target architecture (e.g. `x86`, `x64`, `arm64`, `ia64`) (`-arch`).
    pub arch: Option<String>,
    /// Preprocessor variable definitions `(Name, Value)` (`-d`).
    pub defines: Vec<(String, String)>,
    /// `WiX` extensions to load (e.g. `WixUIExtension`) (`-ext`).
    pub extensions: Vec<String>,
    /// Directory search paths for `<?include ?>` files (`-I`).
    pub include_dirs: Vec<PathBuf>,
    /// Explicit output file or directory path (`-o`, `-out`).
    pub output: Option<PathBuf>,
    /// Preprocess only to stdout or specified file (`-p`).
    pub preprocess_only: Option<PathBuf>,
    /// Enforce FIPS-compliant cryptographic algorithms (`-fips`).
    pub fips: bool,
    /// Enforce pedantic schema validation and warnings (`-pedantic`).
    pub pedantic: bool,
    /// Suppress informational status output (`-q`, `-quiet`).
    pub quiet: bool,
    /// Suppress schema validation checks (`-ss`).
    pub suppress_schema: bool,
    /// Suppress specific warning IDs (`-sw<id>`).
    pub suppressed_warnings: Vec<String>,
    /// Suppress all warnings (`-swall`).
    pub suppress_all_warnings: bool,
    /// Verbose diagnostic logging (`-v`, `-verbose`).
    pub verbose: bool,
    /// Treat all warnings as fatal errors (`-wx`).
    pub warnings_as_errors: bool,
    /// Compiled cache directory for include file precompilation (`-cc`).
    pub cache_dir: Option<PathBuf>,
    /// Input `.wxs` source files to compile.
    pub sources: Vec<PathBuf>,
}

impl CandleOptions {
    /// Creates a new empty [`CandleOptions`].
    ///
    /// # Returns
    ///
    /// Default empty options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses command-line arguments into a [`CandleOptions`] instance.
    ///
    /// Supports both standard Unix style (`-flag`), Windows style (`/flag`),
    /// and response file expansion (`@file`).
    ///
    /// # Arguments
    ///
    /// * `raw_args` - Command-line argument slice (excluding executable name).
    ///
    /// # Returns
    ///
    /// Parsed [`CandleOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] on invalid flags or missing required argument values.
    #[allow(clippy::too_many_lines, clippy::branches_sharing_code)]
    pub fn parse(raw_args: &[String]) -> Result<Self> {
        let args = expand_response_files(raw_args)?;
        let mut opts = Self::new();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if let Some(flag) = strip_flag_prefix(arg) {
                let lower = flag.to_ascii_lowercase();
                if lower == "nologo" {
                    opts.nologo = true;
                    idx += 1;
                } else if lower == "arch" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "candle".to_string(),
                            message: "missing argument value for '-arch'".to_string(),
                        });
                    }
                    opts.arch = Some(args[idx].clone());
                    idx += 1;
                } else if let Some(def) = flag.strip_prefix(['d', 'D']) {
                    if let Some((k, v)) = def.split_once('=') {
                        opts.defines.push((k.to_string(), v.to_string()));
                    } else {
                        opts.defines.push((def.to_string(), "1".to_string()));
                    }
                    idx += 1;
                } else if lower == "ext" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "candle".to_string(),
                            message: "missing argument value for '-ext'".to_string(),
                        });
                    }
                    opts.extensions.push(args[idx].clone());
                    idx += 1;
                } else if let Some(inc_path) = flag.strip_prefix(['I', 'i']) {
                    if inc_path.is_empty() {
                        idx += 1;
                        if idx >= args.len() {
                            return Err(Error::WixCompiler {
                                element: "candle".to_string(),
                                message: "missing path for '-I'".to_string(),
                            });
                        }
                        opts.include_dirs.push(PathBuf::from(&args[idx]));
                    } else {
                        opts.include_dirs.push(PathBuf::from(inc_path));
                    }
                    idx += 1;
                } else if lower == "o" || lower == "out" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "candle".to_string(),
                            message: "missing output path for '-out'".to_string(),
                        });
                    }
                    opts.output = Some(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if lower == "pedantic" {
                    opts.pedantic = true;
                    idx += 1;
                } else if let Some(p_str) = flag.strip_prefix(['p', 'P']) {
                    if p_str.is_empty() {
                        opts.preprocess_only = Some(PathBuf::new());
                    } else {
                        opts.preprocess_only = Some(PathBuf::from(p_str));
                    }
                    idx += 1;
                } else if lower == "fips" {
                    opts.fips = true;
                    idx += 1;
                } else if lower == "q" || lower == "quiet" {
                    opts.quiet = true;
                    idx += 1;
                } else if lower == "ss" {
                    opts.suppress_schema = true;
                    idx += 1;
                } else if lower == "swall" {
                    opts.suppress_all_warnings = true;
                    idx += 1;
                } else if let Some(sw_id) = lower.strip_prefix("sw") {
                    opts.suppressed_warnings.push(sw_id.to_string());
                    idx += 1;
                } else if lower == "v" || lower == "verbose" {
                    opts.verbose = true;
                    idx += 1;
                } else if lower == "wx" {
                    opts.warnings_as_errors = true;
                    idx += 1;
                } else if lower == "cc" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "candle".to_string(),
                            message: "missing cache directory for '-cc'".to_string(),
                        });
                    }
                    opts.cache_dir = Some(PathBuf::from(&args[idx]));
                    idx += 1;
                } else {
                    idx += 1;
                }
            } else {
                opts.sources.push(PathBuf::from(arg));
                idx += 1;
            }
        }

        if opts.sources.is_empty() {
            return Err(Error::WixCompiler {
                element: "candle".to_string(),
                message: "no source files specified for compilation".to_string(),
            });
        }

        Ok(opts)
    }

    /// Compiles all specified `.wxs` source files into intermediate `.wixobj` objects,
    /// or preprocesses them directly if `-p` was specified.
    ///
    /// # Returns
    ///
    /// List of paths to the written `.wixobj` or preprocessed output files.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on preprocessing, parsing, or I/O failure.
    #[allow(clippy::option_if_let_else)]
    pub fn execute(&self) -> Result<Vec<PathBuf>> {
        let mut outputs = Vec::new();

        for src_path in &self.sources {
            let content = fs::read_to_string(src_path)?;

            let mut ctx = PreprocessorContext::new();

            if let Some(ref arch_str) = self.arch {
                ctx.define_var("sys.BUILDARCH", arch_str);
                ctx.define_var("arch", arch_str);
            }

            for (k, v) in &self.defines {
                ctx.define_var(k, v);
            }

            for inc in &self.include_dirs {
                ctx.add_include_path(inc);
            }

            if let Some(ref p_target) = self.preprocess_only {
                let prep = crate::wix::Preprocessor::new();
                let preprocessed = prep.process(&content, &mut ctx)?;
                let out_file = if p_target.as_os_str().is_empty() {
                    src_path.with_extension("pp.xml")
                } else {
                    p_target.clone()
                };
                ensure_parent_dir_exists(&out_file);
                fs::write(&out_file, preprocessed)?;
                outputs.push(out_file);
                continue;
            }

            let obj = crate::wix::compile_wix(&content, &mut ctx)?;
            let bytes = obj.serialize();

            let out_file = match &self.output {
                None => src_path.with_extension("wixobj"),
                Some(out_target) => {
                    if self.sources.len() == 1 && !out_target.is_dir() {
                        out_target.clone()
                    } else {
                        let stem = src_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("output");
                        out_target.join(format!("{stem}.wixobj"))
                    }
                }
            };

            ensure_parent_dir_exists(&out_file);

            fs::write(&out_file, bytes)?;

            outputs.push(out_file);
        }

        Ok(outputs)
    }
}

/// Helper expanding `@response_file` arguments recursively.
fn expand_response_files(args: &[String]) -> Result<Vec<String>> {
    let mut expanded = Vec::new();
    for arg in args {
        if let Some(rsp_path_str) = arg.strip_prefix('@') {
            let rsp_path = PathBuf::from(rsp_path_str);
            let content = fs::read_to_string(&rsp_path)?;
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    expanded.push(trimmed.to_string());
                }
            }
        } else {
            expanded.push(arg.clone());
        }
    }
    Ok(expanded)
}

/// Command-line options for the `light` linker/binder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct LightOptions {
    /// Suppress copyright banner output (`-nologo`).
    pub nologo: bool,
    /// `WiX` extensions to load (e.g. `WixUIExtension`) (`-ext`).
    pub extensions: Vec<String>,
    /// Ordered culture priority list (e.g. `["en-us", "de-de"]`) (`-cultures:`).
    pub cultures: Vec<String>,
    /// Localization files (`.wxl`) to load (`-loc`).
    pub loc_files: Vec<PathBuf>,
    /// Base directory search paths for binding payload files (`-b`).
    pub base_dirs: Vec<PathBuf>,
    /// Named bind paths mapping identifier to directory (`-bd`).
    pub bind_paths: HashMap<String, PathBuf>,
    /// Output package path (`-o`, `-out`).
    pub output: Option<PathBuf>,
    /// Suppress all ICE validation (`-sval`).
    pub suppress_ice: bool,
    /// Specific ICE validation rules to selectively run (`-ice:<ICE>`).
    pub selected_ice: Vec<String>,
    /// Specific ICE validation rules to suppress (`-sice:<ICE>`).
    pub suppressed_ice: Vec<String>,
    /// Specific warning IDs to suppress (`-sw<id>`).
    pub suppressed_warnings: Vec<String>,
    /// Suppress all linker warnings (`-swall`).
    pub suppress_all_warnings: bool,
    /// Treat warnings as fatal errors (`-wx`).
    pub warnings_as_errors: bool,
    /// Component-split cabinet layout (one CAB per component).
    pub cab_per_component: bool,
    /// Allow identical rows in database tables (`-ai`).
    pub allow_identical_rows: bool,
    /// Allow unresolved references during linking (`-au`).
    pub allow_unresolved_references: bool,
    /// Bind physical files before assembling cabinets (`-bf`).
    pub bind_files_early: bool,
    /// Drop unrealized registry entries (`-dreg`).
    pub drop_unrealized_registry: bool,
    /// Do not delete temporary binder files on exit (`-notidy`).
    pub no_tidy: bool,
    /// Generate intermediate link output (`.wixpdb`) (`-pdb`).
    pub generate_pdb: bool,
    /// Enforce pedantic warnings (`-pedantic`).
    pub pedantic: bool,
    /// Reuse cabinets from previous build (`-reusecab`).
    pub reuse_cab: bool,
    /// Suppress assembly file processing (`-sa`).
    pub suppress_assemblies: bool,
    /// Suppress ACL and security permission stamping (`-sacl`).
    pub suppress_acls: bool,
    /// Suppress administrative image sequencing actions (`-sadmin`).
    pub suppress_admin_sequences: bool,
    /// Suppress advertisement table generation (`-sadv`).
    pub suppress_advt_sequences: bool,
    /// Suppress file existence validation (`-sc`).
    pub suppress_file_checks: bool,
    /// Suppress payload file layout to disk (`-sf`).
    pub suppress_file_layout: bool,
    /// Suppress automatic file checksum hashing (`-sh`).
    pub suppress_file_hash: bool,
    /// Suppress directory and file layout actions (`-sl`).
    pub suppress_layout: bool,
    /// Suppress intermediate debug database creation (`-spdb`).
    pub suppress_pdb: bool,
    /// Suppress database schema checks (`-ss`).
    pub suppress_schema: bool,
    /// Suppress default UI dialog injection (`-sui`).
    pub suppress_ui: bool,
    /// Timestamp database summary info table (`-ts`).
    pub timestamp_summary_info: bool,
    /// Verbose diagnostic logging (`-v`, `-verbose`).
    pub verbose: bool,
    /// Reusable cabinet caching directory (`-cc`).
    pub cabinet_cache: Option<PathBuf>,
    /// Parallel compression worker thread count (`-ct`).
    pub compression_threads: Option<usize>,
    /// Late preprocessor defines `(Name, Value)` (`-d`).
    pub defines: Vec<(String, String)>,
    /// Drop unrealized directory identifiers (`-dr`).
    pub drop_unrealized_directories: Vec<String>,
    /// Output file path for unreferenced symbols log (`-usf`).
    pub unreferenced_symbols_file: Option<PathBuf>,
    /// Input intermediate `.wixobj` or `.wixlib` object files.
    pub inputs: Vec<PathBuf>,
}

impl LightOptions {
    /// Creates a new empty [`LightOptions`].
    ///
    /// # Returns
    ///
    /// Default empty options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses command-line arguments into a [`LightOptions`] instance.
    ///
    /// Supports both standard Unix style (`-flag`), Windows style (`/flag`),
    /// and response file expansion (`@file`).
    ///
    /// # Arguments
    ///
    /// * `raw_args` - Command-line argument slice.
    ///
    /// # Returns
    ///
    /// Parsed [`LightOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixLinker`] on invalid flags or missing required argument values.
    #[allow(clippy::too_many_lines, clippy::branches_sharing_code)]
    pub fn parse(raw_args: &[String]) -> Result<Self> {
        let args = expand_response_files(raw_args)?;
        let mut opts = Self::new();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if let Some(flag) = strip_flag_prefix(arg) {
                let lower = flag.to_ascii_lowercase();
                if lower == "nologo" {
                    opts.nologo = true;
                    idx += 1;
                } else if lower == "sval" {
                    opts.suppress_ice = true;
                    idx += 1;
                } else if lower == "wx" {
                    opts.warnings_as_errors = true;
                    idx += 1;
                } else if lower == "ai" {
                    opts.allow_identical_rows = true;
                    idx += 1;
                } else if lower == "au" {
                    opts.allow_unresolved_references = true;
                    idx += 1;
                } else if lower == "bf" {
                    opts.bind_files_early = true;
                    idx += 1;
                } else if lower == "dreg" {
                    opts.drop_unrealized_registry = true;
                    idx += 1;
                } else if lower == "notidy" {
                    opts.no_tidy = true;
                    idx += 1;
                } else if lower == "pdb" {
                    opts.generate_pdb = true;
                    idx += 1;
                } else if lower == "pedantic" {
                    opts.pedantic = true;
                    idx += 1;
                } else if lower == "reusecab" {
                    opts.reuse_cab = true;
                    idx += 1;
                } else if lower == "sa" {
                    opts.suppress_assemblies = true;
                    idx += 1;
                } else if lower == "sacl" {
                    opts.suppress_acls = true;
                    idx += 1;
                } else if lower == "sadmin" {
                    opts.suppress_admin_sequences = true;
                    idx += 1;
                } else if lower == "sadv" {
                    opts.suppress_advt_sequences = true;
                    idx += 1;
                } else if lower == "sc" {
                    opts.suppress_file_checks = true;
                    idx += 1;
                } else if lower == "sf" {
                    opts.suppress_file_layout = true;
                    idx += 1;
                } else if lower == "sh" {
                    opts.suppress_file_hash = true;
                    idx += 1;
                } else if lower == "sl" {
                    opts.suppress_layout = true;
                    idx += 1;
                } else if lower == "spdb" {
                    opts.suppress_pdb = true;
                    idx += 1;
                } else if lower == "ss" {
                    opts.suppress_schema = true;
                    idx += 1;
                } else if lower == "sui" {
                    opts.suppress_ui = true;
                    idx += 1;
                } else if lower == "swall" {
                    opts.suppress_all_warnings = true;
                    idx += 1;
                } else if lower == "ts" {
                    opts.timestamp_summary_info = true;
                    idx += 1;
                } else if lower == "v" || lower == "verbose" {
                    opts.verbose = true;
                    idx += 1;
                } else if lower == "cc" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixLinker {
                            message: "missing cabinet cache directory for '-cc'".to_string(),
                        });
                    }
                    opts.cabinet_cache = Some(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if let Some(ct_rest) = lower.strip_prefix("ct") {
                    let ct_str = if ct_rest.is_empty() {
                        idx += 1;
                        if idx >= args.len() {
                            return Err(Error::WixLinker {
                                message: "missing thread count for '-ct'".to_string(),
                            });
                        }
                        &args[idx]
                    } else {
                        &flag[2..]
                    };
                    if let Ok(threads) = ct_str.parse::<usize>() {
                        opts.compression_threads = Some(threads);
                    }
                    idx += 1;
                } else if let Some(dr_rest) = lower.strip_prefix("dr") {
                    let dr_str = if dr_rest.is_empty() {
                        idx += 1;
                        if idx >= args.len() {
                            return Err(Error::WixLinker {
                                message: "missing directory ID for '-dr'".to_string(),
                            });
                        }
                        &args[idx]
                    } else {
                        &flag[2..]
                    };
                    opts.drop_unrealized_directories.push(dr_str.to_string());
                    idx += 1;
                } else if let Some(def_str) = flag.strip_prefix(['d', 'D']) {
                    if let Some((k, v)) = def_str.split_once('=') {
                        opts.defines.push((k.to_string(), v.to_string()));
                    } else {
                        opts.defines.push((def_str.to_string(), "1".to_string()));
                    }
                    idx += 1;
                } else if let Some(usf_rest) = lower.strip_prefix("usf") {
                    let usf_path = if usf_rest.is_empty() {
                        idx += 1;
                        if idx >= args.len() {
                            return Err(Error::WixLinker {
                                message: "missing output path for '-usf'".to_string(),
                            });
                        }
                        &args[idx]
                    } else {
                        &flag[3..]
                    };
                    opts.unreferenced_symbols_file = Some(PathBuf::from(usf_path));
                    idx += 1;
                } else if lower == "ext" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixLinker {
                            message: "missing argument value for '-ext'".to_string(),
                        });
                    }
                    opts.extensions.push(args[idx].clone());
                    idx += 1;
                } else if lower.starts_with("cultures:") {
                    let orig_cult = &flag[9..];
                    for c in orig_cult.split(';') {
                        if !c.is_empty() {
                            opts.cultures.push(c.to_string());
                        }
                    }
                    idx += 1;
                } else if lower == "loc" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixLinker {
                            message: "missing argument value for '-loc'".to_string(),
                        });
                    }
                    opts.loc_files.push(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if lower == "b" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixLinker {
                            message: "missing argument value for '-b'".to_string(),
                        });
                    }
                    opts.base_dirs.push(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if let Some(bd_rest) = lower.strip_prefix("bd") {
                    let rest = if bd_rest.is_empty() {
                        idx += 1;
                        if idx >= args.len() {
                            return Err(Error::WixLinker {
                                message: "missing argument value for '-bd'".to_string(),
                            });
                        }
                        &args[idx]
                    } else {
                        &flag[2..]
                    };
                    if let Some((id, path)) = rest.split_once('=') {
                        opts.bind_paths.insert(id.to_string(), PathBuf::from(path));
                    }
                    idx += 1;
                } else if lower == "out" || lower == "o" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixLinker {
                            message: "missing argument value for '-out'".to_string(),
                        });
                    }
                    opts.output = Some(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if let Some(ice) = flag.strip_prefix("ice:") {
                    opts.selected_ice.push(ice.to_string());
                    idx += 1;
                } else if let Some(sice) = flag.strip_prefix("sice:") {
                    opts.suppressed_ice.push(sice.to_string());
                    idx += 1;
                } else if let Some(sw) = lower.strip_prefix("sw") {
                    opts.suppressed_warnings.push(sw.to_string());
                    idx += 1;
                } else {
                    idx += 1;
                }
            } else {
                opts.inputs.push(PathBuf::from(arg));
                idx += 1;
            }
        }

        if opts.inputs.is_empty() {
            return Err(Error::WixLinker {
                message: "no input objects specified for linking".to_string(),
            });
        }

        Ok(opts)
    }

    /// Executes the linker to produce the final `.msi` package.
    ///
    /// # Returns
    ///
    /// Path to the written output `.msi` file.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on linking, binding, or I/O failure.
    pub fn execute(&self) -> Result<PathBuf> {
        let mut linker = Linker::new();

        // 1. Ingest intermediate input files (.wixobj, .wixlib)
        for input_path in &self.inputs {
            let data = fs::read(input_path)?;

            if let Ok(lib) = WixLibrary::from_bytes(&data) {
                linker.add_library(lib);
            } else {
                let obj = WixObject::deserialize(&data)?;
                linker.add_object(obj);
            }
        }

        // 2. Configure Linker options
        for dir in &self.base_dirs {
            linker.add_base_dir(dir);
        }
        for (k, v) in &self.bind_paths {
            linker.add_bind_path(k, v);
        }
        if self.suppress_ice {
            linker.set_suppress_ice(true);
        }
        for ice in &self.selected_ice {
            linker.select_ice(ice);
        }
        for ice in &self.suppressed_ice {
            linker.suppress_ice(ice);
        }
        for sw in &self.suppressed_warnings {
            linker.suppress_warning(sw);
        }
        if self.warnings_as_errors {
            linker.set_warnings_as_errors(true);
        }
        if !self.cultures.is_empty() {
            linker.set_cultures(self.cultures.clone());
        }
        if self.cab_per_component {
            linker.set_cab_per_component(true);
        }

        // 3. Ingest localization files (.wxl)
        let mut loc_catalog = crate::wix::localization::LocalizationCatalog::new();
        for loc_path in &self.loc_files {
            let content = fs::read_to_string(loc_path)?;
            let doc = WixLocalization::parse(&content)?;
            loc_catalog.add_document(doc);
        }
        linker.set_localization_catalog(loc_catalog);

        // 4. Perform linking and binding
        let mut db = linker.link()?;

        // Extensions (e.g. UI)
        let mut needs_ui = false;
        for ext in &self.extensions {
            let e = ext.to_ascii_lowercase();
            if e == "wixuiextension" || e == "wixtoolset.ui.wixext" {
                needs_ui = true;
                break;
            }
        }
        if needs_ui && db.get_records("Dialog").is_empty() {
            let _ = inject_ui_library(&mut db, WixUiDialogSet::InstallDir, None, None, None);
        }

        // 6. Build and save Package
        let cabs = linker.take_embedded_cabinets();
        let package = Package::from_database(db, cabs);

        let out_path = self
            .output
            .clone()
            .unwrap_or_else(|| self.inputs[0].with_extension("msi"));

        ensure_parent_dir_exists(&out_path);

        package.save(&out_path)?;
        Ok(out_path)
    }
}

/// Subcommand selection for the unified `wix` tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WixSubcommand {
    /// Build command compiling and linking sources directly into an MSI.
    Build(WixBuildOptions),
}

/// Command-line options for `wix build`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WixBuildOptions {
    /// Target architecture (`x86`, `x64`, `arm64`).
    pub arch: Option<String>,
    /// Extension identifiers or paths.
    pub extensions: Vec<String>,
    /// Primary culture specification.
    pub culture: Option<String>,
    /// Base directory path for payload resolution.
    pub base_dirs: Vec<PathBuf>,
    /// Output package path.
    pub output: Option<PathBuf>,
    /// Preprocessor variable definitions.
    pub defines: Vec<(String, String)>,
    /// Include search directories for preprocessor.
    pub include_dirs: Vec<PathBuf>,
    /// Suppress ICE validation.
    pub suppress_ice: bool,
    /// Source files (`.wxs`, `.wxl`, etc.).
    pub sources: Vec<PathBuf>,
}

impl WixBuildOptions {
    /// Creates a new empty [`WixBuildOptions`].
    ///
    /// # Returns
    ///
    /// Default empty build options.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses command-line arguments for `wix build`.
    ///
    /// # Arguments
    ///
    /// * `args` - Argument slice (starting after `build`).
    ///
    /// # Returns
    ///
    /// Parsed [`WixBuildOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] on missing flags or required arguments.
    #[allow(clippy::branches_sharing_code)]
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut opts = Self::new();
        let mut idx = usize::from(!args.is_empty() && args[0].eq_ignore_ascii_case("build"));

        while idx < args.len() {
            let arg = &args[idx];

            if let Some(flag) = strip_flag_prefix(arg) {
                let lower = flag.to_ascii_lowercase();
                if lower == "arch" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "wix".to_string(),
                            message: "missing argument value for '-arch'".to_string(),
                        });
                    }
                    opts.arch = Some(args[idx].clone());
                    idx += 1;
                } else if lower == "ext" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "wix".to_string(),
                            message: "missing argument value for '-ext'".to_string(),
                        });
                    }
                    opts.extensions.push(args[idx].clone());
                    idx += 1;
                } else if lower == "culture" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "wix".to_string(),
                            message: "missing argument value for '-culture'".to_string(),
                        });
                    }
                    opts.culture = Some(args[idx].clone());
                    idx += 1;
                } else if lower == "b" || lower == "bind-path" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "wix".to_string(),
                            message: "missing argument value for '-b'".to_string(),
                        });
                    }
                    opts.base_dirs.push(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if lower == "i" || lower == "include" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "wix".to_string(),
                            message: "missing argument value for '-I'".to_string(),
                        });
                    }
                    opts.include_dirs.push(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if lower == "o" || lower == "out" || lower == "output" {
                    idx += 1;
                    if idx >= args.len() {
                        return Err(Error::WixCompiler {
                            element: "wix".to_string(),
                            message: "missing argument value for '-o'".to_string(),
                        });
                    }
                    opts.output = Some(PathBuf::from(&args[idx]));
                    idx += 1;
                } else if let Some(def) = flag.strip_prefix(['d', 'D']) {
                    if let Some((k, v)) = def.split_once('=') {
                        opts.defines.push((k.to_string(), v.to_string()));
                    } else {
                        opts.defines.push((def.to_string(), "1".to_string()));
                    }
                    idx += 1;
                } else if lower == "sval" || lower == "suppress-validation" {
                    opts.suppress_ice = true;
                    idx += 1;
                } else {
                    idx += 1;
                }
            } else {
                opts.sources.push(PathBuf::from(arg));
                idx += 1;
            }
        }

        if opts.sources.is_empty() {
            return Err(Error::WixCompiler {
                element: "wix".to_string(),
                message: "no source files specified for wix build".to_string(),
            });
        }

        Ok(opts)
    }

    /// Compiles and links sources directly into an `.msi` package.
    ///
    /// # Returns
    ///
    /// Path to the generated `.msi` package.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on compilation, linking, or I/O failure.
    pub fn execute(&self) -> Result<PathBuf> {
        let mut wxs_sources = Vec::new();
        let mut loc_files = Vec::new();
        let mut obj_files = Vec::new();
        let mut lib_files = Vec::new();

        for s in &self.sources {
            if s.extension().is_some_and(|e| e.eq_ignore_ascii_case("wxl")) {
                loc_files.push(s.clone());
            } else if s
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wixobj"))
            {
                obj_files.push(s.clone());
            } else if s
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wixlib"))
            {
                lib_files.push(s.clone());
            } else {
                wxs_sources.push(s.clone());
            }
        }

        let mut linker = Linker::new();

        // 1. Ingest libraries (.wixlib)
        for lib_path in &lib_files {
            let data = fs::read(lib_path)?;
            let lib = WixLibrary::from_bytes(&data)?;
            linker.add_library(lib);
        }

        // 2. Ingest intermediate objects (.wixobj)
        for obj_path in &obj_files {
            let data = fs::read(obj_path)?;
            let obj = WixObject::deserialize(&data)?;
            linker.add_object(obj);
        }

        // 3. Compile each .wxs source file to WixObject
        for src in &wxs_sources {
            let content = fs::read_to_string(src)?;

            let mut ctx = PreprocessorContext::new();
            if let Some(ref arch_str) = self.arch {
                ctx.define_var("sys.BUILDARCH", arch_str);
                ctx.define_var("arch", arch_str);
            }
            for (k, v) in &self.defines {
                ctx.define_var(k, v);
            }
            for inc in &self.include_dirs {
                ctx.add_include_path(inc);
            }

            let obj = crate::wix::compile_wix(&content, &mut ctx)?;
            linker.add_object(obj);
        }

        // 4. Configure Linker
        for dir in &self.base_dirs {
            linker.add_base_dir(dir);
        }
        if self.suppress_ice {
            linker.set_suppress_ice(true);
        }
        if let Some(ref c) = self.culture {
            linker.set_cultures(vec![c.clone()]);
        }

        // Ingest localization
        let mut loc_catalog = crate::wix::localization::LocalizationCatalog::new();
        for loc_path in &loc_files {
            let content = fs::read_to_string(loc_path)?;
            let doc = WixLocalization::parse(&content)?;
            loc_catalog.add_document(doc);
        }
        linker.set_localization_catalog(loc_catalog);

        // 5. Link and bind
        let mut db = linker.link()?;

        // Extensions (e.g. UI)
        let mut needs_ui = false;
        for ext in &self.extensions {
            let e = ext.to_ascii_lowercase();
            if e == "wixuiextension" || e == "wixtoolset.ui.wixext" {
                needs_ui = true;
                break;
            }
        }
        if needs_ui && db.get_records("Dialog").is_empty() {
            let _ = inject_ui_library(&mut db, WixUiDialogSet::InstallDir, None, None, None);
        }

        let cabs = linker.take_embedded_cabinets();
        let package = Package::from_database(db, cabs);

        let out_path = self
            .output
            .clone()
            .unwrap_or_else(|| self.sources[0].with_extension("msi"));

        ensure_parent_dir_exists(&out_path);

        package.save(&out_path)?;
        Ok(out_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests parsing `CandleOptions` and compiling `WiX` documents.
    #[test]
    fn test_candle_options_parse_and_execute() {
        let temp_dir = std::env::temp_dir().join("msi_test_candle");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("app.wxs");
        let out_file = temp_dir.join("custom_out.wixobj");

        let wxs_content = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="CandleApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs_content).is_ok());

        let args = vec![
            "-nologo".to_string(),
            "-arch".to_string(),
            "x64".to_string(),
            "-dAppDef=1".to_string(),
            "-dSimpleDef".to_string(),
            "-ext".to_string(),
            "WixUIExtension".to_string(),
            format!("-I{}", temp_dir.display()),
            "-out".to_string(),
            out_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ];

        let opts = CandleOptions::parse(&args).unwrap_or_default();
        assert!(opts.nologo);
        assert_eq!(opts.arch.as_deref(), Some("x64"));
        assert_eq!(opts.defines.len(), 2);
        assert_eq!(opts.extensions, vec!["WixUIExtension"]);
        assert_eq!(opts.output, Some(out_file.clone()));
        assert_eq!(opts.sources, vec![src_file]);

        let outputs = opts.execute().unwrap_or_default();
        assert_eq!(outputs.len(), 1);
        assert!(out_file.exists());

        // Test missing source files error
        assert!(CandleOptions::parse(&["-nologo".to_string()]).is_err());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests `CandleOptions` argument parsing validation and error reporting.
    #[test]
    fn test_candle_options_parse_errors() {
        assert!(CandleOptions::parse(&["-arch".to_string()]).is_err());
        assert!(CandleOptions::parse(&["-ext".to_string()]).is_err());
        assert!(CandleOptions::parse(&["-I".to_string()]).is_err());
        assert!(CandleOptions::parse(&["-out".to_string()]).is_err());
        assert!(CandleOptions::parse(&["-cc".to_string()]).is_err());
        assert!(CandleOptions::parse(&["@nonexistent_rsp.txt".to_string()]).is_err());
    }

    /// Tests extended compiler and linker flags, response file reading, and preprocess modes.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_candle_and_light_extended_flags_and_response_file() {
        let temp_dir = std::env::temp_dir().join("msi_test_toolchain_extended");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("ext.wxs");
        let rsp_file = temp_dir.join("flags.rsp");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{55555555-5555-5555-5555-555555555555}" Name="ExtApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        // Response file with flags
        let rsp_content = format!(
            "-nologo
# comment line

-arch
x64
-fips
-pedantic
-q
-ss
-swall
-sw1075
-v
-wx
-cc
{}
{}
",
            temp_dir.display(),
            src_file.display()
        );
        assert!(fs::write(&rsp_file, rsp_content).is_ok());

        let c_opts =
            CandleOptions::parse(&[format!("@{}", rsp_file.display())]).unwrap_or_default();
        assert!(c_opts.nologo);
        assert_eq!(c_opts.arch.as_deref(), Some("x64"));
        assert!(c_opts.fips);
        assert!(c_opts.pedantic);
        assert!(c_opts.quiet);
        assert!(c_opts.suppress_schema);
        assert!(c_opts.suppress_all_warnings);
        assert_eq!(c_opts.suppressed_warnings, vec!["1075"]);
        assert!(c_opts.verbose);
        assert!(c_opts.warnings_as_errors);
        assert_eq!(c_opts.cache_dir, Some(temp_dir.clone()));
        assert_eq!(c_opts.sources, vec![src_file.clone()]);

        // Preprocess-only tests (-p stdout/default and -p file)
        let c_prep_default =
            CandleOptions::parse(&["-p".to_string(), src_file.to_string_lossy().to_string()])
                .unwrap_or_default();
        let prep_outs1 = c_prep_default.execute().unwrap_or_default();
        assert_eq!(prep_outs1.len(), 1);
        assert!(prep_outs1[0].exists());

        let pp_file = temp_dir.join("nested").join("custom.pp.xml");
        let c_prep_custom = CandleOptions::parse(&[
            format!("-p{}", pp_file.display()),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default();
        let prep_outs2 = c_prep_custom.execute().unwrap_or_default();
        assert_eq!(prep_outs2[0], pp_file);
        assert!(pp_file.exists());

        // Light extended flags and parse errors
        assert!(LightOptions::parse(&["-cc".to_string()]).is_err());
        assert!(LightOptions::parse(&["-ct".to_string()]).is_err());
        assert!(LightOptions::parse(&["-dr".to_string()]).is_err());
        assert!(LightOptions::parse(&["-usf".to_string()]).is_err());

        let usf_file = temp_dir.join("symbols.txt");
        let obj_file = src_file.with_extension("wixobj");
        let _ = c_opts.execute();

        let light_args = vec![
            "-ai".to_string(),
            "-au".to_string(),
            "-bf".to_string(),
            "-dreg".to_string(),
            "-notidy".to_string(),
            "-pdb".to_string(),
            "-pedantic".to_string(),
            "-reusecab".to_string(),
            "-sa".to_string(),
            "-sacl".to_string(),
            "-sadmin".to_string(),
            "-sadv".to_string(),
            "-sc".to_string(),
            "-sf".to_string(),
            "-sh".to_string(),
            "-sl".to_string(),
            "-spdb".to_string(),
            "-ss".to_string(),
            "-sui".to_string(),
            "-swall".to_string(),
            "-ts".to_string(),
            "-verbose".to_string(),
            "-cc".to_string(),
            temp_dir.to_string_lossy().to_string(),
            "-ct8".to_string(),
            "-dLateVar=1".to_string(),
            "-drTARGETDIR".to_string(),
            format!("-usf{}", usf_file.display()),
            "-sval".to_string(),
            obj_file.to_string_lossy().to_string(),
        ];
        let l_opts = LightOptions::parse(&light_args).unwrap_or_default();
        assert!(l_opts.allow_identical_rows);
        assert!(l_opts.allow_unresolved_references);
        assert!(l_opts.bind_files_early);
        assert!(l_opts.drop_unrealized_registry);
        assert!(l_opts.no_tidy);
        assert!(l_opts.generate_pdb);
        assert!(l_opts.pedantic);
        assert!(l_opts.reuse_cab);
        assert!(l_opts.suppress_assemblies);
        assert!(l_opts.suppress_acls);
        assert!(l_opts.suppress_admin_sequences);
        assert!(l_opts.suppress_advt_sequences);
        assert!(l_opts.suppress_file_checks);
        assert!(l_opts.suppress_file_layout);
        assert!(l_opts.suppress_file_hash);
        assert!(l_opts.suppress_layout);
        assert!(l_opts.suppress_pdb);
        assert!(l_opts.suppress_schema);
        assert!(l_opts.suppress_ui);
        assert!(l_opts.suppress_all_warnings);
        assert!(l_opts.timestamp_summary_info);
        assert!(l_opts.verbose);
        assert_eq!(l_opts.cabinet_cache, Some(temp_dir.clone()));
        assert_eq!(l_opts.compression_threads, Some(8));
        assert_eq!(l_opts.defines.len(), 1);
        assert_eq!(l_opts.drop_unrealized_directories, vec!["TARGETDIR"]);
        assert_eq!(l_opts.unreferenced_symbols_file, Some(usf_file));

        let msi_res = l_opts.execute().unwrap_or_default();
        assert!(msi_res.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests `LightOptions` parsing and execution to produce an MSI package.
    #[test]
    fn test_light_options_parse_and_execute() {
        let temp_dir = std::env::temp_dir().join("msi_test_light");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("test.wxs");
        let obj_file = temp_dir.join("test.wixobj");
        let msi_file = temp_dir.join("nested").join("test.msi");

        let wxs_content = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{22222222-2222-2222-2222-222222222222}" Name="LightApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs_content).is_ok());

        // First compile to .wixobj
        let mut ctx = PreprocessorContext::new();
        let obj = crate::wix::compile_wix(wxs_content, &mut ctx).unwrap_or_default();
        assert!(fs::write(&obj_file, obj.serialize()).is_ok());

        let args = vec![
            "-nologo".to_string(),
            "-sval".to_string(),
            "-wx".to_string(),
            "-ext".to_string(),
            "WixUIExtension".to_string(),
            "-cultures:en-us;de-de".to_string(),
            "-b".to_string(),
            temp_dir.to_string_lossy().to_string(),
            format!("-bdBind1={}", temp_dir.display()),
            "-ice:ICE01".to_string(),
            "-sice:ICE38".to_string(),
            "-sw101".to_string(),
            "-out".to_string(),
            msi_file.to_string_lossy().to_string(),
            obj_file.to_string_lossy().to_string(),
        ];

        let mut opts = LightOptions::parse(&args).unwrap_or_default();
        opts.cab_per_component = true;
        assert!(opts.nologo);
        assert!(opts.suppress_ice);
        assert!(opts.warnings_as_errors);
        assert_eq!(opts.cultures, vec!["en-us", "de-de"]);
        assert_eq!(opts.base_dirs, vec![temp_dir.clone()]);
        assert!(opts.bind_paths.contains_key("Bind1"));
        assert_eq!(opts.selected_ice, vec!["ICE01"]);
        assert_eq!(opts.suppressed_ice, vec!["ICE38"]);
        assert_eq!(opts.suppressed_warnings, vec!["101"]);
        assert_eq!(opts.output, Some(msi_file.clone()));

        let produced = opts.execute().unwrap_or_default();
        assert_eq!(produced, msi_file);
        assert!(msi_file.exists());

        // Test missing input objects error
        assert!(LightOptions::parse(&["-nologo".to_string()]).is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests `LightOptions` argument parse error conditions.
    #[test]
    fn test_light_options_parse_errors() {
        assert!(LightOptions::parse(&["-ext".to_string()]).is_err());
        assert!(LightOptions::parse(&["-loc".to_string()]).is_err());
        assert!(LightOptions::parse(&["-b".to_string()]).is_err());
        assert!(LightOptions::parse(&["-bd".to_string()]).is_err());
        assert!(LightOptions::parse(&["-out".to_string()]).is_err());
    }

    /// Tests `WixBuildOptions` parsing and end-to-end execution.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_wix_build_options_parse_and_execute() {
        let temp_dir = std::env::temp_dir().join("msi_test_wix_build");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("wix_app.wxs");
        let loc_file = temp_dir.join("strings.wxl");
        let msi_file = temp_dir.join("nested").join("wix_app.msi");

        let wxs_content = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{33333333-3333-3333-3333-333333333333}" Name="WixBuildApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="!(loc.PackageDesc)" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        let wxl_source = r#"
<WixLocalization Culture="en-US">
    <String Id="PackageDesc">Wix Build Description</String>
</WixLocalization>
"#;
        assert!(fs::write(&src_file, wxs_content).is_ok());
        assert!(fs::write(&loc_file, wxl_source).is_ok());

        let args = vec![
            "--arch".to_string(),
            "x64".to_string(),
            "--ext".to_string(),
            "WixToolset.UI.wixext".to_string(),
            "--culture".to_string(),
            "en-US".to_string(),
            "--bind-path".to_string(),
            temp_dir.to_string_lossy().to_string(),
            "-dAppDef=Val".to_string(),
            "-dFlagOnly".to_string(),
            "--suppress-validation".to_string(),
            "--output".to_string(),
            msi_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
            loc_file.to_string_lossy().to_string(),
        ];

        let opts = WixBuildOptions::parse(&args).unwrap_or_default();
        assert_eq!(opts.arch.as_deref(), Some("x64"));
        assert_eq!(opts.extensions, vec!["WixToolset.UI.wixext"]);
        assert_eq!(opts.culture.as_deref(), Some("en-US"));
        assert_eq!(opts.defines.len(), 2);
        assert!(opts.suppress_ice);
        assert_eq!(opts.sources.len(), 2);

        let out = opts.execute().unwrap_or_default();
        assert_eq!(out, msi_file);
        assert!(msi_file.exists());

        // Test build with -i, -include, and .wixlib library ingestion
        let dummy_lib = WixLibrary::new(Vec::new());
        let lib_path = temp_dir.join("test.wixlib");
        assert!(fs::write(&lib_path, dummy_lib.to_bytes()).is_ok());
        let inc_dir = temp_dir.join("includes");
        assert!(fs::create_dir_all(&inc_dir).is_ok());

        let build_with_lib_args = vec![
            "build".to_string(),
            "-i".to_string(),
            inc_dir.to_string_lossy().to_string(),
            "-include".to_string(),
            inc_dir.to_string_lossy().to_string(),
            "--suppress-validation".to_string(),
            "-o".to_string(),
            msi_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
            loc_file.to_string_lossy().to_string(),
            lib_path.to_string_lossy().to_string(),
        ];
        let lib_opts = WixBuildOptions::parse(&build_with_lib_args).unwrap_or_default();
        assert_eq!(lib_opts.include_dirs.len(), 2);
        let lib_out = lib_opts.execute().unwrap_or_default();
        assert_eq!(lib_out, msi_file);

        // Test build with .wixobj intermediate object
        let mut candle_opts = CandleOptions::new();
        candle_opts.sources.push(src_file.clone());
        candle_opts.output = Some(temp_dir.clone());
        let candle_res = candle_opts.execute().unwrap_or_default();
        assert!(!candle_res.is_empty());
        let obj_path = candle_res[0].clone();
        assert!(obj_path.exists());

        let build_with_obj_args = vec![
            "build".to_string(),
            "--suppress-validation".to_string(),
            "-o".to_string(),
            msi_file.to_string_lossy().to_string(),
            obj_path.to_string_lossy().to_string(),
            loc_file.to_string_lossy().to_string(),
        ];
        let obj_opts = WixBuildOptions::parse(&build_with_obj_args).unwrap_or_default();
        let obj_out = obj_opts.execute().unwrap_or_default();
        assert_eq!(obj_out, msi_file);

        // Test default output derivation when self.output is None
        let build_no_out_args = vec![
            "build".to_string(),
            "--suppress-validation".to_string(),
            src_file.to_string_lossy().to_string(),
            loc_file.to_string_lossy().to_string(),
        ];
        let no_out_opts = WixBuildOptions::parse(&build_no_out_args).unwrap_or_default();
        assert_eq!(no_out_opts.output, None);
        let auto_out = no_out_opts.execute().unwrap_or_default();
        assert_eq!(auto_out, src_file.with_extension("msi"));
        let _ = fs::remove_file(&auto_out);

        // Parse error tests
        assert!(WixBuildOptions::parse(&[]).is_err());
        assert!(WixBuildOptions::parse(&["-arch".to_string()]).is_err());
        assert!(WixBuildOptions::parse(&["-ext".to_string()]).is_err());
        assert!(WixBuildOptions::parse(&["-culture".to_string()]).is_err());
        assert!(WixBuildOptions::parse(&["-b".to_string()]).is_err());
        assert!(WixBuildOptions::parse(&["-o".to_string()]).is_err());
        assert!(WixBuildOptions::parse(&["--suppress-validation".to_string()]).is_err());
        assert!(WixBuildOptions::parse(&["-I".to_string()]).is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests edge cases, fallback paths, Windows slash flag variations, and unknown arguments.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_toolchain_edge_cases() {
        let temp_dir = std::env::temp_dir().join("msi_test_toolchain_edge");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("edge.wxs");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{44444444-4444-4444-4444-444444444444}" Name="EdgeApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        // Test defaults
        assert_eq!(CandleOptions::new(), CandleOptions::default());
        assert_eq!(LightOptions::new(), LightOptions::default());
        assert_eq!(WixBuildOptions::new(), WixBuildOptions::default());

        // Test WixSubcommand
        let sub = WixSubcommand::Build(WixBuildOptions::new());
        assert_eq!(sub.clone(), sub);
        assert!(format!("{sub:?}").contains("Build"));

        // Test candle with Windows slash flags, separate -I, and directory output
        let args_slash = vec![
            "/nologo".to_string(),
            "/arch".to_string(),
            "x86".to_string(),
            "/dVarOnly".to_string(),
            "/ext".to_string(),
            "WixUIExtension".to_string(),
            "-I".to_string(),
            temp_dir.to_string_lossy().to_string(),
            "-o".to_string(),
            temp_dir.to_string_lossy().to_string(),
            "-quiet".to_string(),
            "-unknown-flag".to_string(),
            src_file.to_string_lossy().to_string(),
        ];
        let c_opts = CandleOptions::parse(&args_slash).unwrap_or_default();
        assert!(c_opts.nologo);
        assert!(c_opts.quiet);
        assert_eq!(c_opts.arch.as_deref(), Some("x86"));
        let outs = c_opts.execute().unwrap_or_default();
        assert_eq!(outs.len(), 1);

        // Test candle without output specified (fallback to .wixobj next to source)
        let c_no_out =
            CandleOptions::parse(&[src_file.to_string_lossy().to_string()]).unwrap_or_default();
        let outs2 = c_no_out.execute().unwrap_or_default();
        assert_eq!(outs2[0], src_file.with_extension("wixobj"));

        // Test candle execution with output filename without parent directory
        let local_wixobj = PathBuf::from("toolchain_edge_local.wixobj");
        let c_local = CandleOptions {
            sources: vec![src_file.clone()],
            output: Some(local_wixobj.clone()),
            ..CandleOptions::new()
        };
        let c_local_outs = c_local.execute().unwrap_or_default();
        assert_eq!(c_local_outs, vec![local_wixobj.clone()]);
        assert!(local_wixobj.exists());
        let _ = fs::remove_file(&local_wixobj);

        // Test candle execution with preprocess_only filename without parent directory
        let local_pp = PathBuf::from("toolchain_edge_local.pp.xml");
        let c_pp_local = CandleOptions {
            sources: vec![src_file.clone()],
            preprocess_only: Some(local_pp.clone()),
            ..CandleOptions::new()
        };
        let c_pp_outs = c_pp_local.execute().unwrap_or_default();
        assert_eq!(c_pp_outs, vec![local_pp.clone()]);
        assert!(local_pp.exists());
        let _ = fs::remove_file(&local_pp);

        // Test candle execution with multiple sources into a directory
        let src_file2 = temp_dir.join("edge2.wxs");
        assert!(fs::write(&src_file2, wxs).is_ok());
        let c_multi = CandleOptions {
            sources: vec![src_file.clone(), src_file2],
            output: Some(temp_dir.clone()),
            ..CandleOptions::new()
        };
        let multi_outs = c_multi.execute().unwrap_or_default();
        assert_eq!(multi_outs.len(), 2);

        // Test single source with directory output (covers !out_target.is_dir() false branch)
        let c_single_dir = CandleOptions {
            sources: vec![src_file.clone()],
            output: Some(temp_dir.clone()),
            ..CandleOptions::new()
        };
        let single_dir_outs = c_single_dir.execute().unwrap_or_default();
        assert_eq!(single_dir_outs.len(), 1);

        // Test LightOptions with non-matching and WixToolset.UI.wixext extensions
        let l_ext_test = LightOptions {
            inputs: vec![outs[0].clone()],
            extensions: vec![
                "OtherExtension".to_string(),
                "WixToolset.UI.wixext".to_string(),
            ],
            suppress_ice: true,
            output: Some(temp_dir.join("ext_test.msi")),
            ..LightOptions::new()
        };
        let l_ext_built = l_ext_test.execute().unwrap_or_default();
        assert!(l_ext_built.exists());

        // Test WixBuildOptions with non-matching and WixToolset.UI.wixext extensions
        let w_ext_test = WixBuildOptions {
            sources: vec![src_file.clone()],
            extensions: vec![
                "OtherExtension".to_string(),
                "WixToolset.UI.wixext".to_string(),
            ],
            suppress_ice: true,
            output: Some(temp_dir.join("w_ext_test.msi")),
            ..WixBuildOptions::new()
        };
        let w_ext_built = w_ext_test.execute().unwrap_or_default();
        assert!(w_ext_built.exists());

        // Test LightOptions and WixBuildOptions with WixUIExtension and pre-existing Dialog table
        let wxs_with_dialog = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{66666666-6666-6666-6666-666666666666}" Name="DialogApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
        <UI>
            <Dialog Id="ExistingDlg" Width="200" Height="100" Title="Existing" />
        </UI>
    </Product>
</Wix>
"#;
        let dlg_src_file = temp_dir.join("dlg_app.wxs");
        assert!(fs::write(&dlg_src_file, wxs_with_dialog).is_ok());

        // WixBuildOptions with WixUIExtension and existing Dialog
        let w_dlg = WixBuildOptions {
            sources: vec![dlg_src_file],
            extensions: vec!["WixUIExtension".to_string()],
            suppress_ice: true,
            output: Some(temp_dir.join("w_dlg.msi")),
            ..WixBuildOptions::new()
        };
        let w_dlg_out = w_dlg.execute().unwrap_or_default();
        assert!(w_dlg_out.exists());

        // LightOptions with WixUIExtension and existing Dialog
        let mut dlg_ctx = PreprocessorContext::new();
        let dlg_obj = crate::wix::compile_wix(wxs_with_dialog, &mut dlg_ctx).unwrap_or_default();
        let dlg_obj_file = temp_dir.join("dlg_app.wixobj");
        assert!(fs::write(&dlg_obj_file, dlg_obj.serialize()).is_ok());

        let l_dlg = LightOptions {
            inputs: vec![dlg_obj_file],
            extensions: vec!["WixUIExtension".to_string()],
            suppress_ice: true,
            output: Some(temp_dir.join("l_dlg.msi")),
            ..LightOptions::new()
        };
        let l_dlg_out = l_dlg.execute().unwrap_or_default();
        assert!(l_dlg_out.exists());

        // Test light with Windows slash flags, separate arguments, and unknown flags
        let msi_out = temp_dir.join("edge.msi");
        let light_slash_args = vec![
            "/nologo".to_string(),
            "/sval".to_string(),
            "/wx".to_string(),
            "/ext".to_string(),
            "WixToolset.UI.wixext".to_string(),
            "/cultures:en-us;;de-de".to_string(),
            "/b".to_string(),
            temp_dir.to_string_lossy().to_string(),
            "-bd".to_string(),
            format!("BindEdge={}", temp_dir.display()),
            "-bd".to_string(),
            "InvalidBindNoEquals".to_string(),
            "-ct".to_string(),
            "invalid_threads".to_string(),
            "-dr".to_string(),
            "TARGETDIR".to_string(),
            "-dFlagOnly".to_string(),
            "-usf".to_string(),
            temp_dir.join("usf.txt").to_string_lossy().to_string(),
            "/ice:ICE01".to_string(),
            "/sice:ICE38".to_string(),
            "/sw101".to_string(),
            "-o".to_string(),
            msi_out.to_string_lossy().to_string(),
            "-unknown-flag".to_string(),
            outs[0].to_string_lossy().to_string(),
        ];
        let l_opts = LightOptions::parse(&light_slash_args).unwrap_or_default();
        assert!(l_opts.nologo);
        assert!(l_opts.suppress_ice);
        assert_eq!(l_opts.compression_threads, None);
        assert_eq!(l_opts.drop_unrealized_directories, vec!["TARGETDIR"]);
        assert_eq!(l_opts.defines.len(), 1);
        let built = l_opts.execute().unwrap_or_default();
        assert!(built.exists());

        // Test light execution with filename without parent directory and without ICE suppression
        let local_msi = PathBuf::from("toolchain_edge_local.msi");
        let l_local = LightOptions {
            inputs: vec![outs[0].clone()],
            output: Some(local_msi.clone()),
            suppress_ice: false,
            ..LightOptions::new()
        };
        let built_local = l_local.execute().unwrap_or_default();
        assert_eq!(built_local, local_msi);
        assert!(local_msi.exists());
        let _ = fs::remove_file(&local_msi);

        // Test light with WixLibrary and without -out
        let lib = WixLibrary::new(Vec::new());
        let lib_file = temp_dir.join("test.wixlib");
        assert!(fs::write(&lib_file, lib.to_bytes()).is_ok());
        let l_lib_args = vec![
            "-sval".to_string(),
            outs[0].to_string_lossy().to_string(),
            lib_file.to_string_lossy().to_string(),
        ];
        let l_lib_opts = LightOptions::parse(&l_lib_args).unwrap_or_default();
        let built_lib = l_lib_opts.execute().unwrap_or_default();
        assert!(built_lib.exists());

        // Test light with localization file
        let loc_file = temp_dir.join("edge.wxl");
        let loc_content = r#"
<WixLocalization Culture="en-US">
    <String Id="LocStr">Localized</String>
</WixLocalization>
"#;
        assert!(fs::write(&loc_file, loc_content).is_ok());
        let l_loc_args = vec![
            "-sval".to_string(),
            "-loc".to_string(),
            loc_file.to_string_lossy().to_string(),
            outs[0].to_string_lossy().to_string(),
        ];
        let l_loc_opts = LightOptions::parse(&l_loc_args).unwrap_or_default();
        let built_loc = l_loc_opts.execute().unwrap_or_default();
        assert!(built_loc.exists());

        // Test wix build without -o (fallback to .msi next to source) and unknown flag
        let w_no_out = WixBuildOptions::parse(&[
            "-sval".to_string(),
            "-dAppDef".to_string(),
            "-unknown-flag".to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default();
        let built_w = w_no_out.execute().unwrap_or_default();
        assert!(built_w.exists());

        // Test wix build execution with filename without parent directory and without ICE suppression
        let local_wix_msi = PathBuf::from("toolchain_edge_wix_local.msi");
        let w_local = WixBuildOptions {
            sources: vec![src_file.clone()],
            output: Some(local_wix_msi.clone()),
            suppress_ice: false,
            ..WixBuildOptions::new()
        };
        let built_w_local = w_local.execute().unwrap_or_default();
        assert_eq!(built_w_local, local_wix_msi);
        assert!(local_wix_msi.exists());
        let _ = fs::remove_file(&local_wix_msi);

        // Test candle with -verbose flag
        let c_verb = CandleOptions::parse(&[
            "-verbose".to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default();
        assert!(c_verb.verbose);

        // Test light with -v flag
        let l_verb = LightOptions::parse(&[
            "-v".to_string(),
            "-sval".to_string(),
            outs[0].to_string_lossy().to_string(),
        ])
        .unwrap_or_default();
        assert!(l_verb.verbose);

        // Test wix build with -out flag
        let w_out = WixBuildOptions::parse(&[
            "-out".to_string(),
            temp_dir.join("w_out.msi").to_string_lossy().to_string(),
            "-sval".to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default();
        assert!(w_out.output.is_some());

        // Test ensure_parent_dir_exists edge cases
        ensure_parent_dir_exists(std::path::Path::new(""));
        ensure_parent_dir_exists(std::path::Path::new("plain_no_dir.txt"));
        ensure_parent_dir_exists(&temp_dir.join("sub_ensure").join("file.txt"));

        // Test execution errors for nonexistent files
        let bad_candle = CandleOptions {
            sources: vec![PathBuf::from("nonexistent_path_12345.wxs")],
            ..CandleOptions::new()
        };
        assert!(bad_candle.execute().is_err());

        let bad_light = LightOptions {
            inputs: vec![PathBuf::from("nonexistent_path_12345.wixobj")],
            ..LightOptions::new()
        };
        assert!(bad_light.execute().is_err());

        let bad_wix = WixBuildOptions {
            sources: vec![PathBuf::from("nonexistent_path_12345.wxs")],
            ..WixBuildOptions::new()
        };
        assert!(bad_wix.execute().is_err());

        // Test is_flag and strip_flag_prefix variations
        assert!(is_flag("-flag"));
        assert!(is_flag("--flag"));
        assert!(is_flag("/flag"));
        assert!(is_flag("@response.rsp"));
        assert!(!is_flag("/a/b/c"));
        assert!(!is_flag("not_a_flag"));

        assert_eq!(strip_flag_prefix("--opt"), Some("opt"));
        assert_eq!(strip_flag_prefix("-opt"), Some("opt"));
        assert_eq!(strip_flag_prefix("/opt"), Some("opt"));
        assert_eq!(strip_flag_prefix("/nested/path"), None);
        assert_eq!(strip_flag_prefix("plain"), None);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests error propagation branches across Candle, Light, and `WixBuild` options.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_toolchain_error_branches() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_toolchain_err_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(&temp_dir).is_ok());

        // 1. Candle preprocess error: source contains invalid preprocessor syntax (line 282)
        let prep_err_file = temp_dir.join("prep_err.wxs");
        assert!(fs::write(&prep_err_file, "<?error PreprocessorFailure?>").is_ok());
        let c_prep_err = CandleOptions {
            sources: vec![prep_err_file],
            preprocess_only: Some(temp_dir.join("out.pp.xml")),
            ..CandleOptions::new()
        };
        assert!(c_prep_err.execute().is_err());

        // 2. Candle preprocess write error: out_file is an existing directory (line 289)
        let valid_wxs = temp_dir.join("valid.wxs");
        assert!(fs::write(&valid_wxs, "<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\"><Product Id=\"{11111111-1111-1111-1111-111111111111}\" Name=\"App\" Version=\"1.0.0\" Manufacturer=\"Test\"><Package Description=\"Test\"/><Directory Id=\"TARGETDIR\" Name=\"SourceDir\"/></Product></Wix>").is_ok());
        let existing_dir = temp_dir.join("dir_blocking_file");
        assert!(fs::create_dir_all(&existing_dir).is_ok());
        let c_prep_write_err = CandleOptions {
            sources: vec![valid_wxs.clone()],
            preprocess_only: Some(existing_dir.clone()),
            ..CandleOptions::new()
        };
        assert!(c_prep_write_err.execute().is_err());

        // 3. Candle compile error: invalid WiX XML (line 294)
        let bad_wix_file = temp_dir.join("bad.wxs");
        assert!(fs::write(&bad_wix_file, "<unclosed tag").is_ok());
        let c_compile_err = CandleOptions {
            sources: vec![bad_wix_file.clone()],
            ..CandleOptions::new()
        };
        assert!(c_compile_err.execute().is_err());

        // 4. Candle write obj error: out_file is blocked by a directory (line 314)
        let blocked_obj = temp_dir.join("blocked_obj.wixobj");
        assert!(fs::create_dir_all(&blocked_obj).is_ok());
        assert!(fs::create_dir_all(blocked_obj.join("valid.wixobj")).is_ok());
        let c_obj_blocked = CandleOptions {
            sources: vec![valid_wxs.clone()],
            output: Some(blocked_obj),
            ..CandleOptions::new()
        };
        assert!(c_obj_blocked.execute().is_err());

        // 5. LightOptions::parse response file error (line 460)
        assert!(LightOptions::parse(&["@nonexistent_light_rsp.rsp".to_string()]).is_err());

        // 6. LightOptions::execute deserialization error: invalid obj data (line 710)
        let corrupt_obj = temp_dir.join("corrupt.wixobj");
        assert!(fs::write(&corrupt_obj, b"not a valid wixobj or wixlib").is_ok());
        let l_corrupt_obj = LightOptions {
            inputs: vec![corrupt_obj.clone()],
            ..LightOptions::new()
        };
        assert!(l_corrupt_obj.execute().is_err());

        // 7. LightOptions::execute localization read error & parse error (line 747, 748)
        let valid_obj_file = temp_dir.join("valid.wixobj");
        let mut ctx = PreprocessorContext::new();
        let valid_obj = crate::wix::compile_wix("<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\"><Product Id=\"{22222222-2222-2222-2222-222222222222}\" Name=\"App\" Version=\"1.0.0\" Manufacturer=\"Test\"><Package Description=\"Test\"/><Directory Id=\"TARGETDIR\" Name=\"SourceDir\"/></Product></Wix>", &mut ctx).unwrap_or_default();
        assert!(fs::write(&valid_obj_file, valid_obj.serialize()).is_ok());

        // 7a. loc read error
        let l_loc_read_err = LightOptions {
            inputs: vec![valid_obj_file.clone()],
            loc_files: vec![temp_dir.join("nonexistent.wxl")],
            suppress_ice: true,
            ..LightOptions::new()
        };
        assert!(l_loc_read_err.execute().is_err());

        // 7b. loc parse error
        let bad_wxl = temp_dir.join("bad.wxl");
        assert!(fs::write(&bad_wxl, "<InvalidLoc/>").is_ok());
        let l_loc_parse_err = LightOptions {
            inputs: vec![valid_obj_file.clone()],
            loc_files: vec![bad_wxl.clone()],
            suppress_ice: true,
            ..LightOptions::new()
        };
        assert!(l_loc_parse_err.execute().is_err());

        // 8. LightOptions::execute linker error (line 754)
        let mut unres_ctx = PreprocessorContext::new();
        let unres_obj = crate::wix::compile_wix("<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\"><Product Id=\"{33333333-3333-3333-3333-333333333333}\" Name=\"App\" Version=\"1.0.0\" Manufacturer=\"Test\"><Package Description=\"Test\"/><Directory Id=\"TARGETDIR\" Name=\"SourceDir\"/><Feature Id=\"F1\" Title=\"F1\" Level=\"1\"><ComponentRef Id=\"MissingComponent\"/></Feature></Product></Wix>", &mut unres_ctx).unwrap_or_default();
        let unres_obj_file = temp_dir.join("unresolved.wixobj");
        assert!(fs::write(&unres_obj_file, unres_obj.serialize()).is_ok());
        let l_link_err = LightOptions {
            inputs: vec![unres_obj_file.clone()],
            suppress_ice: true,
            ..LightOptions::new()
        };
        assert!(l_link_err.execute().is_err());

        // 9. LightOptions::execute package.save error (line 780): output is an existing directory
        let l_save_err = LightOptions {
            inputs: vec![valid_obj_file],
            output: Some(existing_dir.clone()),
            suppress_ice: true,
            ..LightOptions::new()
        };
        assert!(l_save_err.execute().is_err());

        // 10. WixBuildOptions::execute errors:
        // 10a. lib read and deserialize error (line 975, 976)
        let w_missing_lib = WixBuildOptions {
            sources: vec![PathBuf::from("missing.wixlib")],
            ..WixBuildOptions::new()
        };
        assert!(w_missing_lib.execute().is_err());

        let corrupt_lib = temp_dir.join("corrupt.wixlib");
        assert!(fs::write(&corrupt_lib, b"corrupted wixlib data").is_ok());
        let w_lib_err = WixBuildOptions {
            sources: vec![corrupt_lib],
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_lib_err.execute().is_err());

        // 10b. obj read and deserialize error (line 982, 983)
        let w_missing_obj = WixBuildOptions {
            sources: vec![PathBuf::from("missing.wixobj")],
            ..WixBuildOptions::new()
        };
        assert!(w_missing_obj.execute().is_err());

        let w_obj_err = WixBuildOptions {
            sources: vec![corrupt_obj],
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_obj_err.execute().is_err());

        // 10c. wxs compile error (line 1003)
        let w_wxs_err = WixBuildOptions {
            sources: vec![bad_wix_file],
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_wxs_err.execute().is_err());

        // 10d. loc read and parse error (line 1021, 1022)
        let w_loc_read_err = WixBuildOptions {
            sources: vec![valid_wxs.clone(), temp_dir.join("missing_wix_loc.wxl")],
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_loc_read_err.execute().is_err());

        let w_loc_parse_err = WixBuildOptions {
            sources: vec![valid_wxs.clone(), bad_wxl],
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_loc_parse_err.execute().is_err());

        // 10e. linker link error (line 1028)
        let w_link_err = WixBuildOptions {
            sources: vec![unres_obj_file],
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_link_err.execute().is_err());

        // 10f. package.save error (line 1053): output is an existing directory
        let w_save_err = WixBuildOptions {
            sources: vec![valid_wxs],
            output: Some(existing_dir),
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        assert!(w_save_err.execute().is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
