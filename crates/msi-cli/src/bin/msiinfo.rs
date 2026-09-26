//! # msiinfo
//!
//! Windows Installer Information and Inspection tool replicating GNOME `msiinfo`.
//!
//! Provides inspection and modification of `SummaryInformation` streams,
//! table listing, schema inspection, stream listing, table exporting, and raw stream extraction.
//!
//! ## Usage
//!
//! ```sh
//! msiinfo <msi> [options]
//! msiinfo tables <msi>
//! msiinfo schema <msi> [table]
//! msiinfo streams <msi>
//! msiinfo export <msi> <table>
//! msiinfo extract <msi> <stream> [out_file]
//! ```

use msi::cfb::directory::ObjectType;
use msi::cfb::reader::CfbReader;
use msi::package::Package;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Parsed options for `msiinfo`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsiInfoOptions {
    /// Action mode (`summary`, `tables`, `schema`, `streams`, `export`, `extract`).
    pub action: String,
    /// Path to target MSI database.
    pub msi_path: PathBuf,
    /// Target table name for `schema` or `export`.
    pub table: Option<String>,
    /// Target stream name for `extract`.
    pub stream: Option<String>,
    /// Target output file for `extract`.
    pub out_file: Option<PathBuf>,
    /// Summary info property updates: title
    pub set_title: Option<String>,
    /// Summary info property updates: subject
    pub set_subject: Option<String>,
    /// Summary info property updates: author
    pub set_author: Option<String>,
    /// Summary info property updates: keywords
    pub set_keywords: Option<String>,
    /// Summary info property updates: comments
    pub set_comments: Option<String>,
    /// Summary info property updates: template
    pub set_template: Option<String>,
}

impl MsiInfoOptions {
    /// Parses arguments into [`MsiInfoOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiInfoOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target files.
    #[allow(clippy::too_many_lines)]
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Err("missing arguments. Usage: msiinfo <msi> [options] | msiinfo <tables|schema|streams|export|extract> <msi> [args]".to_string());
        }

        let first = &args[0];
        if first == "tables"
            || first == "schema"
            || first == "streams"
            || first == "export"
            || first == "extract"
        {
            if args.len() < 2 {
                return Err(format!("missing package path for '{first}' command"));
            }
            let msi_path = PathBuf::from(&args[1]);
            let mut opts = Self {
                action: first.clone(),
                msi_path,
                ..Self::default()
            };

            match first.as_str() {
                "schema" => {
                    if args.len() >= 3 {
                        opts.table = Some(args[2].clone());
                    }
                }
                "export" => {
                    if args.len() < 3 {
                        return Err("missing table name for 'export' command".to_string());
                    }
                    opts.table = Some(args[2].clone());
                }
                "extract" => {
                    if args.len() < 3 {
                        return Err("missing stream name for 'extract' command".to_string());
                    }
                    opts.stream = Some(args[2].clone());
                    if args.len() >= 4 {
                        opts.out_file = Some(PathBuf::from(&args[3]));
                    }
                }
                _ => {}
            }

            return Ok(opts);
        }

        // Default mode: summary inspection or property editing
        let msi_path = PathBuf::from(first);
        let mut opts = Self {
            action: "summary".to_string(),
            msi_path,
            ..Self::default()
        };

        let mut idx = 1;
        while idx < args.len() {
            let arg = &args[idx];
            if arg == "-t" || arg == "--title" {
                idx += 1;
                if idx < args.len() {
                    opts.set_title = Some(args[idx].clone());
                }
            } else if arg == "-j" || arg == "--subject" {
                idx += 1;
                if idx < args.len() {
                    opts.set_subject = Some(args[idx].clone());
                }
            } else if arg == "-a" || arg == "--author" {
                idx += 1;
                if idx < args.len() {
                    opts.set_author = Some(args[idx].clone());
                }
            } else if arg == "-k" || arg == "--keywords" {
                idx += 1;
                if idx < args.len() {
                    opts.set_keywords = Some(args[idx].clone());
                }
            } else if arg == "-c" || arg == "--comments" {
                idx += 1;
                if idx < args.len() {
                    opts.set_comments = Some(args[idx].clone());
                }
            } else if arg == "-p" || arg == "--template" {
                idx += 1;
                if idx < args.len() {
                    opts.set_template = Some(args[idx].clone());
                }
            }
            idx += 1;
        }

        Ok(opts)
    }

    /// Executes the specified inspection or modification action.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or database failure.
    #[allow(clippy::too_many_lines)]
    pub fn execute(&self) -> Result<(), String> {
        match self.action.as_str() {
            "summary" => {
                let mut pkg = Package::open(&self.msi_path).map_err(|e| {
                    format!("failed opening MSI '{}': {e}", self.msi_path.display())
                })?;

                let has_edits = self.set_title.is_some()
                    || self.set_subject.is_some()
                    || self.set_author.is_some()
                    || self.set_keywords.is_some()
                    || self.set_comments.is_some()
                    || self.set_template.is_some();

                if has_edits {
                    if let Some(ref title) = self.set_title {
                        pkg.summary_info_mut().title = Some(title.clone());
                    }
                    if let Some(ref subject) = self.set_subject {
                        pkg.summary_info_mut().subject = Some(subject.clone());
                    }
                    if let Some(ref author) = self.set_author {
                        pkg.summary_info_mut().author = Some(author.clone());
                    }
                    if let Some(ref keywords) = self.set_keywords {
                        pkg.summary_info_mut().keywords = Some(keywords.clone());
                    }
                    if let Some(ref comments) = self.set_comments {
                        pkg.summary_info_mut().comments = Some(comments.clone());
                    }
                    if let Some(ref template) = self.set_template {
                        pkg.summary_info_mut().template = Some(template.clone());
                    }
                    pkg.save(&self.msi_path)
                        .map_err(|e| format!("failed saving updated MSI: {e}"))?;
                    println!(
                        "msiinfo: updated summary information in '{}'",
                        self.msi_path.display()
                    );
                } else {
                    let s = pkg.summary_info();
                    println!("Title: {}", s.title.as_deref().unwrap_or(""));
                    println!("Subject: {}", s.subject.as_deref().unwrap_or(""));
                    println!("Author: {}", s.author.as_deref().unwrap_or(""));
                    println!("Keywords: {}", s.keywords.as_deref().unwrap_or(""));
                    println!("Comments: {}", s.comments.as_deref().unwrap_or(""));
                    println!("Template: {}", s.template.as_deref().unwrap_or(""));
                    println!("PackageCode: {}", s.rev_number.as_deref().unwrap_or(""));
                    println!("SchemaVersion: {}", s.page_count.unwrap_or(0));
                    println!("WordCount: {}", s.word_count.unwrap_or(0));
                }
                Ok(())
            }
            "tables" => {
                let pkg = Package::open(&self.msi_path).map_err(|e| {
                    format!("failed opening MSI '{}': {e}", self.msi_path.display())
                })?;
                for name in pkg.database().catalog.table_names() {
                    println!("{name}");
                }
                Ok(())
            }
            "schema" => {
                let pkg = Package::open(&self.msi_path).map_err(|e| {
                    format!("failed opening MSI '{}': {e}", self.msi_path.display())
                })?;
                let catalog = &pkg.database().catalog;

                if let Some(ref table_name) = self.table {
                    if let Some(schema) = catalog.get_table(table_name) {
                        println!("Table: {table_name}");
                        for col in &schema.columns {
                            let pk_str = if col.primary_key { " [PK]" } else { "" };
                            let null_str = if col.nullable { " [Nullable]" } else { "" };
                            println!("  - {}: {:?}{pk_str}{null_str}", col.name, col.data_type);
                        }
                    } else {
                        return Err(format!("table '{table_name}' not found in database"));
                    }
                } else {
                    for name in catalog.table_names() {
                        let col_count = catalog.get_table(name).map_or(0, |s| s.columns.len());
                        println!("Table: {name} ({col_count} columns)");
                    }
                }
                Ok(())
            }
            "streams" => {
                let bytes = fs::read(&self.msi_path).map_err(|e| {
                    format!("failed reading file '{}': {e}", self.msi_path.display())
                })?;
                let reader = CfbReader::new(&bytes)
                    .map_err(|e| format!("failed reading CFB container: {e}"))?;

                for entry in reader.entries() {
                    if entry.object_type() == ObjectType::Stream {
                        println!("{} ({} bytes)", entry.name(), entry.stream_size());
                    }
                }
                Ok(())
            }
            "export" => {
                let pkg = Package::open(&self.msi_path).map_err(|e| {
                    format!("failed opening MSI '{}': {e}", self.msi_path.display())
                })?;
                let table_name = self.table.as_deref().unwrap_or("");
                pkg.database().tables.get(table_name).map_or_else(
                    || Err(format!("table '{table_name}' not found in database")),
                    |rows| {
                        for r in rows {
                            let line = r
                                .fields()
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\t");
                            println!("{line}");
                        }
                        Ok(())
                    },
                )
            }
            "extract" => {
                let bytes = fs::read(&self.msi_path).map_err(|e| {
                    format!("failed reading file '{}': {e}", self.msi_path.display())
                })?;
                let reader = CfbReader::new(&bytes)
                    .map_err(|e| format!("failed reading CFB container: {e}"))?;

                let stream_name = self.stream.as_deref().unwrap_or("");
                let data = reader
                    .read_stream(stream_name)
                    .map_err(|e| format!("failed reading stream '{stream_name}': {e}"))?;

                if let Some(ref out_p) = self.out_file {
                    if let Some(parent) = out_p.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    fs::write(out_p, &data)
                        .map_err(|e| format!("failed writing output file: {e}"))?;
                    println!("msiinfo: extracted {stream_name} to '{}'", out_p.display());
                } else {
                    println!("msiinfo: extracted {stream_name} ({} bytes)", data.len());
                }
                Ok(())
            }
            other => Err(format!("unknown msiinfo command '{other}'")),
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
/// Exit code: `0` on success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match MsiInfoOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msiinfo : error : {err}");
            return 1;
        }
    };

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("msiinfo : error : {err}");
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

/// Entry point for the `msiinfo` executable.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests all branches and error handling of `msiinfo` binary execution.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_msiinfo_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiinfo_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("info.wxs");
        let msi_file = temp_dir.join("info.msi");
        let out_extract = temp_dir.join("sub_extract").join("extracted_stream.bin");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-3333-5555-7777-999999999999}" Name="InfoApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        let _ = msi::wix::WixBuildOptions::parse(&[
            "-sval".to_string(),
            "-o".to_string(),
            msi_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default()
        .execute();

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["tables".to_string()]), 1);
        assert_eq!(run(&["schema".to_string()]), 1);
        assert_eq!(run(&["streams".to_string()]), 1);
        assert_eq!(
            run(&["export".to_string(), msi_file.to_string_lossy().to_string()]),
            1
        );
        assert_eq!(
            run(&[
                "extract".to_string(),
                msi_file.to_string_lossy().to_string()
            ]),
            1
        );

        // Long options parsing and trailing options without value
        let long_edit_args = vec![
            msi_file.to_string_lossy().to_string(),
            "--title".to_string(),
            "Long Title".to_string(),
            "--subject".to_string(),
            "Long Subject".to_string(),
            "--author".to_string(),
            "Long Author".to_string(),
            "--keywords".to_string(),
            "Long Keywords".to_string(),
            "--comments".to_string(),
            "Long Comments".to_string(),
            "--template".to_string(),
            "Intel;1033".to_string(),
        ];
        assert_eq!(run(&long_edit_args), 0);

        let trailing_opts = [
            "-t",
            "-j",
            "-a",
            "-k",
            "-c",
            "-p",
            "--title",
            "--subject",
            "--author",
            "--keywords",
            "--comments",
            "--template",
        ];
        for opt in trailing_opts {
            assert_eq!(
                run(&[msi_file.to_string_lossy().to_string(), opt.to_string()]),
                0
            );
        }
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "--unknown-flag".to_string()
            ]),
            0
        );

        // 2. Summary inspection
        assert_eq!(run(&[msi_file.to_string_lossy().to_string()]), 0);

        // 3. Summary editing: individual properties
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-t".to_string(),
                "T".to_string()
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-j".to_string(),
                "J".to_string()
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-a".to_string(),
                "A".to_string()
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-k".to_string(),
                "K".to_string()
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-c".to_string(),
                "C".to_string()
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-p".to_string(),
                "P".to_string()
            ]),
            0
        );

        let edit_args = vec![
            msi_file.to_string_lossy().to_string(),
            "-t".to_string(),
            "New Title".to_string(),
            "-j".to_string(),
            "New Subject".to_string(),
            "-a".to_string(),
            "New Author".to_string(),
            "-k".to_string(),
            "New Keywords".to_string(),
            "-c".to_string(),
            "New Comments".to_string(),
            "-p".to_string(),
            "x64;1033".to_string(),
        ];
        assert_eq!(run(&edit_args), 0);
        assert_eq!(run_app(&edit_args), ExitCode::SUCCESS);

        // Summary edit save failure on read-only file
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let ro_msi = temp_dir.join("readonly.msi");
            assert!(fs::copy(&msi_file, &ro_msi).is_ok());
            assert!(fs::set_permissions(&ro_msi, fs::Permissions::from_mode(0o400)).is_ok());
            assert_eq!(
                run(&[
                    ro_msi.to_string_lossy().to_string(),
                    "-t".to_string(),
                    "FailTitle".to_string(),
                ]),
                1
            );
            assert!(fs::set_permissions(&ro_msi, fs::Permissions::from_mode(0o644)).is_ok());
        }

        // 4. Tables listing
        assert_eq!(
            run(&["tables".to_string(), msi_file.to_string_lossy().to_string()]),
            0
        );
        assert_eq!(
            run(&["tables".to_string(), "nonexistent.msi".to_string()]),
            1
        );

        // 5. Schema inspection (all, single table, and failure)
        assert_eq!(
            run(&["schema".to_string(), msi_file.to_string_lossy().to_string()]),
            0
        );
        assert_eq!(
            run(&[
                "schema".to_string(),
                msi_file.to_string_lossy().to_string(),
                "Property".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                "schema".to_string(),
                msi_file.to_string_lossy().to_string(),
                "Directory".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                "schema".to_string(),
                msi_file.to_string_lossy().to_string(),
                "NonexistentTable".to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&["schema".to_string(), "nonexistent.msi".to_string()]),
            1
        );

        // 6. Streams listing
        assert_eq!(
            run(&[
                "streams".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&["streams".to_string(), "nonexistent.msi".to_string()]),
            1
        );

        let corrupt_msi = temp_dir.join("corrupt.msi");
        assert!(fs::write(&corrupt_msi, b"not cfb format").is_ok());
        assert_eq!(
            run(&[
                "streams".to_string(),
                corrupt_msi.to_string_lossy().to_string(),
            ]),
            1
        );

        // 7. Export table
        assert_eq!(
            run(&[
                "export".to_string(),
                msi_file.to_string_lossy().to_string(),
                "Property".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                "export".to_string(),
                msi_file.to_string_lossy().to_string(),
                "Nonexistent".to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&[
                "export".to_string(),
                "nonexistent.msi".to_string(),
                "Property".to_string(),
            ]),
            1
        );

        // 8. Extract stream (stdout, file, errors)
        assert_eq!(
            run(&[
                "extract".to_string(),
                msi_file.to_string_lossy().to_string(),
                "\u{0005}SummaryInformation".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                "extract".to_string(),
                msi_file.to_string_lossy().to_string(),
                "\u{0005}SummaryInformation".to_string(),
                out_extract.to_string_lossy().to_string(),
            ]),
            0
        );
        assert!(out_extract.exists());

        assert_eq!(
            run(&[
                "extract".to_string(),
                "nonexistent.msi".to_string(),
                "stream".to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&[
                "extract".to_string(),
                corrupt_msi.to_string_lossy().to_string(),
                "stream".to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&[
                "extract".to_string(),
                msi_file.to_string_lossy().to_string(),
                "NonexistentStream".to_string(),
            ]),
            1
        );

        // Extract write failure
        let blocking_file = temp_dir.join("blocking_parent_extract");
        assert!(fs::write(&blocking_file, "blocking").is_ok());
        assert_eq!(
            run(&[
                "extract".to_string(),
                msi_file.to_string_lossy().to_string(),
                "\u{0005}SummaryInformation".to_string(),
                blocking_file.join("out.bin").to_string_lossy().to_string(),
            ]),
            1
        );

        let empty_out_extract = MsiInfoOptions {
            action: "extract".to_string(),
            msi_path: msi_file,
            stream: Some("\u{0005}SummaryInformation".to_string()),
            out_file: Some(PathBuf::from("")),
            ..MsiInfoOptions::default()
        };
        assert!(empty_out_extract.execute().is_err());

        // 9. Error on non-existent package
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        // 10. Unsupported command
        let other_opts = MsiInfoOptions {
            action: "unknown_cmd".to_string(),
            ..MsiInfoOptions::default()
        };
        assert!(other_opts.execute().is_err());

        // 11. Derives testing
        let default_opts = MsiInfoOptions::default();
        let mut cloned_opts = default_opts.clone();
        cloned_opts.clone_from(&default_opts);
        let different_opts = MsiInfoOptions {
            action: "schema".to_string(),
            ..MsiInfoOptions::default()
        };
        assert_eq!(default_opts, cloned_opts);
        assert_ne!(default_opts, different_opts);
        assert!(format!("{default_opts:?}").contains("MsiInfoOptions"));

        // 12. Test invoking main directly and run_app error
        assert_eq!(run_app(&[]), ExitCode::FAILURE);
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
