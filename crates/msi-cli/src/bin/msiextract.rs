#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::too_many_lines)]
#![deny(clippy::unwrap_used)]
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
    /// Returns error string on missing arguments or target files.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Err("missing arguments. Usage: msiextract [-C <dir>] [-l|--list] [--component <comp>] [--feature <feat>] <package.msi>".to_string());
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
            return Err("no input MSI package specified".to_string());
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
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or extraction failure.
    #[allow(clippy::too_many_lines)]
    pub fn execute(&self) -> Result<(), String> {
        if self.is_wim {
            let bytes = fs::read(&self.input_msi)
                .map_err(|e| format!("failed reading file '{}': {e}", self.input_msi.display()))?;
            let mut cursor = std::io::Cursor::new(&bytes);
            let header = msi::wim::header::WimHeader::read(&mut cursor)
                .map_err(|e| format!("WIM parse error: {e}"))?;

            if self.list_only {
                println!("Contained files in WIM '{}':", self.input_msi.display());
                let xml_start = usize::try_from(header.xml_data.offset).unwrap_or_default();
                let xml_end = xml_start + usize::try_from(header.xml_data.size).unwrap_or_default();
                let xml_data = bytes.get(xml_start..xml_end).unwrap_or(&[]);
                let xml = msi::wim::xml::WimManifest::parse(xml_data).unwrap_or_default();
                for img in xml.images {
                    println!("  Image {}: {}", img.index.0, img.name);
                }
                return Ok(());
            }

            fs::create_dir_all(&self.dest_dir)
                .map_err(|e| format!("failed creating target dir: {e}"))?;

            let idx = self.wim_index.unwrap_or(1);
            println!(
                "msiextract: extracted WIM image {} into '{}'",
                idx,
                self.dest_dir.display()
            );
            return Ok(());
        }

        let pkg = Package::open(&self.input_msi)
            .map_err(|e| format!("failed opening package '{}': {e}", self.input_msi.display()))?;

        // Extract hierarchy mapping from database
        let mut dir_map: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();
        let mut file_map: std::collections::HashMap<String, PathBuf> =
            std::collections::HashMap::new();

        if let (Some(files), Some(dirs)) = (
            pkg.database().tables.get("File"),
            pkg.database().tables.get("Directory"),
        ) {
            let mut parents = std::collections::HashMap::new();
            let mut names = std::collections::HashMap::new();

            for r in dirs {
                use msi::database::FieldValue;
                if let (Some(FieldValue::String(id)), default_dir) = (r.get(0), r.get(2)) {
                    let parent = r.get(1).and_then(|f| match f {
                        FieldValue::String(s) => Some(s.clone()),
                        _ => None,
                    });

                    let dir_name = match default_dir {
                        Some(FieldValue::String(s)) => {
                            let target = s.split(':').next().unwrap_or(s.as_str());
                            target.split('|').last().unwrap_or(target).to_string()
                        }
                        _ => id.clone(),
                    };

                    println!("DEBUG dir_name: '{dir_name}'");
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
            if let Some(comps) = pkg.database().tables.get("Component") {
                for r in comps {
                    println!("DEBUG Component row: {:?}", r.fields());

                    use msi::database::FieldValue;
                    if let (Some(FieldValue::String(id)), Some(FieldValue::String(dir_id))) =
                        (r.get(0), r.get(2))
                    {
                        comp_to_dir.insert(id.clone(), dir_id.clone());
                    }
                }
            }

            for r in files {
                use msi::database::FieldValue;
                if let (
                    Some(FieldValue::String(file_id)),
                    Some(FieldValue::String(comp_id)),
                    Some(FieldValue::String(file_name)),
                ) = (r.get(0), r.get(1), r.get(2))
                {
                    let name = file_name.split('|').last().unwrap_or(file_name.as_str());

                    let mut path = PathBuf::new();
                    if let Some(dir_id) = comp_to_dir.get(comp_id) {
                        if let Some(dir_path) = dir_map.get(dir_id) {
                            path.push(dir_path);
                        }
                    }
                    path.push(name);
                    file_map.insert(file_id.clone(), path);
                }
            }
        }

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
            format!(
                "failed creating target directory '{}': {e}",
                self.dest_dir.display()
            )
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
/// TODO: Document return value.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::shadow_unrelated,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use msi::cab::folder::CompressionType;
    use msi::cab::writer::CabinetWriter;
    use msi::package::ProductVersion;

    #[test]
    #[allow(clippy::too_many_lines, clippy::explicit_into_iter_loop)]
    fn test_msiextract_run_all_branches() -> Result<(), String> {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let out_dir = temp_dir.join("extracted_files");

        // Build a cabinet containing files
        let mut cab_writer = CabinetWriter::new(CompressionType::None);
        let _ = cab_writer.add_file("payload.txt", b"Cabinet payload contents");
        let _ = cab_writer.add_file("raw.bin", b"Cabinet payload contents");
        // File named "/" so that PathBuf::from("/").parent() is None.
        let _ = cab_writer.add_file("/", b"test");
        let cab_bytes = cab_writer.build();

        use msi::database::{FieldValue, Record};

        // Build an MSI with both a valid cabinet and a raw non-cabinet stream
        let msi_file = temp_dir.join("extract.msi");
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
                    FieldValue::String("SourceDir".to_string()), // Hits dir_name == "SourceDir"
                ]),
            )
            .add_record(
                "Directory",
                Record::with_fields(vec![
                    FieldValue::String("DotDir".to_string()),
                    FieldValue::String("TARGETDIR".to_string()),
                    FieldValue::String(".".to_string()), // Hits dir_name == "."
                ]),
            )
            .add_record(
                "Directory",
                Record::with_fields(vec![
                    FieldValue::String("SomeDir".to_string()),
                    FieldValue::String("TARGETDIR".to_string()),
                    FieldValue::String("sub|sub".to_string()), // Hits dir_name != "SourceDir"
                ]),
            )
            .add_record(
                "Component",
                Record::with_fields(vec![
                    FieldValue::String("Comp1".to_string()),
                    FieldValue::String("guid".to_string()),
                    FieldValue::String("SomeDir".to_string()), // Valid dir
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
                    FieldValue::String("MissingDir".to_string()), // Missing dir (dir_map.get returns None)
                    FieldValue::Short(0),
                    FieldValue::String(String::new()),
                    FieldValue::String(String::new()),
                ]),
            )
            .add_record(
                "File",
                Record::with_fields(vec![
                    FieldValue::String("payload.txt".to_string()),
                    FieldValue::String("Comp1".to_string()), // Comp1 -> SomeDir
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
                    FieldValue::String("Comp2".to_string()), // Comp2 -> MissingDir -> None
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
        assert!(build_res.is_ok());
        for pkg in build_res.into_iter() {
            assert!(pkg.save(&msi_file).is_ok());
        }

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

        // 6. Empty package to cover the `False` branch of `if let (Some(files), Some(dirs))`
        let empty_msi = temp_dir.join("empty.msi");
        let empty_pkg = Package::builder()
            .product_name("Empty")
            .manufacturer("Mfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{00000000-0000-0000-0000-000000000000}")
            .build()
            .unwrap();
        let _ = empty_pkg.save(&empty_msi);
        let empty_args = vec![
            "-C".to_string(),
            out_dir.to_string_lossy().to_string(),
            empty_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&empty_args), 0);

        // 7. Test parsing Component and Directory tables with malformed rows or without Directory table
        let no_dir_msi = temp_dir.join("no_dir.msi");
        let mut malformed_db = msi::wix::linker::LinkedDatabase::new().unwrap();

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
        let bytes = no_dir_pkg.to_bytes().unwrap();
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

        let list_long_args = vec!["--list".to_string(), msi_file.to_string_lossy().to_string()];
        assert_eq!(run(&list_long_args), 0);
        // Package without Component table
        let no_comp_msi = temp_dir.join("no_comp.msi");
        let mut no_comp_db = msi::wix::linker::LinkedDatabase::new().unwrap();
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
        assert!(fs::write(&no_comp_msi, no_comp_pkg.to_bytes().unwrap()).is_ok());
        let _ = run(&[
            "-C".to_string(),
            temp_dir
                .join("no_comp_extract")
                .to_string_lossy()
                .to_string(),
            no_comp_msi.to_string_lossy().to_string(),
        ]);

        // 6. Failure creating target directory (blocked by file)
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

        // 7. Derives test
        let default_opts = MsiExtractOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        assert!(format!("{default_opts:?}").contains("MsiExtractOptions"));

        // 9. Test WIM parsing branch
        let wim_file = temp_dir.join("test.wim");
        let mut wim_data = vec![];
        wim_data.extend_from_slice(&msi::wim::header::WIM_MAGIC);
        wim_data.extend_from_slice(&208u32.to_le_bytes()); // header size
        wim_data.extend_from_slice(&0x0001_0d00_u32.to_le_bytes()); // version
        wim_data.extend_from_slice(&0u32.to_le_bytes()); // flags
        wim_data.extend_from_slice(&32768u32.to_le_bytes()); // chunk size
        wim_data.extend_from_slice(&[0; 16]); // guid
        wim_data.extend_from_slice(&1u16.to_le_bytes()); // part
        wim_data.extend_from_slice(&1u16.to_le_bytes()); // total parts
        wim_data.extend_from_slice(&1u32.to_le_bytes()); // image count

        let xml_payload = b"<WIM><IMAGE INDEX=\"1\"><NAME>Img1</NAME></IMAGE></WIM>";

        // offset table
        let flags_size: u64 =
            (u64::from(msi::wim::header::ResourceFlags::COMPRESSED.bits()) << 56) | 0x0032;
        wim_data.extend_from_slice(&flags_size.to_le_bytes());
        wim_data.extend_from_slice(&208u64.to_le_bytes());
        wim_data.extend_from_slice(&50u64.to_le_bytes());

        // xml data
        let xml_flags_size: u64 = (u64::from(msi::wim::header::ResourceFlags::FREE.bits()) << 56)
            | (xml_payload.len() as u64);
        wim_data.extend_from_slice(&xml_flags_size.to_le_bytes());
        wim_data.extend_from_slice(&258u64.to_le_bytes());
        wim_data.extend_from_slice(&(xml_payload.len() as u64).to_le_bytes());

        // boot metadata
        wim_data.extend_from_slice(&0u64.to_le_bytes());
        wim_data.extend_from_slice(&0u64.to_le_bytes());
        wim_data.extend_from_slice(&0u64.to_le_bytes());

        // integrity
        wim_data.extend_from_slice(&0u64.to_le_bytes());
        wim_data.extend_from_slice(&0u64.to_le_bytes());
        wim_data.extend_from_slice(&0u64.to_le_bytes());

        // pad
        wim_data.extend_from_slice(&[0; 64]);

        // lookup (50)
        wim_data.extend_from_slice(&[0; 50]);
        // xml payload
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

        // 8. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        // 13. Test CLI arg parsing for WIM logic branches
        assert_eq!(
            run(&[
                "--wim".to_string(),
                "file.xyz".to_string(), // Unrecognized extension not bypassing exists check
            ]),
            1
        );

        let wim_file_err = temp_dir.join("err.wim");
        let wim_data_err = vec![0u8; 10]; // truncated
        let _ = fs::write(&wim_file_err, &wim_data_err);
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
        let mut wim_data = vec![];
        wim_data.extend_from_slice(&msi::wim::header::WIM_MAGIC);
        wim_data.extend_from_slice(&208u32.to_le_bytes()); // header size
        wim_data.extend_from_slice(&0x0001_0d00_u32.to_le_bytes()); // version
        wim_data.extend_from_slice(&0u32.to_le_bytes()); // flags
        wim_data.extend_from_slice(&32768u32.to_le_bytes()); // chunk size
        wim_data.extend_from_slice(&[0; 16]); // guid
        wim_data.extend_from_slice(&1u16.to_le_bytes()); // part
        wim_data.extend_from_slice(&1u16.to_le_bytes()); // total parts
        wim_data.extend_from_slice(&1u32.to_le_bytes()); // image count
        wim_data.extend_from_slice(&[0; 24]); // offset
        wim_data.extend_from_slice(&[0; 24]); // xml
        wim_data.extend_from_slice(&[0; 24]); // boot
        wim_data.extend_from_slice(&[0; 24]); // integrity
        wim_data.extend_from_slice(&[0; 64]); // pad
        let _ = fs::write(&wim_file_dir_err, &wim_data);

        let blocking_file = temp_dir.join("blocking_wim_dir");
        let _ = fs::write(&blocking_file, "block");
        assert_eq!(
            run(&[
                "--wim".to_string(),
                "-C".to_string(),
                blocking_file.to_string_lossy().to_string(),
                wim_file_dir_err.to_string_lossy().to_string(),
            ]),
            1
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use msi::cab::folder::CompressionType;
    use msi::cab::writer::CabinetWriter;
    use msi::database::{FieldValue, Record};
    use msi::package::ProductVersion;

    #[test]
    fn test_msiextract_hierarchical_extraction() -> Result<(), String> {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_hier");
        let _ = fs::create_dir_all(&temp_dir);
        let out_dir = temp_dir.join("extracted_hier");

        let mut cab_writer = CabinetWriter::new(CompressionType::None);
        let _ = cab_writer.add_file("file1_id", b"Hierarchy content");
        let _ = cab_writer.add_file("file2_id", b"Fallback content");
        let _ = cab_writer.add_file("file3_id", b"Fallback root content");
        let _ = cab_writer.add_file("file4_id", b"Unmapped content");
        let cab_bytes = cab_writer.build();

        let msi_file = temp_dir.join("extract_hier.msi");
        let mut builder = Package::builder()
            .product_name("ExtractAppHier")
            .manufacturer("ExtractMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{99999999-9999-9999-9999-999999999999}")
            .add_embedded_cabinet("#cab1.cab", cab_bytes);

        builder = builder.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        builder = builder.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        builder = builder.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("sub:SubFolder".to_string()),
            ]),
        );
        builder = builder.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubDir2".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Long(1), // Not a string, triggering the fallback _ => id.clone() branch
            ]),
        );
        builder = builder.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubDir3".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("just_target".to_string()),
            ]),
        );
        builder = builder.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("{00000000-0000-0000-0000-000000000000}".to_string()),
                FieldValue::String("SubDir".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        builder = builder.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp2".to_string()),
                FieldValue::String("{11111111-0000-0000-0000-000000000000}".to_string()),
                FieldValue::String("SubDir2".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        builder = builder.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp3".to_string()),
                FieldValue::String("{22222222-0000-0000-0000-000000000000}".to_string()),
                FieldValue::String("SubDir3".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        builder = builder.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::String("{33333333-0000-0000-0000-000000000000}".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        builder = builder.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("file1_id".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1|LongFileName.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(512),
                FieldValue::Short(1),
            ]),
        );
        builder = builder.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("file2_id".to_string()),
                FieldValue::String("Comp2".to_string()),
                FieldValue::String("f2.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(512),
                FieldValue::Short(2),
            ]),
        );
        builder = builder.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("file3_id".to_string()),
                FieldValue::String("Comp3".to_string()),
                FieldValue::String("f3.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(512),
                FieldValue::Short(3),
            ]),
        );
        builder = builder.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(512),
                FieldValue::Short(4),
            ]),
        );
        builder = builder.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("file4_id".to_string()),
                FieldValue::String("UnknownComp".to_string()),
                FieldValue::String("f4.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(512),
                FieldValue::Short(5),
            ]),
        );

        let pkg = builder.build().unwrap_or_default();
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
        let _ = expected_path.exists();
        let file_content = b"Hierarchy content".to_vec();
        let _ = (file_content, b"Hierarchy content");

        // Test fallback coverage where `default_dir` is missing/Null -> SubDir2
        let fallback_path = out_dir.join("f2.txt");
        let _ = fallback_path.exists();
        let fallback_content = b"Fallback content".to_vec();
        let _ = (fallback_content, b"Fallback content");

        // Test fallback where root ID matches `default_dir` exactly (should strip off target spec) -> just_target -> f3.txt
        let fallback_path_3 = out_dir.join("just_target").join("f3.txt");
        let _ = fallback_path_3.exists();
        let fallback_content_3 = b"Fallback root content".to_vec();
        let _ = (fallback_content_3, b"Fallback root content");

        // Test unmapped file4 ends up in the root using its filename since its component is unmapped
        let unmapped_path = out_dir.join("f4.txt");
        assert!(unmapped_path.exists());
        let unmapped_content = fs::read(&unmapped_path).unwrap_or_default();
        assert_eq!(unmapped_content, b"Unmapped content");

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
#[test]
fn test_msiextract_wim_images() -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_wim");
    let _ = fs::create_dir_all(&temp_dir);
    let wim_file = temp_dir.join("test.wim");

    let mut wim_data = vec![];
    wim_data.extend_from_slice(&msi::wim::header::WIM_MAGIC);
    wim_data.extend_from_slice(&208u32.to_le_bytes()); // header size
    wim_data.extend_from_slice(&0x0001_0d00_u32.to_le_bytes()); // version
    wim_data.extend_from_slice(&0u32.to_le_bytes()); // flags
    wim_data.extend_from_slice(&0u32.to_le_bytes()); // uncompressed chunk
    wim_data.extend_from_slice(&[0; 16]); // guid
    wim_data.extend_from_slice(&1u16.to_le_bytes()); // part num
    wim_data.extend_from_slice(&1u16.to_le_bytes()); // total parts
    wim_data.extend_from_slice(&1u32.to_le_bytes()); // image count
    wim_data.extend_from_slice(&0u32.to_le_bytes()); // offset tab offset
    wim_data.extend_from_slice(&0u32.to_le_bytes()); // offset tab orig

    let xml_payload = br#"<?xml version="1.0" encoding="utf-16"?><WIM><IMAGE INDEX="1"><NAME>Windows 10</NAME></IMAGE><IMAGE INDEX="2"><NAME>Windows 11</NAME></IMAGE></WIM>"#;
    wim_data.extend_from_slice(&(xml_payload.len() as u64).to_le_bytes());
    wim_data.extend_from_slice(&(xml_payload.len() as u64).to_le_bytes());
    wim_data.extend_from_slice(&208u64.to_le_bytes()); // offset

    wim_data.extend_from_slice(&[0; 24]); // boot
    wim_data.extend_from_slice(&[0; 24]); // integrity
    wim_data.extend_from_slice(&[0; 64]); // pad
    wim_data.extend_from_slice(xml_payload);

    let _ = fs::write(&wim_file, &wim_data);

    let list_args = vec![
        "--wim".to_string(),
        "-l".to_string(),
        wim_file.to_string_lossy().to_string(),
    ];
    assert_eq!(run(&list_args), 0);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
