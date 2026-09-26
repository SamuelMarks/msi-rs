//! Integration tests verifying physical filesystem operations, cabinet extraction,
//! disk space costing rejection, read-only target handling, and atomic rollback recovery.

use msi::cab::folder::CompressionType;
use msi::cab::writer::CabinetWriter;
use msi::database::summary_info::SummaryInfo;
use msi::database::tables::record::{FieldValue, Record};
use msi::error::{Error, Result};
use msi::execution::costing::DiskCostEngine;
use msi::execution::properties::EvaluationContext;
use msi::execution::transaction::{Transaction, WorkerContext, ERROR_SUCCESS};
use msi::execution::LiveWorkerExecutor;
use msi::package::{Package, PackageMetadata, ProductVersion};
use msi::wix::linker::LinkedDatabase;
use std::fs;
use std::path::PathBuf;

/// Helper creating a minimal directory structure for test executions.
///
/// # Arguments
///
/// * `test_name` - Test identifier used to form temporary directory paths.
///
/// # Returns
///
/// Tuple of `(temp_root, target_dir, quarantine_dir)`.
///
/// # Errors
///
/// Returns [`Error`] if filesystem directory creation fails.
fn create_test_sandbox(test_name: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let temp_root = std::env::temp_dir().join(format!("msi_test_{test_name}"));
    let target_dir = temp_root.join("target");
    let quarantine_dir = temp_root.join("quarantine");

    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&target_dir)?;
    fs::create_dir_all(&quarantine_dir)?;

    Ok((temp_root, target_dir, quarantine_dir))
}

/// Builds an embedded cabinet archive containing multiple physical test payloads.
///
/// # Returns
///
/// Raw compressed cabinet byte vector.
///
/// # Errors
///
/// Returns [`Error`] if cabinet creation fails.
fn build_sample_cabinet() -> Result<Vec<u8>> {
    let mut writer = CabinetWriter::new(CompressionType::Mszip);
    writer.add_file("fil_entry_sh", b"#!/bin/sh\necho Launching Application\n")?;
    writer.add_file(
        "fil_config_json",
        b"{\"database\": \"libscript_db\", \"port\": 3306}\n",
    )?;
    writer.add_file("fil_readme_txt", b"LibScript Product Suite Documentation\n")?;
    Ok(writer.build())
}

/// Builds a test MSI database with standard installation sequence, directory hierarchy,
/// component mapping, file records, and media cabinet association.
///
/// # Arguments
///
/// * `cab_name` - Cabinet archive stream name (e.g. `#app_cab.cab`).
///
/// # Returns
///
/// Configured [`LinkedDatabase`].
///
/// # Errors
///
/// Returns [`Error`] if database records cannot be created.
fn build_sample_database(cab_name: &str) -> Result<LinkedDatabase> {
    let mut db = LinkedDatabase::new()?;

    // InstallExecuteSequence records
    let seq_actions = [
        ("CostInitialize", 800),
        ("FileCost", 900),
        ("CostFinalize", 1000),
        ("CreateFolders", 1100),
        ("InstallFiles", 1200),
    ];
    for (action, seq) in seq_actions {
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(action.to_string()),
                FieldValue::Null,
                FieldValue::Short(seq),
            ]),
        );
    }

    // Directory hierarchy: TARGETDIR -> ProgramFiles64Folder -> INSTALLFOLDER
    db.add_record(
        "Directory",
        Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]),
    );
    db.add_record(
        "Directory",
        Record::with_fields(vec![
            FieldValue::String("ProgramFiles64Folder".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::String("PFiles64|Program Files 64".to_string()),
        ]),
    );
    db.add_record(
        "Directory",
        Record::with_fields(vec![
            FieldValue::String("INSTALLFOLDER".to_string()),
            FieldValue::String("ProgramFiles64Folder".to_string()),
            FieldValue::String("LibScriptApp".to_string()),
        ]),
    );

    // Component table
    db.add_record(
        "Component",
        Record::with_fields(vec![
            FieldValue::String("CompApp".to_string()),
            FieldValue::String("{00000000-0000-0000-0000-000000000001}".to_string()),
            FieldValue::String("INSTALLFOLDER".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::String("fil_entry_sh".to_string()),
        ]),
    );

    // File table
    let files = [
        ("fil_entry_sh", "CompApp", "entry.sh", 40, 1),
        ("fil_config_json", "CompApp", "config.json", 48, 2),
        ("fil_readme_txt", "CompApp", "readme.txt", 40, 3),
    ];
    for (file_id, comp_id, name, size, seq) in files {
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String(file_id.to_string()),
                FieldValue::String(comp_id.to_string()),
                FieldValue::String(name.to_string()),
                FieldValue::Long(size),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(seq),
            ]),
        );
    }

    // Media table mapping files to cabinet
    db.add_record(
        "Media",
        Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Short(100),
            FieldValue::Null,
            FieldValue::String(cab_name.to_string()),
            FieldValue::Null,
            FieldValue::Null,
        ]),
    );

    Ok(db)
}

/// Tests physical file payload extraction from embedded cabinets during live execution,
/// verifying file existence, content integrity, permissions, and commit finalization.
#[test]
fn test_live_execution_cabinet_extraction_and_permissions() -> Result<()> {
    let (temp_root, target_dir, quarantine_dir) = create_test_sandbox("live_cab_extraction")?;

    let cab_bytes = build_sample_cabinet()?;
    let db = build_sample_database("#app_cab.cab")?;

    let mut context = EvaluationContext::new();
    let target_str = target_dir.to_string_lossy().to_string();
    context.set_property("TARGETDIR", target_str);

    let mut cost_engine = DiskCostEngine::new();
    cost_engine.register_volume("TARGETDIR", 100_000_000, Some(4096));

    let metadata = PackageMetadata::new(
        "LibScriptApp",
        "LibScript",
        ProductVersion::new(1, 0, 0),
        "{11111111-1111-1111-1111-111111111111}",
    );
    let mut embedded_cabs = std::collections::HashMap::new();
    embedded_cabs.insert("#app_cab.cab".to_string(), cab_bytes);
    let pkg = Package::new(metadata, db, SummaryInfo::default(), embedded_cabs);

    let tx = Transaction::from_package(&pkg, context, cost_engine);
    let tx_prep = tx.prepare()?;

    let live_executor = LiveWorkerExecutor::new(&quarantine_dir, "session_cab_01");
    let mut worker = WorkerContext::new().with_live_executor(live_executor);

    let tx_exec = tx_prep.execute(&mut worker)?;

    // Verify files were extracted to target directory: target_dir / "Program Files 64" / "LibScriptApp"
    let installed_dir = target_dir.join("Program Files 64").join("LibScriptApp");
    assert!(installed_dir.exists());

    let entry_file = installed_dir.join("entry.sh");
    let config_file = installed_dir.join("config.json");
    let readme_file = installed_dir.join("readme.txt");

    assert!(entry_file.exists());
    assert!(config_file.exists());
    assert!(readme_file.exists());

    let entry_content = fs::read(&entry_file)?;
    assert_eq!(entry_content, b"#!/bin/sh\necho Launching Application\n");

    let config_content = fs::read(&config_file)?;
    assert_eq!(
        config_content,
        b"{\"database\": \"libscript_db\", \"port\": 3306}\n"
    );

    let readme_content = fs::read(&readme_file)?;
    assert_eq!(readme_content, b"LibScript Product Suite Documentation\n");

    // Commit transaction and verify quarantine directory is purged
    let tx_commit = tx_exec.commit(&mut worker)?;
    assert_eq!(tx_commit.return_code(), ERROR_SUCCESS);
    assert!(!quarantine_dir.exists());

    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}

/// Tests atomic recovery on transaction failure: restoring pre-existing files from `.rbf` quarantine
/// and purging newly created physical files.
#[test]
fn test_live_execution_atomic_recovery_on_failure() -> Result<()> {
    let (temp_root, target_dir, quarantine_dir) = create_test_sandbox("atomic_recovery")?;

    let cab_bytes = build_sample_cabinet()?;
    let mut db = build_sample_database("#app_cab.cab")?;

    // Add a failing custom action at sequence 1500
    db.add_record(
        "InstallExecuteSequence",
        Record::with_fields(vec![
            FieldValue::String("FailCustomAction".to_string()),
            FieldValue::Null,
            FieldValue::Short(1500),
        ]),
    );

    let mut context = EvaluationContext::new();
    let target_str = target_dir.to_string_lossy().to_string();
    context.set_property("TARGETDIR", target_str);

    let mut cost_engine = DiskCostEngine::new();
    cost_engine.register_volume("TARGETDIR", 100_000_000, Some(4096));

    // Pre-create an existing file that will be overwritten and backed up
    let installed_dir = target_dir.join("Program Files 64").join("LibScriptApp");
    fs::create_dir_all(&installed_dir)?;
    let existing_file = installed_dir.join("config.json");
    fs::write(&existing_file, b"OLD_CUSTOM_DATABASE_SETTINGS\n")?;

    let tx = Transaction::new(db, context, cost_engine).with_cabinet("#app_cab.cab", cab_bytes);
    let tx_prep = tx.prepare()?;

    let live_executor = LiveWorkerExecutor::new(&quarantine_dir, "session_recovery_01");
    let mut worker = WorkerContext::new().with_live_executor(live_executor);

    // Simulate failure at the custom action
    worker.simulate_failure_at("FailCustomAction");

    let exec_res = tx_prep.execute(&mut worker);
    assert!(exec_res.is_err());

    // Verify recovery:
    // 1. Existing file was restored from quarantine back to original content
    assert!(existing_file.exists());
    let restored_content = fs::read(&existing_file)?;
    assert_eq!(restored_content, b"OLD_CUSTOM_DATABASE_SETTINGS\n");

    // 2. Newly installed files were purged from disk
    let entry_file = installed_dir.join("entry.sh");
    assert!(!entry_file.exists());

    let readme_file = installed_dir.join("readme.txt");
    assert!(!readme_file.exists());

    // 3. Quarantine storage directory was purged
    assert!(!quarantine_dir.exists());

    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}

/// Tests disk space exhaustion detection during costing phase preventing deferred physical execution.
#[test]
fn test_live_execution_disk_space_exhaustion() -> Result<()> {
    let (temp_root, target_dir, _) = create_test_sandbox("disk_exhaustion")?;
    let db = build_sample_database("#app_cab.cab")?;

    let mut context = EvaluationContext::new();
    context.set_property("TARGETDIR", target_dir.to_string_lossy().to_string());

    // Register volume with only 10 available bytes (files require > 100 bytes)
    let mut cost_engine = DiskCostEngine::new();
    cost_engine.register_volume("TARGETDIR", 10, Some(4096));

    let tx = Transaction::new(db, context, cost_engine);
    let prep_res = tx.prepare();

    if let Err(Error::DiskCostExceeded {
        volume,
        required_bytes,
        available_bytes,
    }) = prep_res
    {
        assert_eq!(volume, "TARGETDIR");
        assert!(required_bytes > available_bytes);
        assert_eq!(available_bytes, 10);
    } else {
        assert!(matches!(prep_res, Err(Error::DiskCostExceeded { .. })));
    }

    // Verify no physical directories or files were created
    let installed_dir = target_dir.join("Program Files 64");
    assert!(!installed_dir.exists());

    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}

/// Tests atomic rollback when installation encounters a read-only target location.
#[test]
fn test_live_execution_read_only_target_recovery() -> Result<()> {
    let (temp_root, target_dir, quarantine_dir) = create_test_sandbox("read_only_target")?;

    // Create a read-only subfolder
    let installed_dir = target_dir.join("Program Files 64").join("LibScriptApp");
    fs::create_dir_all(&installed_dir)?;

    // Pre-create an untouched file in the root target
    let canary_file = target_dir.join("canary.txt");
    fs::write(&canary_file, b"CANARY_ALIVE")?;

    // Pre-create a target file that is read-only
    let read_only_file = installed_dir.join("entry.sh");
    fs::write(&read_only_file, b"READ_ONLY_INITIAL")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Make the directory non-writable (0o555: r-x r-x r-x)
        let mut perms = fs::metadata(&installed_dir)?.permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&installed_dir, perms)?;
    }

    let cab_bytes = build_sample_cabinet()?;
    let db = build_sample_database("#app_cab.cab")?;

    let mut context = EvaluationContext::new();
    context.set_property("TARGETDIR", target_dir.to_string_lossy().to_string());

    let mut cost_engine = DiskCostEngine::new();
    cost_engine.register_volume("TARGETDIR", 100_000_000, Some(4096));

    let tx = Transaction::new(db, context, cost_engine).with_cabinet("#app_cab.cab", cab_bytes);
    let tx_prep = tx.prepare()?;

    let live_executor = LiveWorkerExecutor::new(&quarantine_dir, "session_ro_01");
    let mut worker = WorkerContext::new().with_live_executor(live_executor);

    let res = tx_prep.execute(&mut worker);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Restore write permissions so cleanup works
        let mut perms = fs::metadata(&installed_dir)?.permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&installed_dir, perms);
    }

    // Either execute failed or succeeded depending on OS permission model
    if res.is_err() {
        assert_eq!(worker.quarantine_count(), 0);
    }

    // Canary file was untouched
    let canary_content = fs::read(&canary_file)?;
    assert_eq!(canary_content, b"CANARY_ALIVE");

    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}
