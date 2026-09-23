//! `WiX` XML schemas and namespace definitions.
//!
//! Supports:
//! - `WiX` v3: `http://schemas.microsoft.com/wix/2006/wi`
//! - `WiX` v4: `http://wixtoolset.org/schemas/v4/wxs`
//! - `WiX` v5: `http://wixtoolset.org/schemas/v5/wxs`
//! - Cross-Platform POSIX Extension: `http://schemas.msi-rs.org/wix/posix/v1`

use crate::error::{Error, Result};
use std::fmt;

/// Standard `WiX` v3 XML namespace URI.
pub const WIX_V3_NAMESPACE: &str = "http://schemas.microsoft.com/wix/2006/wi";

/// Standard `WiX` v4 XML namespace URI.
pub const WIX_V4_NAMESPACE: &str = "http://wixtoolset.org/schemas/v4/wxs";

/// Standard `WiX` v5 XML namespace URI.
pub const WIX_V5_NAMESPACE: &str = "http://wixtoolset.org/schemas/v5/wxs";

/// Cross-Platform POSIX extension namespace URI.
pub const WIX_POSIX_V1_NAMESPACE: &str = "http://schemas.msi-rs.org/wix/posix/v1";

/// Supported `WiX` schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WixSchemaVersion {
    /// `WiX` Toolset v3 XML schema.
    V3,
    /// `WiX` Toolset v4 XML schema.
    V4,
    /// `WiX` Toolset v5 XML schema.
    V5,
    /// Cross-Platform POSIX v1 extension schema.
    PosixV1,
}

impl WixSchemaVersion {
    /// Identifies the `WiX` schema version from an XML namespace URI.
    ///
    /// # Arguments
    ///
    /// * `uri` - XML namespace URI string.
    ///
    /// # Returns
    ///
    /// The matching [`WixSchemaVersion`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the namespace URI is unrecognized.
    pub fn from_uri(uri: &str) -> Result<Self> {
        match uri.trim() {
            WIX_V3_NAMESPACE => Ok(Self::V3),
            WIX_V4_NAMESPACE => Ok(Self::V4),
            WIX_V5_NAMESPACE => Ok(Self::V5),
            WIX_POSIX_V1_NAMESPACE => Ok(Self::PosixV1),
            other => Err(Error::Validation {
                element: "WixSchemaVersion".to_string(),
                reason: format!("unrecognized WiX schema namespace URI: '{other}'"),
            }),
        }
    }

    /// Returns the official XML namespace URI corresponding to this schema version.
    ///
    /// # Returns
    ///
    /// Namespace URI string slice.
    #[must_use]
    pub const fn namespace_uri(self) -> &'static str {
        match self {
            Self::V3 => WIX_V3_NAMESPACE,
            Self::V4 => WIX_V4_NAMESPACE,
            Self::V5 => WIX_V5_NAMESPACE,
            Self::PosixV1 => WIX_POSIX_V1_NAMESPACE,
        }
    }
}

impl fmt::Display for WixSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V3 => write!(f, "WiX v3 ({WIX_V3_NAMESPACE})"),
            Self::V4 => write!(f, "WiX v4 ({WIX_V4_NAMESPACE})"),
            Self::V5 => write!(f, "WiX v5 ({WIX_V5_NAMESPACE})"),
            Self::PosixV1 => write!(f, "POSIX v1 ({WIX_POSIX_V1_NAMESPACE})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests roundtrip conversion between `WiX` schema versions and namespace URIs.
    #[test]
    fn test_schema_versions_roundtrip() {
        assert_eq!(
            WixSchemaVersion::from_uri(WIX_V3_NAMESPACE),
            Ok(WixSchemaVersion::V3)
        );
        assert_eq!(
            WixSchemaVersion::from_uri(WIX_V4_NAMESPACE),
            Ok(WixSchemaVersion::V4)
        );
        assert_eq!(
            WixSchemaVersion::from_uri(WIX_V5_NAMESPACE),
            Ok(WixSchemaVersion::V5)
        );
        assert_eq!(
            WixSchemaVersion::from_uri(WIX_POSIX_V1_NAMESPACE),
            Ok(WixSchemaVersion::PosixV1)
        );

        assert_eq!(WixSchemaVersion::V3.namespace_uri(), WIX_V3_NAMESPACE);
        assert_eq!(WixSchemaVersion::V4.namespace_uri(), WIX_V4_NAMESPACE);
        assert_eq!(WixSchemaVersion::V5.namespace_uri(), WIX_V5_NAMESPACE);
        assert_eq!(
            WixSchemaVersion::PosixV1.namespace_uri(),
            WIX_POSIX_V1_NAMESPACE
        );

        assert!(format!("{}", WixSchemaVersion::V3).contains("WiX v3"));
        assert!(format!("{}", WixSchemaVersion::V4).contains("WiX v4"));
        assert!(format!("{}", WixSchemaVersion::V5).contains("WiX v5"));
        assert!(format!("{}", WixSchemaVersion::PosixV1).contains("POSIX v1"));

        assert!(WixSchemaVersion::from_uri("http://example.com/invalid").is_err());
    }
}
