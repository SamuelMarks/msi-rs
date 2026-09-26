//! # `LibScript` Zero-.EXE Modular MSI & Side-by-Side Coexistence Integration Test Suite
//!
//! Validates the architectural roadmap specified in `TODO_PLAN1.md`:
//! - Multi-package transaction chaining using Windows Installer 4.5 `<EmbeddedChainer>`.
//! - Standalone component packages with `<ServiceInstall>` and `<ServiceControl>`.
//! - Standard service actions (`StopServices`, `DeleteServices`, `InstallServices`, `StartServices`) injection.
//! - Shared component reference counting (`SharedDllRefCount="yes"`) and non-destructive uninstallation.
//! - Direct in-process relational schema provisioning with administrative credentials and rollback protection.

use msi::database::tables::record::FieldValue;
use msi::error::Result;
use msi::execution::native_action::{
    SqlProvisionerAction, SqlProvisionerClient, SqlProvisionerConfig,
};
use msi::execution::properties::EvaluationContext;
use msi::execution::transaction::{
    InstallState, MultiPackageTransactionManager, TransactionState, WorkerContext, ERROR_SUCCESS,
};
use msi::package::Package;
use msi::platform::registry_store::RegistryStore;
use msi::wix::toolchain::{CandleOptions, LightOptions};
use std::fs;

/// Tests authoring, compiling, linking, and executing the master orchestrator package with `<EmbeddedChainer>`.
#[test]
#[allow(clippy::too_many_lines)]
fn test_openedx_master_orchestrator_chainer() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("libscript_orch_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let chainer_dll = temp_dir.join("libscript_chainer.dll");
    fs::write(&chainer_dll, b"MZ_MOCK_CHAINER_DLL_BYTES")?;

    let dummy_payload = temp_dir.join("openedx_core.cmd");
    fs::write(
        &dummy_payload,
        b"@echo off
echo OpenEdX Core
",
    )?;

    let wxs_file = temp_dir.join("OpenEdX_Master.wxs");
    let wxs_content = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Open edX Platform" Language="1033" Version="2.0.0" Manufacturer="LibScript" UpgradeCode="{{E0F45901-83B4-4B21-9B5A-01D38FE81000}}">
    <Package Description="Open edX Master Orchestrator" Manufacturer="LibScript" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#master.cab" EmbedCab="yes" />

    <Binary Id="Bin_Chainer" SourceFile="{}" />
    <EmbeddedChainer Id="OpenEdXChainer" BinaryKey="Bin_Chainer" CommandLine="/quiet" Condition="NOT Installed" />

    <Property Id="PROP_MYSQL_PORT" Value="3306" Secure="yes" />
    <Property Id="PROP_REDIS_PORT" Value="6379" Secure="yes" />
    <Property Id="INSTALL_MYSQL" Value="1" Secure="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles64">
        <Directory Id="OpenEdXFolder" Name="OpenEdX">
          <Component Id="Comp_Core" Guid="{{E0F45901-83B4-4B21-9B5A-01D38FE81099}}">
            <File Id="File_Core" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="Main" Title="Open edX" Level="1">
      <ComponentRef Id="Comp_Core" />
    </Feature>
  </Product>
</Wix>"##,
        chainer_dll.display(),
        dummy_payload.display(),
    );
    fs::write(&wxs_file, wxs_content)?;

    // 1. Compile with Candle
    let candle_opts = CandleOptions {
        sources: vec![wxs_file],
        output: Some(temp_dir.join("OpenEdX_Master.wixobj")),
        arch: Some("x64".to_string()),
        ..CandleOptions::new()
    };
    let wixobjs = candle_opts.execute()?;
    assert_eq!(wixobjs.len(), 1);

    // 2. Link with Light
    let out_msi = temp_dir.join("openedx-2.0.0.msi");
    let light_opts = LightOptions {
        inputs: wixobjs,
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..LightOptions::new()
    };
    let linked_path = light_opts.execute()?;
    assert_eq!(linked_path, out_msi);
    assert!(out_msi.exists());

    // 3. Inspect generated database tables
    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify MsiEmbeddedChainer table presence and fields
    let chainer_records = db.get_records("MsiEmbeddedChainer");
    assert_eq!(chainer_records.len(), 1);
    let ch_rec = &chainer_records[0];
    assert_eq!(
        ch_rec.get(0),
        Some(&FieldValue::String("OpenEdXChainer".to_string()))
    );
    assert_eq!(
        ch_rec.get(1),
        Some(&FieldValue::String("NOT Installed".to_string()))
    );
    assert_eq!(
        ch_rec.get(2),
        Some(&FieldValue::String("/quiet".to_string()))
    );
    assert_eq!(
        ch_rec.get(3),
        Some(&FieldValue::String("Bin_Chainer".to_string()))
    );
    assert_eq!(ch_rec.get(4), Some(&FieldValue::Long(1))); // Type 1: Binary DLL

    // 4. Simulate Transaction Chaining execution
    let mut tx_mgr =
        MultiPackageTransactionManager::begin_transaction("OpenEdX_Install_Transaction")?;
    assert_eq!(tx_mgr.state(), Some(TransactionState::Active));

    // Child package 1: MySQL
    tx_mgr.install_product_nested(
        "libscript-mysql.msi",
        r#"PROP_MYSQL_PORT=3306 ROOT_PASSWORD="root_pass""#,
    )?;
    // Child package 2: Redis
    tx_mgr.install_product_nested("libscript-redis.msi", "PROP_REDIS_PORT=6379")?;
    // Child package 3: Open edX Core
    tx_mgr.install_product_nested("openedx-core.msi", "")?;

    assert_eq!(tx_mgr.chained_packages().len(), 3);
    assert_eq!(
        tx_mgr.chained_packages()[0]
            .properties
            .get("PROP_MYSQL_PORT"),
        Some(&"3306".to_string())
    );

    // Commit transaction
    let commit_code = tx_mgr.end_transaction(true)?;
    assert_eq!(commit_code, 0);
    assert_eq!(tx_mgr.state(), Some(TransactionState::Committed));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests compiling standalone service components (`libscript-mysql.msi`, `libscript-redis.msi`)
/// and verifying service lifecycle standard action injection.
#[test]
#[allow(clippy::too_many_lines)]
fn test_standalone_component_service_lifecycle() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("libscript_svc_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let mysql_bin = temp_dir.join("mysqld.exe");
    fs::write(&mysql_bin, b"MZ_MYSQL_SERVER_BINARY")?;

    let wxs_file = temp_dir.join("libscript-mysql.wxs");
    let wxs_content = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="LibScript MySQL Service" Language="1033" Version="8.0.36" Manufacturer="LibScript" UpgradeCode="{{E0F45901-83B4-4B21-9B5A-01D38FE81001}}">
    <Package Description="MySQL Server and Windows Service" Manufacturer="LibScript" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#mysql.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles64">
        <Directory Id="LibScriptFolder" Name="LibScript">
          <Directory Id="MySQLFolder" Name="MySQL">
            <Component Id="MySQLServiceComponent" Guid="{{5B2783B0-9A1F-4348-9F93-87CE43C21001}}" SharedDllRefCount="yes">
              <File Id="mysqld_exe" Source="{}" KeyPath="yes" />
              <ServiceInstall
                Id="InstallMySQLService"
                Name="LibScript_MySQL"
                DisplayName="LibScript MySQL 8.0 Server"
                Type="ownProcess"
                Start="auto"
                ErrorControl="normal"
                Account="NT AUTHORITY\NetworkService"
                Description="Shared Relational Database Engine"
              />
              <ServiceControl
                Id="ControlMySQLService"
                Name="LibScript_MySQL"
                Start="install"
                Stop="both"
                Remove="uninstall"
                Wait="yes"
              />
            </Component>
          </Directory>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="MySQLFeature" Title="MySQL Server" Level="1">
      <ComponentRef Id="MySQLServiceComponent" />
    </Feature>
  </Product>
</Wix>"##,
        mysql_bin.display(),
    );
    fs::write(&wxs_file, wxs_content)?;

    // 1. Compile
    let candle_opts = CandleOptions {
        sources: vec![wxs_file],
        output: Some(temp_dir.join("libscript-mysql.wixobj")),
        arch: Some("x64".to_string()),
        ..CandleOptions::new()
    };
    let wixobjs = candle_opts.execute()?;

    // 2. Link
    let out_msi = temp_dir.join("libscript-mysql-8.0.36.msi");
    let light_opts = LightOptions {
        inputs: wixobjs,
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..LightOptions::new()
    };
    let linked = light_opts.execute()?;
    assert_eq!(linked, out_msi);

    // 3. Inspect database
    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify Component.Attributes has 0x0020 (SharedDllRefCount)
    let comp_records = db.get_records("Component");
    assert_eq!(comp_records.len(), 1);
    let comp_attrs = match comp_records[0].get(3) {
        Some(FieldValue::Short(v)) => i32::from(*v),
        Some(FieldValue::Long(v)) => *v,
        _ => 0,
    };
    assert_eq!(comp_attrs & 0x0020, 0x0020);

    // Verify ServiceInstall fields
    let svc_records = db.get_records("ServiceInstall");
    assert_eq!(svc_records.len(), 1);
    assert_eq!(
        svc_records[0].get(1),
        Some(&FieldValue::String("LibScript_MySQL".to_string()))
    );
    assert_eq!(
        svc_records[0].get(8),
        Some(&FieldValue::String(
            r"NT AUTHORITY\NetworkService".to_string()
        ))
    );

    // Verify ServiceControl fields
    let ctrl_records = db.get_records("ServiceControl");
    assert_eq!(ctrl_records.len(), 1);
    // Start="install" (0x0001), Stop="both" (0x0022), Remove="uninstall" (0x0080) -> 0x00A3 = 163
    assert_eq!(ctrl_records[0].get(2), Some(&FieldValue::Short(0x00A3)));
    assert_eq!(ctrl_records[0].get(4), Some(&FieldValue::Short(1))); // Wait="yes"

    // Verify automatic standard action injection in InstallExecuteSequence
    let ies_records = db.get_records("InstallExecuteSequence");
    let has_action = |name: &str| {
        ies_records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String(name.to_string())))
    };
    assert!(has_action("StopServices"));
    assert!(has_action("DeleteServices"));
    assert!(has_action("InstallServices"));
    assert!(has_action("StartServices"));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Simulates side-by-side coexistence of Open edX and `WordPress` sharing a single
/// MySQL service on port 3306 without destructive premature uninstallation.
///
/// Validates Windows Installer shared component reference counting (`SharedDllRefCount="yes"`):
/// - Tracking client `ProductCode` associations per `ComponentId` GUID in the local registry/state store.
/// - Incrementing component ref-count on install linking multiple products.
/// - Decrementing ref-count on uninstall; retaining files and services when ref-count > 0.
/// - Verifying database operational continuity for remaining client products.
/// - Stopping and removing shared services and files only when ref-count drops to 0.
#[test]
#[allow(clippy::too_many_lines)]
fn test_side_by_side_coexistence_and_sql_provisioning() -> Result<()> {
    let mut worker = WorkerContext::default();
    let mut reg_store = RegistryStore::new();

    let shared_mysql_bin = r"C:\Program Files\LibScript\MySQL\mysqld.exe";
    let mysql_service = "LibScript_MySQL";
    let mysql_comp_guid = "{5B2783B0-9A1F-4348-9F93-87CE43C21001}";
    let prod_openedx = "{E0F45901-83B4-4B21-9B5A-01D38FE81001}";
    let prod_wordpress = "{E0F45901-83B4-4B21-9B5A-01D38FE81002}";

    // -------------------------------------------------------------
    // Scenario 1: Install Open edX linking shared MySQL component
    // -------------------------------------------------------------
    worker.install_component(
        mysql_comp_guid,
        prod_openedx,
        shared_mysql_bin,
        b"mysqld_8_0_binary".to_vec(),
        true,
        Some(mysql_service),
    );
    let reg_count1 = reg_store.register_component_client(
        mysql_comp_guid,
        prod_openedx,
        Some(shared_mysql_bin),
    )?;
    worker.start_service(mysql_service);

    assert_eq!(reg_count1, 1);
    assert_eq!(worker.get_component_client_count(mysql_comp_guid), 1);
    assert_eq!(reg_store.get_component_client_count(mysql_comp_guid)?, 1);
    assert_eq!(worker.get_shared_dll_ref(shared_mysql_bin), 1);
    assert_eq!(reg_store.get_shared_dll_ref(shared_mysql_bin)?, 1);
    assert!(worker.is_service_running(mysql_service));
    assert!(!reg_store.is_component_shared(mysql_comp_guid)?);

    // Provision Open edX database schema in-process
    let mut edx_ctx = EvaluationContext::new();
    edx_ctx.set_property("PROP_MYSQL_PORT", "3306");
    edx_ctx.set_property("PROP_PROVISION_DB_NAME", "openedx");
    edx_ctx.set_property("PROP_PROVISION_USER", "openedx");
    edx_ctx.set_property("PROP_PROVISION_PASSWORD", "edx_secret_pass");
    edx_ctx.set_property("SQL_PROVISION_MOCK", "1");

    let edx_cfg = SqlProvisionerConfig::from_context(&edx_ctx);
    let edx_sql_client = SqlProvisionerClient::new(edx_cfg);
    let edx_res = edx_sql_client.execute(SqlProvisionerAction::Install)?;
    assert!(edx_res.success);
    assert!(edx_res.executed_statements[0].contains("CREATE DATABASE IF NOT EXISTS `openedx`"));

    // -------------------------------------------------------------
    // Scenario 1 (continued): Install WordPress side-by-side sharing MySQL
    // -------------------------------------------------------------
    worker.install_component(
        mysql_comp_guid,
        prod_wordpress,
        shared_mysql_bin,
        b"mysqld_8_0_binary".to_vec(),
        true,
        Some(mysql_service),
    );
    let reg_count2 = reg_store.register_component_client(
        mysql_comp_guid,
        prod_wordpress,
        Some(shared_mysql_bin),
    )?;

    assert_eq!(reg_count2, 2);
    assert_eq!(worker.get_component_client_count(mysql_comp_guid), 2);
    assert_eq!(reg_store.get_component_client_count(mysql_comp_guid)?, 2);
    assert!(reg_store.is_component_shared(mysql_comp_guid)?);
    assert_eq!(worker.get_shared_dll_ref(shared_mysql_bin), 2);
    assert_eq!(reg_store.get_shared_dll_ref(shared_mysql_bin)?, 2);

    let clients = reg_store.get_component_clients(mysql_comp_guid)?;
    assert_eq!(clients.len(), 2);
    assert!(clients.contains(&prod_openedx.to_string()));
    assert!(clients.contains(&prod_wordpress.to_string()));

    // Provision WordPress database schema on the same MySQL service
    let mut wp_ctx = EvaluationContext::new();
    wp_ctx.set_property("PROP_MYSQL_PORT", "3306");
    wp_ctx.set_property("PROP_PROVISION_DB_NAME", "wordpress");
    wp_ctx.set_property("PROP_PROVISION_USER", "wordpress");
    wp_ctx.set_property("PROP_PROVISION_PASSWORD", "wp_secret_pass");
    wp_ctx.set_property("PROP_PROVISION_COLLATION", "utf8mb4_unicode_520_ci");
    wp_ctx.set_property("SQL_PROVISION_MOCK", "1");

    let wp_cfg = SqlProvisionerConfig::from_context(&wp_ctx);
    let wp_sql_client = SqlProvisionerClient::new(wp_cfg.clone());
    let wp_res = wp_sql_client.execute(SqlProvisionerAction::Install)?;
    assert!(wp_res.success);
    assert!(wp_res.executed_statements[0].contains("CREATE DATABASE IF NOT EXISTS `wordpress`"));

    // -------------------------------------------------------------
    // Scenario 2: Independent Uninstall of Open edX
    // -------------------------------------------------------------
    // Open edX uninstalls without PURGE_DATA -> database must not be dropped!
    edx_ctx.set_property("PURGE_DATA", "0");
    let edx_uninst_cfg = SqlProvisionerConfig::from_context(&edx_ctx);
    let edx_uninst_client = SqlProvisionerClient::new(edx_uninst_cfg);
    let edx_uninst_res = edx_uninst_client.execute(SqlProvisionerAction::Uninstall)?;
    assert!(edx_uninst_res.executed_statements.is_empty()); // No destructive DROP statements

    // Component reference counting prevents deletion of MySQL binary and teardown of service
    let uninst_removed_a = worker.uninstall_component_guarded(mysql_comp_guid, prod_openedx);
    let reg_uninst_count = reg_store.unregister_component_client(
        mysql_comp_guid,
        prod_openedx,
        Some(shared_mysql_bin),
    )?;

    assert!(!uninst_removed_a); // Teardown suppressed because WordPress still holds reference!
    assert_eq!(reg_uninst_count, 1);
    assert_eq!(worker.get_component_client_count(mysql_comp_guid), 1);
    assert_eq!(reg_store.get_component_client_count(mysql_comp_guid)?, 1);
    assert!(!reg_store.is_component_shared(mysql_comp_guid)?);
    assert_eq!(worker.get_shared_dll_ref(shared_mysql_bin), 1);
    assert_eq!(reg_store.get_shared_dll_ref(shared_mysql_bin)?, 1);
    assert!(worker.get_file_content(shared_mysql_bin).is_some());
    assert!(worker.is_service_running(mysql_service));

    // Verify WordPress operational continuity: WordPress can still execute operations on MySQL
    let wp_continuity_client = SqlProvisionerClient::new(wp_cfg);
    let wp_ping = wp_continuity_client.execute(SqlProvisionerAction::Install)?;
    assert!(wp_ping.success);
    assert!(worker.is_service_running(mysql_service));

    // -------------------------------------------------------------
    // Scenario 3: Final Service Teardown on WordPress Uninstall
    // -------------------------------------------------------------
    // WordPress uninstalls with PURGE_DATA="1"
    wp_ctx.set_property("PURGE_DATA", "1");
    let wp_uninst_cfg = SqlProvisionerConfig::from_context(&wp_ctx);
    let wp_uninst_client = SqlProvisionerClient::new(wp_uninst_cfg);
    let wp_uninst_res = wp_uninst_client.execute(SqlProvisionerAction::Uninstall)?;
    assert_eq!(wp_uninst_res.executed_statements.len(), 3);
    assert!(wp_uninst_res.executed_statements[0].contains("DROP DATABASE IF EXISTS `wordpress`"));

    // Component ref count drops 1 -> 0
    let uninst_removed_b = worker.uninstall_component_guarded(mysql_comp_guid, prod_wordpress);
    let reg_final_count = reg_store.unregister_component_client(
        mysql_comp_guid,
        prod_wordpress,
        Some(shared_mysql_bin),
    )?;

    assert!(uninst_removed_b); // Now removed!
    assert_eq!(reg_final_count, 0);
    assert_eq!(worker.get_component_client_count(mysql_comp_guid), 0);
    assert_eq!(reg_store.get_component_client_count(mysql_comp_guid)?, 0);
    assert_eq!(worker.get_shared_dll_ref(shared_mysql_bin), 0);
    assert_eq!(reg_store.get_shared_dll_ref(shared_mysql_bin)?, 0);
    assert!(worker.get_file_content(shared_mysql_bin).is_none());

    // Service is now cleanly stopped and removed
    assert!(!worker.is_service_running(mysql_service));

    Ok(())
}

/// Comprehensive integration test for the Open edX Master Orchestrator deployment and rollback lifecycle.
///
/// Validates:
/// 1. Building child standalone packages (`libscript-mysql.msi`, `libscript-redis.msi`, `openedx-core.msi`).
/// 2. Embedding child packages into the master orchestrator package with `<EmbeddedChainer>`.
/// 3. Extracting embedded child packages into temporary spool storage.
/// 4. Forwarding public configuration properties (`PROP_MYSQL_PORT`, `PROP_MYSQL_ROOT_PASSWORD`, `INSTALL_MYSQL`).
/// 5. Querying product installation state with `query_product_state` / `MsiQueryProductState`.
/// 6. Evaluating launch conditions and skipping pre-existing components.
/// 7. Cascading rollback: failure in a child package unwinds newly installed packages in reverse order
///    while preserving pre-existing shared services.
#[test]
#[allow(clippy::too_many_lines)]
fn test_openedx_master_orchestrator_deployment_and_rollback_lifecycle() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("openedx_lifecycle_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    // Dummy payloads
    let mysql_bin = temp_dir.join("mysqld.exe");
    fs::write(&mysql_bin, b"MYSQL_SERVER_PAYLOAD")?;
    let redis_bin = temp_dir.join("redis-server.exe");
    fs::write(&redis_bin, b"REDIS_SERVER_PAYLOAD")?;
    let core_bin = temp_dir.join("openedx-lms.exe");
    fs::write(&core_bin, b"OPENEDX_LMS_PAYLOAD")?;
    let chainer_bin = temp_dir.join("chainer.dll");
    fs::write(&chainer_bin, b"CHAINER_DLL_PAYLOAD")?;

    // Helper closure to compile and link a WiX source to MSI
    let compile_and_link = |name: &str, content: &str| -> Result<std::path::PathBuf> {
        let wxs_path = temp_dir.join(format!("{name}.wxs"));
        fs::write(&wxs_path, content)?;
        let obj_path = temp_dir.join(format!("{name}.wixobj"));
        let candle_opts = CandleOptions {
            sources: vec![wxs_path],
            output: Some(obj_path),
            arch: Some("x64".to_string()),
            ..CandleOptions::new()
        };
        let objs = candle_opts.execute()?;
        let msi_path = temp_dir.join(format!("{name}.msi"));
        let light_opts = LightOptions {
            inputs: objs,
            output: Some(msi_path.clone()),
            suppress_ice: true,
            ..LightOptions::new()
        };
        light_opts.execute()?;
        Ok(msi_path)
    };

    // 1. Build child package: libscript-mysql.msi
    let mysql_wxs = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="MySQL Server" Language="1033" Version="8.0.36" Manufacturer="LibScript" UpgradeCode="{{E0F45901-83B4-4B21-9B5A-01D38FE81001}}">
    <Package Description="MySQL Service" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#mysql.cab" EmbedCab="yes" />
    <Property Id="PROP_MYSQL_PORT" Value="3306" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles64">
        <Directory Id="MySQLDir" Name="MySQL">
          <Component Id="MySQLComp" Guid="{{E0F45901-83B4-4B21-9B5A-01D38FE81011}}">
            <File Id="MySQLFile" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="MySQLFeat" Level="1">
      <ComponentRef Id="MySQLComp" />
    </Feature>
  </Product>
</Wix>"##,
        mysql_bin.display()
    );
    let mysql_msi = compile_and_link("libscript-mysql", &mysql_wxs)?;

    // 2. Build child package: libscript-redis.msi
    let redis_wxs = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Redis Server" Language="1033" Version="7.2.4" Manufacturer="LibScript" UpgradeCode="{{E0F45901-83B4-4B21-9B5A-01D38FE81002}}">
    <Package Description="Redis Service" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#redis.cab" EmbedCab="yes" />
    <Property Id="PROP_REDIS_PORT" Value="6379" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles64">
        <Directory Id="RedisDir" Name="Redis">
          <Component Id="RedisComp" Guid="{{E0F45901-83B4-4B21-9B5A-01D38FE81012}}">
            <File Id="RedisFile" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="RedisFeat" Level="1">
      <ComponentRef Id="RedisComp" />
    </Feature>
  </Product>
</Wix>"##,
        redis_bin.display()
    );
    let redis_msi = compile_and_link("libscript-redis", &redis_wxs)?;

    // 3. Build child package: openedx-core.msi (with condition on FAIL_INSTALL)
    let core_wxs = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Open edX Core" Language="1033" Version="2.0.0" Manufacturer="LibScript" UpgradeCode="{{E0F45901-83B4-4B21-9B5A-01D38FE81003}}">
    <Package Description="Open edX Core Application" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#core.cab" EmbedCab="yes" />
    <Condition Message="Simulated core installation failure">NOT (FAIL_INSTALL = "1")</Condition>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles64">
        <Directory Id="CoreDir" Name="OpenEdX">
          <Component Id="CoreComp" Guid="{{E0F45901-83B4-4B21-9B5A-01D38FE81013}}">
            <File Id="CoreFile" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="CoreFeat" Level="1">
      <ComponentRef Id="CoreComp" />
    </Feature>
  </Product>
</Wix>"##,
        core_bin.display()
    );
    let core_msi = compile_and_link("openedx-core", &core_wxs)?;

    // 4. Build master orchestrator package embedding child MSIs into Binary table
    let master_wxs = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Open edX Orchestrator" Language="1033" Version="2.0.0" Manufacturer="LibScript" UpgradeCode="{{E0F45901-83B4-4B21-9B5A-01D38FE81000}}">
    <Package Description="Open edX Master Suite" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#master.cab" EmbedCab="yes" />

    <Binary Id="Bin_Chainer" SourceFile="{}" />
    <Binary Id="libscript-mysql.msi" SourceFile="{}" />
    <Binary Id="libscript-redis.msi" SourceFile="{}" />
    <Binary Id="openedx-core.msi" SourceFile="{}" />

    <EmbeddedChainer Id="OpenEdXMasterChainer" BinaryKey="Bin_Chainer" CommandLine="/quiet" Condition="NOT Installed" />

    <Property Id="PROP_MYSQL_PORT" Value="3307" Secure="yes" />
    <Property Id="PROP_MYSQL_ROOT_PASSWORD" Value="SecretPassword123" Secure="yes" />
    <Property Id="INSTALL_MYSQL" Value="1" Secure="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder" Name="PFiles64">
        <Directory Id="LibScriptDir" Name="LibScript">
          <Component Id="MasterComp" Guid="{{E0F45901-83B4-4B21-9B5A-01D38FE81099}}">
            <File Id="MasterStub" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="MasterFeat" Level="1">
      <ComponentRef Id="MasterComp" />
    </Feature>
  </Product>
</Wix>"##,
        chainer_bin.display(),
        mysql_msi.display(),
        redis_msi.display(),
        core_msi.display(),
        chainer_bin.display(),
    );
    let master_msi = compile_and_link("openedx-master", &master_wxs)?;

    // 5. Open master package and verify embedded chainer records
    let master_pkg = Package::open(&master_msi)?;
    let chainers = MultiPackageTransactionManager::read_embedded_chainers(&master_pkg)?;
    assert_eq!(chainers.len(), 1);
    assert_eq!(chainers[0].chainer, "OpenEdXMasterChainer");
    assert_eq!(chainers[0].condition.as_deref(), Some("NOT Installed"));

    // 6. Extract child packages to temporary spool directory
    let spool_dir = temp_dir.join("spool");
    let mut tx_mgr = MultiPackageTransactionManager::begin_transaction("OpenEdX_Master_Session")?;
    let extracted = tx_mgr.extract_all_child_packages(&master_pkg, &spool_dir)?;
    assert_eq!(extracted.len(), 3);
    assert!(extracted.contains_key("libscript-mysql.msi"));
    assert!(extracted.contains_key("libscript-redis.msi"));
    assert!(extracted.contains_key("openedx-core.msi"));

    for (name, path) in &extracted {
        assert!(
            path.exists(),
            "spooled child package {name} should exist on disk"
        );
        assert!(fs::metadata(path)?.len() > 0);
    }

    // 7. Verify property forwarding
    let mut master_ctx = EvaluationContext::new();
    master_ctx.set_property("PROP_MYSQL_PORT", "3307");
    master_ctx.set_property("PROP_MYSQL_ROOT_PASSWORD", "SecretPassword123");
    master_ctx.set_property("INSTALL_MYSQL", "1");
    let forwarded_cmd = MultiPackageTransactionManager::forward_public_properties(&master_ctx);
    assert!(forwarded_cmd.contains("PROP_MYSQL_PORT=3307"));
    assert!(
        forwarded_cmd.contains("PROP_MYSQL_ROOT_PASSWORD=SecretPassword123")
            || forwarded_cmd.contains(r#"PROP_MYSQL_ROOT_PASSWORD="SecretPassword123""#)
    );
    assert!(forwarded_cmd.contains("INSTALL_MYSQL=1"));

    // 8. Scenario A: Successful Multi-Package Deployment Lifecycle
    let child_plan: Vec<(&str, Option<&str>)> = vec![
        ("libscript-mysql.msi", None),
        ("libscript-redis.msi", None),
        ("openedx-core.msi", None),
    ];
    let exit_code =
        tx_mgr.orchestrate_master_package(&master_pkg, &master_ctx, &spool_dir, &child_plan)?;
    assert_eq!(exit_code, ERROR_SUCCESS);
    assert_eq!(tx_mgr.state(), Some(TransactionState::Committed));
    assert_eq!(tx_mgr.chained_packages().len(), 3);
    assert!(!tx_mgr.chained_packages()[0].preexisting);
    assert!(!tx_mgr.chained_packages()[1].preexisting);
    assert!(!tx_mgr.chained_packages()[2].preexisting);

    // 9. Scenario B: Rollback Cascade Lifecycle
    // Pre-condition: Redis was already installed on the system beforehand
    let mut rb_tx_mgr =
        MultiPackageTransactionManager::begin_transaction("OpenEdX_Rollback_Session")?;
    rb_tx_mgr.register_product(
        "{E0F45901-83B4-4B21-9B5A-01D38FE81002}",
        InstallState::Default,
    );
    assert!(rb_tx_mgr.is_product_installed("{E0F45901-83B4-4B21-9B5A-01D38FE81002}"));

    // Run orchestration where openedx-core specifies FAIL_INSTALL=1
    let child_fail_plan: Vec<(&str, Option<&str>)> = vec![
        ("libscript-mysql.msi", None),
        ("libscript-redis.msi", None),
        ("openedx-core.msi", Some("FAIL_INSTALL=1")),
    ];
    let rb_result = rb_tx_mgr.orchestrate_master_package(
        &master_pkg,
        &master_ctx,
        &spool_dir,
        &child_fail_plan,
    );
    assert!(
        rb_result.is_err(),
        "orchestration must fail when core package launch condition fails"
    );
    assert_eq!(rb_tx_mgr.state(), Some(TransactionState::RolledBack));

    // Verify cascading rollback:
    // - MySQL was newly installed -> rolled back
    // - Redis was pre-existing -> preserved (skip rollback)
    let executed_actions = rb_tx_mgr.worker().executed_actions();
    assert!(executed_actions.contains(&"InstallProduct:MySQL Server".to_string()));
    assert!(executed_actions
        .iter()
        .any(|a| a.starts_with("SkipChildPackagePreexisting:Redis Server")));
    assert!(executed_actions.contains(&"RollbackPackage:MySQL Server".to_string()));
    assert!(executed_actions.contains(&"SkipRollbackPreexisting:Redis Server".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
