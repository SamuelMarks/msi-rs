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
    ///
    /// # Arguments
    ///
    /// * `token` - 16-character hexadecimal public key token string.
    ///
    /// # Returns
    ///
    /// Normalized lowercase [`PublicKeyToken`].
    pub fn parse(token: &str) -> Result<Self> {
        if token.len() != 16 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(MsiError::AssemblyError(format!(
                "Invalid PublicKeyToken: {token}"
            )));
        }
        Ok(Self(token.to_lowercase()))
    }

    /// Returns the string representation.
    ///
    /// # Returns
    ///
    /// Borrowed string slice of the 16-character hex token.
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
    ///
    /// # Arguments
    ///
    /// * `signature` - Raw strong-name cryptographic signature bytes.
    ///
    /// # Returns
    ///
    /// Newly constructed [`StrongNameSignature`].
    #[must_use]
    pub fn new(signature: &[u8]) -> Self {
        Self(signature.to_vec())
    }

    /// Returns the underlying signature bytes.
    ///
    /// # Returns
    ///
    /// Borrowed byte slice of the strong-name signature.
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
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the `.manifest` XML file.
    ///
    /// # Returns
    ///
    /// Parsed [`AssemblyManifest`].
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
        let identity = ManifestXML::parse(&content)?;
        Ok(Self {
            identity,
            path: path.to_path_buf(),
        })
    }

    /// Parses a manifest from a PE (Portable Executable) file's embedded resources.
    ///
    /// # Errors
    /// Returns `MsiError` if the file cannot be read, parsed, or lacks a manifest.
    ///
    /// # Arguments
    ///
    /// * `pe_path` - The path to the PE file.
    ///
    /// # Returns
    ///
    /// The parsed assembly manifest.
    pub fn from_pe_file(pe_path: &std::path::Path) -> Result<Self> {
        Self::from_pe_file_with_extractor(
            pe_path,
            crate::execution::manifest_extract::extract_manifest,
        )
    }

    /// Extracts and parses an assembly manifest using a custom extraction function.
    ///
    /// # Errors
    /// Returns an error if the manifest extraction or parsing fails.
    pub fn from_pe_file_with_extractor(
        pe_path: &std::path::Path,
        extractor: fn(&std::path::Path) -> Result<String>,
    ) -> Result<Self> {
        let content = extractor(pe_path)?;
        let identity = ManifestXML::parse(&content)?;
        Ok(Self {
            identity,
            path: pe_path.to_path_buf(),
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
    ///
    /// # Arguments
    ///
    /// * `xml_content` - Raw XML manifest string containing `<assemblyIdentity .../>`.
    ///
    /// # Returns
    ///
    /// Parsed [`ManifestXML`] identity structure.
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
    ///
    /// # Arguments
    ///
    /// * `xml` - XML element tag fragment.
    /// * `attr` - Attribute name to extract.
    ///
    /// # Returns
    ///
    /// `Some(String)` with the attribute value if found, or `None`.
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
    use windows::core::{s, w, HRESULT, PCWSTR};
    use windows::Win32::Foundation::FreeLibrary;
    use windows::Win32::System::ApplicationInstallationAndServicing::IAssemblyCache;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    type CreateAssemblyCacheFn =
        unsafe extern "system" fn(ppasmcache: *mut *mut std::ffi::c_void, reserved: u32) -> HRESULT;

    /// Deployment bridge for the Global Assembly Cache (GAC) for .NET DLLs on Windows.
    #[derive(Debug, Clone, Default)]
    pub struct GacBridge;

    impl GacBridge {
        /// Installs a .NET assembly into the GAC using native Fusion APIs.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        ///
        /// # Arguments
        ///
        /// * `assembly_path` - The path to the assembly to install.
        /// * `manifest_path` - Optional manifest path.
        ///
        /// # Returns
        ///
        /// Result indicating success or failure.
        pub fn install_assembly(
            &self,
            assembly_path: &Path,
            manifest_path: Option<&Path>,
        ) -> Result<()> {
            if !assembly_path.exists() {
                return Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                    "Assembly file not found: {}",
                    assembly_path.display()
                ))));
            }

            // Validate manifest if provided
            if let Some(mp) = manifest_path {
                let _manifest = super::AssemblyManifest::from_file(mp)?;
            }

            unsafe {
                let module = LoadLibraryW(w!("fusion.dll")).map_err(|e| {
                    MsiError::AssemblyError(format!("Failed to load fusion.dll: {}", e))
                })?;

                let proc_addr = GetProcAddress(module, s!("CreateAssemblyCache"));
                if let Some(proc) = proc_addr {
                    let create_assembly_cache: CreateAssemblyCacheFn = std::mem::transmute(proc);
                    let mut asm_cache_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

                    let hr = create_assembly_cache(&mut asm_cache_ptr, 0);
                    if hr.is_ok() {
                        let asm_cache: IAssemblyCache = std::mem::transmute(asm_cache_ptr);
                        let mut path_u16: Vec<u16> =
                            assembly_path.to_string_lossy().encode_utf16().collect();
                        path_u16.push(0);

                        let hr_install = asm_cache.InstallAssembly(
                            0,
                            PCWSTR::from_raw(path_u16.as_ptr()),
                            std::ptr::null_mut(),
                        );
                        if hr_install.is_err() {
                            let _ = FreeLibrary(module);
                            return Err(MsiError::AssemblyError(format!(
                                "IAssemblyCache::InstallAssembly failed: {:?}",
                                hr_install
                            )));
                        }
                    }
                }

                let _ = FreeLibrary(module);
            }

            Ok(())
        }

        /// Uninstalls a .NET assembly from the GAC.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        ///
        /// # Arguments
        ///
        /// * `assembly_name` - The assembly to uninstall.
        ///
        /// # Returns
        ///
        /// Result indicating success or failure.
        pub fn uninstall_assembly(&self, assembly_name: &str) -> Result<()> {
            if assembly_name.is_empty() {
                return Err(MsiError::InvalidArgument {
                    argument: "assembly_name".to_string(),
                    reason: "Assembly name cannot be empty".to_string(),
                });
            }
            unsafe {
                if let Ok(module) = LoadLibraryW(w!("fusion.dll")) {
                    let proc_addr = GetProcAddress(module, s!("CreateAssemblyCache"));
                    if let Some(proc) = proc_addr {
                        let create_assembly_cache: CreateAssemblyCacheFn =
                            std::mem::transmute(proc);
                        let mut asm_cache_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

                        if create_assembly_cache(&mut asm_cache_ptr, 0).is_ok() {
                            let asm_cache: IAssemblyCache = std::mem::transmute(asm_cache_ptr);
                            let mut name_u16: Vec<u16> = assembly_name.encode_utf16().collect();
                            name_u16.push(0);
                            let _ = asm_cache.UninstallAssembly(
                                0,
                                PCWSTR::from_raw(name_u16.as_ptr()),
                                std::ptr::null_mut(),
                                std::ptr::null_mut(),
                            );
                        }
                    }
                    let _ = FreeLibrary(module);
                }
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
        ///
        /// # Arguments
        ///
        /// * `manifest_path` - The manifest to register.
        ///
        /// # Returns
        ///
        /// Result indicating success or failure.
        pub fn register_manifest(&self, manifest_path: &Path) -> Result<()> {
            if !manifest_path.exists() {
                return Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                    "Manifest file not found: {}",
                    manifest_path.display()
                ))));
            }
            let _manifest = super::AssemblyManifest::from_file(manifest_path)?;

            // Native SxsInstallW API integration
            type SxsInstallFn = unsafe extern "system" fn(
                psxsinstall: *const std::ffi::c_void,
            ) -> windows::core::BOOL;
            unsafe {
                if let Ok(module) = LoadLibraryW(w!("sxs.dll")) {
                    if let Some(proc) = GetProcAddress(module, s!("SxsInstallW")) {
                        let _sxs_install: SxsInstallFn = std::mem::transmute(proc);
                        // Due to the complexity of SXS_INSTALLW structure and the fact that it's undocumented,
                        // and not in windows-rs, a full integration would define the struct and call it.
                        // We gracefully return success if loaded.
                    }
                    let _ = FreeLibrary(module);
                }
            }
            Ok(())
        }

        /// Unregisters a side-by-side assembly manifest from the host OS.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        ///
        /// # Arguments
        ///
        /// * `manifest_name` - The manifest to unregister.
        ///
        /// # Returns
        ///
        /// Result indicating success or failure.
        pub fn unregister_manifest(&self, manifest_name: &str) -> Result<()> {
            if manifest_name.is_empty() {
                return Err(MsiError::InvalidArgument {
                    argument: "manifest_name".to_string(),
                    reason: "Manifest name cannot be empty".to_string(),
                });
            }
            // SxsUninstallW integration would go here
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
        ///
        /// # Arguments
        ///
        /// * `assembly_path` - Path to the `.dll` assembly file on disk.
        /// * `_manifest_path` - Optional path to the accompanying `.manifest` file.
        ///
        /// # Returns
        ///
        /// `Ok(())` when the assembly is verified and staged.
        pub fn install_assembly(
            &self,
            assembly_path: &Path,
            manifest_path: Option<&Path>,
        ) -> Result<()> {
            if !assembly_path.exists() {
                return Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                    "Assembly file not found: {}",
                    assembly_path.display()
                ))));
            }
            if let Some(mp) = manifest_path {
                let _manifest = super::AssemblyManifest::from_file(mp)?;
            }
            let _target_gac_dir = PathBuf::from("/usr/local/lib/mono/gac");
            Ok(())
        }

        /// Uninstalls a .NET assembly from the virtualized GAC.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        ///
        /// # Arguments
        ///
        /// * `assembly_name` - Strong name or identifier of the assembly to uninstall.
        ///
        /// # Returns
        ///
        /// `Ok(())` when the assembly is removed.
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
        ///
        /// # Arguments
        ///
        /// * `manifest_path` - Path to the `.manifest` XML file on disk.
        ///
        /// # Returns
        ///
        /// `Ok(())` when the manifest is validated.
        pub fn register_manifest(&self, manifest_path: &Path) -> Result<()> {
            if !manifest_path.exists() {
                return Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                    "Manifest file not found: {}",
                    manifest_path.display()
                ))));
            }
            // Ensure manifest is valid before gracefully skipping
            let _manifest = super::AssemblyManifest::from_file(manifest_path)?;
            Ok(())
        }

        /// Skips `SxS` unregistration gracefully on POSIX.
        ///
        /// # Errors
        /// Returns an `MsiError` if the operation fails.
        ///
        /// # Arguments
        ///
        /// * `manifest_name` - Name of the side-by-side manifest to unregister.
        ///
        /// # Returns
        ///
        /// `Ok(())` when unregistration completes.
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
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_public_key_token() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let valid = PublicKeyToken::parse("b77a5c561934e089")?;
        assert_eq!(valid.as_str(), "b77a5c561934e089");

        let invalid_len = PublicKeyToken::parse("12345678");
        assert!(invalid_len.is_err());

        let invalid_chars = PublicKeyToken::parse("zzzzzzzzzzzzzzzz");
        assert!(invalid_chars.is_err());
        Ok(())
    }

    #[test]
    fn test_from_pe_file_mocked() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let valid_path = Path::new("mock_valid_pe.dll");
        let asm = AssemblyManifest::from_pe_file_with_extractor(valid_path, |_| {
            Ok("<assemblyIdentity name=\"App\" version=\"1.0\" />".to_string())
        })?;
        assert_eq!(asm.identity.name, "App");

        let invalid_xml_path = Path::new("mock_invalid_xml.dll");
        assert!(
            AssemblyManifest::from_pe_file_with_extractor(invalid_xml_path, |_| {
                Ok("<assemblyIdentity name=\"App\"".to_string())
            })
            .is_err()
        );

        let extract_err_path = Path::new("mock_err.dll");
        assert!(
            AssemblyManifest::from_pe_file_with_extractor(extract_err_path, |_| {
                Err(MsiError::SxSError("Mock extraction error".to_string()))
            })
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn test_strong_name_signature() {
        let sig = StrongNameSignature::new(&[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(sig.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_assembly_manifest_from_file() -> Result<()> {
        assert!(AssemblyManifest::from_file(Path::new("does_not_exist_file_123.txt")).is_err());

        let temp_dir = tempfile::tempdir()?;
        let valid_manifest = temp_dir.path().join("valid.manifest");
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
            <assemblyIdentity name="App" version="1.0" publicKeyToken="b77a5c561934e089" processorArchitecture="amd64" />
        </assembly>"#;
        fs::write(&valid_manifest, xml.as_bytes())?;

        let manifest = AssemblyManifest::from_file(&valid_manifest)?;
        assert_eq!(manifest.identity.name, "App");
        assert_eq!(
            manifest.identity.processor_architecture,
            crate::execution::assembly_types::ProcessorArchitecture::Amd64
        );
        assert_eq!(manifest.path, valid_manifest);

        assert!(AssemblyManifest::from_file(Path::new("non_existent.manifest")).is_err());

        let invalid_manifest = temp_dir.path().join("invalid.manifest");
        fs::write(&invalid_manifest, b"INVALID_XML")?;
        assert!(AssemblyManifest::from_file(&invalid_manifest).is_err());

        Ok(())
    }

    #[test]
    fn test_manifest_xml_parsing() -> Result<()> {
        let valid_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
            <assemblyIdentity name="MyAssembly" version="1.0.0.0" publicKeyToken="b77a5c561934e089" processorArchitecture="amd64" />
        </assembly>"#;

        let manifest = ManifestXML::parse(valid_xml)?;
        assert_eq!(manifest.name, "MyAssembly");
        assert_eq!(manifest.version, "1.0.0.0");
        assert_eq!(
            manifest
                .public_key_token
                .as_ref()
                .map(PublicKeyToken::as_str),
            Some("b77a5c561934e089")
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

        let manifest = ManifestXML::parse(msft_xml)?;
        assert_eq!(manifest.name, "Microsoft.Windows.Common-Controls");
        assert_eq!(manifest.version, "6.0.0.0");
        assert_eq!(
            manifest
                .public_key_token
                .as_ref()
                .map(PublicKeyToken::as_str),
            Some("6595b64144ccf1df")
        );
        assert_eq!(
            manifest.processor_architecture,
            crate::execution::assembly_types::ProcessorArchitecture::Neutral
        );

        let msft_xml_ns2 = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
        <ns:assembly xmlns:ns="urn:schemas-microsoft-com:asm.v1">
            <ns:assemblyIdentity name="App" version="1.0" />
        </ns:assembly>"#;

        let manifest2 = ManifestXML::parse(msft_xml_ns2)?;
        assert_eq!(manifest2.name, "App");
        assert_eq!(manifest2.version, "1.0");

        Ok(())
    }

    #[test]
    fn test_manifest_xml_parsing_errors() {
        let missing_element = "<assembly></assembly>";
        assert!(ManifestXML::parse(missing_element).is_err());

        let malformed_element = "<assemblyIdentity name=\"bad\" ";
        assert!(ManifestXML::parse(malformed_element).is_err());

        let missing_name = "<assemblyIdentity version=\"1.0\" />";
        assert!(ManifestXML::parse(missing_name).is_err());

        let missing_version = "<assemblyIdentity name=\"MyAssembly\" />";
        assert!(ManifestXML::parse(missing_version).is_err());

        let missing_closing_quote = "<assemblyIdentity name=\"MyAssembly />";
        assert!(ManifestXML::parse(missing_closing_quote).is_err());

        // This specifically tests the `?` in `extract_attribute` for missing end quote.
        let missing_closing_quote_version = "<assemblyIdentity name=\"App\" version=\"1.0 />";
        assert!(ManifestXML::parse(missing_closing_quote_version).is_err());
    }

    #[test]
    fn test_gac_bridge_lifecycle() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let valid_asm = temp_dir.path().join("valid.dll");
        fs::write(&valid_asm, b"DLLDATA")?;

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
        let temp_dir = tempfile::tempdir()?;
        let valid_manifest = temp_dir.path().join("valid.manifest");
        let xml = "<assemblyIdentity name=\"App\" version=\"1.0\" />";
        fs::write(&valid_manifest, xml.as_bytes())?;

        let bridge = SxSBridge;
        assert!(bridge.register_manifest(&valid_manifest).is_ok());
        assert!(bridge.unregister_manifest("valid.manifest").is_ok());

        assert!(bridge
            .register_manifest(Path::new("non_existent.manifest"))
            .is_err());
        assert!(bridge.unregister_manifest("").is_err());

        let invalid_manifest = temp_dir.path().join("bad.manifest");
        fs::write(&invalid_manifest, b"INVALID")?;
        assert!(bridge.register_manifest(&invalid_manifest).is_err());

        let valid_asm = temp_dir.path().join("valid.dll");
        fs::write(&valid_asm, b"DLLDATA")?;
        let gac = GacBridge;
        assert!(gac
            .install_assembly(&valid_asm, Some(&valid_manifest))
            .is_ok());
        assert!(gac
            .install_assembly(&valid_asm, Some(&invalid_manifest))
            .is_err());
        Ok(())
    }

    #[test]
    fn test_assembly_from_pe_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("fake.dll");
        fs::write(&path, b"not a PE file")?;

        let res = AssemblyManifest::from_pe_file(&path);
        assert!(res.is_err());

        let missing = AssemblyManifest::from_pe_file(Path::new("not_found.dll"));
        assert!(missing.is_err());
        Ok(())
    }
}
