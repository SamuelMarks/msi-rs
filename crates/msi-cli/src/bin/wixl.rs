//! # wixl
//!
//! Cross-compiler executable shim replicating GNOME `wixl` (msitools).
//!
//! Compiles and links `.wxs` source manifests into complete `.msi` binary packages
//! with full support for UI dialogs, custom actions, multi-cabinet media partitions,
//! and embedded chainers.
//!
//! ## Usage
//!
//! ```sh
//! wixl [-a <arch>] [-o <output.msi>] [-D <VAR=VAL>] [-I <includedir>] [-b <basedir>] [-v] <source.wxs...>
//! ```

use msi::wix::toolchain::WixBuildOptions;
use std::path::PathBuf;
use std::process::ExitCode;

/// Parsed options for `wixl`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WixlOptions {
    /// Target architecture (`-a`, `--arch`).
    pub arch: Option<String>,
    /// Explicit output MSI path (`-o`, `--output`).
    pub output: Option<PathBuf>,
    /// Preprocessor variable definitions (`-D`, `-d`, `--define`).
    pub defines: Vec<(String, String)>,
    /// Directory search paths for `<?include ?>` files (`-I`, `--include`).
    pub include_dirs: Vec<PathBuf>,
    /// Base directories for resolving relative payload files (`-b`, `--basedir`).
    pub base_dirs: Vec<PathBuf>,
    /// Verbose diagnostic output (`-v`, `--verbose`).
    pub verbose: bool,
    /// Enable UI dialogs (`-u`, `--ui`).
    pub ui: bool,
    /// Input `.wxs` source manifests to compile and link.
    pub sources: Vec<PathBuf>,
}

impl WixlOptions {
    /// Parses arguments into [`WixlOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`WixlOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target files.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Err(
                "missing arguments. Usage: wixl [-a <arch>] [-o <output.msi>] [-D <VAR=VAL>] <source.wxs...>"
                    .to_string(),
            );
        }

        let mut opts = Self::default();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if arg == "-a" || arg == "--arch" {
                idx += 1;
                if idx < args.len() {
                    opts.arch = Some(args[idx].clone());
                    idx += 1;
                }
            } else if let Some(rest) = arg.strip_prefix("-a") {
                opts.arch = Some(rest.to_string());
                idx += 1;
            } else if arg == "-o" || arg == "--output" {
                idx += 1;
                if idx < args.len() {
                    opts.output = Some(PathBuf::from(&args[idx]));
                    idx += 1;
                }
            } else if let Some(rest) = arg.strip_prefix("-o") {
                opts.output = Some(PathBuf::from(rest));
                idx += 1;
            } else if arg == "-D" || arg == "-d" || arg == "--define" {
                idx += 1;
                if idx < args.len() {
                    if let Some((k, v)) = args[idx].split_once('=') {
                        opts.defines.push((k.to_string(), v.to_string()));
                    } else {
                        opts.defines.push((args[idx].clone(), "1".to_string()));
                    }
                    idx += 1;
                }
            } else if let Some(rest) = arg.strip_prefix("-D").or_else(|| arg.strip_prefix("-d")) {
                if let Some((k, v)) = rest.split_once('=') {
                    opts.defines.push((k.to_string(), v.to_string()));
                } else {
                    opts.defines.push((rest.to_string(), "1".to_string()));
                }
                idx += 1;
            } else if arg == "-I" || arg == "--include" {
                idx += 1;
                if idx < args.len() {
                    opts.include_dirs.push(PathBuf::from(&args[idx]));
                    idx += 1;
                }
            } else if let Some(rest) = arg.strip_prefix("-I") {
                opts.include_dirs.push(PathBuf::from(rest));
                idx += 1;
            } else if arg == "-b" || arg == "--basedir" {
                idx += 1;
                if idx < args.len() {
                    opts.base_dirs.push(PathBuf::from(&args[idx]));
                    idx += 1;
                }
            } else if arg == "-v" || arg == "--verbose" {
                opts.verbose = true;
                idx += 1;
            } else if arg == "-u" || arg == "--ui" {
                opts.ui = true;
                idx += 1;
            } else if !arg.starts_with('-') {
                opts.sources.push(PathBuf::from(arg));
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if opts.sources.is_empty() {
            return Err("no input .wxs source files specified".to_string());
        }

        Ok(opts)
    }
}

/// Main execution routine returning process exit code.
///
/// # Arguments
///
/// * `args` - Command-line arguments excluding the executable name.
///
/// # Returns
///
/// Integer return code: `0` on success, `1` on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match WixlOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("wixl: error: {err}");
            return 1;
        }
    };

    let build_opts = WixBuildOptions {
        arch: opts.arch,
        output: opts.output,
        defines: opts.defines,
        include_dirs: opts.include_dirs,
        base_dirs: opts.base_dirs,
        suppress_ice: true,
        sources: opts.sources,
        ..WixBuildOptions::default()
    };

    match build_opts.execute() {
        Ok(out) => {
            if opts.verbose {
                println!("wixl: wrote {}", out.display());
            }
            0
        }
        Err(err) => {
            eprintln!("wixl: error: {err}");
            1
        }
    }
}

/// Execution helper converting return code to process [`ExitCode`].
///
/// # Arguments
///
/// * `args` - Command-line arguments.
///
/// # Returns
///
/// Process [`ExitCode`].
#[must_use = "process exit code must be handled"]
pub fn run_app(args: &[String]) -> ExitCode {
    if run(args) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Entry point for the `wixl` executable.
///
/// # Returns
///
/// TODO: Document return value.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wixl_options_parse_errors() {
        assert!(WixlOptions::parse(&[]).is_err());
        assert!(WixlOptions::parse(&["-a".to_string()]).is_err());
        assert!(WixlOptions::parse(&["-o".to_string()]).is_err());
        assert!(WixlOptions::parse(&["-D".to_string()]).is_err());
        assert!(WixlOptions::parse(&["-I".to_string()]).is_err());
        assert!(WixlOptions::parse(&["-b".to_string()]).is_err());
        assert!(WixlOptions::parse(&["-a".to_string(), "x64".to_string()]).is_err());
    }

    #[test]
    fn test_wixl_options_parse_flags() {
        let args = vec![
            "-a".to_string(),
            "x64".to_string(),
            "-o".to_string(),
            "out.msi".to_string(),
            "-D".to_string(),
            "VAR=1".to_string(),
            "-D".to_string(),
            "SINGLE_DEF".to_string(),
            "-DDEF".to_string(),
            "-I".to_string(),
            "/inc".to_string(),
            "-b".to_string(),
            "/base".to_string(),
            "-v".to_string(),
            "-u".to_string(),
            "main.wxs".to_string(),
            "payload.wxs".to_string(),
        ];
        let parsed = WixlOptions::parse(&args);
        assert!(parsed.is_ok());
        let opts = parsed.unwrap_or_default();
        assert_eq!(opts.arch, Some("x64".to_string()));
        assert_eq!(opts.output, Some(PathBuf::from("out.msi")));
        assert_eq!(
            opts.defines,
            vec![
                ("VAR".to_string(), "1".to_string()),
                ("SINGLE_DEF".to_string(), "1".to_string()),
                ("DEF".to_string(), "1".to_string())
            ]
        );
        assert_eq!(opts.include_dirs, vec![PathBuf::from("/inc")]);
        assert_eq!(opts.base_dirs, vec![PathBuf::from("/base")]);
        assert!(opts.verbose);
        assert!(opts.ui);
        assert_eq!(
            opts.sources,
            vec![PathBuf::from("main.wxs"), PathBuf::from("payload.wxs")]
        );

        // Test long flags and alternate forms
        let long_args = vec![
            "--arch".to_string(),
            "arm64".to_string(),
            "--output".to_string(),
            "long_out.msi".to_string(),
            "--define".to_string(),
            "LONG_VAR=42".to_string(),
            "-d".to_string(),
            "SHORT_D=99".to_string(),
            "--include".to_string(),
            "/long/inc".to_string(),
            "--basedir".to_string(),
            "/long/base".to_string(),
            "--verbose".to_string(),
            "--ui".to_string(),
            "long_input.wxs".to_string(),
        ];
        let long_parsed = WixlOptions::parse(&long_args);
        assert!(long_parsed.is_ok());
        let long_opts = long_parsed.unwrap_or_default();
        assert_eq!(long_opts.arch, Some("arm64".to_string()));
        assert_eq!(long_opts.output, Some(PathBuf::from("long_out.msi")));
        assert!(long_opts.verbose);
        assert!(long_opts.ui);

        // Test combined short flags
        let short_args = vec![
            "-ax86".to_string(),
            "-ooutput2.msi".to_string(),
            "-dVAL=2".to_string(),
            "-dFLAG".to_string(),
            "-I/search".to_string(),
            "--unknown-ignored".to_string(),
            "input.wxs".to_string(),
        ];
        let short_parsed = WixlOptions::parse(&short_args);
        assert!(short_parsed.is_ok());
        let short_opts = short_parsed.unwrap_or_default();
        assert_eq!(short_opts.arch, Some("x86".to_string()));
        assert_eq!(short_opts.output, Some(PathBuf::from("output2.msi")));
        assert_eq!(short_opts.sources, vec![PathBuf::from("input.wxs")]);
    }

    #[test]
    fn test_wixl_run_app_invalid() {
        assert_eq!(run(&[]), 1);
        assert_ne!(run_app(&[]), ExitCode::SUCCESS);
    }

    #[test]
    fn test_wixl_run_success_and_error() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_test_wixl_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("test.wxs");
        let out_file = temp_dir.join("out.msi");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(std::fs::write(&src_file, wxs).is_ok());

        let args = vec![
            "-v".to_string(),
            "-a".to_string(),
            "x64".to_string(),
            "-o".to_string(),
            out_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&args), 0);
        assert_eq!(run_app(&args), ExitCode::SUCCESS);

        let pkg = msi::Package::open(&out_file).unwrap_or_default();
        assert_eq!(pkg.summary_info().template.as_deref(), Some("x64;1033"));

        // Run non-verbose to cover !opts.verbose branch
        let non_verbose_args = vec![
            "-o".to_string(),
            out_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&non_verbose_args), 0);

        // Error during build execution
        let bad_src = temp_dir.join("bad.wxs");
        assert!(std::fs::write(&bad_src, "not valid wxs xml").is_ok());
        let bad_args = vec![bad_src.to_string_lossy().to_string()];
        assert_eq!(run(&bad_args), 1);

        // Test main
        let main_exit = main();
        assert_eq!(main_exit, ExitCode::FAILURE);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
