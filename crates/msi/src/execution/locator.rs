//! Component and Assembly Locator Subsystem.
//!
//! Handles component provision, path resolution, and self-healing.

use crate::error::MsiError;

/// Strongly-typed Install Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstallMode(pub i32);

/// Strongly-typed Component Tracking representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentId(String);

impl ComponentId {
    /// Creates a new `ComponentId`.
    ///
    /// # Arguments
    ///
    /// * `id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Returns the string representation.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provides a component, returning its path.
///
/// # Arguments
///
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature ID.
/// * `component_id` - Component ID.
/// * `install_mode` - Installation mode.
///
/// # Errors
///
/// Returns `MsiError::InvalidParameter` if parameters are invalid.
///
/// # Returns
///
/// TODO: Document return value.
pub fn provide_component(
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
    // TODO: Implement actual provision logic (resolve, heal, return path).
    Ok(String::new())
}

/// Provides a component from a descriptor string.
///
/// # Arguments
///
/// * `descriptor` - Descriptor string.
///
/// # Errors
///
/// Returns `MsiError::InvalidParameter` if descriptor is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn provide_component_from_descriptor(descriptor: &str) -> Result<String, MsiError> {
    if descriptor.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "descriptor".to_string(),
            reason: "empty".to_string(),
        });
    }
    // TODO: Parse descriptor and provide component.
    Ok(String::new())
}

/// Locates a component and returns its state.
///
/// # Arguments
///
/// * `component_id` - Component GUID.
///
/// # Errors
///
/// Returns `MsiError::InvalidParameter` if `component_id` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn locate_component(component_id: &str) -> Result<(String, i32), MsiError> {
    if component_id.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "component_id".to_string(),
            reason: "empty".to_string(),
        });
    }
    // TODO: Locate component path and determine install state (e.g., INSTALLSTATE_LOCAL).
    Ok((String::new(), 3)) // Return empty path and INSTALLSTATE_LOCAL
}

/// Provides an assembly.
///
/// # Arguments
///
/// * `assembly_name` - Assembly name.
/// * `app_context` - Application context path.
/// * `install_mode` - Installation mode.
///
/// # Errors
///
/// Returns `MsiError::InvalidParameter` if `assembly_name` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn provide_assembly(
    assembly_name: &str,
    _app_context: Option<&str>,
    _install_mode: InstallMode,
) -> Result<String, MsiError> {
    if assembly_name.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "assembly_name".to_string(),
            reason: "empty".to_string(),
        });
    }
    // TODO: Locate and verify assembly.
    Ok(String::new())
}

/// Increments the usage count for a feature and returns its state.
///
/// # Arguments
///
/// * `product_code` - Product code GUID.
/// * `feature_id` - Feature ID.
/// * `install_mode` - Installation mode.
///
/// # Errors
///
/// Returns `MsiError::InvalidParameter` if strings are empty.
///
/// # Returns
///
/// TODO: Document return value.
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
    // TODO: Connect to registry store
    Ok(3) // INSTALLSTATE_LOCAL
}

/// Parses a shortcut file and returns its target descriptor components.
///
/// # Arguments
///
/// * `shortcut_path` - Path to the shortcut file.
///
/// # Errors
///
/// Returns `MsiError::InvalidParameter` if `shortcut_path` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn get_shortcut_target(shortcut_path: &str) -> Result<(String, String, String), MsiError> {
    if shortcut_path.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "shortcut_path".to_string(),
            reason: "empty".to_string(),
        });
    }
    // TODO: Parse actual .lnk / .desktop
    Ok((String::new(), String::new(), String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_component_id() {
        let cid = ComponentId::new("1234");
        assert_eq!(cid.as_str(), "1234");
        assert_eq!(cid, ComponentId::new("1234"));
        assert_eq!(format!("{cid:?}"), "ComponentId(\"1234\")");
        let cid2 = cid.clone();
        assert_eq!(cid, cid2);
    }

    #[test]
    fn test_install_mode_default() {
        assert_eq!(InstallMode::default(), InstallMode(0));
        assert_eq!(format!("{:?}", InstallMode::default()), "InstallMode(0)");
    }

    #[test]
    fn test_provide_component() {
        assert!(provide_component("", "f", "c", InstallMode::default()).is_err());
        assert!(provide_component("p", "", "c", InstallMode::default()).is_err());
        assert!(provide_component("p", "f", "", InstallMode::default()).is_err());
        assert!(provide_component("p", "f", "c", InstallMode::default()).is_ok());
    }

    #[test]
    fn test_provide_component_from_descriptor() {
        assert!(provide_component_from_descriptor("").is_err());
        assert!(provide_component_from_descriptor("desc").is_ok());
    }

    #[test]
    fn test_locate_component() {
        assert!(locate_component("").is_err());
        let res = locate_component("comp").expect("test");
        assert_eq!(res.0, "");
        assert_eq!(res.1, 3);
    }

    #[test]
    fn test_provide_assembly() {
        assert!(provide_assembly("", None, InstallMode::default()).is_err());
        assert!(provide_assembly("asm", None, InstallMode::default()).is_ok());
    }

    #[test]
    fn test_use_feature() {
        assert!(use_feature("", "f", InstallMode::default()).is_err());
        assert!(use_feature("p", "", InstallMode::default()).is_err());
        assert_eq!(
            use_feature("p", "f", InstallMode::default()).expect("test"),
            3
        );
    }

    #[test]
    fn test_get_shortcut_target() {
        assert!(get_shortcut_target("").is_err());
        let res = get_shortcut_target("path").expect("test");
        assert_eq!(res.0, "");
        assert_eq!(res.1, "");
        assert_eq!(res.2, "");
    }
}
