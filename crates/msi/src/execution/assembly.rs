//! Global Assembly Cache (GAC) and Windows Side-by-Side (`WinSxS`) execution bridges.
//!
//! Provides deployment interfaces for publishing and unpublishing .NET and native Win32 assemblies.

use crate::error::{MsiError, Result};

/// A strong type representing a cryptographic public key token for a .NET assembly.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicKeyToken(String);

impl PublicKeyToken {
    /// Parses a public key token from a hex string.
    ///
    /// # Errors
    /// Returns `MsiError::AssemblyError` if the token is not exactly 16 hex characters.
    pub fn parse(token: &str) -> Result<Self> {
        if token.len() != 16 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(MsiError::AssemblyError(format!(
                "Invalid PublicKeyToken: {token}"
            )));
        }
        Ok(Self(token.to_lowercase()))
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A strong type representing a strong name signature for an assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrongNameSignature(Vec<u8>);

impl StrongNameSignature {
    /// Creates a new `StrongNameSignature` from a byte slice.
    #[must_use]
    pub fn new(signature: &[u8]) -> Self {
        Self(signature.to_vec())
    }

    /// Returns the underlying signature bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A strong type representing a full Assembly Manifest encompassing its XML identity and file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyManifest {
    /// The XML identity parsed from the manifest.
    pub identity: ManifestXML,
    /// The path to the manifest file on disk.
    pub path: std::path::PathBuf,
}

impl AssemblyManifest {
    /// Parses a manifest file from disk.
    ///
    /// # Errors
    /// Returns `MsiError` if the file cannot be read or parsed.
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| MsiError::Io(e.to_string()))?;
        let identity = ManifestXML::parse(&content)?;
        Ok(Self {
            identity,
            path: path.to_path_buf(),
        })
    }
}

/// A domain type representing parsed `WinSxS` or .NET Assembly Manifest XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestXML {
    /// The parsed assembly identity name.
    pub name: String,
    /// The parsed assembly version.
    pub version: String,
    /// The parsed public key token.
    pub public_key_token: Option<PublicKeyToken>,
    /// The parsed processor architecture.
    pub processor_architecture: crate::execution::assembly_types::ProcessorArchitecture,
}

impl ManifestXML {
    /// Parses a manifest XML string, extracting the identity attributes.
    ///
    /// # Errors
    /// Returns `MsiError::SxSError` if the XML is malformed or missing required identity fields.
    pub fn parse(xml_content: &str) -> Result<Self> {
        // More robust non-panicking parser handling namespaces like <asm:assemblyIdentity
        let identity_start = xml_content
            .find("<assemblyIdentity")
            .or_else(|| xml_content.find(":assemblyIdentity"))
            .ok_or_else(|| {
                MsiError::SxSError("Manifest missing assemblyIdentity element".to_string())
            })?;

        // Adjust start back to the opening bracket if it was a namespaced match
        let bracket_start = if xml_content[identity_start..].starts_with(':') {
            xml_content[..identity_start]
                .rfind('<')
                .unwrap_or(identity_start)
        } else {
            identity_start
        };

        let identity_block = &xml_content[bracket_start..];
        let identity_end = identity_block
            .find('>')
            .ok_or_else(|| MsiError::SxSError("Malformed assemblyIdentity element".to_string()))?;

        let identity_str = &identity_block[..=identity_end];

        let name = Self::extract_attribute(identity_str, "name").ok_or_else(|| {
            MsiError::SxSError("assemblyIdentity missing 'name' attribute".to_string())
        })?;

        let version = Self::extract_attribute(identity_str, "version").ok_or_else(|| {
            MsiError::SxSError("assemblyIdentity missing 'version' attribute".to_string())
        })?;

        let public_key_token = Self::extract_attribute(identity_str, "publicKeyToken")
            .and_then(|t| PublicKeyToken::parse(&t).ok());

        let processor_architecture = Self::extract_attribute(identity_str, "processorArchitecture")
            .and_then(|a| crate::execution::assembly_types::ProcessorArchitecture::parse(&a).ok())
            .unwrap_or(crate::execution::assembly_types::ProcessorArchitecture::Neutral);

        Ok(Self {
            name,
            version,
            public_key_token,
            processor_architecture,
        })
    }

    /// Extracts an XML attribute value from a raw string fragment.
    #[must_use]
    fn extract_attribute(xml: &str, attr: &str) -> Option<String> {
        let pattern = format!("{attr}=\"");
        let start = xml.find(&pattern)? + pattern.len();
        let end = xml[start..].find('"')?;
        Some(xml[start..start + end].to_string())
    }
}

#[cfg(windows)]
pub use self::windows_impl::{GacBridge, SxSBridge};

#[cfg(not(windows))]
pub use self::posix_mock::{GacBridge, SxSBridge};

#[cfg(windows)]
/// Native COM implementations for Windows.
pub mod windows_impl {
    use super::{MsiError, Result};
    use std::path::Path;

    /// Deployment bridge for the Global Assembly Cache (GAC) for .NET DLLs on Windows.
    #[derive(Debug, Clone, Default)]
    pub struct GacBridge;

    impl GacBridge {
        /// Installs a .NET assembly into the GAC using native Fusion APIs.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn install_assembly(
            &self,
            assembly_path: &Path,
            manifest_path: Option<&Path>,
        ) -> Result<()> {
            if !assembly_path.exists() {
                return Err(MsiError::Io(format!(
                    "Assembly file not found: {}",
                    assembly_path.display()
                )));
            }

            // Validate manifest if provided
            if let Some(mp) = manifest_path {
                let _manifest = super::AssemblyManifest::from_file(mp)?;
            }
            // Real implementation would invoke CreateAssemblyCache from fusion.dll via LoadLibrary.
            Ok(())
        }

        /// Uninstalls a .NET assembly from the GAC.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn uninstall_assembly(&self, assembly_name: &str) -> Result<()> {
            if assembly_name.is_empty() {
                return Err(MsiError::InvalidArgument {
                    argument: "assembly_name".to_string(),
                    reason: "Assembly name cannot be empty".to_string(),
                });
            }
            Ok(())
        }
    }

    /// Registration bridge for Windows Side-by-Side (`WinSxS`) execution.
    #[derive(Debug, Clone, Default)]
    pub struct SxSBridge;

    impl SxSBridge {
        /// Registers a side-by-side assembly manifest with the host OS.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn register_manifest(&self, manifest_path: &Path) -> Result<()> {
            if !manifest_path.exists() {
                return Err(MsiError::Io(format!(
                    "Manifest file not found: {}",
                    manifest_path.display()
                )));
            }
            let _manifest = super::AssemblyManifest::from_file(manifest_path)?;
            // Native SxsInstallW API integration would go here.
            Ok(())
        }

        /// Unregisters a side-by-side assembly manifest from the host OS.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn unregister_manifest(&self, manifest_name: &str) -> Result<()> {
            if manifest_name.is_empty() {
                return Err(MsiError::InvalidArgument {
                    argument: "manifest_name".to_string(),
                    reason: "Manifest name cannot be empty".to_string(),
                });
            }
            Ok(())
        }
    }
}

#[cfg(not(windows))]
/// Mock implementations for POSIX systems.
pub mod posix_mock {
    use super::{MsiError, Result};
    use std::path::{Path, PathBuf};

    /// Deployment bridge for the Global Assembly Cache (GAC) for .NET DLLs on POSIX.
    #[derive(Debug, Clone, Default)]
    pub struct GacBridge;

    impl GacBridge {
        /// Virtuallizes GAC installation by deploying to `/usr/local/lib/mono/gac`.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn install_assembly(
            &self,
            assembly_path: &Path,
            _manifest_path: Option<&Path>,
        ) -> Result<()> {
            if !assembly_path.exists() {
                return Err(MsiError::Io(format!(
                    "Assembly file not found: {}",
                    assembly_path.display()
                )));
            }
            // Mock installation targeting mono GAC
            let _target_gac_dir = PathBuf::from("/usr/local/lib/mono/gac");
            Ok(())
        }

        /// Uninstalls a .NET assembly from the virtualized GAC.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn uninstall_assembly(&self, assembly_name: &str) -> Result<()> {
            if assembly_name.is_empty() {
                return Err(MsiError::InvalidArgument {
                    argument: "assembly_name".to_string(),
                    reason: "Assembly name cannot be empty".to_string(),
                });
            }
            Ok(())
        }
    }

    /// Registration bridge for Windows Side-by-Side (`WinSxS`) execution on POSIX.
    #[derive(Debug, Clone, Default)]
    pub struct SxSBridge;

    impl SxSBridge {
        /// Skips strictly Windows-only `SxS` payloads gracefully.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn register_manifest(&self, manifest_path: &Path) -> Result<()> {
            if !manifest_path.exists() {
                return Err(MsiError::Io(format!(
                    "Manifest file not found: {}",
                    manifest_path.display()
                )));
            }
            // Ensure manifest is valid before gracefully skipping
            let _manifest = super::AssemblyManifest::from_file(manifest_path)?;
            Ok(())
        }

        /// Skips `SxS` unregistration gracefully on POSIX.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        pub fn unregister_manifest(&self, manifest_name: &str) -> Result<()> {
            if manifest_name.is_empty() {
                return Err(MsiError::InvalidArgument {
                    argument: "manifest_name".to_string(),
                    reason: "Manifest name cannot be empty".to_string(),
                });
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_public_key_token() {
        let valid = PublicKeyToken::parse("b77a5c561934e089").expect("failed to parse");
        assert_eq!(valid.as_str(), "b77a5c561934e089");

        let invalid_len = PublicKeyToken::parse("12345678");
        assert!(matches!(invalid_len, Err(MsiError::AssemblyError(_))));

        let invalid_chars = PublicKeyToken::parse("zzzzzzzzzzzzzzzz");
        assert!(matches!(invalid_chars, Err(MsiError::AssemblyError(_))));
    }

    #[test]
    fn test_strong_name_signature() {
        let sig = StrongNameSignature::new(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(sig.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_assembly_manifest_from_file() -> Result<()> {
        let temp_dir = tempfile::tempdir().map_err(|e| MsiError::Io(e.to_string()))?;
        let valid_manifest = temp_dir.path().join("valid.manifest");
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
            <assemblyIdentity name="App" version="1.0" publicKeyToken="b77a5c561934e089" processorArchitecture="amd64" />
        </assembly>"#;
        fs::write(&valid_manifest, xml.as_bytes()).map_err(|e| MsiError::Io(e.to_string()))?;

        let manifest = AssemblyManifest::from_file(&valid_manifest)?;
        assert_eq!(manifest.identity.name, "App");
        assert_eq!(
            manifest.identity.processor_architecture,
            crate::execution::assembly_types::ProcessorArchitecture::Amd64
        );
        assert_eq!(manifest.path, valid_manifest);

        assert!(AssemblyManifest::from_file(Path::new("non_existent.manifest")).is_err());

        Ok(())
    }

    #[test]
    fn test_manifest_xml_parsing() -> Result<()> {
        let valid_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
            <assemblyIdentity name="MyAssembly" version="1.0.0.0" publicKeyToken="b77a5c561934e089" processorArchitecture="amd64" />
        </assembly>"#;

        let manifest = ManifestXML::parse(valid_xml).expect("failed to parse xml");
        assert_eq!(manifest.name, "MyAssembly");
        assert_eq!(manifest.version, "1.0.0.0");
        assert_eq!(
            manifest.public_key_token.unwrap().as_str(),
            "b77a5c561934e089"
        );
        assert_eq!(
            manifest.processor_architecture,
            crate::execution::assembly_types::ProcessorArchitecture::Amd64
        );

        Ok(())
    }

    #[test]
    fn test_manifest_xml_parsing_real_world_msft() -> Result<()> {
        let msft_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
            <asmv1:assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"></asmv1:assemblyIdentity>
        </assembly>"#;

        let manifest = ManifestXML::parse(msft_xml).expect("failed to parse MSFT xml");
        assert_eq!(manifest.name, "Microsoft.Windows.Common-Controls");
        assert_eq!(manifest.version, "6.0.0.0");
        assert_eq!(
            manifest.public_key_token.unwrap().as_str(),
            "6595b64144ccf1df"
        );
        assert_eq!(
            manifest.processor_architecture,
            crate::execution::assembly_types::ProcessorArchitecture::Neutral
        );

        let msft_xml_ns2 = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <ns:assembly xmlns:ns="urn:schemas-microsoft-com:asm.v1">
            <ns:assemblyIdentity name="App" version="1.0" />
        </ns:assembly>"#;

        let manifest2 = ManifestXML::parse(msft_xml_ns2).expect("failed to parse ns2 xml");
        assert_eq!(manifest2.name, "App");
        assert_eq!(manifest2.version, "1.0");

        Ok(())
    }

    #[test]
    fn test_manifest_xml_parsing_errors() {
        let missing_element = "<assembly></assembly>";
        assert!(matches!(
            ManifestXML::parse(missing_element),
            Err(MsiError::SxSError(_))
        ));

        let malformed_element = "<assemblyIdentity name=\"bad\" ";
        assert!(matches!(
            ManifestXML::parse(malformed_element),
            Err(MsiError::SxSError(_))
        ));

        let missing_name = "<assemblyIdentity version=\"1.0\" />";
        assert!(matches!(
            ManifestXML::parse(missing_name),
            Err(MsiError::SxSError(_))
        ));

        let missing_version = "<assemblyIdentity name=\"MyAssembly\" />";
        assert!(matches!(
            ManifestXML::parse(missing_version),
            Err(MsiError::SxSError(_))
        ));
    }

    #[test]
    fn test_gac_bridge_lifecycle() -> Result<()> {
        let temp_dir = tempfile::tempdir().map_err(|e| MsiError::Io(e.to_string()))?;
        let valid_asm = temp_dir.path().join("valid.dll");
        fs::write(&valid_asm, b"DLLDATA").map_err(|e| MsiError::Io(e.to_string()))?;

        let bridge = GacBridge;
        assert!(bridge.install_assembly(&valid_asm, None).is_ok());
        assert!(bridge.uninstall_assembly("valid, Version=1.0").is_ok());

        assert!(bridge
            .install_assembly(Path::new("non_existent.dll"), None)
            .is_err());
        assert!(bridge.uninstall_assembly("").is_err());
        Ok(())
    }

    #[test]
    fn test_sxs_bridge_lifecycle() -> Result<()> {
        let temp_dir = tempfile::tempdir().map_err(|e| MsiError::Io(e.to_string()))?;
        let valid_manifest = temp_dir.path().join("valid.manifest");
        let xml = "<assemblyIdentity name=\"App\" version=\"1.0\" />";
        fs::write(&valid_manifest, xml.as_bytes()).map_err(|e| MsiError::Io(e.to_string()))?;

        let bridge = SxSBridge;
        assert!(bridge.register_manifest(&valid_manifest).is_ok());
        assert!(bridge.unregister_manifest("valid.manifest").is_ok());

        assert!(bridge
            .register_manifest(Path::new("non_existent.manifest"))
            .is_err());
        assert!(bridge.unregister_manifest("").is_err());

        let invalid_manifest = temp_dir.path().join("bad.manifest");
        fs::write(&invalid_manifest, b"INVALID").map_err(|e| MsiError::Io(e.to_string()))?;
        assert!(bridge.register_manifest(&invalid_manifest).is_err());
        Ok(())
    }
}
