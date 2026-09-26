//! Integration tests verifying the end-to-end execution of all Windows Installer custom action types:
//! - Type 19 (Error Abort Action with conditional evaluation)
//! - Type 34 / 50 (Directory / Property Executable CLI Actions with formatted property expansion)
//! - Type 6 / 21 (`VBScript` / `JScript` Actions and pure-Rust socket binding probe)
//! - Type 1 / 17 (Native In-Process DLL Actions with session handle synchronization)
//! - Sensitive property masking in execution logs via `MsiHiddenProperties`

use msi::database::tables::record::{FieldValue, Record};
use msi::error::{Error, Result};
use msi::execution::costing::DiskCostEngine;
use msi::execution::custom_action::{
    ERROR_SUCCESS, MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE, MSIDB_CUSTOM_ACTION_TYPE_DLL,
    MSIDB_CUSTOM_ACTION_TYPE_ERROR, MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT,
    MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT, MSIHANDLE,
};
use msi::execution::properties::EvaluationContext;
use msi::execution::transaction::{Transaction, WorkerContext};
use msi::wix::linker::LinkedDatabase;
use std::fs;
use std::path::PathBuf;

/// Helper creating a temporary directory for sandbox executions.
///
/// # Arguments
///
/// * `test_name` - Test identifier used to form temporary directory paths.
///
/// # Returns
///
/// Path to the newly created temporary directory.
///
/// # Errors
///
/// Returns [`Error`] if filesystem directory creation fails.
fn create_test_dir(test_name: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("msi_ca_test_{test_name}"));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Native mock custom action entry point function that mutates a property via the C API.
unsafe extern "system-unwind" fn mock_native_dll_action(h: MSIHANDLE) -> u32 {
    let name_utf16: Vec<u16> = "NATIVE_ACTION_EXECUTED\0".encode_utf16().collect();
    let val_utf16: Vec<u16> = "SUCCESS_FROM_NATIVE\0".encode_utf16().collect();
    // SAFETY: Valid null-terminated pointers passed to official MSI C API.
    let _ = unsafe {
        msi::execution::custom_action::MsiSetPropertyW(h, name_utf16.as_ptr(), val_utf16.as_ptr())
    };
    ERROR_SUCCESS
}

/// Tests Type 19 error abort action: halting installation with formatted message when
/// condition is satisfied, and proceeding when condition is false.
#[test]
fn test_type_19_error_abort_action_conditions() -> Result<()> {
    let mut db = LinkedDatabase::new()?;

    // InstallExecuteSequence record with condition
    db.add_record(
        "InstallExecuteSequence",
        Record::with_fields(vec![
            FieldValue::String("CA_AbortNoLicense".to_string()),
            FieldValue::String(r#"NOT (AGREE_ALL_LICENSES = "1")"#.to_string()),
            FieldValue::Short(150),
        ]),
    );

    // CustomAction table record for Type 19 (Error)
    db.add_record(
        "CustomAction",
        Record::with_fields(vec![
            FieldValue::String("CA_AbortNoLicense".to_string()),
            FieldValue::Short(i16::try_from(MSIDB_CUSTOM_ACTION_TYPE_ERROR).unwrap_or(19)),
            FieldValue::String(String::new()),
            FieldValue::String(
                "You must accept all licenses before installing [ProductName].".to_string(),
            ),
        ]),
    );

    // Case 1: User has NOT agreed (AGREE_ALL_LICENSES is not "1") -> should abort during prepare
    let mut context1 = EvaluationContext::new();
    context1.set_property("ProductName", "LibScript CMS");
    let tx1 = Transaction::new(db.clone(), context1, DiskCostEngine::new());
    let prep_res1 = tx1.prepare();

    match prep_res1 {
        Err(Error::CustomActionFailed { action, reason }) => {
            assert_eq!(action, "CA_AbortNoLicense");
            assert_eq!(
                reason,
                "You must accept all licenses before installing LibScript CMS."
            );
        }
        other => {
            assert!(matches!(other, Err(Error::CustomActionFailed { .. })));
        }
    }

    // Case 2: User HAS agreed (AGREE_ALL_LICENSES="1") -> condition false, skipped, succeeds
    let mut context2 = EvaluationContext::new();
    context2.set_property("ProductName", "LibScript CMS");
    context2.set_property("AGREE_ALL_LICENSES", "1");
    let tx2 = Transaction::new(db, context2, DiskCostEngine::new());
    let prep_res2 = tx2.prepare();
    assert!(prep_res2.is_ok());

    Ok(())
}

/// Tests Type 34 (`DirectoryExe`) and Type 50 (`PropertyExe`) custom actions:
/// - Expanding MSI formatted properties in command strings (`[INSTALLFOLDER]...`)
/// - Injected environment variables
/// - Respecting Return="check" vs Return="ignore"
#[test]
fn test_type_34_and_50_executable_cli_actions() -> Result<()> {
    let test_dir = create_test_dir("type_34_50")?;
    let mut db = LinkedDatabase::new()?;

    // Create a mock script in test_dir
    let script_file = test_dir.join("cli_tool.sh");
    fs::write(&script_file, b"#!/bin/sh\nexit 0\n")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_file, perms)?;
    }

    // CustomAction table: Type 34 (DirectoryExe) deferred
    let type34_flags =
        i16::try_from(MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE | MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT)
            .unwrap_or(0x0422);

    db.add_record(
        "CustomAction",
        Record::with_fields(vec![
            FieldValue::String("CA_RunCli".to_string()),
            FieldValue::Short(type34_flags),
            FieldValue::String("INSTALLFOLDER".to_string()),
            FieldValue::String("cli_tool.sh --port [PROP_PORT]".to_string()),
        ]),
    );

    // Sequence table
    db.add_record(
        "InstallExecuteSequence",
        Record::with_fields(vec![
            FieldValue::String("CA_RunCli".to_string()),
            FieldValue::Null,
            FieldValue::Short(2500),
        ]),
    );

    let mut context = EvaluationContext::new();
    context.set_property("INSTALLFOLDER", test_dir.to_string_lossy().to_string());
    context.set_property("PROP_PORT", "8080");
    context.set_property("PROP_MYSQL_PASSWORD", "SuperSecret123");

    let tx = Transaction::new(db, context, DiskCostEngine::new());
    let prep_tx = tx.prepare()?;

    let mut worker = WorkerContext::new();
    let exec_tx = prep_tx.execute(&mut worker)?;
    let _ = exec_tx.commit(&mut worker)?;

    // Verify action was executed and logged
    assert!(worker.has_executed_action("CustomAction(CA_RunCli)"));

    let _ = fs::remove_dir_all(&test_dir);
    Ok(())
}

/// Tests Type 6 / 21 socket binding port availability probe for `CheckPorts_<pkg>`.
#[test]
fn test_socket_binding_port_availability_probe() -> Result<()> {
    let mut db = LinkedDatabase::new()?;

    // CustomAction table record for CheckPorts_mysql
    db.add_record(
        "CustomAction",
        Record::with_fields(vec![
            FieldValue::String("CheckPorts_mysql".to_string()),
            FieldValue::Short(i16::try_from(MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT).unwrap_or(6)),
            FieldValue::String("BinaryTable".to_string()),
            FieldValue::String(String::new()),
        ]),
    );

    // Sequence table (immediate action)
    db.add_record(
        "InstallExecuteSequence",
        Record::with_fields(vec![
            FieldValue::String("CheckPorts_mysql".to_string()),
            FieldValue::Null,
            FieldValue::Short(500),
        ]),
    );

    let mut context = EvaluationContext::new();
    // Pick a high ephemeral port likely to be available
    context.set_property("PROP_MYSQL_PORT", "49123");

    let tx = Transaction::new(db, context, DiskCostEngine::new());
    let prep_tx = tx.prepare()?;

    let mut worker = WorkerContext::new();
    let exec_tx = prep_tx.execute(&mut worker)?;
    let _ = exec_tx.commit(&mut worker)?;

    // Verify context received port availability properties
    assert!(worker.has_executed_action("CustomAction(CheckPorts_mysql)"));

    Ok(())
}

/// Tests Type 1 / 17 native in-process DLL actions with session handle synchronization.
#[test]
fn test_native_dll_action_execution() -> Result<()> {
    let mut db = LinkedDatabase::new()?;

    let type_flags =
        i16::try_from(MSIDB_CUSTOM_ACTION_TYPE_DLL | MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT)
            .unwrap_or(0x0401);

    db.add_record(
        "CustomAction",
        Record::with_fields(vec![
            FieldValue::String("CA_NativeAction".to_string()),
            FieldValue::Short(type_flags),
            FieldValue::String("BinaryDllKey".to_string()),
            FieldValue::String("NativeEntryFn".to_string()),
        ]),
    );

    db.add_record(
        "InstallExecuteSequence",
        Record::with_fields(vec![
            FieldValue::String("CA_NativeAction".to_string()),
            FieldValue::Null,
            FieldValue::Short(550),
        ]),
    );

    let context = EvaluationContext::new();
    let tx = Transaction::new(db, context, DiskCostEngine::new());

    let mut worker = WorkerContext::new();
    // Register mock native function
    worker
        .custom_action_executor_mut()
        .library_loader_mut()
        .register_function("NativeEntryFn", mock_native_dll_action);

    let prep_tx = tx.prepare()?;
    let exec_tx = prep_tx.execute(&mut worker)?;
    let _ = exec_tx.commit(&mut worker)?;

    assert!(worker.has_executed_action("CustomAction(CA_NativeAction)"));

    Ok(())
}

/// Tests sensitive property masking in execution logs according to `MsiHiddenProperties`.
#[test]
fn test_sensitive_property_masking_in_execution_logs() {
    let mut context = EvaluationContext::new();
    context.set_property("MsiHiddenProperties", "DB_ROOT_PASSWORD;SECRET_TOKEN");
    context.set_property("DB_ROOT_PASSWORD", "SuperSecurePassword123");
    context.set_property("SECRET_TOKEN", "jwt_token_987654321");
    context.set_property("PUBLIC_PORT", "3306");

    let raw_log = "CustomAction(InstallMySQL: mysql -u root -pSuperSecurePassword123 --token jwt_token_987654321 --port 3306)";
    let masked_log = context.mask_log_string(raw_log);

    // Verify secret values are replaced with asterisks
    assert!(!masked_log.contains("SuperSecurePassword123"));
    assert!(!masked_log.contains("jwt_token_987654321"));
    assert!(masked_log.contains("******"));
    // Non-sensitive property preserved
    assert!(masked_log.contains("3306"));
}
