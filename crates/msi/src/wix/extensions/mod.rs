#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! Dynamic `WiX` Extension Architecture.
//!
//! This module defines the plugin system for `msi-rs` to support `WiX` extensions like
//! `WixUtilExtension`, `WixUIExtension`, and custom third-party extensions.
//!
//! # Architecture
//!
//! A `WiX` extension is a backend module capable of participating in multiple phases of
//! the MSI generation pipeline:
//!
//! 1. **Namespace Resolution**: Each extension registers one or more XML namespaces
//!    that it supports (e.g., `http://schemas.microsoft.com/wix/UtilExtension`).
//! 2. **Compiler Injection**: During compilation (Candle phase), extensions parse custom
//!    XML elements in their namespace and translate them into intermediate `WixObject`
//!    tables, symbols, or sections.
//! 3. **Linker Injection**: During linking (Light phase), extensions can inject pre-built
//!    Custom Actions (CAs), mutate the database tables directly (e.g., generating `CustomAction`
//!    and `InstallExecuteSequence` entries based on intermediate tables), and resolve
//!    extension-specific variables.
//!
//! This architecture replaces hardcoded table generation with an extensible registry.

use crate::error::Result;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;
use std::sync::Arc;

pub mod bal;
pub mod util;

pub mod complus;
pub mod firewall;
pub mod http;
pub mod iis;
pub mod netfx;
pub mod sql;

pub mod dependency;

pub mod ca;

/// Trait defining the lifecycle and hooks for a dynamic `WiX` extension.
///
/// Extensions should implement this trait to integrate custom schemas and backend
/// custom actions into the `msi-rs` compiler and linker pipelines.
pub trait WixExtension: Send + Sync {
    /// Gets the unique string identifier for this extension (e.g., `"WixUtilExtension"`).
    ///
    /// # Returns
    ///
    /// Extension identifier string slice.
    fn id(&self) -> &'static str;

    /// Gets the XML namespace URIs supported by this extension.
    ///
    /// # Returns
    ///
    /// A list of supported namespace URIs.
    fn supported_namespaces(&self) -> &[&'static str];

    /// Hook invoked during the compiler phase (Candle) when an XML element belonging
    /// to this extension's namespace is encountered.
    ///
    /// The extension should process the `node` and emit rows into intermediate tables,
    /// or emit new symbols/references into the `IntermediateSection` structure.
    ///
    /// # Arguments
    ///
    /// * `node` - The XML node belonging to the extension's namespace.
    /// * `parent_id` - The optional ID of the parent element (e.g., Component Id).
    /// * `section` - The intermediate section being built.
    /// * `tables` - The intermediate tables for the current section.
    ///
    /// # Returns
    ///
    /// Result indicating success or compilation failure.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MsiError`] if processing the XML node fails or schema validation fails.
    fn compile_node(
        &self,
        node: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()>;

    /// Hook invoked during the linker phase (Light) before finalizing the MSI database.
    ///
    /// Extensions typically use this phase to:
    /// - Read intermediate tables (e.g., `_util:User`) from the database.
    /// - Inject custom action binaries into the `Binary` table.
    /// - Schedule custom actions in the `InstallExecuteSequence` table.
    /// - Mutate standard MSI tables (like `Property` or `Component`).
    ///
    /// # Arguments
    ///
    /// * `db` - The MSI database being linked.
    ///
    /// # Returns
    ///
    /// Result indicating success or linker failure.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::MsiError`] if database mutations or table injection fail.
    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()>;
}

use std::fmt;

/// A registry managing active `WiX` extensions.
#[derive(Default, Clone)]
pub struct ExtensionRegistry {
    /// Maps an extension ID to the registered extension instance.
    extensions: HashMap<String, Arc<dyn WixExtension>>,
    /// Maps a namespace URI to the corresponding extension ID.
    namespaces: HashMap<String, String>,
}

impl fmt::Debug for ExtensionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionRegistry")
            .field("namespaces", &self.namespaces)
            .field("extensions", &self.extensions.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ExtensionRegistry {
    /// Creates a new, empty extension registry.
    ///
    /// # Returns
    ///
    /// A new `ExtensionRegistry` instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
            namespaces: HashMap::new(),
        }
    }

    /// Creates an [`ExtensionRegistry`] pre-populated with all native built-in `WiX` extensions.
    ///
    /// # Returns
    ///
    /// An [`ExtensionRegistry`] containing standard extensions:
    /// - `WixUtilExtension`
    /// - `WixFirewallExtension`
    /// - `WixBalExtension`
    /// - `WixNetFxExtension`
    /// - `WixHttpExtension`
    /// - `WixIIsExtension`
    /// - `WixSqlExtension`
    /// - `WixComPlusExtension`
    /// - `WixDependencyExtension`
    #[must_use]
    pub fn with_builtin_extensions() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(util::UtilExtension::new()));
        reg.register(Arc::new(firewall::FirewallExtension::new()));
        reg.register(Arc::new(bal::BalExtension::new()));
        reg.register(Arc::new(netfx::NetFxExtension::new()));
        reg.register(Arc::new(http::HttpExtension::new()));
        reg.register(Arc::new(iis::IisExtension::new()));
        reg.register(Arc::new(sql::SqlExtension::new()));
        reg.register(Arc::new(complus::ComPlusExtension::new()));
        reg.register(Arc::new(dependency::DependencyExtension::new()));
        reg
    }

    /// Registers a new `WiX` extension.
    ///
    /// # Arguments
    ///
    /// * `extension` - The extension instance to register.
    pub fn register(&mut self, extension: Arc<dyn WixExtension>) {
        let id = extension.id().to_string();
        for &ns in extension.supported_namespaces() {
            self.namespaces.insert(ns.to_string(), id.clone());
        }
        self.extensions.insert(id, extension);
    }

    /// Gets an extension by its namespace URI.
    ///
    /// # Arguments
    ///
    /// * `namespace_uri` - The XML namespace URI to look up.
    ///
    /// # Returns
    ///
    /// An `Option` containing the matching `WixExtension` if found.
    #[must_use]
    pub fn get_by_namespace(&self, namespace_uri: &str) -> Option<Arc<dyn WixExtension>> {
        self.namespaces
            .get(namespace_uri)
            .and_then(|id| self.extensions.get(id))
            .cloned()
    }

    /// Gets an extension by its unique identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - The extension identifier (e.g., `"WixUtilExtension"`).
    ///
    /// # Returns
    ///
    /// An `Option` containing the matching `WixExtension` if found.
    #[must_use]
    pub fn get_by_id(&self, id: &str) -> Option<Arc<dyn WixExtension>> {
        self.extensions.get(id).cloned()
    }

    /// Returns an iterator over all registered extensions.
    ///
    /// # Returns
    ///
    /// An iterator yielding `Arc<dyn WixExtension>`.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn WixExtension>> {
        self.extensions.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock extension used for validating registry behavior.
    struct MockExtension;

    impl WixExtension for MockExtension {
        fn id(&self) -> &'static str {
            "MockExtension"
        }

        fn supported_namespaces(&self) -> &[&'static str] {
            &["http://example.com/mock1", "http://example.com/mock2"]
        }

        fn compile_node(
            &self,
            _node: &XmlNode,
            _parent_id: Option<&str>,
            _section: &mut IntermediateSection,
            _tables: &mut HashMap<String, IntermediateTable>,
        ) -> Result<()> {
            Ok(())
        }

        fn link_database(&self, _db: &mut LinkedDatabase) -> Result<()> {
            Ok(())
        }
    }

    /// Tests `ExtensionRegistry` registration, lookups, iteration, cloning, default constructor, and debug formatting.
    #[test]
    fn test_extension_registry() {
        let mut registry = ExtensionRegistry::new();
        let _ = ExtensionRegistry::default();
        let mock: Arc<dyn WixExtension> = Arc::new(MockExtension);
        registry.register(Arc::clone(&mock));

        let cloned_registry = registry.clone();
        assert_eq!(
            cloned_registry.get_by_id("MockExtension").map(|e| e.id()),
            Some("MockExtension")
        );

        assert_eq!(
            registry.get_by_id("MockExtension").map(|e| e.id()),
            Some("MockExtension")
        );
        assert_eq!(
            registry
                .get_by_namespace("http://example.com/mock1")
                .map(|e| e.id()),
            Some("MockExtension")
        );
        assert_eq!(
            registry
                .get_by_namespace("http://example.com/mock2")
                .map(|e| e.id()),
            Some("MockExtension")
        );
        assert!(registry
            .get_by_namespace("http://example.com/missing")
            .is_none());

        let all_exts: Vec<_> = registry.iter().collect();
        assert_eq!(all_exts.len(), 1);
        assert_eq!(all_exts[0].id(), "MockExtension");

        // Verify trait methods on MockExtension
        let xml_node = XmlNode {
            tag: "test".to_string(),
            ..Default::default()
        };
        let mut section = IntermediateSection::new(
            crate::wix::wixobj::SectionType::Product,
            Some("P1".to_string()),
        );
        let mut tables = HashMap::new();
        mock.compile_node(&xml_node, None, &mut section, &mut tables)
            .unwrap_or_default();
        let mut db = LinkedDatabase::new().unwrap_or_default();
        mock.link_database(&mut db).unwrap_or_default();

        // Verify Debug representation
        assert!(format!("{registry:?}").contains("MockExtension"));
    }

    /// Tests that `with_builtin_extensions` registers all expected standard extensions.
    #[test]
    fn test_builtin_extensions() {
        let registry = ExtensionRegistry::with_builtin_extensions();
        assert!(registry.get_by_id("WixUtilExtension").is_some());
        assert!(registry.get_by_id("WixFirewallExtension").is_some());
        assert!(registry.get_by_id("WixBalExtension").is_some());
        assert!(registry.get_by_id("WixNetFxExtension").is_some());
        assert!(registry.get_by_id("WixHttpExtension").is_some());
        assert!(registry.get_by_id("WixIisExtension").is_some());
        assert!(registry.get_by_id("WixSqlExtension").is_some());
        assert!(registry.get_by_id("WixComPlusExtension").is_some());
        assert!(registry.get_by_id("WixDependencyExtension").is_some());
    }
}
