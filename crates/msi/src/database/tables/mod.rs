//! Complete MSI standard table schemas, typed rows, and serialization.
//!
//! Grounded directly in the official Windows Installer SDK table schema reference.

pub mod com;
pub mod config;
pub mod core;
pub mod file_mgmt;
pub mod posix;
pub mod record;
pub mod sequence;
pub mod types;
pub mod ui;

pub use com::*;
pub use config::*;
pub use core::*;
pub use file_mgmt::*;
pub use posix::*;
pub use record::*;
pub use sequence::*;
pub use types::*;
pub use ui::*;

use crate::database::catalogs::{DatabaseCatalog, TableSchema};
use crate::error::Result;

/// Returns all 54 standard MSI table schemas defined across the Windows Installer specification.
///
/// # Returns
///
/// Vector of [`TableSchema`].
#[must_use]
pub fn all_standard_schemas() -> Vec<TableSchema> {
    vec![
        // Core Packaging
        component_schema(),
        feature_schema(),
        feature_components_schema(),
        directory_schema(),
        file_schema(),
        file_hash_schema(),
        media_schema(),
        property_schema(),
        binary_schema(),
        font_schema(),
        patch_package_schema(),
        module_configuration_schema(),
        module_substitution_schema(),
        module_ignore_modularization_schema(),
        module_signature_schema(),
        module_components_schema(),
        module_dependency_schema(),
        module_exclusion_schema(),
        // Sequence Tables
        install_execute_sequence_schema(),
        install_ui_sequence_schema(),
        admin_execute_sequence_schema(),
        admin_ui_sequence_schema(),
        advt_execute_sequence_schema(),
        custom_action_schema(),
        // Configuration & Registration
        registry_schema(),
        remove_registry_schema(),
        environment_schema(),
        shortcut_schema(),
        icon_schema(),
        service_install_schema(),
        service_control_schema(),
        upgrade_schema(),
        condition_schema(),
        launch_condition_schema(),
        app_search_schema(),
        comp_locator_schema(),
        dr_locator_schema(),
        file_search_schema(),
        ini_locator_schema(),
        reg_locator_schema(),
        signature_schema(),
        // File Management
        create_folder_schema(),
        duplicate_file_schema(),
        move_file_schema(),
        remove_file_schema(),
        ini_file_schema(),
        remove_ini_file_schema(),
        // COM, OLE & Shell
        class_schema(),
        prog_id_schema(),
        type_lib_schema(),
        extension_schema(),
        verb_schema(),
        mime_schema(),
        app_id_schema(),
        self_reg_schema(),
        // UI & Presentation
        dialog_schema(),
        control_schema(),
        control_condition_schema(),
        control_event_schema(),
        event_mapping_schema(),
        text_style_schema(),
        radio_button_schema(),
        check_box_schema(),
        combo_box_schema(),
        list_box_schema(),
        list_view_schema(),
        billboard_schema(),
        bb_control_schema(),
        action_text_schema(),
        error_schema(),
        // POSIX Cross-Platform Extensions
        posix_file_schema(),
        posix_symlink_schema(),
        posix_daemon_schema(),
        posix_acl_schema(),
        posix_desktop_schema(),
    ]
}

/// Extends [`DatabaseCatalog`] with all standard MSI tables.
///
/// # Arguments
///
/// * `catalog` - The [`DatabaseCatalog`] to populate.
///
/// # Errors
///
/// Returns [`crate::error::Error`] if adding any schema fails.
pub fn populate_standard_tables(catalog: &mut DatabaseCatalog) -> Result<()> {
    for schema in all_standard_schemas() {
        catalog.add_table(schema)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_standard_schemas_count() {
        let schemas = all_standard_schemas();
        // 18 Core + 6 Seq + 17 Config + 6 FileMgmt + 8 COM + 15 UI + 5 POSIX = 75 tables!
        assert_eq!(schemas.len(), 75);

        let mut catalog = DatabaseCatalog::new();
        assert!(populate_standard_tables(&mut catalog).is_ok());

        // Verify each table exists in catalog
        for schema in schemas {
            assert!(catalog.get_table(&schema.name).is_some());
        }
    }

    /// Tests error propagation when duplicate schema is added.
    #[test]
    fn test_populate_standard_tables_duplicate_error() {
        let mut catalog = DatabaseCatalog::new();
        assert!(catalog.add_table(component_schema()).is_ok());
        assert!(populate_standard_tables(&mut catalog).is_err());
    }
}
