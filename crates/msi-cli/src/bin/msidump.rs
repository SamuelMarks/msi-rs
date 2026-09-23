//! # msidump
//!
//! MSI Package Archive dumper replicating GNOME `msidump`.
//!
//! Dumps an MSI package into tables (`.idt`), binary streams, and metadata.
//!
//! ## Usage
//!
//! ```sh
//! msidump [options] <package.msi>
//! ```

use msi::cfb::directory::ObjectType;
use msi::cfb::reader::CfbReader;
use msi::package::Package;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Parsed options for `msidump`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsiDumpOptions {
    /// Destination directory to dump into (`-d`, `-t`, `-s`).
    pub dest_dir: PathBuf,
    /// Path to input `.msi` file.
    pub input_msi: PathBuf,
}

impl MsiDumpOptions {
    /// Parses arguments into [`MsiDumpOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiDumpOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target package.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Err(
                "missing input MSI package. Usage: msidump [options] <package.msi>".to_string(),
            );
        }

        let mut dest_dir = PathBuf::from(".");
        let mut input_msi = PathBuf::new();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];
            if arg == "-d" || arg == "--directory" || arg == "-t" || arg == "-s" {
                idx += 1;
                if idx < args.len() {
                    dest_dir = PathBuf::from(&args[idx]);
                    idx += 1;
                }
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || std::path::Path::new(arg)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
                    || PathBuf::from(arg).exists())
            {
                input_msi = PathBuf::from(arg);
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if input_msi.as_os_str().is_empty() {
            return Err("no input MSI package specified".to_string());
        }

        Ok(Self {
            dest_dir,
            input_msi,
        })
    }

    /// Executes the dumping process.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or dump failure.
    pub fn execute(&self) -> Result<(), String> {
        fs::create_dir_all(&self.dest_dir).map_err(|e| {
            format!(
                "failed creating directory '{}': {e}",
                self.dest_dir.display()
            )
        })?;

        let bytes = fs::read(&self.input_msi)
            .map_err(|e| format!("failed reading file '{}': {e}", self.input_msi.display()))?;
        let reader =
            CfbReader::new(&bytes).map_err(|e| format!("failed reading CFB container: {e}"))?;
        let pkg = Package::from_bytes(&bytes)
            .map_err(|e| format!("failed opening package '{}': {e}", self.input_msi.display()))?;

        // 1. Export tables as IDT
        for (table_name, rows) in &pkg.database().tables {
            let idt_path = self.dest_dir.join(format!("{table_name}.idt"));
            let mut idt_content = String::new();
            for r in rows {
                let line = r
                    .fields()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("	");
                idt_content.push_str(&line);
                idt_content.push('\n');
            }
            let _ = fs::write(idt_path, idt_content);
        }

        // 2. Export streams from CFB
        let streams_dir = self.dest_dir.join("streams");
        let _ = fs::create_dir_all(&streams_dir);

        for entry in reader.entries() {
            if entry.object_type() == ObjectType::Stream {
                let name = entry.name().replace('\u{0005}', "_sum_");
                let stream_data = reader.read_stream(entry.name()).unwrap_or_default();
                let _ = fs::write(streams_dir.join(name), stream_data);
            }
        }

        println!(
            "msidump: dumped package '{}' to '{}'",
            self.input_msi.display(),
            self.dest_dir.display()
        );
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
/// Exit code: `0` on success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match MsiDumpOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msidump : error : {err}");
            return 1;
        }
    };

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("msidump : error : {err}");
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

/// Entry point for the `msidump` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_msidump_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msidump_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("dump.wxs");
        let msi_file = temp_dir.join("dump.msi");
        let dump_out = temp_dir.join("dump_output");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{77777777-7777-7777-7777-777777777777}" Name="DumpApp" Version="1.0.0" Manufacturer="Test">
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
        assert_eq!(run(&["-d".to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Non-existent package
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        // 3. Successful dump with -d
        let dump_args = vec![
            "-d".to_string(),
            dump_out.to_string_lossy().to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&dump_args), 0);
        assert_eq!(run_app(&dump_args), ExitCode::SUCCESS);
        assert!(dump_out.exists());

        // 4. Test option variations: --directory, -t, -s, unknown flag, slash path variations
        let dump_out2 = temp_dir.join("dump_output2");
        assert_eq!(
            run(&[
                "--directory".to_string(),
                dump_out2.to_string_lossy().to_string(),
                "-t".to_string(),
                dump_out2.to_string_lossy().to_string(),
                "-s".to_string(),
                dump_out2.to_string_lossy().to_string(),
                "-unknown_flag".to_string(),
                "/invalid/slash/path.xyz".to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            0
        );

        // Existing file without standard extension
        let noext_file = temp_dir.join("dump_noext");
        assert!(fs::copy(&msi_file, &noext_file).is_ok());
        let dump_out3 = temp_dir.join("dump_output3");
        assert_eq!(
            run(&[
                "-d".to_string(),
                dump_out3.to_string_lossy().to_string(),
                noext_file.to_string_lossy().to_string(),
            ]),
            0
        );

        // 5. Failure creating destination directory (file blocking dir creation)
        let blocking_file = temp_dir.join("blocking_file");
        assert!(fs::write(&blocking_file, b"block").is_ok());
        let bad_dest = blocking_file.join("subfolder");
        assert_eq!(
            run(&[
                "-d".to_string(),
                bad_dest.to_string_lossy().to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            1
        );

        // 6. Invalid CFB container
        let invalid_cfb = temp_dir.join("invalid.msi");
        assert!(fs::write(&invalid_cfb, b"Not a CFB file header").is_ok());
        assert_eq!(
            run(&[
                "-d".to_string(),
                dump_out.to_string_lossy().to_string(),
                invalid_cfb.to_string_lossy().to_string(),
            ]),
            1
        );

        // 6b. Valid CFB container but corrupted MSI package (corrupted StringPool fails Package::from_bytes)
        let corrupt_msi = temp_dir.join("corrupt_msi.msi");
        let mut bad_pool_writer =
            msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
        let pool_name =
            msi::cfb::stream_name::encode_msi_stream_name("_StringPool", false).unwrap_or_default();
        let data_name =
            msi::cfb::stream_name::encode_msi_stream_name("_StringData", false).unwrap_or_default();
        let _ = bad_pool_writer.add_stream(&pool_name, b"short");
        let _ = bad_pool_writer.add_stream(&data_name, b"data");
        assert!(fs::write(&corrupt_msi, bad_pool_writer.build()).is_ok());
        assert_eq!(
            run(&[
                "-d".to_string(),
                dump_out.to_string_lossy().to_string(),
                corrupt_msi.to_string_lossy().to_string(),
            ]),
            1
        );

        // 7. Derives test
        let default_opts = MsiDumpOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        assert!(format!("{default_opts:?}").contains("MsiDumpOptions"));

        // 8. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
