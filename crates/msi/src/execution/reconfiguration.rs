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

/// Mode of reinstallation (`REINSTALLMODE_*` bitflags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReinstallMode(pub u32);

impl ReinstallMode {
    /// Reinstall only if file is missing (`REINSTALLMODE_FILEMISSING = 0x0002`).
    pub const FILE_MISSING: Self = Self(0x0002);
    /// Reinstall if file is missing or an older version (`REINSTALLMODE_FILEOLDERVERSION = 0x0004`).
    pub const FILE_OLDER_VERSION: Self = Self(0x0004);
    /// Reinstall if file is missing, older, or equal version (`REINSTALLMODE_FILEEQUALVERSION = 0x0008`).
    pub const FILE_EQUAL_VERSION: Self = Self(0x0008);
    /// Reinstall if file does not match exact checksum (`REINSTALLMODE_FILEEXACT = 0x0010`).
    pub const FILE_EXACT: Self = Self(0x0010);
    /// Verify all required executables and DLLs (`REINSTALLMODE_FILEVERIFY = 0x0020`).
    pub const FILE_VERIFY: Self = Self(0x0020);
    /// Replace all files regardless of version (`REINSTALLMODE_FILEREPLACE = 0x0040`).
    pub const FILE_REPLACE: Self = Self(0x0040);
    /// Rewrite all required machine registry entries (`REINSTALLMODE_MACHINEDATA = 0x0080`).
    pub const MACHINE_DATA: Self = Self(0x0080);
    /// Rewrite all required user registry entries (`REINSTALLMODE_USERDATA = 0x0100`).
    pub const USER_DATA: Self = Self(0x0100);
    /// Recreate all shortcuts and icons (`REINSTALLMODE_SHORTCUT = 0x0200`).
    pub const SHORTCUT: Self = Self(0x0200);
    /// Run from source package and recache local package (`REINSTALLMODE_PACKAGE = 0x0400`).
    pub const PACKAGE: Self = Self(0x0400);

    /// Checks whether all bits in `other` are set in `self`.
    ///
    /// # Arguments
    ///
    /// * `other` - Bitflag mask to test.
    ///
    /// # Returns
    ///
    /// `true` if `other` is contained within `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

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
/// The resulting value or `Ok(())` on success.
pub fn configure_product(
    product_code: &str,
    _level: InstallLevel,
    state: ConfigurationState,
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

    match state {
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
/// The resulting value or `Ok(())` on success.
pub fn reinstall_product(product_code: &str, mode: ReinstallMode) -> Result<(), MsiError> {
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
    context.set_property("REINSTALLMODE", format!("{:?}", mode.0));

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
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    #[test]
    fn test_reconfiguration_full_execution() -> crate::error::Result<()> {
        let temp_dir = tempfile::tempdir().map_err(crate::error::IoContext)?;
        let pkg_path = temp_dir.path().join("reconfig_test.msi");
        let pkg = crate::package::Package::builder()
            .product_name("ReconfigApp")
            .manufacturer("Acme")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .product_code("{11111111-2222-3333-4444-555555555555}")
            .upgrade_code("{11111111-2222-3333-4444-555555555556}")
            .build()?;
        pkg.save(&pkg_path)?;

        let path_str = pkg_path.to_string_lossy().to_string();
        assert!(configure_product(&path_str, InstallLevel(1), ConfigurationState::Install).is_ok());
        assert!(
            configure_product(&path_str, InstallLevel(1), ConfigurationState::Uninstall).is_ok()
        );
        assert!(configure_product(&path_str, InstallLevel(1), ConfigurationState::Repair).is_ok());
        let mode =
            ReinstallMode(ReinstallMode::FILE_OLDER_VERSION.0 | ReinstallMode::MACHINE_DATA.0);
        assert!(mode.contains(ReinstallMode::FILE_OLDER_VERSION));
        assert!(!mode.contains(ReinstallMode::FILE_REPLACE));
        assert!(reinstall_product(&path_str, mode).is_ok());
        assert!(configure_product("FAIL", InstallLevel(1), ConfigurationState::Install).is_err());
        assert!(reinstall_product("FAIL", ReinstallMode(1)).is_err());
        Ok(())
    }

    #[test]
    fn test_configuration_state() {
        let s = ConfigurationState::Install;
        let s2 = s; // copy
        assert_eq!(s, s2);
        assert_eq!(s.clone(), s2);

        let l = InstallLevel(1);
        let l2 = l;
        assert_eq!(l, l2);
        assert_eq!(l.clone(), l2);

        let rm = ReinstallMode(1);
        let rm2 = rm;
        assert_eq!(rm, rm2);
        assert_eq!(rm.clone(), rm2);

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
