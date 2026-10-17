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
//! msiinfo suminfo <msi> [options]
//! msiinfo tables <msi>
//! msiinfo schema <msi> [table]
//! msiinfo streams <msi>
//! msiinfo export <msi> <table>
//! msiinfo extract <msi> <stream> [out_file]
//! ```

use msi::cfb::directory::ObjectType;
use msi::cfb::reader::CfbReader;
use msi::package::Package;
use msi::MsiError;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

/// Writes a line to the provided writer, gracefully ignoring [`std::io::ErrorKind::BrokenPipe`].
///
/// # Arguments
///
/// * `writer` - Destination writer (such as [`std::io::Stdout`]).
/// * `line` - Text line to write.
///
/// # Errors
///
/// Returns an [`MsiError`] if an I/O error other than `BrokenPipe` occurs.
fn write_stdout_line<W: Write>(writer: &mut W, line: &str) -> Result<(), MsiError> {
    if let Err(err) = writeln!(writer, "{line}") {
        if err.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(MsiError::CommandLineError(format!(
                "stdout write error: {err}"
            )));
        }
    }
    Ok(())
}

/// Prints a line to standard output, gracefully ignoring [`std::io::ErrorKind::BrokenPipe`].
///
/// # Arguments
///
/// * `line` - Text line to write to stdout.
///
/// # Errors
///
/// Returns an [`MsiError`] if an I/O error other than `BrokenPipe` occurs.
fn print_line(line: &str) -> Result<(), MsiError> {
    write_stdout_line(&mut std::io::stdout(), line)
}

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
    /// Parses summary modification flags from an argument slice into `self`.
    ///
    /// # Arguments
    ///
    /// * `args` - Slice of trailing arguments after the MSI path / `suminfo` subcommand.
    fn parse_summary_flags(&mut self, args: &[String]) {
        let mut idx = 0;
        while idx < args.len() {
            let arg = &args[idx];
            if arg == "-t" || arg == "--title" {
                idx += 1;
                if idx < args.len() {
                    self.set_title = Some(args[idx].clone());
                }
            } else if arg == "-j" || arg == "--subject" {
                idx += 1;
                if idx < args.len() {
                    self.set_subject = Some(args[idx].clone());
                }
            } else if arg == "-a" || arg == "--author" {
                idx += 1;
                if idx < args.len() {
                    self.set_author = Some(args[idx].clone());
                }
            } else if arg == "-k" || arg == "--keywords" {
                idx += 1;
                if idx < args.len() {
                    self.set_keywords = Some(args[idx].clone());
                }
            } else if arg == "-c" || arg == "--comments" {
                idx += 1;
                if idx < args.len() {
                    self.set_comments = Some(args[idx].clone());
                }
            } else if arg == "-p" || arg == "--template" {
                idx += 1;
                if idx < args.len() {
                    self.set_template = Some(args[idx].clone());
                }
            }
            idx += 1;
        }
    }

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
    /// Returns [`MsiError`] on missing arguments or target files.
    #[allow(clippy::too_many_lines)]
    pub fn parse(args: &[String]) -> Result<Self, MsiError> {
        if args.is_empty() {
            return Err(MsiError::CommandLineError(
                "missing arguments. Usage: msiinfo <msi> [options] | msiinfo <suminfo|tables|schema|streams|export|extract> <msi> [args]"
                    .to_string(),
            ));
        }

        let first = &args[0];
        if first == "suminfo"
            || first == "summary"
            || first == "tables"
            || first == "schema"
            || first == "streams"
            || first == "export"
            || first == "extract"
        {
            if args.len() < 2 {
                return Err(MsiError::CommandLineError(format!(
                    "missing package path for '{first}' command"
                )));
            }
            let action = if first == "suminfo" {
                "summary".to_string()
            } else {
                first.clone()
            };
            let msi_path = PathBuf::from(&args[1]);
            let mut opts = Self {
                action,
                msi_path,
                ..Self::default()
            };

            match first.as_str() {
                "suminfo" | "summary" => {
                    opts.parse_summary_flags(&args[2..]);
                }
                "schema" => {
                    if args.len() >= 3 {
                        opts.table = Some(args[2].clone());
                    }
                }
                "export" => {
                    if args.len() < 3 {
                        return Err(MsiError::CommandLineError(
                            "missing table name for 'export' command".to_string(),
                        ));
                    }
                    opts.table = Some(args[2].clone());
                }
                "extract" => {
                    if args.len() < 3 {
                        return Err(MsiError::CommandLineError(
                            "missing stream name for 'extract' command".to_string(),
                        ));
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

        if first.starts_with('-') {
            return Err(MsiError::CommandLineError(
                "missing arguments. Usage: msiinfo <msi> [options] | msiinfo <suminfo|tables|schema|streams|export|extract> <msi> [args]"
                    .to_string(),
            ));
        }

        // Check if second argument is a subcommand (`msiinfo <file.msi> <subcommand> [args...]`)
        if args.len() >= 2 {
            let second = &args[1];
            if second == "suminfo"
                || second == "summary"
                || second == "tables"
                || second == "schema"
                || second == "streams"
                || second == "export"
                || second == "extract"
            {
                let action = if second == "suminfo" {
                    "summary".to_string()
                } else {
                    second.clone()
                };
                let mut opts = Self {
                    action,
                    msi_path: PathBuf::from(first),
                    ..Self::default()
                };
                match second.as_str() {
                    "suminfo" | "summary" => {
                        opts.parse_summary_flags(&args[2..]);
                    }
                    "schema" => {
                        if args.len() >= 3 {
                            opts.table = Some(args[2].clone());
                        }
                    }
                    "export" => {
                        if args.len() < 3 {
                            return Err(MsiError::CommandLineError(
                                "missing table name for 'export' command".to_string(),
                            ));
                        }
                        opts.table = Some(args[2].clone());
                    }
                    "extract" => {
                        if args.len() < 3 {
                            return Err(MsiError::CommandLineError(
                                "missing stream name for 'extract' command".to_string(),
                            ));
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
        }

        // Default mode: summary inspection or property editing
        let msi_path = PathBuf::from(first);
        let mut opts = Self {
            action: "summary".to_string(),
            msi_path,
            ..Self::default()
        };

        opts.parse_summary_flags(&args[1..]);

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
    /// Returns [`MsiError`] on I/O or database failure.
    #[allow(clippy::too_many_lines)]
    pub fn execute(&self) -> Result<(), MsiError> {
        match self.action.as_str() {
            "summary" => {
                let mut pkg = Package::open(&self.msi_path).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed opening MSI '{}': {e}",
                        self.msi_path.display()
                    ))
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
                    pkg.save(&self.msi_path).map_err(|e| {
                        MsiError::CommandLineError(format!("failed saving updated MSI: {e}"))
                    })?;
                    print_line(&format!(
                        "msiinfo: updated summary information in '{}'",
                        self.msi_path.display()
                    ))?;
                } else {
                    let s = pkg.summary_info();
                    for line in [
                        format!("Title: {}", s.title.as_deref().unwrap_or("")),
                        format!("Subject: {}", s.subject.as_deref().unwrap_or("")),
                        format!("Author: {}", s.author.as_deref().unwrap_or("")),
                        format!("Keywords: {}", s.keywords.as_deref().unwrap_or("")),
                        format!("Comments: {}", s.comments.as_deref().unwrap_or("")),
                        format!("Template: {}", s.template.as_deref().unwrap_or("")),
                        format!("PackageCode: {}", s.rev_number.as_deref().unwrap_or("")),
                        format!("SchemaVersion: {}", s.page_count.unwrap_or(0)),
                        format!("WordCount: {}", s.word_count.unwrap_or(0)),
                    ] {
                        print_line(&line)?;
                    }
                }
                Ok(())
            }
            "tables" => {
                let pkg = Package::open(&self.msi_path).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed opening MSI '{}': {e}",
                        self.msi_path.display()
                    ))
                })?;
                for name in pkg.database().catalog.table_names() {
                    print_line(name)?;
                }
                Ok(())
            }
            "schema" => {
                let pkg = Package::open(&self.msi_path).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed opening MSI '{}': {e}",
                        self.msi_path.display()
                    ))
                })?;
                let catalog = &pkg.database().catalog;

                if let Some(ref table_name) = self.table {
                    if let Some(schema) = catalog.get_table(table_name) {
                        print_line(&format!("Table: {table_name}"))?;
                        for col in &schema.columns {
                            let pk_str = if col.primary_key { " [PK]" } else { "" };
                            let null_str = if col.nullable { " [Nullable]" } else { "" };
                            print_line(&format!(
                                "  - {}: {:?}{pk_str}{null_str}",
                                col.name, col.data_type
                            ))?;
                        }
                    } else {
                        return Err(MsiError::CommandLineError(format!(
                            "table '{table_name}' not found in database"
                        )));
                    }
                } else {
                    for name in catalog.table_names() {
                        let col_count = catalog.get_table(name).map_or(0, |s| s.columns.len());
                        print_line(&format!("Table: {name} ({col_count} columns)"))?;
                    }
                }
                Ok(())
            }
            "streams" => {
                let bytes = fs::read(&self.msi_path).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed reading file '{}': {e}",
                        self.msi_path.display()
                    ))
                })?;
                let reader = CfbReader::new(&bytes).map_err(|e| {
                    MsiError::CommandLineError(format!("failed reading CFB container: {e}"))
                })?;

                for entry in reader.entries() {
                    if entry.object_type() == ObjectType::Stream {
                        print_line(&format!("{} ({} bytes)", entry.name(), entry.stream_size()))?;
                    }
                }
                Ok(())
            }
            "export" => {
                let pkg = Package::open(&self.msi_path).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed opening MSI '{}': {e}",
                        self.msi_path.display()
                    ))
                })?;
                let table_name = self.table.as_deref().unwrap_or("");
                pkg.database().tables.get(table_name).map_or_else(
                    || {
                        Err(MsiError::CommandLineError(format!(
                            "table '{table_name}' not found in database"
                        )))
                    },
                    |rows| {
                        for r in rows {
                            let line = r
                                .fields()
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("	");
                            print_line(&line)?;
                        }
                        Ok(())
                    },
                )
            }
            "extract" => {
                let bytes = fs::read(&self.msi_path).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed reading file '{}': {e}",
                        self.msi_path.display()
                    ))
                })?;
                let reader = CfbReader::new(&bytes).map_err(|e| {
                    MsiError::CommandLineError(format!("failed reading CFB container: {e}"))
                })?;

                let stream_name = self.stream.as_deref().unwrap_or("");
                let data = reader.read_stream(stream_name).map_err(|e| {
                    MsiError::CommandLineError(format!(
                        "failed reading stream '{stream_name}': {e}"
                    ))
                })?;

                if let Some(ref out_p) = self.out_file {
                    if let Some(parent) = out_p.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    fs::write(out_p, &data).map_err(|e| {
                        MsiError::CommandLineError(format!("failed writing output file: {e}"))
                    })?;
                    print_line(&format!(
                        "msiinfo: extracted {stream_name} to '{}'",
                        out_p.display()
                    ))?;
                } else {
                    print_line(&format!(
                        "msiinfo: extracted {stream_name} ({} bytes)",
                        data.len()
                    ))?;
                }
                Ok(())
            }
            other => Err(MsiError::CommandLineError(format!(
                "unknown msiinfo command '{other}'"
            ))),
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
///
/// # Returns
///
/// Process [`ExitCode`] (`ExitCode::SUCCESS` or `ExitCode::FAILURE`).
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Mock writer that simulates a `BrokenPipe` or other I/O error.
    struct MockStdoutWriter {
        /// Error kind to return on write.
        error_kind: Option<std::io::ErrorKind>,
    }

    impl Write for MockStdoutWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.error_kind.map_or_else(
                || Ok(buf.len()),
                |kind| Err(std::io::Error::new(kind, "mock write error")),
            )
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Exercises `tables`, `schema`, and `streams` subcommands in `msiinfo`.
    fn run_msiinfo_tables_schema_streams_cases(temp_dir: &Path, msi_file: &Path) {
        assert_eq!(
            run(&["tables".to_string(), msi_file.to_string_lossy().to_string()]),
            0
        );
        assert_eq!(
            run(&[msi_file.to_string_lossy().to_string(), "tables".to_string()]),
            0
        );
        assert_eq!(
            run(&["tables".to_string(), "nonexistent.msi".to_string()]),
            1
        );

        assert_eq!(
            run(&["schema".to_string(), msi_file.to_string_lossy().to_string()]),
            0
        );
        assert_eq!(
            run(&[msi_file.to_string_lossy().to_string(), "schema".to_string()]),
            0
        );
        for tbl in &["Property", "Directory"] {
            assert_eq!(
                run(&[
                    "schema".to_string(),
                    msi_file.to_string_lossy().to_string(),
                    (*tbl).to_string(),
                ]),
                0
            );
            assert_eq!(
                run(&[
                    msi_file.to_string_lossy().to_string(),
                    "schema".to_string(),
                    (*tbl).to_string(),
                ]),
                0
            );
        }
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

        assert_eq!(
            run(&[
                "streams".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "streams".to_string(),
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
    }

    /// Exercises `export` and `extract` subcommands in `msiinfo`.
    #[allow(clippy::too_many_lines)]
    fn run_msiinfo_export_and_extract_cases(temp_dir: &Path, msi_file: &Path, out_extract: &Path) {
        let corrupt_msi = temp_dir.join("corrupt.msi");
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
                msi_file.to_string_lossy().to_string(),
                "export".to_string(),
                "Property".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[msi_file.to_string_lossy().to_string(), "export".to_string()]),
            1
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
                msi_file.to_string_lossy().to_string(),
                "extract".to_string(),
                "\u{0005}SummaryInformation".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "extract".to_string(),
                "\u{0005}SummaryInformation".to_string(),
                out_extract.to_string_lossy().to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "extract".to_string(),
            ]),
            1
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

        for bad_target in &["nonexistent.msi", &corrupt_msi.to_string_lossy()] {
            assert_eq!(
                run(&[
                    "extract".to_string(),
                    (*bad_target).to_string(),
                    "stream".to_string(),
                ]),
                1
            );
        }
        assert_eq!(
            run(&[
                "extract".to_string(),
                msi_file.to_string_lossy().to_string(),
                "NonexistentStream".to_string(),
            ]),
            1
        );

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
            msi_path: msi_file.to_path_buf(),
            stream: Some("\u{0005}SummaryInformation".to_string()),
            out_file: Some(PathBuf::from("")),
            ..MsiInfoOptions::default()
        };
        assert!(empty_out_extract.execute().is_err());
    }

    /// Exercises summary editing options and read-only failures in `msiinfo`.
    #[allow(clippy::too_many_lines)]
    fn run_msiinfo_summary_edit_cases(temp_dir: &Path, msi_file: &Path) {
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

        // Both `msiinfo suminfo <file.msi>` and `msiinfo <file.msi> suminfo`
        assert_eq!(
            run(&[
                "suminfo".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                "summary".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "suminfo".to_string(),
            ]),
            0
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "summary".to_string(),
            ]),
            0
        );

        for opt in [
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
            "--unknown-flag",
        ] {
            assert_eq!(
                run(&[msi_file.to_string_lossy().to_string(), opt.to_string()]),
                0
            );
        }

        assert_eq!(run(&[msi_file.to_string_lossy().to_string()]), 0);

        for (flag, val) in [
            ("-t", "T"),
            ("-j", "J"),
            ("-a", "A"),
            ("-k", "K"),
            ("-c", "C"),
            ("-p", "P"),
        ] {
            assert_eq!(
                run(&[
                    msi_file.to_string_lossy().to_string(),
                    flag.to_string(),
                    val.to_string()
                ]),
                0
            );
        }

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

        let ro_msi = temp_dir.join("readonly.msi");
        assert!(fs::copy(msi_file, &ro_msi).is_ok());
        assert!(fs::metadata(&ro_msi).is_ok_and(|m| {
            let mut perms = m.permissions();
            perms.set_readonly(true);
            fs::set_permissions(&ro_msi, perms).is_ok()
        }));
        #[cfg(unix)]
        let is_root = msi::platform::users::is_elevated();
        #[cfg(not(unix))]
        let is_root = false;
        assert_eq!(
            run(&[
                ro_msi.to_string_lossy().to_string(),
                "-t".to_string(),
                "FailTitle".to_string(),
            ]),
            i32::from(!is_root)
        );
        assert!(fs::metadata(&ro_msi).is_ok_and(|m| {
            let mut restore_perms = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            restore_perms.set_readonly(false);
            fs::set_permissions(&ro_msi, restore_perms).is_ok()
        }));
    }

    /// Tests all branches and error handling of `msiinfo` binary execution.
    #[test]
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

        assert_eq!(run(&[]), 1);
        for sub in &[
            "-invalid", "suminfo", "summary", "tables", "schema", "streams",
        ] {
            assert_eq!(run(&[(*sub).to_string()]), 1);
        }
        for sub in &["export", "extract"] {
            assert_eq!(
                run(&[(*sub).to_string(), msi_file.to_string_lossy().to_string()]),
                1
            );
        }

        // Test BrokenPipe handling on write_stdout_line
        let mut ok_writer = MockStdoutWriter { error_kind: None };
        assert!(write_stdout_line(&mut ok_writer, "ok").is_ok());
        assert!(ok_writer.flush().is_ok());
        let mut broken_pipe_writer = MockStdoutWriter {
            error_kind: Some(std::io::ErrorKind::BrokenPipe),
        };
        assert!(write_stdout_line(&mut broken_pipe_writer, "ignored").is_ok());
        let mut other_err_writer = MockStdoutWriter {
            error_kind: Some(std::io::ErrorKind::PermissionDenied),
        };
        assert!(write_stdout_line(&mut other_err_writer, "failed").is_err());

        run_msiinfo_summary_edit_cases(&temp_dir, &msi_file);
        run_msiinfo_tables_schema_streams_cases(&temp_dir, &msi_file);
        run_msiinfo_export_and_extract_cases(&temp_dir, &msi_file, &out_extract);

        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        let other_opts = MsiInfoOptions {
            action: "unknown_cmd".to_string(),
            ..MsiInfoOptions::default()
        };
        assert!(other_opts.execute().is_err());

        let default_opts = MsiInfoOptions::default();
        let mut cloned_opts = default_opts.clone();
        cloned_opts.clone_from(&default_opts);
        let different_opts = MsiInfoOptions {
            action: "schema".to_string(),
            ..MsiInfoOptions::default()
        };
        assert_eq!(default_opts, cloned_opts);
        assert_ne!(default_opts, different_opts);
        let opts_dbg = format!("{default_opts:?}");
        assert!(opts_dbg.contains("MsiInfoOptions"));

        assert_eq!(run_app(&[]), ExitCode::FAILURE);
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
