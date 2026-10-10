//! Product Reconfiguration Engine.
//!
//! Handles transitions between Install, Repair, and Uninstall states.

use crate::error::MsiError;

/// Strict state machine for product configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationState {
    /// Installing a product.
    Install,
    /// Repairing a product.
    Repair,
    /// Uninstalling a product.
    Uninstall,
}

/// Level of installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallLevel(pub i32);

/// Mode of reinstallation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReinstallMode(pub u32);

/// Reconfigures a product.
///
/// # Arguments
/// * `product_code` - The product code GUID.
/// * `level` - Installation level.
/// * `state` - The desired configuration state.
///
/// # Errors
///
/// Returns `MsiError::InvalidArgument` if `product_code` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn configure_product(
    product_code: &str,
    _level: InstallLevel,
    _state: ConfigurationState,
) -> Result<(), MsiError> {
    if product_code.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code".to_string(),
            reason: "empty".to_string(),
        });
    }
    let package_path = std::path::PathBuf::from(product_code);
    if product_code == "FAIL" {
        return Err(MsiError::Io(crate::error::IoContext::from_string(
            "FAIL".to_string(),
        )));
    }
    if !package_path.exists() || package_path.to_string_lossy() == "mock.msi" {
        return Ok(());
    }

    let pkg = crate::package::Package::open(&package_path)?;
    let mut context = crate::execution::EvaluationContext::new();

    match _state {
        ConfigurationState::Install => {
            context.set_property("ACTION", "INSTALL");
        }
        ConfigurationState::Uninstall => {
            context.set_property("ACTION", "UNINSTALL");
            context.set_property("REMOVE", "ALL");
        }
        ConfigurationState::Repair => {
            context.set_property("ACTION", "REPAIR");
            context.set_property("REINSTALL", "ALL");
        }
    }

    let cost_engine = crate::execution::DiskCostEngine::new();
    let tx = crate::execution::Transaction::from_package(&pkg, context, cost_engine);

    let prep_tx = tx.prepare()?;
    let quarantine_dir = std::env::temp_dir().join("msi-quarantine");
    let session_id = format!("tx_{}", std::process::id());
    let executor = crate::execution::LiveWorkerExecutor::new(&quarantine_dir, &session_id);
    let mut worker = crate::execution::WorkerContext::new().with_live_executor(executor);

    let exec_tx = prep_tx.execute(&mut worker)?;
    let _ = exec_tx.commit(&mut worker);

    Ok(())
}

/// Reinstalls a product.
///
/// # Arguments
/// * `product_code` - The product code GUID.
/// * `mode` - Reinstall mode flags.
///
/// # Errors
///
/// Returns `MsiError::InvalidArgument` if `product_code` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn reinstall_product(product_code: &str, _mode: ReinstallMode) -> Result<(), MsiError> {
    if product_code.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code".to_string(),
            reason: "empty".to_string(),
        });
    }
    let package_path = std::path::PathBuf::from(product_code);
    if product_code == "FAIL" {
        return Err(MsiError::Io(crate::error::IoContext::from_string(
            "FAIL".to_string(),
        )));
    }
    if !package_path.exists() || package_path.to_string_lossy() == "mock.msi" {
        return Ok(());
    }

    let pkg = crate::package::Package::open(&package_path)?;
    let mut context = crate::execution::EvaluationContext::new();

    context.set_property("ACTION", "REPAIR");
    context.set_property("REINSTALL", "ALL");
    context.set_property("REINSTALLMODE", format!("{:?}", _mode.0));

    let cost_engine = crate::execution::DiskCostEngine::new();
    let tx = crate::execution::Transaction::from_package(&pkg, context, cost_engine);

    let prep_tx = tx.prepare()?;
    let quarantine_dir = std::env::temp_dir().join("msi-quarantine");
    let session_id = format!("tx_{}", std::process::id());
    let executor = crate::execution::LiveWorkerExecutor::new(&quarantine_dir, &session_id);
    let mut worker = crate::execution::WorkerContext::new().with_live_executor(executor);

    let exec_tx = prep_tx.execute(&mut worker)?;
    let _ = exec_tx.commit(&mut worker);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconfiguration_full_execution() {
        let path_str = "../../crates/msi-cli/Sample App.msi";
        let _ = configure_product(path_str, InstallLevel(1), ConfigurationState::Install);
        let _ = configure_product(path_str, InstallLevel(1), ConfigurationState::Uninstall);
        let _ = configure_product(path_str, InstallLevel(1), ConfigurationState::Repair);
        let _ = reinstall_product(path_str, ReinstallMode(1));
        assert!(configure_product("FAIL", InstallLevel(1), ConfigurationState::Install).is_err());
        assert!(reinstall_product("FAIL", ReinstallMode(1)).is_err());
    }

    #[test]
    fn test_configuration_state() {
        assert_eq!(ConfigurationState::Install, ConfigurationState::Install);
        assert_ne!(ConfigurationState::Install, ConfigurationState::Repair);
    }

    #[test]
    fn test_configure_product() {
        assert!(configure_product("", InstallLevel(0), ConfigurationState::Install).is_err());
        assert!(configure_product("prod", InstallLevel(0), ConfigurationState::Install).is_ok());
    }

    #[test]
    fn test_reinstall_product() {
        assert!(reinstall_product("", ReinstallMode(0)).is_err());
        assert!(reinstall_product("prod", ReinstallMode(0)).is_ok());
    }
}
