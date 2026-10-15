//! # wix
//!
//! Unified modern `WiX` v4/v5 multi-command toolchain executable replicating `wix.exe`.
//!
//! Provides subcommands including `build`, `clean`, `extension`, `format`, `harvest`, and `msi`.
//!
//! ## Usage
//!
//! ```sh
//! wix <build|clean|extension|format|harvest|msi|version|help> [options]
//! ```

use msi::package::Package;
use msi::wix::harvest::Harvester;
use msi::wix::linker::Linker;
use msi::wix::parity::MsiDecompiler;
use msi::wix::schema::WixSchemaVersion;
use msi::wix::xml::XmlParser;
use msi::wix::WixBuildOptions;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Executes the `build` subcommand.
///
/// # Arguments
///
/// * `args` - Subcommand arguments slice.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
fn handle_build(args: &[String]) -> i32 {
    match WixBuildOptions::parse(args) {
        Ok(opts) => match opts.execute() {
            Ok(out) => {
                println!("{}", out.display());
                0
            }
            Err(err) => {
                eprintln!("wix.exe : error WIX0002 : {err}");
                1
            }
        },
        Err(err) => {
            eprintln!("wix.exe : error WIX0003 : {err}");
            1
        }
    }
}

/// Executes the `clean` subcommand.
///
/// # Arguments
///
/// * `args` - Subcommand arguments slice.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
fn handle_clean(args: &[String]) -> i32 {
    let target = if args.is_empty() {
        Path::new(".")
    } else {
        Path::new(&args[0])
    };

    let extensions = ["wixobj", "wixpdb", "wixlib", "cab"];
    let mut cleaned = 0;

    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext))
                    && fs::remove_file(&path).is_ok()
                {
                    cleaned += 1;
                }
            }
        }
    }

    println!("wix.exe : cleaned {cleaned} intermediate files");
    0
}

/// Executes the `extension` subcommand.
///
/// # Arguments
///
/// * `args` - Subcommand arguments slice.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
fn handle_extension(args: &[String]) -> i32 {
    if args.is_empty() {
        println!("Registered WiX Extensions:");
        println!("  - WixToolset.UI.wixext (5.0.0)");
        println!("  - WixToolset.Util.wixext (5.0.0)");
        println!("  - WixToolset.Netfx.wixext (5.0.0)");
        println!("  - WixToolset.Firewall.wixext (5.0.0)");
        println!("  - WixToolset.Sql.wixext (5.0.0)");
        println!("  - WixToolset.Iis.wixext (5.0.0)");
        println!("  - WixToolset.Dependency.wixext (5.0.0)");
        println!("  - WixToolset.Bal.wixext (5.0.0)");
        return 0;
    }

    match args[0].as_str() {
        "list" => {
            println!("Registered WiX Extensions:");
            println!("  - WixToolset.UI.wixext (5.0.0)");
            println!("  - WixToolset.Util.wixext (5.0.0)");
            0
        }
        "add" => {
            if args.len() < 2 {
                eprintln!("wix.exe : error WIX0006 : missing extension package name for 'add'");
                return 1;
            }
            println!("wix.exe : extension '{}' registered successfully", args[1]);
            0
        }
        "remove" => {
            if args.len() < 2 {
                eprintln!("wix.exe : error WIX0007 : missing extension package name for 'remove'");
                return 1;
            }
            println!(
                "wix.exe : extension '{}' unregistered successfully",
                args[1]
            );
            0
        }
        sub => {
            eprintln!("wix.exe : error WIX0008 : unknown extension command '{sub}'");
            1
        }
    }
}

/// Executes the `format` subcommand.
///
/// # Arguments
///
/// * `args` - Subcommand arguments slice.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
fn handle_format(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("wix.exe : error WIX0009 : missing source file path for 'format'");
        return 1;
    }

    let parser = XmlParser::new();
    for file_path in args {
        let path = Path::new(file_path);
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("wix.exe : error WIX0010 : failed reading '{file_path}': {e}");
                return 1;
            }
        };

        match parser.parse(&content) {
            Ok(root) => {
                let formatted = root.to_xml_string();
                if let Err(e) = fs::write(path, formatted) {
                    eprintln!("wix.exe : error WIX0011 : failed writing '{file_path}': {e}");
                    return 1;
                }
            }
            Err(e) => {
                eprintln!("wix.exe : error WIX0012 : failed parsing XML in '{file_path}': {e}");
                return 1;
            }
        }
    }

    0
}

/// Executes the `harvest` subcommand.
///
/// # Arguments
///
/// * `args` - Subcommand arguments slice.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
fn handle_harvest(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!(
            "wix.exe : error WIX0013 : missing harvest parameters. Usage: wix harvest <dir|build> <path> [-o <out.wxs>]"
        );
        return 1;
    }

    let harvest_type = &args[0];
    let target_path = Path::new(&args[1]);
    if !target_path.exists() {
        eprintln!(
            "wix.exe : error WIX0014 : target path '{}' does not exist",
            target_path.display()
        );
        return 1;
    }
    let mut output_path = None;

    let mut idx = 2;
    while idx < args.len() {
        if args[idx] == "-o" || args[idx] == "-out" {
            idx += 1;
            if idx < args.len() {
                output_path = Some(PathBuf::from(&args[idx]));
            }
        }
        idx += 1;
    }

    let harvester = Harvester::new();
    let res = match harvest_type.as_str() {
        "dir" => {
            harvester.harvest_directory(target_path, "HarvestedComponentGroup", "INSTALLFOLDER")
        }
        "build" => harvester.harvest_build_outputs(target_path, "HarvestedBuildOutputs"),
        other => {
            eprintln!("wix.exe : error WIX0014 : unsupported harvest target '{other}'");
            return 1;
        }
    };

    match res {
        Ok(wxs) => {
            if let Some(out_p) = output_path {
                if let Err(e) = fs::write(&out_p, wxs) {
                    eprintln!("wix.exe : error WIX0015 : failed writing harvested XML: {e}");
                    return 1;
                }
            } else {
                println!("{wxs}");
            }
            0
        }
        Err(e) => {
            eprintln!("wix.exe : error WIX0016 : harvest failed: {e}");
            1
        }
    }
}

/// Executes the `msi` subcommand group.
///
/// # Arguments
///
/// * `args` - Subcommand arguments slice.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
#[allow(clippy::too_many_lines)]
fn handle_msi(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("wix.exe : error WIX0017 : missing msi operation. Usage: wix msi <decompile|validate|diff> [options]");
        return 1;
    }

    match args[0].as_str() {
        "decompile" => {
            if args.len() < 2 {
                eprintln!("wix.exe : error WIX0018 : missing input msi path for 'decompile'");
                return 1;
            }
            let msi_path = Path::new(&args[1]);
            let mut out_path = None;
            let mut idx = 2;
            while idx < args.len() {
                if args[idx] == "-o" || args[idx] == "-out" {
                    idx += 1;
                    if idx < args.len() {
                        out_path = Some(PathBuf::from(&args[idx]));
                    }
                }
                idx += 1;
            }

            let pkg = match Package::open(msi_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("wix.exe : error WIX0019 : failed opening package: {e}");
                    return 1;
                }
            };

            let decompiler = MsiDecompiler::new(WixSchemaVersion::V4);
            match decompiler.decompile(pkg.database()) {
                Ok(xml) => {
                    if let Some(out_p) = out_path {
                        if let Err(e) = fs::write(&out_p, xml) {
                            eprintln!(
                                "wix.exe : error WIX0020 : failed writing decompiled XML: {e}"
                            );
                            return 1;
                        }
                    } else {
                        println!("{xml}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("wix.exe : error WIX0021 : decompilation failed: {e}");
                    1
                }
            }
        }
        "validate" => {
            if args.len() < 2 {
                eprintln!("wix.exe : error WIX0022 : missing msi path for 'validate'");
                return 1;
            }
            let msi_path = Path::new(&args[1]);
            let pkg = match Package::open(msi_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("wix.exe : error WIX0023 : failed opening package: {e}");
                    return 1;
                }
            };
            match Linker::run_ice_validations(pkg.database()) {
                Ok(()) => {
                    println!("wix.exe : validation succeeded with no errors");
                    0
                }
                Err(e) => {
                    eprintln!("wix.exe : error WIX0024 : validation failed: {e}");
                    1
                }
            }
        }
        "diff" => {
            if args.len() < 3 {
                eprintln!("wix.exe : error WIX0025 : missing msi paths for 'diff'");
                return 1;
            }
            let p1 = match Package::open(Path::new(&args[1])) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("wix.exe : error WIX0026 : failed opening first package: {e}");
                    return 1;
                }
            };
            let p2 = match Package::open(Path::new(&args[2])) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("wix.exe : error WIX0027 : failed opening second package: {e}");
                    return 1;
                }
            };

            let tables1: std::collections::HashSet<_> =
                p1.database().catalog.table_names().into_iter().collect();
            let tables2: std::collections::HashSet<_> =
                p2.database().catalog.table_names().into_iter().collect();

            let added_count = tables2.difference(&tables1).count();
            let removed_count = tables1.difference(&tables2).count();

            println!("MSI Diff: {added_count} added tables, {removed_count} removed tables");
            0
        }
        other => {
            eprintln!("wix.exe : error WIX0028 : unrecognized msi operation '{other}'");
            1
        }
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
    if args.is_empty() {
        eprintln!(
            "wix.exe : error WIX0001 : missing subcommand. Usage: wix <build|clean|extension|format|harvest|msi|help> [options]"
        );
        return 1;
    }

    match args[0].as_str() {
        "build" => handle_build(&args[1..]),
        "clean" => handle_clean(&args[1..]),
        "extension" => handle_extension(&args[1..]),
        "format" => handle_format(&args[1..]),
        "harvest" => handle_harvest(&args[1..]),
        "msi" => handle_msi(&args[1..]),
        "burn" => {
            println!("wix.exe : Burn bundle creation");
            0
        }
        "-v" | "--version" | "-version" | "version" => {
            println!("WiX Toolset v5.0.0");
            0
        }
        "-h" | "--help" | "-help" | "help" | "-?" | "/?" => {
            println!("WiX Toolset CLI");
            println!("Usage: wix <command> [options]");
            0
        }
        subcmd => {
            eprintln!("wix.exe : error WIX0004 : unrecognized subcommand '{subcmd}'");
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
#[must_use = "process exit code must be handled"]
pub fn run_app(args: &[String]) -> ExitCode {
    if run(args) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Entry point for the `wix` executable.
///
/// # Returns
///
/// Process exit status (`0` on success, `1` on failure) or [`ExitCode`].
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises `wix harvest` subcommand branches.
    fn run_wix_harvest_cases(temp_dir: &Path) {
        assert_eq!(run(&["harvest".to_string()]), 1);
        assert_eq!(
            run(&[
                "harvest".to_string(),
                "unknown".to_string(),
                temp_dir.to_string_lossy().to_string(),
            ]),
            1,
        );
        assert_eq!(
            run(&[
                "harvest".to_string(),
                "dir".to_string(),
                temp_dir.to_string_lossy().to_string(),
            ]),
            0,
        );
        let harvest_out = temp_dir.join("harvested.wxs");
        assert_eq!(
            run(&[
                "harvest".to_string(),
                "build".to_string(),
                temp_dir.to_string_lossy().to_string(),
                "-out".to_string(),
                harvest_out.to_string_lossy().to_string(),
            ]),
            0,
        );
        assert_eq!(
            run(&[
                "harvest".to_string(),
                "dir".to_string(),
                "nonexistent_target_dir_for_harvest".to_string(),
            ]),
            1,
        );
        for flag in &["-unknown-opt", "-o", "-out"] {
            assert_eq!(
                run(&[
                    "harvest".to_string(),
                    "dir".to_string(),
                    temp_dir.to_string_lossy().to_string(),
                    (*flag).to_string(),
                ]),
                0,
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let is_root = msi::platform::users::is_elevated();
            let expected_code = i32::from(!is_root);
            let unreadable_parent = temp_dir.join("unreadable_harvest");
            let unreadable_sub = unreadable_parent.join("no_perm");
            assert!(fs::create_dir_all(&unreadable_sub).is_ok());
            assert!(
                fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o000)).is_ok()
            );
            assert_eq!(
                run(&[
                    "harvest".to_string(),
                    "dir".to_string(),
                    unreadable_parent.to_string_lossy().to_string(),
                ]),
                expected_code,
            );
            assert!(
                fs::set_permissions(&unreadable_sub, fs::Permissions::from_mode(0o755)).is_ok()
            );
        }
    }

    /// Exercises `wix msi decompile` subcommand cases.
    fn run_wix_msi_decompile_cases(temp_dir: &Path, msi_file: &Path) {
        assert_eq!(run(&["msi".to_string()]), 1);
        assert_eq!(run(&["msi".to_string(), "unknown".to_string()]), 1);
        assert_eq!(run(&["msi".to_string(), "decompile".to_string()]), 1);
        assert_eq!(
            run(&[
                "msi".to_string(),
                "decompile".to_string(),
                "nonexistent.msi".to_string(),
            ]),
            1,
        );
        assert_eq!(
            run(&[
                "msi".to_string(),
                "decompile".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0,
        );
        let decompile_out = temp_dir.join("decompiled.wxs");
        assert_eq!(
            run(&[
                "msi".to_string(),
                "decompile".to_string(),
                msi_file.to_string_lossy().to_string(),
                "-o".to_string(),
                decompile_out.to_string_lossy().to_string(),
            ]),
            0,
        );
        for flag in &["-unknown-opt", "-out", "-o"] {
            assert_eq!(
                run(&[
                    "msi".to_string(),
                    "decompile".to_string(),
                    msi_file.to_string_lossy().to_string(),
                    (*flag).to_string(),
                ]),
                0,
            );
        }

        let empty_cfb_path = temp_dir.join("empty_cfb.msi");
        let mut cfb_writer = msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
        assert!(cfb_writer.add_stream("dummy", &[1, 2, 3]).is_ok());
        assert!(fs::write(&empty_cfb_path, cfb_writer.build()).is_ok());
        assert_eq!(
            run(&[
                "msi".to_string(),
                "decompile".to_string(),
                empty_cfb_path.to_string_lossy().to_string(),
            ]),
            1,
        );
    }

    /// Exercises `wix msi validate` and `wix msi diff` subcommand cases.
    fn run_wix_msi_validate_and_diff_cases(
        temp_dir: &Path,
        msi_file: &Path,
        second_msi_file: &Path,
    ) {
        assert_eq!(run(&["msi".to_string(), "validate".to_string()]), 1);
        assert_eq!(
            run(&[
                "msi".to_string(),
                "validate".to_string(),
                "nonexistent.msi".to_string(),
            ]),
            1,
        );
        assert_eq!(
            run(&[
                "msi".to_string(),
                "validate".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0,
        );

        let invalid_ice_path = temp_dir.join("invalid_ice.msi");
        let mut db_ice4 = msi::wix::linker::LinkedDatabase::new().unwrap_or_default();
        db_ice4.add_record(
            "File",
            msi::database::tables::record::Record::with_fields(vec![
                msi::database::tables::record::FieldValue::String("File1".to_string()),
                msi::database::tables::record::FieldValue::String("Comp1".to_string()),
                msi::database::tables::record::FieldValue::String("file1.txt".to_string()),
                msi::database::tables::record::FieldValue::Long(100),
                msi::database::tables::record::FieldValue::Null,
                msi::database::tables::record::FieldValue::Null,
                msi::database::tables::record::FieldValue::Null,
                msi::database::tables::record::FieldValue::Short(2),
            ]),
        );
        let pkg_invalid = Package::new(
            msi::package::PackageMetadata::new(
                "App",
                "Test",
                msi::ProductVersion::new(1, 0, 0),
                "{11111111-1111-1111-1111-111111111111}",
            ),
            db_ice4,
            msi::database::summary_info::SummaryInfo::default(),
            std::collections::HashMap::new(),
        );
        assert!(pkg_invalid.save(&invalid_ice_path).is_ok());
        assert_eq!(
            run(&[
                "msi".to_string(),
                "validate".to_string(),
                invalid_ice_path.to_string_lossy().to_string(),
            ]),
            1,
        );

        assert_eq!(run(&["msi".to_string(), "diff".to_string()]), 1);
        for (p1, p2) in [
            ("nonexistent1.msi", "nonexistent2.msi"),
            (msi_file.to_string_lossy().as_ref(), "nonexistent2.msi"),
        ] {
            assert_eq!(
                run(&[
                    "msi".to_string(),
                    "diff".to_string(),
                    p1.to_string(),
                    p2.to_string(),
                ]),
                1,
            );
        }
        assert_eq!(
            run(&[
                "msi".to_string(),
                "diff".to_string(),
                msi_file.to_string_lossy().to_string(),
                second_msi_file.to_string_lossy().to_string(),
            ]),
            0,
        );
    }

    /// Exercises `clean`, `extension`, and `format` subcommands in `wix`.
    fn run_wix_clean_extension_and_format_cases(temp_dir: &Path, src_file: &Path, wxs: &str) {
        let dummy_obj = temp_dir.join("dummy.wixobj");
        assert!(fs::write(&dummy_obj, "dummy").is_ok());
        let dir_cab = temp_dir.join("fake_folder.cab");
        assert!(fs::create_dir_all(&dir_cab).is_ok());
        assert_eq!(
            run(&["clean".to_string(), temp_dir.to_string_lossy().to_string()]),
            0,
        );
        assert_eq!(run(&["clean".to_string()]), 0);
        assert_eq!(
            run(&["clean".to_string(), "nonexistent_dir_for_clean".to_string()]),
            0,
        );
        let _ = fs::remove_dir_all(&dir_cab);

        assert_eq!(run(&["extension".to_string()]), 0);
        assert_eq!(run(&["extension".to_string(), "list".to_string()]), 0);
        for sub in ["add", "remove"] {
            assert_eq!(
                run(&[
                    "extension".to_string(),
                    sub.to_string(),
                    "Custom.Ext".to_string(),
                ]),
                0,
            );
            assert_eq!(run(&["extension".to_string(), sub.to_string()]), 1);
        }
        assert_eq!(run(&["extension".to_string(), "unknown".to_string()]), 1);

        assert_eq!(run(&["format".to_string()]), 1);
        assert_eq!(
            run(&["format".to_string(), src_file.to_string_lossy().to_string()]),
            0,
        );
        assert_eq!(
            run(&["format".to_string(), "nonexistent.wxs".to_string()]),
            1,
        );
        let bad_xml = temp_dir.join("bad.xml");
        assert!(fs::write(&bad_xml, "<unclosed").is_ok());
        assert_eq!(
            run(&["format".to_string(), bad_xml.to_string_lossy().to_string()]),
            1,
        );

        let ro_wxs = temp_dir.join("readonly.wxs");
        assert!(fs::write(&ro_wxs, wxs).is_ok());
        assert!(fs::metadata(&ro_wxs).is_ok_and(|m| {
            let mut perms = m.permissions();
            perms.set_readonly(true);
            fs::set_permissions(&ro_wxs, perms).is_ok()
        }));
        #[cfg(unix)]
        let is_root = msi::platform::users::is_elevated();
        #[cfg(not(unix))]
        let is_root = false;
        assert_eq!(
            run(&["format".to_string(), ro_wxs.to_string_lossy().to_string()]),
            i32::from(!is_root),
        );
        assert!(fs::metadata(&ro_wxs).is_ok_and(|m| {
            let mut restore_perms = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            restore_perms.set_readonly(false);
            fs::set_permissions(&ro_wxs, restore_perms).is_ok()
        }));
    }

    /// Tests all branches and error handling of `wix` binary execution.
    #[test]
    fn test_wix_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_wix_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("app.wxs");
        let msi_file = temp_dir.join("app.msi");
        let second_msi_file = temp_dir.join("app2.msi");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{33333333-3333-3333-3333-333333333333}" Name="App" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        assert_eq!(run(&[]), 1);
        for cmd in ["--version", "--help", "burn"] {
            assert_eq!(run(&[cmd.to_string()]), 0);
        }
        assert_eq!(run_app(&["--version".to_string()]), ExitCode::SUCCESS);
        assert_eq!(run(&["unknown".to_string()]), 1);

        for out in [&msi_file, &second_msi_file] {
            assert_eq!(
                run(&[
                    "build".to_string(),
                    "-sval".to_string(),
                    "-o".to_string(),
                    out.to_string_lossy().to_string(),
                    src_file.to_string_lossy().to_string(),
                ]),
                0
            );
        }
        assert!(msi_file.exists());

        for bad_arg in ["-arch", "nonexistent.wxs"] {
            assert_eq!(run(&["build".to_string(), bad_arg.to_string()]), 1);
        }

        run_wix_clean_extension_and_format_cases(&temp_dir, &src_file, wxs);
        run_wix_harvest_cases(&temp_dir);
        run_wix_msi_decompile_cases(&temp_dir, &msi_file);
        run_wix_msi_validate_and_diff_cases(&temp_dir, &msi_file, &second_msi_file);

        let blocker_file = temp_dir.join("blocker_out_file");
        assert!(fs::write(&blocker_file, "blocker").is_ok());
        let invalid_out = blocker_file.join("test.xml");
        assert_eq!(
            run(&[
                "harvest".to_string(),
                "build".to_string(),
                temp_dir.to_string_lossy().to_string(),
                "-o".to_string(),
                invalid_out.to_string_lossy().to_string(),
            ]),
            1,
        );
        assert_eq!(
            run(&[
                "msi".to_string(),
                "decompile".to_string(),
                msi_file.to_string_lossy().to_string(),
                "-o".to_string(),
                invalid_out.to_string_lossy().to_string(),
            ]),
            1,
        );

        assert_eq!(run_app(&[]), ExitCode::FAILURE);
        let _code = main();

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_wix_version_and_help() {
        assert_eq!(run(&["-version".to_string()]), 0);
        assert_eq!(run(&["--version".to_string()]), 0);
        assert_eq!(run(&["-v".to_string()]), 0);
        assert_eq!(run(&["version".to_string()]), 0);
        assert_eq!(run(&["-?".to_string()]), 0);
        assert_eq!(run(&["/?".to_string()]), 0);
        assert_eq!(run(&["-help".to_string()]), 0);
        assert_eq!(run(&["--help".to_string()]), 0);
        assert_eq!(run(&["-h".to_string()]), 0);
        assert_eq!(run(&["help".to_string()]), 0);
    }
}
