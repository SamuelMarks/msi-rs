//! External CUB (Validation Module) execution engine (`darice.cub`, `mergemod.cub`).

use crate::database::memory::LinkedDatabase;
use crate::database::tables::record::FieldValue;
use crate::error::Result;
use crate::execution::properties::EvaluationContext;
use crate::execution::script_engine::{ScriptEngine, ScriptLanguage, ScriptSession};
use crate::package::Package;
use crate::wix::ice::rules::{IceDiagnostic, IceDiagnosticType};
use std::path::Path;

/// External CUB (Validation Module) Execution Shim.
///
/// Ingests external `.cub` validation database files (e.g. `darice.cub`, `mergemod.cub`)
/// and executes embedded `VBScript` / `JScript` custom action evaluators using the
/// native script runner.
#[derive(Debug, Clone)]
pub struct CubValidator {
    /// Name or identifier of this validation module.
    name: String,
    /// Ingested CUB relational database.
    database: LinkedDatabase,
}

impl CubValidator {
    /// Creates a new [`CubValidator`] from an in-memory [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `name` - CUB module name.
    /// * `database` - In-memory CUB database.
    ///
    /// # Returns
    ///
    /// A new [`CubValidator`].
    #[must_use]
    pub fn from_database(name: impl Into<String>, database: LinkedDatabase) -> Self {
        Self {
            name: name.into(),
            database,
        }
    }

    /// Loads a [`CubValidator`] from raw `.cub` binary bytes.
    ///
    /// # Arguments
    ///
    /// * `name` - CUB module name.
    /// * `bytes` - Raw binary `.cub` data (CFB format).
    ///
    /// # Returns
    ///
    /// Parsed [`CubValidator`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if container parsing or database deserialization fails.
    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> Result<Self> {
        let pkg = Package::from_bytes(bytes)?;
        Ok(Self {
            name: name.into(),
            database: pkg.database().clone(),
        })
    }

    /// Loads a [`CubValidator`] from a filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the `.cub` file.
    ///
    /// # Returns
    ///
    /// Parsed [`CubValidator`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on file read, parsing, or database errors.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_path(path.as_ref())
    }

    /// Internal non-generic helper for opening a CUB validator from a path.
    ///
    /// # Arguments
    ///
    /// * `p` - The `p` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn open_path(p: &Path) -> Result<Self> {
        let name = p.file_stem().map_or_else(
            || "cub".to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let pkg = Package::open(p)?;
        Ok(Self {
            name,
            database: pkg.database().clone(),
        })
    }

    /// Returns a reference to the CUB's relational database.
    ///
    /// # Returns
    ///
    /// Borrow of the [`LinkedDatabase`].
    #[must_use]
    pub const fn database(&self) -> &LinkedDatabase {
        &self.database
    }

    /// Returns the name of this validation module.
    ///
    /// # Returns
    ///
    /// Name string slice.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the list of validation actions defined in this `.cub`.
    ///
    /// # Returns
    ///
    /// Vector of action name strings.
    #[must_use]
    pub fn action_names(&self) -> Vec<String> {
        let seq_records = self.database.get_records("_ICESequence");
        if !seq_records.is_empty() {
            let mut actions = Vec::new();
            for r in seq_records {
                if let Some(FieldValue::String(act)) = r.get(0) {
                    actions.push(act.clone());
                }
            }
            return actions;
        }

        let ca_records = self.database.get_records("CustomAction");
        let mut actions = Vec::new();
        for r in ca_records {
            if let Some(FieldValue::String(act)) = r.get(0) {
                if act.starts_with("ICE") {
                    actions.push(act.clone());
                }
            }
        }
        actions
    }

    /// Executes the CUB validation actions against the target [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `target_db` - Target [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// Vector of [`IceDiagnostic`] messages produced by the validation scripts.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if script execution fails unexpectedly.
    #[allow(clippy::too_many_lines)]
    pub fn execute(&self, target_db: &LinkedDatabase) -> Result<Vec<IceDiagnostic>> {
        let mut diagnostics = Vec::new();
        let script_db = target_db.to_script_database();
        let ca_records = self.database.get_records("CustomAction");

        let actions = self.action_names();
        let mut engine = ScriptEngine::new();

        for action_name in actions {
            let ca_row = ca_records.iter().find(
                |r| matches!(r.get(0), Some(FieldValue::String(name)) if name == &action_name),
            );

            let Some(ca) = ca_row else {
                continue;
            };

            let ca_type = match ca.get(1) {
                Some(FieldValue::Short(t)) => i32::from(*t),
                Some(FieldValue::Long(t)) => *t,
                _ => 0,
            };

            let is_jscript =
                ca_type == 5 || ca_type == 37 || ca_type == 53 || (ca_type & 0x07 == 5);
            let is_vbscript =
                ca_type == 6 || ca_type == 38 || ca_type == 54 || (ca_type & 0x07 == 6);

            if !is_jscript && !is_vbscript {
                continue;
            }

            let script_content: Option<String> =
                if ca_type == 37 || ca_type == 38 || ca_type == 53 || ca_type == 54 {
                    ca.get(3).and_then(|f| match f {
                        FieldValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                } else {
                    ca.get(2).and_then(|source_f| {
                        if let FieldValue::String(src) = source_f {
                            self.database.get_records("Binary").iter().find_map(|bin_row| {
                            if matches!(bin_row.get(0), Some(FieldValue::String(n)) if n == src) {
                                match bin_row.get(1) {
                                    Some(FieldValue::String(code)) => Some(code.clone()),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                        } else {
                            None
                        }
                    })
                };

            let Some(script) = script_content else {
                continue;
            };

            let mut session =
                ScriptSession::with_database(EvaluationContext::new(), script_db.clone());
            let lang = if is_jscript {
                ScriptLanguage::JScript
            } else {
                ScriptLanguage::VBScript
            };
            let exec_result = engine.execute(lang, &script, &mut session);

            match exec_result {
                Ok(_) => {
                    for msg in session.messages() {
                        diagnostics.push(IceDiagnostic::new(
                            action_name.clone(),
                            if msg.kind == 1 {
                                IceDiagnosticType::Error
                            } else {
                                IceDiagnosticType::Warning
                            },
                            msg.text.clone(),
                        ));
                    }
                }
                Err(err) => {
                    diagnostics.push(IceDiagnostic::new(
                        action_name.clone(),
                        IceDiagnosticType::Error,
                        format!("script evaluation failed: {err}"),
                    ));
                }
            }
        }

        Ok(diagnostics)
    }
}

impl Default for CubValidator {
    /// Creates a default empty [`CubValidator`].
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn default() -> Self {
        Self::from_database("default", LinkedDatabase::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::Record;
    use crate::package::ProductVersion;
    use crate::wix::ice::rules::{IceRegistry, IceReport, IceRule, StandardIceRule};
    use crate::wix::linker::Linker;
    use std::fmt;
    /// Dummy ICE rule for testing custom rule registration.
    struct DummyRule;

    #[allow(clippy::unnecessary_literal_bound)]
    impl IceRule for DummyRule {
        fn name(&self) -> &str {
            "ICE999"
        }

        fn description(&self) -> &str {
            "Dummy test rule"
        }

        fn execute(&self, _db: &LinkedDatabase) -> Vec<IceDiagnostic> {
            vec![IceDiagnostic::new(
                "ICE999",
                IceDiagnosticType::Info,
                "All good",
            )]
        }
    }

    /// Failing formatter writer for testing display error paths.
    struct FailingWriter;

    impl fmt::Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice_harness_and_cub_validator() {
        use std::fmt::Write;

        // 1. Test IceDiagnosticType Display
        assert_eq!(format!("{}", IceDiagnosticType::Error), "ERROR");
        assert_eq!(format!("{}", IceDiagnosticType::Warning), "WARNING");
        assert_eq!(format!("{}", IceDiagnosticType::Info), "INFO");

        // Test FailingWriter on IceDiagnostic Display
        let diag_minimal = IceDiagnostic::new("ICE01", IceDiagnosticType::Error, "Missing table");
        assert_eq!(format!("{diag_minimal}"), "ICE01: ERROR: Missing table");
        let mut fail_w = FailingWriter;
        let _ = write!(fail_w, "{diag_minimal}");

        let diag_table_only = IceDiagnostic {
            ice: "ICE02".to_string(),
            diagnostic_type: IceDiagnosticType::Error,
            description: "Table error".to_string(),
            table: Some("Feature".to_string()),
            column: None,
            row_key: None,
        };
        assert_eq!(
            format!("{diag_table_only}"),
            "ICE02: ERROR: Table error Feature"
        );

        let diag_table_col = IceDiagnostic {
            ice: "ICE02".to_string(),
            diagnostic_type: IceDiagnosticType::Error,
            description: "Table col error".to_string(),
            table: Some("Feature".to_string()),
            column: Some("Feature".to_string()),
            row_key: None,
        };
        assert_eq!(
            format!("{diag_table_col}"),
            "ICE02: ERROR: Table col error Feature Feature"
        );

        let diag_full = IceDiagnostic::new("ICE03", IceDiagnosticType::Warning, "Invalid length")
            .with_location("Component", "Component", "Comp1");
        assert_eq!(
            format!("{diag_full}"),
            "ICE03: WARNING: Invalid length Component Component Comp1"
        );

        let report = IceReport {
            ice: "ICE08".to_string(),
            is_error: true,
            message: "Duplicate GUID".to_string(),
        };
        let diag_from_rep = IceDiagnostic::from(report);
        assert_eq!(diag_from_rep.ice, "ICE08");
        assert_eq!(diag_from_rep.diagnostic_type, IceDiagnosticType::Error);
        assert_eq!(diag_from_rep.description, "Duplicate GUID");

        let rep_from_diag = IceReport::from(diag_from_rep);
        assert_eq!(rep_from_diag.ice, "ICE08");
        assert!(rep_from_diag.is_error);
        assert_eq!(rep_from_diag.message, "Duplicate GUID");

        let warn_report = IceReport {
            ice: "ICE33".to_string(),
            is_error: false,
            message: "Warning msg".to_string(),
        };
        let warn_diag = IceDiagnostic::from(warn_report);
        assert_eq!(warn_diag.diagnostic_type, IceDiagnosticType::Warning);

        // 3. Test IceRegistry with standard rules and custom rule
        let registry = IceRegistry::with_standard_rules();
        assert_eq!(format!("{registry:?}"), "IceRegistry { rule_count: 105 }");
        let rule_names = registry.rule_names();
        assert!(rule_names.contains(&"ICE01"));
        assert!(rule_names.contains(&"ICE103"));
        assert_eq!(registry.rules().len(), 105);

        let db = LinkedDatabase::default();
        let all_diags = registry.execute_all(&db);
        assert_eq!(all_diags.len(), 0);

        let par_diags = registry.execute_parallel(&db);
        assert_eq!(par_diags.len(), 0);

        let mut custom_registry = IceRegistry::default();
        custom_registry.register(DummyRule);
        assert_eq!(custom_registry.rule_names(), vec!["ICE999"]);
        assert_eq!(DummyRule.description(), "Dummy test rule");
        let custom_diags = custom_registry.execute_all(&db);
        assert_eq!(custom_diags.len(), 1);
        assert_eq!(custom_diags[0].ice, "ICE999");
        assert_eq!(custom_diags[0].diagnostic_type, IceDiagnosticType::Info);

        let custom_par_diags = custom_registry.execute_parallel(&db);
        assert_eq!(custom_par_diags.len(), 1);

        // StandardIceRule
        let std_rule = StandardIceRule::new("ICE01", "desc", Linker::validate_ice01);
        assert_eq!(std_rule.name(), "ICE01");
        assert_eq!(std_rule.description(), "desc");
        assert_eq!(std_rule.execute(&db).len(), 0);

        // 4. Test LinkedDatabase::to_script_database
        let mut test_db = LinkedDatabase::default();
        test_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("PROP1".to_string()),
                FieldValue::String("VAL1".to_string()),
                FieldValue::Short(42),
                FieldValue::Long(100_000),
                FieldValue::Stream(crate::database::tables::types::StringPoolId::new(99)),
                FieldValue::Null,
            ]),
        );
        let script_db = test_db.to_script_database();
        assert!(script_db.table_exists("Property"));
        assert_eq!(script_db.row_count("Property"), 1);

        // 5. Test CubValidator with JScript and VBScript evaluators
        let mut cub_db = LinkedDatabase::default();
        // Add _ICESequence
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(99),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NotFound".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE01_JScript".to_string()),
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE02_VBScript".to_string()),
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE03_FailScript".to_string()),
                FieldValue::Null,
                FieldValue::Short(3),
            ]),
        );
        // Custom actions: Type 37 (JScript in Target), Type 38 (VBScript in Target)
        cub_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE01_JScript".to_string()),
                FieldValue::Short(37), // JScript text
                FieldValue::Null,
                FieldValue::String(
                    "Session.Message(1, 'Target database table Property count: ' + Session.Database.RowCount('Property'));".to_string(),
                ),
            ]),
        );
        cub_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE02_VBScript".to_string()),
                FieldValue::Short(38), // VBScript text
                FieldValue::Null,
                FieldValue::String(
                    "Session.Message 2, \"VBScript check executed successfully\"".to_string(),
                ),
            ]),
        );
        cub_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE03_FailScript".to_string()),
                FieldValue::Short(37),
                FieldValue::Null,
                FieldValue::String("throw new Error('Intentional script failure');".to_string()),
            ]),
        );

        let validator = CubValidator::from_database("darice.cub", cub_db);
        assert_eq!(validator.name(), "darice.cub");
        assert_eq!(
            validator.action_names(),
            vec![
                "ICE00_NotFound".to_string(),
                "ICE01_JScript".to_string(),
                "ICE02_VBScript".to_string(),
                "ICE03_FailScript".to_string(),
            ]
        );
        assert!(!validator.database().tables.is_empty());

        let cub_diags = validator.execute(&test_db).unwrap_or_default();
        assert_eq!(cub_diags.len(), 3);
        assert_eq!(cub_diags[0].ice, "ICE01_JScript");
        assert_eq!(cub_diags[0].diagnostic_type, IceDiagnosticType::Error);
        assert!(cub_diags[0].description.contains("Property count: 1"));

        assert_eq!(cub_diags[1].ice, "ICE02_VBScript");
        assert_eq!(cub_diags[1].diagnostic_type, IceDiagnosticType::Warning);
        assert!(cub_diags[1].description.contains("VBScript check executed"));

        assert_eq!(cub_diags[2].ice, "ICE03_FailScript");
        assert_eq!(cub_diags[2].diagnostic_type, IceDiagnosticType::Error);
        assert!(cub_diags[2]
            .description
            .contains("script evaluation failed"));

        // Fallback action_names without _ICESequence and extensive CA variations
        let mut cub_db2 = LinkedDatabase::default();
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NonIceAction".to_string()),
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NullType".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NullSource".to_string()),
                FieldValue::Short(5),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE99".to_string()),
                FieldValue::Short(5), // Type 5 from Binary table
                FieldValue::String("BinaryScript".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NonScript".to_string()),
                FieldValue::Long(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_MissingInline".to_string()),
                FieldValue::Short(37),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_MissingBin".to_string()),
                FieldValue::Short(5),
                FieldValue::String("NoSuchBinary".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NullBin".to_string()),
                FieldValue::Short(5),
                FieldValue::String("NullBin".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_VbBin".to_string()),
                FieldValue::Short(6),
                FieldValue::String("VbBin".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE53".to_string()),
                FieldValue::Short(53),
                FieldValue::Null,
                FieldValue::String("Session.Message(1, '53');".to_string()),
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE54".to_string()),
                FieldValue::Short(54),
                FieldValue::Null,
                FieldValue::String("Session.Message 1, \"54\"".to_string()),
            ]),
        );
        cub_db2.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("BinaryScript".to_string()),
                FieldValue::String("Session.Message(2, 'From Binary Table');".to_string()),
            ]),
        );
        cub_db2.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("NullBin".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("VbBin".to_string()),
                FieldValue::String("Session.Message 1, \"VB from Bin\"".to_string()),
            ]),
        );
        let validator2 = CubValidator::from_database("darice2.cub", cub_db2);
        assert!(validator2.action_names().contains(&"ICE99".to_string()));
        let bin_diags = validator2.execute(&test_db).unwrap_or_default();
        assert_eq!(bin_diags.len(), 4);

        // from_bytes and open error testing
        assert!(CubValidator::from_bytes("test", &[]).is_err());
        assert!(CubValidator::open("non_existent_file_xyz.cub").is_err());

        // Real package write/read for CubValidator::from_bytes and open
        let cub_builder = Package::builder()
            .product_name("CubModule")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{99999999-9999-9999-9999-999999999999}");
        let pkg = cub_builder.build().unwrap_or_default();
        let pkg_bytes = pkg.to_bytes().unwrap_or_default();
        let val_from_bytes = CubValidator::from_bytes("pkg_cub", &pkg_bytes).unwrap_or_default();
        assert_eq!(val_from_bytes.name(), "pkg_cub");

        let temp_dir = std::env::temp_dir();
        let temp_cub = temp_dir.join(format!("test_module_{}.cub", std::process::id()));
        let _ = std::fs::write(&temp_cub, &pkg_bytes);
        let val_from_file = CubValidator::open(&temp_cub).unwrap_or_default();
        assert!(val_from_file.name().starts_with("test_module_"));
        let _ = std::fs::remove_file(&temp_cub);

        // 6. Test Linker::run_filtered_ice_validations
        let mut ice_linker = Linker::new();
        let mut bad_ice_db = LinkedDatabase::default();
        bad_ice_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        bad_ice_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C2".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Error report unsuppressed
        assert!(ice_linker
            .run_filtered_ice_validations(&bad_ice_db)
            .is_err());

        // Suppressed ICE
        ice_linker.suppress_ice("ICE08");
        assert!(ice_linker.run_filtered_ice_validations(&bad_ice_db).is_ok());

        // Selected ICE (filtering out ICE08)
        ice_linker.suppressed_ice.clear();
        ice_linker.select_ice("ICE01");
        assert!(ice_linker.run_filtered_ice_validations(&bad_ice_db).is_ok());

        // Selected ICE (including ICE08)
        ice_linker.select_ice("ICE08");
        assert!(ice_linker
            .run_filtered_ice_validations(&bad_ice_db)
            .is_err());

        // Warnings as errors
        let mut warn_linker = Linker::new();
        warn_linker.set_warnings_as_errors(true);
        let mut warn_ice_db = LinkedDatabase::default();
        warn_ice_db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("Reg1".to_string()),
                FieldValue::Short(0),
                FieldValue::String("CLSID\\MyCom".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        // Suppress warning
        warn_linker.suppress_warning("ICE33");
        assert!(warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_ok());

        // Unsuppressed warning
        warn_linker.suppressed_warnings.clear();
        assert!(warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_err());

        // Default linker with warnings_as_errors = false does not fail on warnings
        let default_warn_linker = Linker::new();
        assert!(default_warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_ok());
    }
}
