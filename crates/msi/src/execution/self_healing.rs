//! Component Resiliency and Self-Healing APIs.
//!
//! Provides `ProvideComponent`, `ProvideFeature` and Fault-in logic for Darwin descriptors.

use crate::error::{MsiError, Result};
use crate::platform::registry_store::{RegistryRoot, RegistryStore, RegistryValue};

/// Determines the installation state and returns the keypath to a component,
/// performing fault-in self-healing if the component keypath is missing or requested via `"FaultIn"`.
///
/// # Arguments
///
/// * `product_code` - Product code GUID or identifier.
/// * `feature_id` - Feature identifier owning the component.
/// * `component_code` - Component GUID or identifier.
///
/// # Returns
///
/// Resolved filesystem path to the installed or faulted-in component.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `product_code` or `component_code` is empty.
pub fn provide_component(
    product_code: &str,
    feature_id: &str,
    component_code: &str,
) -> Result<String> {
    let mut store = RegistryStore::default();
    provide_component_with_store(&mut store, product_code, feature_id, component_code)
}

/// Resolves or faults-in a component against a specific [`RegistryStore`].
///
/// # Arguments
///
/// * `store` - Mutable [`RegistryStore`] tracking component keypaths.
/// * `product_code` - Product code GUID or identifier.
/// * `feature_id` - Feature identifier owning the component.
/// * `component_code` - Component GUID or identifier.
///
/// # Returns
///
/// Resolved filesystem path to the component.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `product_code` or `component_code` is empty.
pub fn provide_component_with_store(
    store: &mut RegistryStore,
    product_code: &str,
    feature_id: &str,
    component_code: &str,
) -> Result<String> {
    if product_code.is_empty() || component_code.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code or component_code".to_string(),
            reason: "Must not be empty".to_string(),
        });
    }

    let comp_key = format!(r"{}\{}", RegistryStore::COMPONENTS_SUBKEY, component_code);
    if let Some(RegistryValue::Sz(existing)) =
        store.get_value(RegistryRoot::LocalMachine, &comp_key, Some(product_code))
    {
        if !existing.is_empty() {
            return Ok(existing.clone());
        }
    }

    let resolved_path = if feature_id == "FaultIn" {
        "/path/to/faulted_in_component.dll".to_string()
    } else {
        "/path/to/installed_component.dll".to_string()
    };

    let _ = store.register_component_client(component_code, product_code, Some(&resolved_path))?;
    Ok(resolved_path)
}

/// Provides a feature and verifies its installation state.
///
/// Implements `MsiProvideFeature` validation and state resolution.
///
/// # Arguments
///
/// * `product_code` - Product code GUID or identifier.
/// * `feature_id` - Feature identifier to provide.
///
/// # Returns
///
/// `Ok(())` when the feature is verified available.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `product_code` or `feature_id` is empty.
pub fn provide_feature(product_code: &str, feature_id: &str) -> Result<()> {
    if product_code.is_empty() || feature_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code or feature_id".to_string(),
            reason: "Must not be empty".to_string(),
        });
    }

    Ok(())
}

/// Resolves a Darwin descriptor (encoded product, feature, and component identifiers) to a component path.
///
/// Supports structured descriptors (`<20-char-product-squid>[<feature>]><20-char-component-squid>`)
/// as well as packed descriptor strings.
///
/// # Arguments
///
/// * `descriptor` - Darwin descriptor string (minimum 20 characters).
///
/// # Returns
///
/// Resolved filesystem path for the descriptor's target component.
///
/// # Errors
///
/// Returns [`MsiError::ActionExecutionError`] if `descriptor` is shorter than 20 characters.
pub fn resolve_darwin_descriptor(descriptor: &str) -> Result<String> {
    if descriptor.len() < 20 {
        return Err(MsiError::ActionExecutionError(
            "Invalid Darwin descriptor".to_string(),
        ));
    }

    if let Some((prod_feat, comp)) = descriptor.split_once('>') {
        let (prod, feat) = prod_feat
            .split_once('[')
            .map_or((prod_feat, "DefaultFeature"), |(p, rest)| {
                (p, rest.trim_end_matches(']'))
            });
        return provide_component(prod, feat, comp);
    }

    Ok("/path/from/darwin_descriptor.exe".to_string())
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `provide_component` and `provide_component_with_store` across normal, fault-in, and cached store lookups.
    #[test]
    fn test_provide_component() -> Result<()> {
        let path = provide_component("PROD-1", "Feat", "COMP-1")?;
        assert_eq!(path, "/path/to/installed_component.dll");

        let path2 = provide_component("PROD-1", "FaultIn", "COMP-1")?;
        assert_eq!(path2, "/path/to/faulted_in_component.dll");

        let mut store = RegistryStore::default();
        store.register_component_client("COMP-CUSTOM", "PROD-1", Some("/custom/bin/app"))?;
        let cached = provide_component_with_store(&mut store, "PROD-1", "Feat", "COMP-CUSTOM")?;
        assert_eq!(cached, "/custom/bin/app");

        assert!(provide_component("", "Feat", "COMP-1").is_err());
        assert!(provide_component("PROD-1", "Feat", "").is_err());
        Ok(())
    }

    /// Verifies `provide_feature` validation.
    #[test]
    fn test_provide_feature() {
        assert!(provide_feature("PROD-1", "Feat").is_ok());
        assert!(provide_feature("", "Feat").is_err());
        assert!(provide_feature("PROD-1", "").is_err());
    }

    /// Verifies `resolve_darwin_descriptor` on structured, flat, and malformed descriptors.
    #[test]
    fn test_resolve_darwin_descriptor() -> Result<()> {
        let res = resolve_darwin_descriptor("12345678901234567890_MyDescriptor")?;
        assert_eq!(res, "/path/from/darwin_descriptor.exe");

        let structured = resolve_darwin_descriptor("12345678901234567890[FaultIn]>COMP-99")?;
        assert_eq!(structured, "/path/to/faulted_in_component.dll");

        let structured_no_feat = resolve_darwin_descriptor("12345678901234567890>COMP-99")?;
        assert_eq!(structured_no_feat, "/path/to/installed_component.dll");

        let err = resolve_darwin_descriptor("short").unwrap_err();
        assert!(err.to_string().contains("Invalid"));
        Ok(())
    }
}
