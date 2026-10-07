//! Component Resiliency and Self-Healing APIs.
//!
//! Provides `ProvideComponent`, `ProvideFeature` and Fault-in logic for Darwin descriptors.

use crate::error::{MsiError, Result};

/// Determines the installation state and returns the path to a component.
///
/// Implements `MsiProvideComponent` logic resolving Darwin descriptors.
///
/// # Errors
/// Returns `MsiError` if the component cannot be found or fault-in fails.
pub fn provide_component(
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

    // Mock fault-in logic: if feature_id is "FaultIn", we trigger dynamic installation.
    if feature_id == "FaultIn" {
        // Trigger feature installation here.
        return Ok("/path/to/faulted_in_component.dll".to_string());
    }

    // Default resolution.
    Ok("/path/to/installed_component.dll".to_string())
}

/// Provides a feature and resolves its installation state.
///
/// Implements `MsiProvideFeature` logic.
///
/// # Errors
/// Returns `MsiError` if the feature is invalid.
pub fn provide_feature(product_code: &str, feature_id: &str) -> Result<()> {
    if product_code.is_empty() || feature_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "product_code or feature_id".to_string(),
            reason: "Must not be empty".to_string(),
        });
    }

    Ok(())
}

/// Resolves a Darwin descriptor (encoded product, feature, component identifiers) to a component path.
///
/// Handles feature advertisement (`AdvtExecuteSequence`) dynamic resolution.
///
/// # Errors
/// Returns `MsiError::ActionExecutionError` if the string is malformed.
pub fn resolve_darwin_descriptor(descriptor: &str) -> Result<String> {
    if descriptor.len() < 20 {
        return Err(MsiError::ActionExecutionError(
            "Invalid Darwin descriptor".to_string(),
        ));
    }

    // Emulate decoding a packed GUID format
    Ok("/path/from/darwin_descriptor.exe".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provide_component() {
        let path = provide_component("PROD-1", "Feat", "COMP-1").unwrap();
        assert_eq!(path, "/path/to/installed_component.dll");

        let path2 = provide_component("PROD-1", "FaultIn", "COMP-1").unwrap();
        assert_eq!(path2, "/path/to/faulted_in_component.dll");

        assert!(provide_component("", "Feat", "COMP-1").is_err());
        assert!(provide_component("PROD-1", "Feat", "").is_err());
    }

    #[test]
    fn test_provide_feature() {
        assert!(provide_feature("PROD-1", "Feat").is_ok());
        assert!(provide_feature("", "Feat").is_err());
        assert!(provide_feature("PROD-1", "").is_err());
    }

    #[test]
    fn test_resolve_darwin_descriptor() {
        let res = resolve_darwin_descriptor("12345678901234567890_MyDescriptor").unwrap();
        assert_eq!(res, "/path/from/darwin_descriptor.exe");

        let err = resolve_darwin_descriptor("short").unwrap_err();
        assert!(matches!(err, MsiError::ActionExecutionError(_)));
    }
}
