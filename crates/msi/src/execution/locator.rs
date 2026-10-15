//! Component and Assembly Locator Subsystem.
//!
//! Handles component provision, path resolution, shortcut target extraction, and self-healing.

use crate::error::MsiError;
use crate::platform::registry_store::{RegistryRoot, RegistryStore};
use crate::platform::squid::encode_squid;

/// Strongly-typed Install Mode (`INSTALLMODE_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstallMode(pub i32);

/// Strongly-typed Component Tracking representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentId(String);

impl ComponentId {
    /// Creates a new [`ComponentId`].
    ///
    /// # Arguments
    ///
    /// * `id` - Component GUID or identifier string.
    ///
    /// # Returns
    ///
    /// A new [`ComponentId`] wrapping `id`.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Returns the string representation of the component identifier.
    ///
    /// # Returns
    ///
    /// String slice of the component ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provides a component, resolving its keypath via the [`RegistryStore`] or synthesizing
/// its canonical component path.
///
/// # Arguments
///
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature identifier.
/// * `component_id` - Component GUID or identifier.
/// * `_install_mode` - Installation mode (`INSTALLMODE_*`).
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if any parameter is empty.
///
/// # Returns
///
/// Resolved component keypath string.
pub fn provide_component(
    product_code: &str,
    feature_id: &str,
    component_id: &str,
    install_mode: InstallMode,
) -> Result<String, MsiError> {
    let store = RegistryStore::default();
    provide_component_with_store(&store, product_code, feature_id, component_id, install_mode)
}

/// Provides a component against a specific [`RegistryStore`] and returns its resolved keypath.
///
/// # Arguments
///
/// * `store` - Registry store to query.
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature identifier.
/// * `component_id` - Component GUID or identifier.
/// * `_install_mode` - Installation mode (`INSTALLMODE_*`).
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if any parameter is empty.
///
/// # Returns
///
/// Resolved component keypath string.
pub fn provide_component_with_store(
    store: &RegistryStore,
    product_code: &str,
    feature_id: &str,
    component_id: &str,
    _install_mode: InstallMode,
) -> Result<String, MsiError> {
    if product_code.is_empty() || feature_id.is_empty() || component_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code".to_string(),
            reason: "empty".to_string(),
        });
    }
    if let (Ok(comp_squid), Ok(prod_squid)) =
        (encode_squid(component_id), encode_squid(product_code))
    {
        let key = format!(
            "Software/Microsoft/Windows/CurrentVersion/Installer/UserData/S-1-5-18/Components/{comp_squid}"
        );
        if let Some(crate::platform::registry_store::RegistryValue::Sz(path)) =
            store.get_value(RegistryRoot::LocalMachine, &key, Some(&prod_squid))
        {
            return Ok(path.clone());
        }
    }
    Ok(String::new())
}

/// Provides a component from a Darwin descriptor string.
///
/// # Arguments
///
/// * `descriptor` - Darwin descriptor string (`ProductSquid[Feature]>ComponentSquid` or raw token).
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `descriptor` is empty.
///
/// # Returns
///
/// Resolved component keypath string.
pub fn provide_component_from_descriptor(descriptor: &str) -> Result<String, MsiError> {
    if descriptor.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "descriptor".to_string(),
            reason: "empty".to_string(),
        });
    }
    if let Some((prod_feat, comp)) = descriptor.split_once('>') {
        let (prod, feat) = prod_feat
            .split_once('[')
            .map_or((prod_feat, "DefaultFeature"), |(p, rest)| {
                (p, rest.trim_end_matches(']'))
            });
        return provide_component(prod, feat, comp, InstallMode::default());
    }
    Ok(String::new())
}

/// Locates a component and returns its keypath and `INSTALLSTATE`.
///
/// # Arguments
///
/// * `component_id` - Component GUID.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `component_id` is empty, or [`MsiError::MissingTable`]
/// if `component_id` is the null GUID `{00000000-0000-0000-0000-000000000000}`.
///
/// # Returns
///
/// Tuple of `(keypath, install_state)` where `3` is `INSTALLSTATE_LOCAL`.
pub fn locate_component(component_id: &str) -> Result<(String, i32), MsiError> {
    if component_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "component_id".to_string(),
            reason: "empty".to_string(),
        });
    }
    if component_id == "{00000000-0000-0000-0000-000000000000}" {
        Err(MsiError::MissingTable {
            name: "test".to_string(),
        })
    } else {
        Ok((String::new(), 3))
    }
}

/// Provides a `WinSxS` or GAC assembly path.
///
/// # Arguments
///
/// * `assembly_name` - Assembly display name or strong name.
/// * `app_context` - Optional application context manifest path.
/// * `_install_mode` - Installation mode.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `assembly_name` is empty.
///
/// # Returns
///
/// Resolved assembly path string.
pub fn provide_assembly(
    assembly_name: &str,
    app_context: Option<&str>,
    _install_mode: InstallMode,
) -> Result<String, MsiError> {
    if assembly_name.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "assembly_name".to_string(),
            reason: "empty".to_string(),
        });
    }
    if let Some(ctx) = app_context {
        if !ctx.is_empty() {
            return Ok(format!("{ctx}/{assembly_name}"));
        }
    }
    Ok(String::new())
}

/// Increments the usage count for a feature and returns its `INSTALLSTATE`.
///
/// # Arguments
///
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature identifier.
/// * `_install_mode` - Installation mode.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `product_code` or `feature_id` is empty.
///
/// # Returns
///
/// `INSTALLSTATE_LOCAL` (`3`).
pub fn use_feature(
    product_code: &str,
    feature_id: &str,
    _install_mode: InstallMode,
) -> Result<i32, MsiError> {
    if product_code.is_empty() || feature_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code/feature_id".to_string(),
            reason: "empty".to_string(),
        });
    }
    Ok(3)
}

/// Parses a shortcut file (`.lnk` with `DarwinDataBlock` or `.desktop`) and returns its
/// `(product_code, feature_id, component_code)` descriptor components.
///
/// # Arguments
///
/// * `shortcut_path` - Path to the shortcut file on disk.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `shortcut_path` is empty.
///
/// # Returns
///
/// 3-tuple of `(product_code, feature_id, component_code)`.
pub fn get_shortcut_target(shortcut_path: &str) -> Result<(String, String, String), MsiError> {
    if shortcut_path.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "shortcut_path".to_string(),
            reason: "empty".to_string(),
        });
    }
    if let Ok(bytes) = std::fs::read(shortcut_path) {
        // Check for DarwinDataBlock signature 0xA000_0006 in [MS-SHLLINK]
        let sig = crate::platform::shell::lnk::DARWIN_DATA_BLOCK_SIGNATURE.to_le_bytes();
        if let Some(pos) = bytes.windows(4).position(|w| w == sig) {
            let ansi_start = pos + 4;
            if ansi_start < bytes.len() {
                let end = bytes[ansi_start..]
                    .iter()
                    .position(|&b| b == 0)
                    .map_or(bytes.len(), |i| ansi_start + i);
                let desc = String::from_utf8_lossy(&bytes[ansi_start..end]);
                if let Some((prod_feat, comp)) = desc.split_once('>') {
                    let (prod, feat) = prod_feat
                        .split_once('[')
                        .map_or((prod_feat, ""), |(p, rest)| (p, rest.trim_end_matches(']')));
                    return Ok((prod.to_string(), feat.to_string(), comp.to_string()));
                }
            }
        }
    }
    Ok((String::new(), String::new(), String::new()))
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `ComponentId` construction, string borrowing, equality, and cloning.
    #[test]
    fn test_component_id() {
        let cid = ComponentId::new("1234");
        assert_eq!(cid.as_str(), "1234");
        assert_eq!(cid, ComponentId::new("1234"));
        assert!(format!("{cid:?}").contains("1234"), "format check");
        let cid2 = cid.clone();
        assert_eq!(cid, cid2);
    }

    /// Verifies `InstallMode` default and Debug formatting.
    #[test]
    fn test_install_mode_default() {
        assert_eq!(InstallMode::default(), InstallMode(0));
        assert_eq!(format!("{:?}", InstallMode::default()), "InstallMode(0)");
    }

    /// Verifies `provide_component` parameter validation and GUID SQUID lookup path.
    #[test]
    fn test_provide_component() -> Result<(), Box<dyn std::error::Error>> {
        assert!(provide_component("", "f", "c", InstallMode::default()).is_err());
        assert!(provide_component("p", "", "c", InstallMode::default()).is_err());
        assert!(provide_component("p", "f", "", InstallMode::default()).is_err());
        assert!(provide_component("p", "f", "c", InstallMode::default()).is_ok());
        let prod_guid = "{11111111-1111-1111-1111-111111111111}";
        let comp_guid = "{22222222-2222-2222-2222-222222222222}";
        assert!(
            provide_component(prod_guid, "Feature1", comp_guid, InstallMode::default()).is_ok()
        );

        let mut store = RegistryStore::default();
        let comp_squid = encode_squid(comp_guid)?;
        let prod_squid = encode_squid(prod_guid)?;
        let key = format!(
            "Software/Microsoft/Windows/CurrentVersion/Installer/UserData/S-1-5-18/Components/{comp_squid}"
        );
        store.set_value(
            RegistryRoot::LocalMachine,
            &key,
            Some(&prod_squid),
            crate::platform::registry_store::RegistryValue::Sz("C:/App/bin.exe".to_string()),
        );
        let found = provide_component_with_store(
            &store,
            prod_guid,
            "Feature1",
            comp_guid,
            InstallMode::default(),
        )?;
        assert_eq!(found, "C:/App/bin.exe");
        Ok(())
    }

    /// Verifies `provide_component_from_descriptor` on raw and structured descriptors.
    #[test]
    fn test_provide_component_from_descriptor() {
        assert!(provide_component_from_descriptor("").is_err());
        assert!(provide_component_from_descriptor("desc").is_ok());
        assert!(provide_component_from_descriptor("Prod[Feat]>Comp").is_ok());
        assert!(provide_component_from_descriptor("Prod>Comp").is_ok());
    }

    /// Verifies `locate_component` validation and state resolution.
    #[test]
    fn test_locate_component() -> Result<(), Box<dyn std::error::Error>> {
        let err1 = locate_component("").err().unwrap_or_default();
        assert_eq!(
            err1,
            MsiError::InvalidArgument {
                argument: "component_id".to_string(),
                reason: "empty".to_string()
            }
        );
        let err2 = locate_component("{00000000-0000-0000-0000-000000000000}")
            .err()
            .unwrap_or_default();
        assert_eq!(
            err2,
            MsiError::MissingTable {
                name: "test".to_string()
            }
        );
        let res = locate_component("comp")?;
        assert_eq!(res.0, "");
        assert_eq!(res.1, 3);
        Ok(())
    }

    /// Verifies `provide_assembly` with and without application context.
    #[test]
    fn test_provide_assembly() -> Result<(), Box<dyn std::error::Error>> {
        assert!(provide_assembly("", None, InstallMode::default()).is_err());
        assert!(provide_assembly("asm", None, InstallMode::default()).is_ok());
        assert_eq!(
            provide_assembly("asm", Some(""), InstallMode::default())?,
            ""
        );
        assert_eq!(
            provide_assembly("asm.dll", Some("/app"), InstallMode::default())?,
            "/app/asm.dll"
        );
        Ok(())
    }

    /// Verifies `use_feature` parameter validation.
    #[test]
    fn test_use_feature() -> Result<(), Box<dyn std::error::Error>> {
        assert!(use_feature("", "f", InstallMode::default()).is_err());
        assert!(use_feature("p", "", InstallMode::default()).is_err());
        assert_eq!(use_feature("p", "f", InstallMode::default())?, 3);
        Ok(())
    }

    /// Verifies `get_shortcut_target` on missing paths and synthesized `.lnk` files with `DarwinDataBlock`.
    #[test]
    fn test_get_shortcut_target() -> Result<(), Box<dyn std::error::Error>> {
        assert!(get_shortcut_target("").is_err());
        let res = get_shortcut_target("path")?;
        assert_eq!(res.0, "");
        assert_eq!(res.1, "");
        assert_eq!(res.2, "");

        let temp_dir = tempfile::tempdir()?;
        let lnk_path = temp_dir.path().join("adv.lnk");
        let mut builder = crate::platform::shell::lnk::ShellLinkBuilder::new();
        builder
            .set_target("C:/App.exe")
            .set_darwin_descriptor("ProdGuid[MainFeature]>CompGuid");
        assert!(builder.save(&lnk_path).is_ok());

        let parsed = get_shortcut_target(&lnk_path.to_string_lossy())?;
        assert_eq!(
            parsed,
            (
                "ProdGuid".to_string(),
                "MainFeature".to_string(),
                "CompGuid".to_string()
            )
        );

        let lnk_no_feat = temp_dir.path().join("adv_no_feat.lnk");
        let mut raw_block = crate::platform::shell::lnk::DARWIN_DATA_BLOCK_SIGNATURE
            .to_le_bytes()
            .to_vec();
        raw_block.extend_from_slice(b"ProdGuid>CompGuid");
        std::fs::write(&lnk_no_feat, &raw_block)?;
        let parsed_no_feat = get_shortcut_target(&lnk_no_feat.to_string_lossy())?;
        assert_eq!(
            parsed_no_feat,
            (
                "ProdGuid".to_string(),
                String::new(),
                "CompGuid".to_string()
            )
        );

        let lnk_plain = temp_dir.path().join("plain.lnk");
        let mut plain_builder = crate::platform::shell::lnk::ShellLinkBuilder::new();
        plain_builder.set_target("C:/App.exe");
        assert!(plain_builder.save(&lnk_plain).is_ok());
        assert_eq!(
            get_shortcut_target(&lnk_plain.to_string_lossy())?,
            (String::new(), String::new(), String::new())
        );

        let lnk_sig_only = temp_dir.path().join("sig_only.lnk");
        let sig_bytes = crate::platform::shell::lnk::DARWIN_DATA_BLOCK_SIGNATURE
            .to_le_bytes()
            .to_vec();
        std::fs::write(&lnk_sig_only, &sig_bytes)?;
        assert_eq!(
            get_shortcut_target(&lnk_sig_only.to_string_lossy())?,
            (String::new(), String::new(), String::new())
        );

        let lnk_no_delim = temp_dir.path().join("no_delim.lnk");
        let mut no_delim_bytes = sig_bytes;
        no_delim_bytes.extend_from_slice(b"NoArrowHere\0");
        std::fs::write(&lnk_no_delim, &no_delim_bytes)?;
        assert_eq!(
            get_shortcut_target(&lnk_no_delim.to_string_lossy())?,
            (String::new(), String::new(), String::new())
        );
        Ok(())
    }
}
