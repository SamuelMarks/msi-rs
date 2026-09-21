//! # heat
//!
//! `WiX` v3 Source Harvester executable shim replicating `heat.exe`.
//!
//! Automatically harvests filesystem directory trees, single files, `.reg` registry files,
//! build outputs, and website assets into valid `WiX` XML fragments.
//!
//! ## Usage
//!
//! ```sh
//! heat <dir|file|reg|project|website|perf> <path> [options]
//! ```

use msi::wix::harvest::Harvester;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Parsed options for `heat` harvester CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct HeatOptions {
    /// Suppress copyright banner (`-nologo`).
    pub nologo: bool,
    /// Harvest target type (`dir`, `file`, `reg`, `project`, `website`, `perf`).
    pub harvest_type: String,
    /// Target path to harvest from.
    pub target_path: PathBuf,
    /// Name of generated `<ComponentGroup>` (`-cg`).
    pub component_group: String,
    /// Root `<DirectoryRef>` ID (`-dr`).
    pub directory_ref: String,
    /// Preprocessor variable prefix for file paths (`-var`).
    pub var_prefix: Option<String>,
    /// Automatically generate GUIDs for all harvested components (`-gg`).
    pub generate_guids: bool,
    /// Generate GUIDs without surrounding curly braces (`-g1`).
    pub guid_without_braces: bool,
    /// Suppress generating multiple `<Fragment>` tags (`-sfrag`).
    pub suppress_fragments: bool,
    /// Suppress harvesting root directory element (`-srd`).
    pub suppress_root_dir: bool,
    /// Suppress COM and registry harvesting from binaries (`-sreg`).
    pub suppress_registry: bool,
    /// Suppress generating unique identifiers (`-suid`).
    pub suppress_unique_ids: bool,
    /// Destination output file path (`-o`, `-out`).
    pub output: Option<PathBuf>,
}

impl HeatOptions {
    /// Parses arguments into [`HeatOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`HeatOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target.
    #[allow(clippy::too_many_lines, clippy::branches_sharing_code)]
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = Self {
            component_group: "HarvestedComponents".to_string(),
            directory_ref: "INSTALLFOLDER".to_string(),
            ..Self::default()
        };

        if args.is_empty() {
            return Err(
                "missing harvest type. Usage: heat <dir|file|reg|project> <path> [options]"
                    .to_string(),
            );
        }

        let mut positional = Vec::new();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if arg.eq_ignore_ascii_case("-nologo") || arg.eq_ignore_ascii_case("/nologo") {
                opts.nologo = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-gg") || arg.eq_ignore_ascii_case("/gg") {
                opts.generate_guids = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-g1") || arg.eq_ignore_ascii_case("/g1") {
                opts.guid_without_braces = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-sfrag") || arg.eq_ignore_ascii_case("/sfrag") {
                opts.suppress_fragments = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-srd") || arg.eq_ignore_ascii_case("/srd") {
                opts.suppress_root_dir = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-sreg") || arg.eq_ignore_ascii_case("/sreg") {
                opts.suppress_registry = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-suid") || arg.eq_ignore_ascii_case("/suid") {
                opts.suppress_unique_ids = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-cg") || arg.eq_ignore_ascii_case("/cg") {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing ComponentGroup identifier for '-cg'".to_string());
                }
                opts.component_group.clone_from(&args[idx]);
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-dr") || arg.eq_ignore_ascii_case("/dr") {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing DirectoryRef identifier for '-dr'".to_string());
                }
                opts.directory_ref.clone_from(&args[idx]);
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-var") || arg.eq_ignore_ascii_case("/var") {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing variable prefix for '-var'".to_string());
                }
                opts.var_prefix = Some(args[idx].clone());
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-o")
                || arg.eq_ignore_ascii_case("-out")
                || arg.eq_ignore_ascii_case("/o")
                || arg.eq_ignore_ascii_case("/out")
            {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing output path for '-out'".to_string());
                }
                opts.output = Some(PathBuf::from(&args[idx]));
                idx += 1;
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/') || positional.is_empty() || Path::new(arg).exists())
            {
                positional.push(arg.clone());
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if positional.is_empty() {
            return Err(
                "missing harvest type. Usage: heat <dir|file|reg|project> <path> [options]"
                    .to_string(),
            );
        }
        opts.harvest_type.clone_from(&positional[0]);

        if positional.len() < 2 {
            return Err("missing target path to harvest".to_string());
        }
        opts.target_path = PathBuf::from(&positional[1]);

        Ok(opts)
    }

    /// Executes the harvesting process.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or XML generation failure.
    pub fn execute(&self) -> Result<(), String> {
        let harvester = Harvester::new();
        let xml = match self.harvest_type.as_str() {
            "dir" => harvester
                .harvest_directory(
                    &self.target_path,
                    &self.component_group,
                    &self.directory_ref,
                )
                .map_err(|e| format!("directory harvesting failed: {e}"))?,
            "file" => {
                let stem = self
                    .target_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file");
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="{}">
      <Component Id="cmp_{stem}" Guid="*">
        <File Id="fil_{stem}" Source="{}" KeyPath="yes" />
      </Component>
    </DirectoryRef>
  </Fragment>
</Wix>
"#,
                    self.directory_ref,
                    self.target_path.display()
                )
            }
            "reg" => {
                let content = fs::read_to_string(&self.target_path).map_err(|e| {
                    format!(
                        "failed reading .reg file '{}': {e}",
                        self.target_path.display()
                    )
                })?;
                harvester
                    .harvest_registry(&content, "HarvestedRegistry")
                    .map_err(|e| format!("registry harvesting failed: {e}"))?
            }
            "project" | "perf" | "website" => {
                let target_dir = if self.target_path.is_dir() {
                    self.target_path.clone()
                } else {
                    self.target_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf()
                };
                harvester
                    .harvest_build_outputs(&target_dir, &self.component_group)
                    .map_err(|e| format!("build output harvesting failed: {e}"))?
            }
            other => return Err(format!("unsupported harvest type '{other}'")),
        };

        if let Some(ref out_p) = self.output {
            if let Some(parent) = out_p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(out_p, xml)
                .map_err(|e| format!("failed writing output file '{}': {e}", out_p.display()))?;
        } else {
            println!("{xml}");
        }

        Ok(())
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
/// Exit code: `0` on command success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match HeatOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("heat.exe : error HEAT0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Harvester version 3.14.0.1703
Copyright (c) .NET Foundation and contributors. All rights reserved.
"
        );
    }

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("heat.exe : error HEAT0002 : {err}");
            1
        }
    }
}

/// Execution helper converting integer exit code to [`ExitCode`].
///
/// # Arguments
///
/// * `args` - Command-line arguments.
///
/// # Returns
///
/// Process [`ExitCode`].
pub fn run_app(args: &[String]) -> ExitCode {
    if run(args) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Entry point for the `heat` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_heat_run_all_branches() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_heat_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let sample_file = temp_dir.join("sample.txt");
        let reg_file = temp_dir.join("test.reg");
        let empty_reg_file = temp_dir.join("empty.reg");
        let project_file = temp_dir.join("app.csproj");
        let out_wxs = temp_dir.join("sub_dir").join("harvested.wxs");

        fs::write(&sample_file, "Sample Content")?;
        let reg_content = r#"Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\Software\Test]
"Value"="1"
"#;
        fs::write(&reg_file, reg_content)?;
        fs::write(&empty_reg_file, "")?;
        fs::write(&project_file, "<Project />")?;

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-nologo".to_string()]), 1);
        assert_eq!(run(&["dir".to_string()]), 1);
        assert_eq!(
            run(&[
                "dir".to_string(),
                "/nonexistent_path_that_does_not_exist_xyz123".to_string(),
            ]),
            1
        );
        assert_eq!(run(&["dir".to_string(), "-cg".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "/cg".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "-dr".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "/dr".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "-var".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "/var".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "-o".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "/o".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "-out".to_string()]), 1);
        assert_eq!(run(&["dir".to_string(), "/out".to_string()]), 1);

        // Parsing with slash options
        assert!(HeatOptions::parse(&[
            "/nologo".to_string(),
            "/gg".to_string(),
            "/g1".to_string(),
            "/sfrag".to_string(),
            "/srd".to_string(),
            "/sreg".to_string(),
            "/suid".to_string(),
            "/cg".to_string(),
            "CGSlash".to_string(),
            "/dr".to_string(),
            "DRSlash".to_string(),
            "/var".to_string(),
            "VARSlash".to_string(),
            "/out".to_string(),
            out_wxs.to_string_lossy().to_string(),
            "/extra_flag".to_string(),
            "dir".to_string(),
            temp_dir.to_string_lossy().to_string(),
        ])
        .is_ok());

        // 2. Unsupported harvest type
        assert_eq!(
            run(&[
                "unknown".to_string(),
                temp_dir.to_string_lossy().to_string()
            ]),
            1
        );

        // 3. Harvest directory with banner and stdout
        assert_eq!(
            run(&["dir".to_string(), temp_dir.to_string_lossy().to_string()]),
            0
        );

        // 4. Harvest directory with all options and output file
        let full_dir_args = vec![
            "-nologo".to_string(),
            "-gg".to_string(),
            "-g1".to_string(),
            "-sfrag".to_string(),
            "-srd".to_string(),
            "-sreg".to_string(),
            "-suid".to_string(),
            "-cg".to_string(),
            "CustomCG".to_string(),
            "-dr".to_string(),
            "CustomDir".to_string(),
            "-var".to_string(),
            "var.TargetDir".to_string(),
            "-o".to_string(),
            out_wxs.to_string_lossy().to_string(),
            "-unknown".to_string(),
            "dir".to_string(),
            temp_dir.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&full_dir_args), 0);
        assert_eq!(run_app(&full_dir_args), ExitCode::SUCCESS);
        assert!(out_wxs.exists());

        // 5. Harvest directory and build output failure (unreadable subdirectory)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unreadable_parent = temp_dir.join("unreadable_harvest");
            let unreadable_sub = unreadable_parent.join("no_perm");
            fs::create_dir_all(&unreadable_sub)?;
            fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o000))?;

            assert_eq!(
                run(&[
                    "-nologo".to_string(),
                    "dir".to_string(),
                    unreadable_parent.to_string_lossy().to_string(),
                ]),
                1
            );
            assert_eq!(
                run(&[
                    "-nologo".to_string(),
                    "perf".to_string(),
                    unreadable_parent.to_string_lossy().to_string(),
                ]),
                1
            );

            fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o755))?;
        }

        // 6. Harvest file
        assert_eq!(
            run(&[
                "file".to_string(),
                sample_file.to_string_lossy().to_string()
            ]),
            0
        );

        // Harvest file with no stem (empty path)
        let no_stem_file_opts = HeatOptions {
            harvest_type: "file".to_string(),
            target_path: PathBuf::from(""),
            ..HeatOptions::default()
        };
        assert!(no_stem_file_opts.execute().is_ok());

        // 7. Harvest reg
        assert_eq!(
            run(&["reg".to_string(), reg_file.to_string_lossy().to_string()]),
            0
        );
        // Harvest reg non-existent file
        assert_eq!(
            run(&["reg".to_string(), "missing_reg_rel.reg".to_string()]),
            1
        );
        // Harvest reg empty file
        assert_eq!(
            run(&[
                "reg".to_string(),
                empty_reg_file.to_string_lossy().to_string()
            ]),
            1
        );

        // 8. Harvest project / perf / website
        assert_eq!(
            run(&[
                "project".to_string(),
                temp_dir.to_string_lossy().to_string()
            ]),
            0
        );
        // Project on a file (target_path.is_dir() is false)
        assert_eq!(
            run(&[
                "project".to_string(),
                project_file.to_string_lossy().to_string()
            ]),
            0
        );
        // Website on a relative empty path (parent() is None)
        let empty_proj_opts = HeatOptions {
            harvest_type: "website".to_string(),
            target_path: PathBuf::from(""),
            ..HeatOptions::default()
        };
        let _ = empty_proj_opts.execute();

        // 9. Output failure on write
        let blocking_file = temp_dir.join("blocking_output_file");
        fs::write(&blocking_file, "blocking")?;
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-o".to_string(),
                blocking_file.join("out.wxs").to_string_lossy().to_string(),
                "file".to_string(),
                sample_file.to_string_lossy().to_string(),
            ]),
            1
        );

        // Direct execute with empty output path (parent is None)
        let empty_out_opts = HeatOptions {
            harvest_type: "file".to_string(),
            target_path: sample_file,
            output: Some(PathBuf::from("")),
            ..HeatOptions::default()
        };
        assert!(empty_out_opts.execute().is_err());

        // 10. Derives testing
        let default_opts = HeatOptions::default();
        let mut cloned_opts = default_opts.clone();
        cloned_opts.clone_from(&default_opts);
        let different_opts = HeatOptions {
            nologo: true,
            ..HeatOptions::default()
        };
        assert_eq!(default_opts, cloned_opts);
        assert_ne!(default_opts, different_opts);
        assert!(format!("{default_opts:?}").contains("HeatOptions"));

        // 11. Test run_app error and main
        assert_eq!(run_app(&[]), ExitCode::FAILURE);
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
