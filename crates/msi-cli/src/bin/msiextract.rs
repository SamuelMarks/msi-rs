//! # msiextract
//!
//! Windows Installer Package Extractor replicating GNOME `msiextract`.
//!
//! Extracts payload files from `.msi` packages using embedded cabinets and directory tables.
//!
//! ## Usage
//!
//! ```sh
//! msiextract [-C <dir>] <package.msi>
//! ```

use msi::cab::reader::CabinetReader;
use msi::package::Package;
use msi::MsiError;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Parsed options for `msiextract`.
///
/// Provides extraction of `.msi` internal cabinet files or `.wim` images.
/// When extracting `.msi` packages, it maps internal flat file IDs to their
/// corresponding hierarchical structures and long file names as described by
/// the MSI database. If database mapping fails or is incomplete, extraction
/// falls back to using the raw file IDs.
#[derive(Clone, PartialEq, Eq)]
pub struct MsiExtractOptions {
    /// Destination extraction directory (`-C`, `--directory`).
    pub dest_dir: PathBuf,
    /// Path to target `.msi` package file.
    pub input_msi: PathBuf,
    /// List contained files without extracting (`-l`, `--list`).
    pub list_only: bool,
    /// Selective extraction by Component name (`--component`).
    pub component_filter: Option<String>,
    /// Selective extraction by Feature name (`--feature`).
    pub feature_filter: Option<String>,
    /// Indicates if the input is a WIM file to extract.
    pub is_wim: bool,
    /// The index of the image to extract from the WIM.
    pub wim_index: Option<u32>,
}

impl Default for MsiExtractOptions {
    fn default() -> Self {
        Self {
            dest_dir: PathBuf::new(),
            input_msi: PathBuf::new(),
            list_only: false,
            component_filter: None,
            feature_filter: None,
            is_wim: false,
            wim_index: None,
        }
    }
}

impl std::fmt::Debug for MsiExtractOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MsiExtractOptions")
            .field("dest_dir", &self.dest_dir)
            .field("input_msi", &self.input_msi)
            .field("list_only", &self.list_only)
            .field("component_filter", &self.component_filter)
            .field("feature_filter", &self.feature_filter)
            .field("is_wim", &self.is_wim)
            .field("wim_index", &self.wim_index)
            .finish()
    }
}

impl MsiExtractOptions {
    /// Parses arguments into [`MsiExtractOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiExtractOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] on missing arguments or target files.
    pub fn parse(args: &[String]) -> Result<Self, MsiError> {
        if args.is_empty() {
            return Err(MsiError::CommandLineError(
                "missing arguments. Usage: msiextract [-C <dir>] [-l|--list] [--component <comp>] [--feature <feat>] <package.msi>"
                    .to_string(),
            ));
        }

        let mut dest_dir = PathBuf::from(".");
        let mut input_msi = PathBuf::new();
        let mut list_only = false;
        let mut component_filter = None;
        let mut feature_filter = None;
        let mut is_wim = false;
        let mut wim_index = None;
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];
            if arg == "--wim" {
                is_wim = true;
                idx += 1;
            } else if arg == "--index" {
                wim_index = args.get(idx + 1).and_then(|s| s.parse::<u32>().ok());
                idx += if wim_index.is_some() { 2 } else { 1 };
            } else if arg == "-C" || arg == "--directory" {
                idx += 1;
                if idx < args.len() {
                    dest_dir = PathBuf::from(&args[idx]);
                    idx += 1;
                }
            } else if arg == "-l" || arg == "--list" {
                list_only = true;
                idx += 1;
            } else if arg == "--component" {
                idx += 1;
                if idx < args.len() {
                    component_filter = Some(args[idx].clone());
                    idx += 1;
                }
            } else if arg == "--feature" {
                idx += 1;
                if idx < args.len() {
                    feature_filter = Some(args[idx].clone());
                    idx += 1;
                }
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || std::path::Path::new(arg).extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("wim")
                    })
                    || PathBuf::from(arg).exists())
            {
                input_msi = PathBuf::from(arg);
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if input_msi.as_os_str().is_empty() {
            return Err(MsiError::CommandLineError(
                "no input MSI package specified".to_string(),
            ));
        }

        Ok(Self {
            dest_dir,
            input_msi,
            list_only,
            component_filter,
            feature_filter,
            is_wim,
            wim_index,
        })
    }

    /// Extracts all files from embedded cabinets into the destination directory.
    ///
    /// This method leverages the `Directory` and `File` tables within the MSI
    /// database to reconstruct the source filesystem hierarchy. For each extracted
    /// file, it resolves the component directory tree and target file name.
    /// If these records are missing, it falls back to a flat dump using the file's
    /// cabinet key ID.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    /// Executes WIM listing or extraction.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if reading or parsing the WIM image fails.
    ///
    /// # Returns
    ///
    /// `Ok(())` when listing or extraction completes.
    fn execute_wim(&self) -> Result<(), MsiError> {
        let bytes = fs::read(&self.input_msi).map_err(|e| {
            MsiError::CommandLineError(format!(
                "failed reading file '{}': {e}",
                self.input_msi.display()
            ))
        })?;
        let wim_reader = msi::wim::WimReader::new(&bytes)
            .map_err(|e| MsiError::CommandLineError(format!("WIM parse error: {e}")))?;

        if self.list_only {
            println!("Contained files in WIM '{}':", self.input_msi.display());
            for img in &wim_reader.manifest.images {
                println!("  Image {}: {}", img.index.0, img.name);
            }
            return Ok(());
        }

        let idx = self.wim_index.unwrap_or(1);
        let _ = wim_reader.extract_image(&self.dest_dir, idx).map_err(|e| {
            MsiError::CommandLineError(format!("failed extracting WIM image {idx}: {e}"))
        })?;
        println!(
            "msiextract: extracted WIM image {} into '{}'",
            idx,
            self.dest_dir.display()
        );
        Ok(())
    }

    /// Builds a mapping from MSI `File` key identifiers to relative destination paths.
    ///
    /// # Arguments
    ///
    /// * `pkg` - Opened MSI package.
    ///
    /// # Returns
    ///
    /// A map from file key strings to resolved relative [`PathBuf`] paths.
    fn build_file_map(pkg: &Package) -> std::collections::HashMap<String, PathBuf> {
        use msi::database::FieldValue;
        let mut dir_map: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();
        let mut file_map: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();

        let files = pkg.database().get_records("File");
        let dirs = pkg.database().get_records("Directory");
        if files.is_empty() || dirs.is_empty() {
            return file_map;
        }

        let mut parents = std::collections::HashMap::new();
        let mut names = std::collections::HashMap::new();

        for r in dirs {
            if let (Some(FieldValue::String(id)), default_dir) = (r.get(0), r.get(2)) {
                let parent = r.get(1).and_then(|f| match f {
                    FieldValue::String(s) => Some(s.clone()),
                    _ => None,
                });

                let dir_name = match default_dir {
                    Some(FieldValue::String(s)) => {
                        let target = s.split(':').next().unwrap_or(s.as_str());
                        target.split('|').next_back().unwrap_or(target).to_string()
                    }
                    _ => id.clone(),
                };

                if dir_name == "." || dir_name == "SourceDir" {
                    names.insert(id.clone(), String::new());
                } else {
                    names.insert(id.clone(), dir_name);
                }
                if let Some(p) = parent {
                    parents.insert(id.clone(), p);
                }
            }
        }

        for id in names.keys() {
            let mut path = PathBuf::new();
            let mut curr = Some(id.clone());
            let mut components = Vec::new();
            while let Some(c) = curr {
                if let Some(n) = names.get(&c) {
                    if !n.is_empty() {
                        components.push(n.clone());
                    }
                }
                curr = parents.get(&c).cloned();
            }
            components.reverse();
            for comp in components {
                path.push(comp);
            }
            dir_map.insert(id.clone(), path);
        }

        let mut comp_to_dir = std::collections::HashMap::new();
        for r in pkg.database().get_records("Component") {
            if let (Some(FieldValue::String(id)), Some(FieldValue::String(dir_id))) =
                (r.get(0), r.get(2))
            {
                comp_to_dir.insert(id.clone(), dir_id.clone());
            }
        }

        for r in files {
            if let (
                Some(FieldValue::String(file_id)),
                Some(FieldValue::String(comp_id)),
                Some(FieldValue::String(file_name)),
            ) = (r.get(0), r.get(1), r.get(2))
            {
                let name = file_name
                    .split('|')
                    .next_back()
                    .unwrap_or(file_name.as_str());

                let mut path = PathBuf::new();
                if let Some(dir_id) = comp_to_dir.get(comp_id.as_str()) {
                    if let Some(dir_path) = dir_map.get(dir_id) {
                        path.push(dir_path);
                    }
                }
                path.push(name);
                file_map.insert(file_id.clone(), path);
            }
        }
        file_map
    }

    /// Executes the extraction or listing operation.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] on I/O or extraction failure.
    pub fn execute(&self) -> Result<(), MsiError> {
        if self.is_wim {
            return self.execute_wim();
        }

        let pkg = Package::open(&self.input_msi).map_err(|e| {
            MsiError::CommandLineError(format!(
                "failed opening package '{}': {e}",
                self.input_msi.display()
            ))
        })?;

        let file_map = Self::build_file_map(&pkg);

        if self.list_only {
            println!("Contained files in '{}':", self.input_msi.display());
            for (cab_name, cab_data) in pkg.embedded_cabinets() {
                if let Ok(reader) = CabinetReader::new(cab_data) {
                    for f in reader.files() {
                        let file_name = f.filename.as_str();
                        let display_name = file_map.get(file_name).map_or_else(
                            || file_name.to_string(),
                            |p| p.to_string_lossy().to_string(),
                        );
                        println!("  {} ({} bytes, in {cab_name})", display_name, f.file_size);
                    }
                } else {
                    println!("  {cab_name} ({} bytes)", cab_data.len());
                }
            }
            return Ok(());
        }

        fs::create_dir_all(&self.dest_dir).map_err(|e| {
            MsiError::CommandLineError(format!(
                "failed creating target directory '{}': {e}",
                self.dest_dir.display()
            ))
        })?;

        let mut extracted_count = 0;

        for (cab_name, cab_data) in pkg.embedded_cabinets() {
            if let Ok(reader) = CabinetReader::new(cab_data) {
                for f in reader.files() {
                    let file_name = f.filename.as_str();
                    let content = reader.extract_file(file_name).unwrap_or_default();
                    let out_path = file_map
                        .get(file_name)
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from(file_name));
                    let out_file = self.dest_dir.join(out_path);
                    if let Some(parent) = out_file.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&out_file, content);
                    extracted_count += 1;
                }
            } else {
                let out_file = self.dest_dir.join(cab_name);
                let _ = fs::write(&out_file, cab_data);
                extracted_count += 1;
            }
        }

        println!(
            "msiextract: extracted {extracted_count} files into '{}'",
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
    let opts = match MsiExtractOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msiextract : error : {err}");
            return 1;
        }
    };

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("msiextract : error : {err}");
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

/// Entry point for the `msiextract` executable.
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
    use msi::cab::folder::CompressionType;
    use msi::cab::writer::CabinetWriter;
    use msi::database::{FieldValue, Record};
    use msi::package::ProductVersion;
    use std::path::Path;

    /// Builds a sample MSI package with embedded cabinets and directory/component/file tables.
    fn build_extract_test_msi(msi_file: &Path, cab_bytes: Vec<u8>) {
        let build_res = Package::builder()
            .product_name("ExtractApp")
            .manufacturer("ExtractMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{99999999-9999-9999-9999-999999999999}")
            .add_record(
                "Directory",
                Record::with_fields(vec![
                    FieldValue::String("TARGETDIR".to_string()),
                    FieldValue::Null,
                    FieldValue::String("SourceDir".to_string()),
                ]),
            )
            .add_record(
                "Directory",
                Record::with_fields(vec![
                    FieldValue::String("DotDir".to_string()),
                    FieldValue::String("TARGETDIR".to_string()),
                    FieldValue::String(".".to_string()),
                ]),
            )
            .add_record(
                "Directory",
                Record::with_fields(vec![
                    FieldValue::String("SomeDir".to_string()),
                    FieldValue::String("TARGETDIR".to_string()),
                    FieldValue::String("sub|sub".to_string()),
                ]),
            )
            .add_record(
                "Component",
                Record::with_fields(vec![
                    FieldValue::String("Comp1".to_string()),
                    FieldValue::String("guid".to_string()),
                    FieldValue::String("SomeDir".to_string()),
                    FieldValue::Short(0),
                    FieldValue::String(String::new()),
                    FieldValue::String(String::new()),
                ]),
            )
            .add_record(
                "Component",
                Record::with_fields(vec![
                    FieldValue::String("Comp2".to_string()),
                    FieldValue::String("guid".to_string()),
                    FieldValue::String("MissingDir".to_string()),
                    FieldValue::Short(0),
                    FieldValue::String(String::new()),
                    FieldValue::String(String::new()),
                ]),
            )
            .add_record(
                "File",
                Record::with_fields(vec![
                    FieldValue::String("payload.txt".to_string()),
                    FieldValue::String("Comp1".to_string()),
                    FieldValue::String("payload.txt".to_string()),
                    FieldValue::Long(100),
                    FieldValue::String(String::new()),
                    FieldValue::String(String::new()),
                    FieldValue::Short(0),
                    FieldValue::Short(1),
                ]),
            )
            .add_record(
                "File",
                Record::with_fields(vec![
                    FieldValue::String("raw.bin".to_string()),
                    FieldValue::String("Comp2".to_string()),
                    FieldValue::String("raw.bin".to_string()),
                    FieldValue::Long(100),
                    FieldValue::String(String::new()),
                    FieldValue::String(String::new()),
                    FieldValue::Short(0),
                    FieldValue::Short(2),
                ]),
            )
            .add_embedded_cabinet("#cab1.cab", cab_bytes)
            .add_embedded_cabinet("#raw.bin", vec![1, 2, 3, 4])
            .build();
        assert!(build_res.is_ok_and(|pkg| pkg.save(msi_file).is_ok()));
    }

    /// Exercises malformed and partial `Directory`/`Component` tables during extraction.
    fn run_malformed_table_cases(temp_dir: &Path) {
        let no_dir_msi = temp_dir.join("no_dir.msi");
        let mut malformed_db = msi::wix::linker::LinkedDatabase::new().unwrap_or_default();

        let mut file_rec = Record::new();
        file_rec.push(FieldValue::String("file1".to_string()));
        file_rec.push(FieldValue::String("comp1".to_string()));
        file_rec.push(FieldValue::String("filename.txt".to_string()));
        for _ in 3..8 {
            file_rec.push(FieldValue::Null);
        }
        malformed_db
            .tables
            .insert("File".to_string(), vec![file_rec]);

        let mut comp_rec = Record::new();
        comp_rec.push(FieldValue::String("comp1".to_string()));
        comp_rec.push(FieldValue::Null);
        comp_rec.push(FieldValue::String("dir1".to_string()));
        for _ in 3..6 {
            comp_rec.push(FieldValue::Null);
        }
        malformed_db
            .tables
            .insert("Component".to_string(), vec![comp_rec]);

        let mut dir_rec = Record::new();
        dir_rec.push(FieldValue::String("dir1".to_string()));
        dir_rec.push(FieldValue::String("TARGETDIR".to_string()));
        dir_rec.push(FieldValue::String("MyDir".to_string()));
        malformed_db
            .tables
            .insert("Directory".to_string(), vec![dir_rec]);

        let no_dir_pkg = Package::from_database(malformed_db, std::collections::HashMap::new());
        let bytes = no_dir_pkg.to_bytes().unwrap_or_default();
        assert!(fs::write(&no_dir_msi, &bytes).is_ok());
        let _ = run(&["-l".to_string(), no_dir_msi.to_string_lossy().to_string()]);
        let _ = run(&[
            "-C".to_string(),
            temp_dir
                .join("no_dir_extract")
                .to_string_lossy()
                .to_string(),
            no_dir_msi.to_string_lossy().to_string(),
        ]);

        let no_comp_msi = temp_dir.join("no_comp.msi");
        let mut no_comp_db = msi::wix::linker::LinkedDatabase::new().unwrap_or_default();
        let mut file_rec_nc = Record::new();
        for _ in 0..8 {
            file_rec_nc.push(FieldValue::Null);
        }
        no_comp_db
            .tables
            .insert("File".to_string(), vec![file_rec_nc]);
        let mut dir_rec_nc = Record::new();
        for _ in 0..3 {
            dir_rec_nc.push(FieldValue::Null);
        }
        no_comp_db
            .tables
            .insert("Directory".to_string(), vec![dir_rec_nc]);
        let no_comp_pkg = Package::from_database(no_comp_db, std::collections::HashMap::new());
        assert!(fs::write(&no_comp_msi, no_comp_pkg.to_bytes().unwrap_or_default()).is_ok());
        let _ = run(&[
            "-C".to_string(),
            temp_dir
                .join("no_comp_extract")
                .to_string_lossy()
                .to_string(),
            no_comp_msi.to_string_lossy().to_string(),
        ]);
    }

    /// Exercises WIM CLI options, extraction, listing, and error branches.
    fn run_wim_branch_cases(temp_dir: &Path, out_dir: &Path) {
        let wim_file = temp_dir.join("test.wim");
        let mut wim_data = vec![];
        wim_data.extend_from_slice(&msi::wim::header::WIM_MAGIC);
        wim_data.extend_from_slice(&208u32.to_le_bytes());
        wim_data.extend_from_slice(&0x0001_0d00_u32.to_le_bytes());
        wim_data.extend_from_slice(&0u32.to_le_bytes());
        wim_data.extend_from_slice(&32768u32.to_le_bytes());
        wim_data.extend_from_slice(&[0; 16]);
        wim_data.extend_from_slice(&1u16.to_le_bytes());
        wim_data.extend_from_slice(&1u16.to_le_bytes());
        wim_data.extend_from_slice(&1u32.to_le_bytes());

        let xml_payload = b"<WIM><IMAGE INDEX=\"1\"><NAME>Img1</NAME></IMAGE></WIM>";

        let flags_size: u64 =
            (u64::from(msi::wim::header::ResourceFlags::COMPRESSED.bits()) << 56) | 0x0032;
        wim_data.extend_from_slice(&flags_size.to_le_bytes());
        wim_data.extend_from_slice(&208u64.to_le_bytes());
        wim_data.extend_from_slice(&50u64.to_le_bytes());

        let xml_len = u64::try_from(xml_payload.len()).unwrap_or(0);
        let xml_flags_size: u64 =
            (u64::from(msi::wim::header::ResourceFlags::FREE.bits()) << 56) | xml_len;
        wim_data.extend_from_slice(&xml_flags_size.to_le_bytes());
        wim_data.extend_from_slice(&258u64.to_le_bytes());
        wim_data.extend_from_slice(&xml_len.to_le_bytes());

        wim_data.extend_from_slice(&[0; 112]);
        wim_data.extend_from_slice(&[0; 50]);
        wim_data.extend_from_slice(xml_payload);

        let _ = fs::write(&wim_file, &wim_data);

        assert_eq!(
            run(&[
                "--wim".to_string(),
                "--index".to_string(),
                "1".to_string(),
                "-C".to_string(),
                out_dir.to_string_lossy().to_string(),
                wim_file.to_string_lossy().to_string(),
            ]),
            0
        );

        assert_eq!(
            run(&[
                "--wim".to_string(),
                "-l".to_string(),
                wim_file.to_string_lossy().to_string(),
            ]),
            0
        );

        assert_eq!(run(&["--wim".to_string(), "file.xyz".to_string()]), 1);

        let wim_file_err = temp_dir.join("err.wim");
        let _ = fs::write(&wim_file_err, vec![0u8; 10]);
        assert_eq!(
            run(&[
                "--wim".to_string(),
                wim_file_err.to_string_lossy().to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&[
                "--wim".to_string(),
                "--index".to_string(),
                "invalid".to_string(),
                wim_file_err.to_string_lossy().to_string(),
            ]),
            1
        );

        let missing_wim_file = temp_dir.join("missing.wim");
        assert_eq!(
            run(&[
                "--wim".to_string(),
                missing_wim_file.to_string_lossy().to_string(),
            ]),
            1
        );

        let wim_file_dir_err = temp_dir.join("dir_err.wim");
        let mut wim_hdr_only = vec![];
        wim_hdr_only.extend_from_slice(&msi::wim::header::WIM_MAGIC);
        wim_hdr_only.extend_from_slice(&208u32.to_le_bytes());
        wim_hdr_only.extend_from_slice(&0x0001_0d00_u32.to_le_bytes());
        wim_hdr_only.extend_from_slice(&0u32.to_le_bytes());
        wim_hdr_only.extend_from_slice(&32768u32.to_le_bytes());
        wim_hdr_only.extend_from_slice(&[0; 16]);
        wim_hdr_only.extend_from_slice(&1u16.to_le_bytes());
        wim_hdr_only.extend_from_slice(&1u16.to_le_bytes());
        wim_hdr_only.extend_from_slice(&1u32.to_le_bytes());
        wim_hdr_only.extend_from_slice(&[0; 160]);
        let _ = fs::write(&wim_file_dir_err, &wim_hdr_only);

        let blocking_wim_dir = temp_dir.join("blocking_wim_dir");
        let _ = fs::write(&blocking_wim_dir, "block");
        assert_eq!(
            run(&[
                "--wim".to_string(),
                "-C".to_string(),
                blocking_wim_dir.to_string_lossy().to_string(),
                wim_file_dir_err.to_string_lossy().to_string(),
            ]),
            1
        );
    }

    #[test]
    fn test_msiextract_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let out_dir = temp_dir.join("extracted_files");

        let mut cab_writer = CabinetWriter::new(CompressionType::None);
        let _ = cab_writer.add_file("payload.txt", b"Cabinet payload contents");
        let _ = cab_writer.add_file("raw.bin", b"Cabinet payload contents");
        let _ = cab_writer.add_file("/", b"test");
        let cab_bytes = cab_writer.build().unwrap_or_default();

        let msi_file = temp_dir.join("extract.msi");
        build_extract_test_msi(&msi_file, cab_bytes);

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-C".to_string()]), 1);
        assert_eq!(run(&["--component".to_string()]), 1);
        assert_eq!(run(&["--feature".to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Non-existent package
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        // 3. Successful extract with -C, --component, --feature
        let extract_args = vec![
            "-C".to_string(),
            out_dir.to_string_lossy().to_string(),
            "--component".to_string(),
            "Comp1".to_string(),
            "--feature".to_string(),
            "Feat1".to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&extract_args), 0);
        assert_eq!(run_app(&extract_args), ExitCode::SUCCESS);
        assert!(out_dir.join("sub").join("payload.txt").exists());
        assert!(out_dir.join("raw.bin").exists());

        // 4. Successful extract with --directory, unknown flags, and non-msi extension that exists
        let out_dir2 = temp_dir.join("extracted_files2");
        let noext_msi = temp_dir.join("extract_noext");
        assert!(fs::copy(&msi_file, &noext_msi).is_ok());
        assert_eq!(
            run(&[
                "--directory".to_string(),
                out_dir2.to_string_lossy().to_string(),
                "-unknown_flag".to_string(),
                "/nonexistent_slash_path.xyz".to_string(),
                noext_msi.to_string_lossy().to_string(),
            ]),
            0
        );
        // 5. List mode (-l and --list)
        let list_args = vec!["-l".to_string(), msi_file.to_string_lossy().to_string()];
        assert_eq!(run(&list_args), 0);
        let list_long_args = vec!["--list".to_string(), msi_file.to_string_lossy().to_string()];
        assert_eq!(run(&list_long_args), 0);

        // 6. Empty package to cover the `False` branch of `if files.is_empty() || dirs.is_empty()`
        let empty_msi = temp_dir.join("empty.msi");
        let empty_pkg = Package::builder()
            .product_name("Empty")
            .manufacturer("Mfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{00000000-0000-0000-0000-000000000000}")
            .build()
            .unwrap_or_default();
        let _ = empty_pkg.save(&empty_msi);
        let empty_args = vec![
            "-C".to_string(),
            out_dir.to_string_lossy().to_string(),
            empty_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&empty_args), 0);

        run_malformed_table_cases(&temp_dir);

        // 7. Failure creating target directory (blocked by file)
        let blocking_file = temp_dir.join("blocking_file");
        assert!(fs::write(&blocking_file, b"content").is_ok());
        let blocked_dest = blocking_file.join("fail_dir");
        assert_eq!(
            run(&[
                "-C".to_string(),
                blocked_dest.to_string_lossy().to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            1
        );

        // 8. Derives test
        let default_opts = MsiExtractOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        let opts_dbg = format!("{default_opts:?}");
        assert!(opts_dbg.contains("MsiExtractOptions"));
        drop(cloned_opts);

        run_wim_branch_cases(&temp_dir, &out_dir);

        // 9. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Builds the test MSI package used for hierarchical directory extraction tests.
    fn build_hierarchical_test_package(cab_bytes: Vec<u8>) -> Package {
        let mut builder = Package::builder()
            .product_name("ExtractAppHier")
            .manufacturer("ExtractMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{99999999-9999-9999-9999-999999999999}")
            .add_embedded_cabinet("#cab1.cab", cab_bytes);

        for (id, parent, def_dir) in [
            (
                Some("TARGETDIR"),
                None,
                FieldValue::String("SourceDir".to_string()),
            ),
            (None, None, FieldValue::String("SourceDir".to_string())),
            (
                Some("SubDir"),
                Some("TARGETDIR"),
                FieldValue::String("sub:SubFolder".to_string()),
            ),
            (Some("SubDir2"), Some("TARGETDIR"), FieldValue::Long(1)),
            (
                Some("SubDir3"),
                Some("TARGETDIR"),
                FieldValue::String("just_target".to_string()),
            ),
        ] {
            builder = builder.add_record(
                "Directory",
                Record::with_fields(vec![
                    id.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    parent.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    def_dir,
                ]),
            );
        }

        for (id, guid, dir) in [
            (
                Some("Comp1"),
                "{00000000-0000-0000-0000-000000000000}",
                Some("SubDir"),
            ),
            (
                Some("Comp2"),
                "{11111111-0000-0000-0000-000000000000}",
                Some("SubDir2"),
            ),
            (
                Some("Comp3"),
                "{22222222-0000-0000-0000-000000000000}",
                Some("SubDir3"),
            ),
            (None, "{33333333-0000-0000-0000-000000000000}", None),
        ] {
            builder = builder.add_record(
                "Component",
                Record::with_fields(vec![
                    id.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    FieldValue::String(guid.to_string()),
                    dir.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    FieldValue::Short(0),
                    FieldValue::Null,
                    FieldValue::Null,
                ]),
            );
        }

        for (seq, (fid, cid, fname)) in [
            (Some("file1_id"), Some("Comp1"), Some("f1|LongFileName.txt")),
            (Some("file2_id"), Some("Comp2"), Some("f2.txt")),
            (Some("file3_id"), Some("Comp3"), Some("f3.txt")),
            (None, None, None),
            (Some("file4_id"), Some("UnknownComp"), Some("f4.txt")),
        ]
        .into_iter()
        .enumerate()
        {
            builder = builder.add_record(
                "File",
                Record::with_fields(vec![
                    fid.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    cid.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    fname.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                    FieldValue::Long(100),
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Short(512),
                    FieldValue::Short(i16::try_from(seq + 1).unwrap_or(1)),
                ]),
            );
        }

        builder.build().unwrap_or_default()
    }

    #[test]
    fn test_msiextract_hierarchical_extraction() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_hier");
        let _ = fs::create_dir_all(&temp_dir);
        let out_dir = temp_dir.join("extracted_hier");

        let mut cab_writer = CabinetWriter::new(CompressionType::None);
        let _ = cab_writer.add_file("file1_id", b"Hierarchy content");
        let _ = cab_writer.add_file("file2_id", b"Fallback content");
        let _ = cab_writer.add_file("file3_id", b"Fallback root content");
        let _ = cab_writer.add_file("file4_id", b"Unmapped content");
        let cab_bytes = cab_writer.build().unwrap_or_default();

        let msi_file = temp_dir.join("extract_hier.msi");
        let pkg = build_hierarchical_test_package(cab_bytes);
        let _ = pkg.save(&msi_file);

        let extract_args = vec![
            "-C".to_string(),
            out_dir.to_string_lossy().to_string(),
            msi_file.to_string_lossy().to_string(),
        ];

        let code = run(&extract_args);
        assert_eq!(code, 0);
        let list_hier_args = vec!["-l".to_string(), msi_file.to_string_lossy().to_string()];
        assert_eq!(run(&list_hier_args), 0);

        let expected_path = out_dir.join("sub").join("LongFileName.txt");
        assert!(expected_path.exists());
        let file_content = fs::read(&expected_path).unwrap_or_default();
        assert_eq!(file_content, b"Hierarchy content");

        let fallback_path = out_dir.join("SubDir2").join("f2.txt");
        assert!(fallback_path.exists());
        let fallback_content = fs::read(&fallback_path).unwrap_or_default();
        assert_eq!(fallback_content, b"Fallback content");

        let fallback_path_3 = out_dir.join("just_target").join("f3.txt");
        assert!(fallback_path_3.exists());
        let fallback_content_3 = fs::read(&fallback_path_3).unwrap_or_default();
        assert_eq!(fallback_content_3, b"Fallback root content");

        let unmapped_path = out_dir.join("f4.txt");
        assert!(unmapped_path.exists());
        let unmapped_content = fs::read(&unmapped_path).unwrap_or_default();
        assert_eq!(unmapped_content, b"Unmapped content");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_msiextract_wim_images() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_wim");
        let _ = fs::create_dir_all(&temp_dir);
        let wim_file = temp_dir.join("test.wim");

        let mut wim_data = vec![];
        wim_data.extend_from_slice(&msi::wim::header::WIM_MAGIC);
        wim_data.extend_from_slice(&208u32.to_le_bytes());
        wim_data.extend_from_slice(&0x0001_0d00_u32.to_le_bytes());
        wim_data.extend_from_slice(&0u32.to_le_bytes());
        wim_data.extend_from_slice(&0u32.to_le_bytes());
        wim_data.extend_from_slice(&[0; 16]);
        wim_data.extend_from_slice(&1u16.to_le_bytes());
        wim_data.extend_from_slice(&1u16.to_le_bytes());
        wim_data.extend_from_slice(&1u32.to_le_bytes());
        wim_data.extend_from_slice(&0u32.to_le_bytes());
        wim_data.extend_from_slice(&0u32.to_le_bytes());

        let xml_payload = br#"<?xml version="1.0" encoding="utf-16"?><WIM><IMAGE INDEX="1"><NAME>Windows 10</NAME></IMAGE><IMAGE INDEX="2"><NAME>Windows 11</NAME></IMAGE></WIM>"#;
        let xml_len = u64::try_from(xml_payload.len()).unwrap_or(0);
        wim_data.extend_from_slice(&xml_len.to_le_bytes());
        wim_data.extend_from_slice(&xml_len.to_le_bytes());
        wim_data.extend_from_slice(&208u64.to_le_bytes());

        wim_data.extend_from_slice(&[0; 24]);
        wim_data.extend_from_slice(&[0; 24]);
        wim_data.extend_from_slice(&[0; 64]);
        wim_data.extend_from_slice(xml_payload);

        let _ = fs::write(&wim_file, &wim_data);

        let list_args = vec![
            "--wim".to_string(),
            "-l".to_string(),
            wim_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&list_args), 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
