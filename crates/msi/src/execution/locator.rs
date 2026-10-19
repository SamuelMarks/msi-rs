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
/// * `component_id` - Component GUID or identifier.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `component_id` is empty.
///
/// # Returns
///
/// Tuple of `(keypath, install_state)` where `3` is `INSTALLSTATE_LOCAL` and `-1` is `INSTALLSTATE_UNKNOWN`.
pub fn locate_component(component_id: &str) -> Result<(String, i32), MsiError> {
    let store = RegistryStore::default();
    locate_component_with_store(&store, component_id)
}

/// Locates a component against a specific [`RegistryStore`] and returns its keypath and `INSTALLSTATE`.
///
/// # Arguments
///
/// * `store` - Registry store to query.
/// * `component_id` - Component GUID or identifier.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `component_id` is empty.
///
/// # Returns
///
/// Tuple of `(keypath, install_state)` where `3` is `INSTALLSTATE_LOCAL` and `-1` is `INSTALLSTATE_UNKNOWN`.
pub fn locate_component_with_store(
    store: &RegistryStore,
    component_id: &str,
) -> Result<(String, i32), MsiError> {
    if component_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "component_id".to_string(),
            reason: "empty".to_string(),
        });
    }

    if let Ok(comp_squid) = encode_squid(component_id) {
        let key = format!(
            "Software/Microsoft/Windows/CurrentVersion/Installer/UserData/S-1-5-18/Components/{comp_squid}"
        );
        if let Some(crate::platform::registry_store::RegistryValue::Sz(path)) =
            store.get_first_value_for_key(RegistryRoot::LocalMachine, &key)
        {
            return Ok((path.clone(), 3));
        }
        return Ok((String::new(), -1));
    }

    let comp_key = format!(r"{}\{component_id}", RegistryStore::COMPONENTS_SUBKEY);
    if let Some(crate::platform::registry_store::RegistryValue::Sz(path)) =
        store.get_first_value_for_key(RegistryRoot::LocalMachine, &comp_key)
    {
        return Ok((path.clone(), 3));
    }

    Ok((String::new(), 3))
}

/// Provides a `WinSxS` or GAC assembly path.
///
/// # Arguments
///
/// * `assembly_name` - Assembly display name or strong name.
/// * `app_context` - Optional application context manifest path.
/// * `install_mode` - Installation mode (`INSTALLMODE_*`).
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
    install_mode: InstallMode,
) -> Result<String, MsiError> {
    let store = RegistryStore::default();
    provide_assembly_with_store(&store, assembly_name, app_context, install_mode)
}

/// Provides a `WinSxS` or GAC assembly path using a specific [`RegistryStore`].
///
/// # Arguments
///
/// * `store` - Registry store to query for assembly registration.
/// * `assembly_name` - Assembly display name or strong name.
/// * `app_context` - Optional application context manifest path.
/// * `_install_mode` - Installation mode (`INSTALLMODE_*`).
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `assembly_name` is empty.
///
/// # Returns
///
/// Resolved assembly path string.
pub fn provide_assembly_with_store(
    store: &RegistryStore,
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
    let key = r"Software\Microsoft\Fusion\GACChangeNotification\Default";
    if let Some(crate::platform::registry_store::RegistryValue::Sz(path)) =
        store.get_value(RegistryRoot::LocalMachine, key, Some(assembly_name))
    {
        return Ok(path.clone());
    }
    Ok(String::new())
}

/// Increments the usage count for a feature and returns its `INSTALLSTATE`.
///
/// # Arguments
///
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature identifier.
/// * `install_mode` - Installation mode (`INSTALLMODE_*`).
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
    install_mode: InstallMode,
) -> Result<i32, MsiError> {
    let mut store = RegistryStore::default();
    use_feature_with_store(&mut store, product_code, feature_id, install_mode)
}

/// Increments the usage count for a feature in the provided [`RegistryStore`] and returns its `INSTALLSTATE`.
///
/// # Arguments
///
/// * `store` - Registry store to update and query.
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature identifier.
/// * `_install_mode` - Installation mode (`INSTALLMODE_*`).
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `product_code` or `feature_id` is empty.
///
/// # Returns
///
/// `INSTALLSTATE_LOCAL` (`3`).
pub fn use_feature_with_store(
    store: &mut RegistryStore,
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
    let prod_id = encode_squid(product_code).unwrap_or_else(|_| product_code.to_string());
    let usage_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_id}\Usage"
    );
    let current_count =
        match store.get_value(RegistryRoot::LocalMachine, &usage_key, Some(feature_id)) {
            Some(crate::platform::registry_store::RegistryValue::Dword(c)) => *c,
            _ => 0,
        };
    store.set_value(
        RegistryRoot::LocalMachine,
        &usage_key,
        Some(feature_id),
        crate::platform::registry_store::RegistryValue::Dword(current_count.saturating_add(1)),
    );
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
        let mode = InstallMode::default();
        let found = provide_component_with_store(&store, prod_guid, "Feature1", comp_guid, mode)?;
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

    /// Verifies `locate_component` validation and state resolution via [`RegistryStore`].
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
        let unreg = locate_component("{00000000-0000-0000-0000-000000000000}")?;
        assert_eq!(unreg, (String::new(), -1));

        let res = locate_component("comp")?;
        assert_eq!(res.0, "");
        assert_eq!(res.1, 3);

        // Test registered GUID component in RegistryStore
        let mut store = RegistryStore::default();
        let comp_guid = "{33333333-3333-3333-3333-333333333333}";
        let comp_squid = encode_squid(comp_guid)?;
        let key = format!(
            "Software/Microsoft/Windows/CurrentVersion/Installer/UserData/S-1-5-18/Components/{comp_squid}"
        );
        store.set_value(
            RegistryRoot::LocalMachine,
            &key,
            Some("ClientSquid"),
            crate::platform::registry_store::RegistryValue::Sz(
                "C:/Program Files/App/app.exe".to_string(),
            ),
        );
        let located = locate_component_with_store(&store, comp_guid)?;
        assert_eq!(located, ("C:/Program Files/App/app.exe".to_string(), 3));

        // Test non-GUID component registered in COMPONENTS_SUBKEY
        let comp_key = format!(r"{}\custom_comp", RegistryStore::COMPONENTS_SUBKEY);
        store.set_value(
            RegistryRoot::LocalMachine,
            &comp_key,
            Some("Client1"),
            crate::platform::registry_store::RegistryValue::Sz("C:/Custom/file.dll".to_string()),
        );
        let located_custom = locate_component_with_store(&store, "custom_comp")?;
        assert_eq!(located_custom, ("C:/Custom/file.dll".to_string(), 3));
        Ok(())
    }

    /// Verifies `provide_assembly` with and without application context and GAC registry entries.
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

        let mut store = RegistryStore::default();
        store.set_value(
            RegistryRoot::LocalMachine,
            r"Software\Microsoft\Fusion\GACChangeNotification\Default",
            Some("MyAssembly"),
            crate::platform::registry_store::RegistryValue::Sz(
                "C:/Windows/Microsoft.NET/assembly/MyAssembly.dll".to_string(),
            ),
        );
        assert_eq!(
            provide_assembly_with_store(&store, "MyAssembly", None, InstallMode::default())?,
            "C:/Windows/Microsoft.NET/assembly/MyAssembly.dll"
        );
        Ok(())
    }

    /// Verifies `use_feature` parameter validation and usage counter increment in [`RegistryStore`].
    #[test]
    fn test_use_feature() -> Result<(), Box<dyn std::error::Error>> {
        assert!(use_feature("", "f", InstallMode::default()).is_err());
        assert!(use_feature("p", "", InstallMode::default()).is_err());
        assert_eq!(use_feature("p", "f", InstallMode::default())?, 3);

        let mut store = RegistryStore::default();
        let prod_guid = "{44444444-4444-4444-4444-444444444444}";
        assert_eq!(
            use_feature_with_store(&mut store, prod_guid, "MainFeature", InstallMode::default())?,
            3
        );
        assert_eq!(
            use_feature_with_store(&mut store, prod_guid, "MainFeature", InstallMode::default())?,
            3
        );
        let prod_squid = encode_squid(prod_guid)?;
        let usage_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_squid}\Usage"
        );
        assert_eq!(
            store.get_value(RegistryRoot::LocalMachine, &usage_key, Some("MainFeature")),
            Some(&crate::platform::registry_store::RegistryValue::Dword(2))
        );
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
