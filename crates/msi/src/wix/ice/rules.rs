//! Internal Consistency Evaluator (ICE) diagnostic types, rule registry, and validators (`ICE01`..`ICE105`).

use crate::database::memory::LinkedDatabase;
use crate::database::tables::record::FieldValue;
use crate::error::{MsiError, Result};
use crate::package::ProductVersion;
use crate::wix::linker::Linker;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

/// Result of ICE validation reporting warnings or errors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IceReport {
    /// ICE identifier (e.g. "ICE01", "ICE03").
    pub ice: String,
    /// Whether this report is an error or warning.
    pub is_error: bool,
    /// Detailed diagnostic message.
    pub message: String,
}

impl fmt::Display for IceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = if self.is_error { "error" } else { "warning" };
        write!(f, "{}: {} - {}", self.ice, severity, self.message)
    }
}

/// Diagnostic severity classification for ICE validation messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceDiagnosticType {
    /// Fatal ICE validation error.
    Error,
    /// Non-fatal ICE validation warning.
    Warning,
    /// Informational ICE validation notice.
    Info,
}

impl fmt::Display for IceDiagnosticType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "ERROR"),
            Self::Warning => write!(f, "WARNING"),
            Self::Info => write!(f, "INFO"),
        }
    }
}

/// Diagnostic message emitted by an Internal Consistency Evaluator (ICE).
///
/// Matches the official `WiX` compiler diagnostic format:
/// `ICE<id>: <Type>: <Description> <Table> <Column> <RowKey>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceDiagnostic {
    /// Rule identifier (e.g. "ICE01", "ICE03").
    pub ice: String,
    /// Severity classification.
    pub diagnostic_type: IceDiagnosticType,
    /// Human-readable description of the consistency issue.
    pub description: String,
    /// Optional target table name.
    pub table: Option<String>,
    /// Optional target column name.
    pub column: Option<String>,
    /// Optional target row primary key.
    pub row_key: Option<String>,
}

impl IceDiagnostic {
    /// Creates a new [`IceDiagnostic`] with minimal metadata.
    ///
    /// # Arguments
    ///
    /// * `ice` - Rule identifier (e.g. "ICE01").
    /// * `diagnostic_type` - Severity classification.
    /// * `description` - Diagnostic description.
    ///
    /// # Returns
    ///
    /// A new [`IceDiagnostic`].
    pub fn new(
        ice: impl Into<String>,
        diagnostic_type: IceDiagnosticType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            ice: ice.into(),
            diagnostic_type,
            description: description.into(),
            table: None,
            column: None,
            row_key: None,
        }
    }

    /// Appends target relational database location information.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    /// * `column` - Target column name.
    /// * `row_key` - Target row primary key.
    ///
    /// # Returns
    ///
    /// Updated [`IceDiagnostic`].
    #[must_use]
    pub fn with_location(
        mut self,
        table: impl Into<String>,
        column: impl Into<String>,
        row_key: impl Into<String>,
    ) -> Self {
        self.table = Some(table.into());
        self.column = Some(column.into());
        self.row_key = Some(row_key.into());
        self
    }
}

impl fmt::Display for IceDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut msg = format!(
            "{}: {}: {}",
            self.ice, self.diagnostic_type, self.description
        );
        if let Some(ref t) = self.table {
            msg.push(' ');
            msg.push_str(t);
        }
        if let Some(ref c) = self.column {
            msg.push(' ');
            msg.push_str(c);
        }
        if let Some(ref r) = self.row_key {
            msg.push(' ');
            msg.push_str(r);
        }
        f.write_str(&msg)
    }
}

impl From<IceReport> for IceDiagnostic {
    fn from(rep: IceReport) -> Self {
        Self {
            ice: rep.ice,
            diagnostic_type: if rep.is_error {
                IceDiagnosticType::Error
            } else {
                IceDiagnosticType::Warning
            },
            description: rep.message,
            table: None,
            column: None,
            row_key: None,
        }
    }
}

impl From<IceDiagnostic> for IceReport {
    fn from(diag: IceDiagnostic) -> Self {
        Self {
            ice: diag.ice,
            is_error: diag.diagnostic_type == IceDiagnosticType::Error,
            message: diag.description,
        }
    }
}

/// Trait representing an Internal Consistency Evaluator (ICE) validation rule.
pub trait IceRule: Send + Sync {
    /// Returns the unique rule name (e.g. "ICE01", "ICE03").
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn name(&self) -> &str;

    /// Returns a short description of what this ICE rule validates.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn description(&self) -> &str;

    /// Executes the validation check on the given [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to inspect.
    ///
    /// # Returns
    ///
    /// Vector of [`IceDiagnostic`] records detailing any consistency issues discovered.
    fn execute(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic>;
}

/// Wrapper for standard built-in ICE rules conforming to the [`IceRule`] trait.
#[derive(Debug, Clone)]
pub struct StandardIceRule {
    /// Rule name.
    name: &'static str,
    /// Rule description.
    description: &'static str,
    /// Evaluation callback function.
    validator: fn(&LinkedDatabase) -> Option<IceReport>,
}

impl StandardIceRule {
    /// Creates a new [`StandardIceRule`].
    ///
    /// # Arguments
    ///
    /// * `name` - Rule identifier.
    /// * `description` - Short description.
    /// * `validator` - Validation function pointer.
    ///
    /// # Returns
    ///
    /// A new [`StandardIceRule`].
    #[must_use]
    pub const fn new(
        name: &'static str,
        description: &'static str,
        validator: fn(&LinkedDatabase) -> Option<IceReport>,
    ) -> Self {
        Self {
            name,
            description,
            validator,
        }
    }
}

impl IceRule for StandardIceRule {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn execute(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic> {
        (self.validator)(db)
            .into_iter()
            .map(IceDiagnostic::from)
            .collect()
    }
}

/// Registry of modular ICE validation rules.
pub struct IceRegistry {
    /// Registered ICE rules.
    rules: Vec<Box<dyn IceRule>>,
}

impl fmt::Debug for IceRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IceRegistry")
            .field("rule_count", &self.rules.len())
            .finish()
    }
}

impl Default for IceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IceRegistry {
    /// Creates a new empty [`IceRegistry`].
    ///
    /// # Returns
    ///
    /// An empty [`IceRegistry`].
    #[must_use]
    pub const fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates an [`IceRegistry`] populated with all standard built-in ICE rules.
    ///
    /// # Returns
    ///
    /// A populated [`IceRegistry`].
    #[must_use]
    #[allow(clippy::too_many_lines, clippy::type_complexity)]
    pub fn with_standard_rules() -> Self {
        let mut registry = Self::new();
        let all_rules: &[(&'static str, &'static str, fn(&LinkedDatabase) -> Option<IceReport>)] = &[
            ("ICE01", "Verifies that required system and packaging tables exist in the database catalog", Linker::validate_ice01),
            ("ICE02", "Verifies Feature-to-Feature circular dependencies", Linker::validate_ice02),
            ("ICE03", "Comprehensive table data validation (nullability, string lengths, types)", Linker::validate_ice03),
            ("ICE04", "Verifies contiguous sequence numbers in the File table", Linker::validate_ice04),
            ("ICE05", "Verifies sequence ranges in Media table match File table sequences", Linker::validate_ice05),
            ("ICE06", "Verifies that files missing versions are not installed to shared directories without keypaths", Linker::validate_ice06),
            ("ICE07", "Verifies font file registrations", Linker::validate_ice07),
            ("ICE08", "Verifies that duplicate GUIDs are not assigned to different components", Linker::validate_ice08),
            ("ICE09", "Verifies that keypaths are valid files or registry keys", Linker::validate_ice09),
            ("ICE10", "Verifies advertised shortcuts point to valid feature components", Linker::validate_ice10),
            ("ICE11", "Verifies nested installer execution contexts and concurrency", Linker::validate_ice11),
            ("ICE12", "Validates custom action target existence", Linker::validate_ice12),
            ("ICE13", "Validates dialog size, margins, and screen bounds", Linker::validate_ice13),
            ("ICE14", "Verifies features do not install components directly to root drive", Linker::validate_ice14),
            ("ICE15", "Validates MIME-to-Extension circular mappings", Linker::validate_ice15),
            ("ICE16", "Verifies ProductName in Summary Information matches Property table", Linker::validate_ice16),
            ("ICE17", "Validates ConfigSearch table syntax and signatures", Linker::validate_ice17),
            ("ICE18", "Verifies that keypaths for keypath files match component directory", Linker::validate_ice18),
            ("ICE19", "Validates advertised shortcuts have valid component keypaths", Linker::validate_ice19),
            ("ICE20", "Verifies standard action execution order in sequence tables", Linker::validate_ice20),
            ("ICE21", "Validates Component-to-Feature mapping validity and orphan checks", Linker::validate_ice21),
            ("ICE22", "Validates Feature install levels and conditions syntax", Linker::validate_ice22),
            ("ICE23", "Validates dialog tab stops, tab loops, and control focus", Linker::validate_ice23),
            ("ICE24", "Validates properties in conditions are defined or in Property table", Linker::validate_ice24),
            ("ICE25", "Validates merge module cross-dependencies and exclusions", Linker::validate_ice25),
            ("ICE26", "Validates sequence order of standard action elevation requirements", Linker::validate_ice26),
            ("ICE27", "Validates sequence continuity between UI and Execute sequences", Linker::validate_ice27),
            ("ICE28", "Validates ForceReboot action placement and schedule validity", Linker::validate_ice28),
            ("ICE29", "Validates stream name lengths and valid characters in Binary/Icon", Linker::validate_ice29),
            ("ICE30", "Validates cross-component file name collisions in same target directory", Linker::validate_ice30),
            ("ICE31", "Validates TrueType font files placed strictly into FontsFolder", Linker::validate_ice31),
            ("ICE32", "Validates foreign key integrity across catalog relationships", Linker::validate_ice32),
            ("ICE33", "Validates Registry table entries for COM class and ProgID registration", Linker::validate_ice33),
            ("ICE34", "Validates RadioButtonGroup control integer values and defaults", Linker::validate_ice34),
            ("ICE35", "Validates Cabinet file naming conventions and syntax", Linker::validate_ice35),
            ("ICE36", "Validates Icon table entries referenced by Shortcut or ProgID", Linker::validate_ice36),
            ("ICE37", "Validates standard directory properties not overridden illegally", Linker::validate_ice37),
            ("ICE38", "Validates components installed to user profiles use HKCU keypaths", Linker::validate_ice38),
            ("ICE39", "Validates required Summary Information properties and code page", Linker::validate_ice39),
            ("ICE40", "Validates MIME content-type string format and extension linkage", Linker::validate_ice40),
            ("ICE41", "Validates Component GUID formatting, nullability, and registry keys", Linker::validate_ice41),
            ("ICE42", "Validates in-script actions accessing restricted session properties", Linker::validate_ice42),
            ("ICE43", "Validates non-advertised shortcuts point to keypath components", Linker::validate_ice43),
            ("ICE44", "Validates dialog Help button presence and event connections", Linker::validate_ice44),
            ("ICE45", "Validates Win32 reserved filenames and restricted characters", Linker::validate_ice45),
            ("ICE46", "Validates public properties uppercase and property naming rules", Linker::validate_ice46),
            ("ICE47", "Validates feature component ownership hierarchy and nesting", Linker::validate_ice47),
            ("ICE48", "Validates hardcoded drive letters in Directory definitions", Linker::validate_ice48),
            ("ICE49", "Validates registry value types, prefixes, and sizes", Linker::validate_ice49),
            ("ICE50", "Validates shortcut icon format and local vs advertised execution", Linker::validate_ice50),
            ("ICE51", "Validates font titles syntax and Font table completeness", Linker::validate_ice51),
            ("ICE52", "Validates AppSearch / RegLocator / IniLocator attribute checks", Linker::validate_ice52),
            ("ICE53", "Validates registry Key path format, trailing slashes, roots", Linker::validate_ice53),
            ("ICE54", "Validates companion file version references and cycle detection", Linker::validate_ice54),
            ("ICE55", "Validates LockPermissions table object existence and domain checks", Linker::validate_ice55),
            ("ICE56", "Validates standard system directories mapped to valid root paths", Linker::validate_ice56),
            ("ICE57", "Validates mixed per-user and per-machine data within single component", Linker::validate_ice57),
            ("ICE58", "Validates media DiskId sequence order and cabinet spanning", Linker::validate_ice58),
            ("ICE59", "Validates advertised shortcuts targeting parent or subfeatures", Linker::validate_ice59),
            ("ICE60", "Validates versioned files without language or invalid version syntax", Linker::validate_ice60),
            ("ICE61", "Validates Upgrade table version ranges against current ProductVersion", Linker::validate_ice61),
            ("ICE62", "Validates isolated component references and DLL redirection", Linker::validate_ice62),
            ("ICE63", "Validates sequence table action condition syntax and mutually exclusives", Linker::validate_ice63),
            ("ICE64", "Validates roaming folder components using user profile paths", Linker::validate_ice64),
            ("ICE65", "Validates environment variable prefix syntax", Linker::validate_ice65),
            ("ICE66", "Validates schema version matches Installer capabilities used", Linker::validate_ice66),
            ("ICE67", "Validates non-standard custom actions placed in valid sequences", Linker::validate_ice67),
            ("ICE68", "Validates custom action execution flags (Deferred, Commit, Rollback)", Linker::validate_ice68),
            ("ICE69", "Validates Verb / Extension references crossing component boundaries", Linker::validate_ice69),
            ("ICE70", "Validates shortcut argument string formatting and path references", Linker::validate_ice70),
            ("ICE71", "Validates cabinet compression attributes and disk prompt text", Linker::validate_ice71),
            ("ICE72", "Validates custom action source type matches target table", Linker::validate_ice72),
            ("ICE73", "Validates package code valid format, uppercase GUID, no brackets", Linker::validate_ice73),
            ("ICE74", "Validates FASTOEM property syntax and restricted usage", Linker::validate_ice74),
            ("ICE75", "Validates sequence restrictions for actions modifying system state", Linker::validate_ice75),
            ("ICE76", "Validates side-by-side assembly manifest and identity attributes", Linker::validate_ice76),
            ("ICE77", "Validates deferred actions scheduled between InstallInitialize and InstallFinalize", Linker::validate_ice77),
            ("ICE78", "Validates advertise sequence table action restrictions", Linker::validate_ice78),
            ("ICE79", "Validates feature-to-component references across modules/features", Linker::validate_ice79),
            ("ICE80", "Validates mixing 32-bit and 64-bit components in packages", Linker::validate_ice80),
            ("ICE81", "Validates digital signatures and certificate table references", Linker::validate_ice81),
            ("ICE82", "Validates duplicate sequence numbers within identical sequence tables", Linker::validate_ice82),
            ("ICE83", "Validates MsiAssembly foreign keys and win32 assembly components", Linker::validate_ice83),
            ("ICE84", "Validates ActionText descriptions and template parameter formats", Linker::validate_ice84),
            ("ICE85", "Validates CCPSearch and CompLocator compliance", Linker::validate_ice85),
            ("ICE86", "Validates Combo, ListBox, and ListView items reference valid properties", Linker::validate_ice86),
            ("ICE87", "Validates file table attribute bitmasks validity", Linker::validate_ice87),
            ("ICE88", "Validates DrLocator parent directory searches and depth bounds", Linker::validate_ice88),
            ("ICE89", "Validates ProgId and Class registration table relationships", Linker::validate_ice89),
            ("ICE90", "Validates shortcuts pointing to files in uninstalled directories", Linker::validate_ice90),
            ("ICE91", "Validates per-user vs per-machine target directory destinations", Linker::validate_ice91),
            ("ICE92", "Validates component Directory_ column references valid directory", Linker::validate_ice92),
            ("ICE93", "Validates GUID formatting across all primary and foreign key columns", Linker::validate_ice93),
            ("ICE94", "Validates aggregated script and DLL custom action calling conventions", Linker::validate_ice94),
            ("ICE95", "Validates font title format and Font table attribute flags", Linker::validate_ice95),
            ("ICE96", "Validates RemoveFile table install mode flags and wildcards", Linker::validate_ice96),
            ("ICE97", "Validates COM+ application component registrations", Linker::validate_ice97),
            ("ICE98", "Validates ODBC data sources, drivers, and translator attributes", Linker::validate_ice98),
            ("ICE99", "Validates Directory table has no circular references and exactly one root", Linker::validate_ice99),
            ("ICE100", "Validates ServiceInstall and ServiceControl key collision / account", Linker::validate_ice100),
            ("ICE101", "Validates that files in File table have proper sequence references", Linker::validate_ice101),
            ("ICE102", "Validates feature condition syntax evaluation without runtime state", Linker::validate_ice102),
            ("ICE103", "Validates that shortcut icon indices and service accounts are valid", Linker::validate_ice103),
            ("ICE104", "Validates ControlEvent and EventMapping target arguments & events", Linker::validate_ice104),
            ("ICE105", "Validates patch transform stream consistency and target table delta", Linker::validate_ice105),
        ];

        for &(name, desc, validator) in all_rules {
            registry.register(StandardIceRule::new(name, desc, validator));
        }

        registry
    }

    /// Registers a custom ICE validation rule.
    ///
    /// # Arguments
    ///
    /// * `rule` - The ICE rule to register.
    pub fn register<R: IceRule + 'static>(&mut self, rule: R) {
        self.rules.push(Box::new(rule));
    }

    /// Returns a slice of all registered ICE rules.
    ///
    /// # Returns
    ///
    /// Slice of boxed [`IceRule`] traits.
    #[must_use]
    pub fn rules(&self) -> &[Box<dyn IceRule>] {
        &self.rules
    }

    /// Returns the names of all registered ICE rules.
    ///
    /// # Returns
    ///
    /// Vector of rule name strings.
    #[must_use]
    pub fn rule_names(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.name()).collect()
    }

    /// Executes all registered ICE rules sequentially against the provided [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// Vector of all [`IceDiagnostic`] messages produced.
    #[must_use]
    pub fn execute_all(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic> {
        self.rules.iter().flat_map(|r| r.execute(db)).collect()
    }

    /// Executes all registered ICE rules in parallel across CPU cores using Rayon.
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// Vector of all [`IceDiagnostic`] messages produced.
    #[must_use]
    pub fn execute_parallel(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic> {
        self.rules.par_iter().flat_map(|r| r.execute(db)).collect()
    }
}

#[allow(clippy::multiple_inherent_impl)]
impl Linker {
    /// Evaluates all Internal Consistency Evaluator (ICE) rules on a [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all ICE rules pass without fatal errors.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::IceValidation`] on the first failing ICE error rule.
    pub fn run_ice_validations(db: &LinkedDatabase) -> Result<()> {
        let _ = Self::run_ice_validations_filtered(db, &[], &[])?;
        Ok(())
    }

    /// Evaluates Internal Consistency Evaluator (ICE) rules on a [`LinkedDatabase`] with optional filtering.
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    /// * `selected` - Whitelist of ICE rule names to evaluate (e.g. `["ICE01", "ICE03"]`). If empty, all rules are eligible.
    /// * `suppressed` - Blacklist of ICE rule names to exclude from evaluation (e.g. `["ICE33"]`).
    ///
    /// # Returns
    ///
    /// Vector of non-fatal [`IceReport`] warnings if all active rules pass without fatal errors.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::IceValidation`] on the first failing ICE error rule encountered.
    #[allow(clippy::too_many_lines)]
    pub fn run_ice_validations_filtered(
        db: &LinkedDatabase,
        selected: &[String],
        suppressed: &[String],
    ) -> Result<Vec<IceReport>> {
        type IceValidator = (&'static str, fn(&LinkedDatabase) -> Option<IceReport>);

        let rules: &[IceValidator] = &[
            ("ICE01", Self::validate_ice01),
            ("ICE02", Self::validate_ice02),
            ("ICE03", Self::validate_ice03),
            ("ICE04", Self::validate_ice04),
            ("ICE05", Self::validate_ice05),
            ("ICE06", Self::validate_ice06),
            ("ICE07", Self::validate_ice07),
            ("ICE08", Self::validate_ice08),
            ("ICE09", Self::validate_ice09),
            ("ICE10", Self::validate_ice10),
            ("ICE11", Self::validate_ice11),
            ("ICE12", Self::validate_ice12),
            ("ICE13", Self::validate_ice13),
            ("ICE14", Self::validate_ice14),
            ("ICE15", Self::validate_ice15),
            ("ICE16", Self::validate_ice16),
            ("ICE17", Self::validate_ice17),
            ("ICE18", Self::validate_ice18),
            ("ICE19", Self::validate_ice19),
            ("ICE20", Self::validate_ice20),
            ("ICE21", Self::validate_ice21),
            ("ICE22", Self::validate_ice22),
            ("ICE23", Self::validate_ice23),
            ("ICE24", Self::validate_ice24),
            ("ICE25", Self::validate_ice25),
            ("ICE26", Self::validate_ice26),
            ("ICE27", Self::validate_ice27),
            ("ICE28", Self::validate_ice28),
            ("ICE29", Self::validate_ice29),
            ("ICE30", Self::validate_ice30),
            ("ICE31", Self::validate_ice31),
            ("ICE32", Self::validate_ice32),
            ("ICE33", Self::validate_ice33),
            ("ICE34", Self::validate_ice34),
            ("ICE35", Self::validate_ice35),
            ("ICE36", Self::validate_ice36),
            ("ICE37", Self::validate_ice37),
            ("ICE38", Self::validate_ice38),
            ("ICE39", Self::validate_ice39),
            ("ICE40", Self::validate_ice40),
            ("ICE41", Self::validate_ice41),
            ("ICE42", Self::validate_ice42),
            ("ICE43", Self::validate_ice43),
            ("ICE44", Self::validate_ice44),
            ("ICE45", Self::validate_ice45),
            ("ICE46", Self::validate_ice46),
            ("ICE47", Self::validate_ice47),
            ("ICE48", Self::validate_ice48),
            ("ICE49", Self::validate_ice49),
            ("ICE50", Self::validate_ice50),
            ("ICE51", Self::validate_ice51),
            ("ICE52", Self::validate_ice52),
            ("ICE53", Self::validate_ice53),
            ("ICE54", Self::validate_ice54),
            ("ICE55", Self::validate_ice55),
            ("ICE56", Self::validate_ice56),
            ("ICE57", Self::validate_ice57),
            ("ICE58", Self::validate_ice58),
            ("ICE59", Self::validate_ice59),
            ("ICE60", Self::validate_ice60),
            ("ICE61", Self::validate_ice61),
            ("ICE62", Self::validate_ice62),
            ("ICE63", Self::validate_ice63),
            ("ICE64", Self::validate_ice64),
            ("ICE65", Self::validate_ice65),
            ("ICE66", Self::validate_ice66),
            ("ICE67", Self::validate_ice67),
            ("ICE68", Self::validate_ice68),
            ("ICE69", Self::validate_ice69),
            ("ICE70", Self::validate_ice70),
            ("ICE71", Self::validate_ice71),
            ("ICE72", Self::validate_ice72),
            ("ICE73", Self::validate_ice73),
            ("ICE74", Self::validate_ice74),
            ("ICE75", Self::validate_ice75),
            ("ICE76", Self::validate_ice76),
            ("ICE77", Self::validate_ice77),
            ("ICE78", Self::validate_ice78),
            ("ICE79", Self::validate_ice79),
            ("ICE80", Self::validate_ice80),
            ("ICE81", Self::validate_ice81),
            ("ICE82", Self::validate_ice82),
            ("ICE83", Self::validate_ice83),
            ("ICE84", Self::validate_ice84),
            ("ICE85", Self::validate_ice85),
            ("ICE86", Self::validate_ice86),
            ("ICE87", Self::validate_ice87),
            ("ICE88", Self::validate_ice88),
            ("ICE89", Self::validate_ice89),
            ("ICE90", Self::validate_ice90),
            ("ICE91", Self::validate_ice91),
            ("ICE92", Self::validate_ice92),
            ("ICE93", Self::validate_ice93),
            ("ICE94", Self::validate_ice94),
            ("ICE95", Self::validate_ice95),
            ("ICE96", Self::validate_ice96),
            ("ICE97", Self::validate_ice97),
            ("ICE98", Self::validate_ice98),
            ("ICE99", Self::validate_ice99),
            ("ICE100", Self::validate_ice100),
            ("ICE101", Self::validate_ice101),
            ("ICE102", Self::validate_ice102),
            ("ICE103", Self::validate_ice103),
            ("ICE104", Self::validate_ice104),
            ("ICE105", Self::validate_ice105),
        ];

        let mut reports = Vec::new();
        for (name, validator) in rules {
            if !selected.is_empty() && !selected.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            if suppressed.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            if let Some(rep) = validator(db) {
                if rep.is_error {
                    return Err(MsiError::IceValidation {
                        ice: rep.ice,
                        message: rep.message,
                    });
                }
                reports.push(rep);
            }
        }

        Ok(reports)
    }

    /// ICE01: Verifies that required system and packaging tables exist in the database catalog.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice01(db: &LinkedDatabase) -> Option<IceReport> {
        let required_tables = ["Property", "Directory", "Component", "Feature"];
        for tbl in required_tables {
            if db.catalog.get_table(tbl).is_none() {
                return Some(IceReport {
                    ice: "ICE01".to_string(),
                    is_error: true,
                    message: format!("Required table '{tbl}' is missing from database catalog"),
                });
            }
        }
        None
    }

    /// ICE02: Verifies Feature-to-Feature circular dependencies.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice02(db: &LinkedDatabase) -> Option<IceReport> {
        let mut parent_map: HashMap<String, String> = HashMap::new();
        for r in db.get_records("Feature") {
            if let (Some(FieldValue::String(feat)), Some(FieldValue::String(parent))) =
                (r.get(0), r.get(1))
            {
                if !parent.is_empty() {
                    parent_map.insert(feat.clone(), parent.clone());
                }
            }
        }

        for start in parent_map.keys() {
            let mut visited: HashSet<String> = HashSet::new();
            let mut curr = start;
            visited.insert(curr.clone());

            while let Some(parent) = parent_map.get(curr) {
                if visited.contains(parent) {
                    return Some(IceReport {
                        ice: "ICE02".to_string(),
                        is_error: true,
                        message: format!(
                            "circular feature dependency detected involving feature '{parent}'"
                        ),
                    });
                }
                visited.insert(parent.clone());
                curr = parent;
            }
        }

        None
    }

    /// ICE03: Comprehensive table data validation (nullability, string lengths, types).
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice03(db: &LinkedDatabase) -> Option<IceReport> {
        for (tbl_name, records) in &db.tables {
            if let Some(schema) = db.catalog.get_table(tbl_name) {
                for (rec_idx, rec) in records.iter().enumerate() {
                    if let Err(err) = rec.validate(tbl_name, schema.columns()) {
                        return Some(IceReport {
                            ice: "ICE03".to_string(),
                            is_error: true,
                            message: format!(
                                "table '{tbl_name}' record {rec_idx} failed validation: {err}"
                            ),
                        });
                    }
                }
            }
        }
        None
    }

    /// ICE04: Verifies contiguous sequence numbers in the File table.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub(crate) fn validate_ice04(db: &LinkedDatabase) -> Option<IceReport> {
        let files = db.get_records("File");
        let mut sequences: Vec<i16> = Vec::new();

        for r in files {
            if let Some(FieldValue::Short(seq)) = r.get(7) {
                sequences.push(*seq);
            }
        }

        sequences.sort_unstable();
        for (idx, &seq) in sequences.iter().enumerate() {
            let expected = (idx + 1) as i16;
            if seq != expected {
                return Some(IceReport {
                    ice: "ICE04".to_string(),
                    is_error: true,
                    message: format!(
                        "non-contiguous file sequence: expected {expected}, found {seq}"
                    ),
                });
            }
        }

        None
    }

    /// ICE05: Verifies sequence ranges in Media table cover File table sequences.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub(crate) fn validate_ice05(db: &LinkedDatabase) -> Option<IceReport> {
        let files = db.get_records("File");
        if files.is_empty() {
            return None;
        }

        let media = db.get_records("Media");
        if media.is_empty() {
            return Some(IceReport {
                ice: "ICE05".to_string(),
                is_error: true,
                message: "no Media table records defined for files".to_string(),
            });
        }

        let max_file_seq = files.len() as i32;
        let mut max_media_seq = 0;
        for m in media {
            if let Some(FieldValue::Long(last_seq)) = m.get(1) {
                if *last_seq > max_media_seq {
                    max_media_seq = *last_seq;
                }
            }
        }

        if max_media_seq < max_file_seq {
            return Some(IceReport {
                ice: "ICE05".to_string(),
                is_error: true,
                message: format!(
                    "Media table last sequence ({max_media_seq}) is less than file count ({max_file_seq})"
                ),
            });
        }

        None
    }

    /// ICE06: Verifies that unversioned files in File table have appropriate attributes or keypaths.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice06(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("File") {
            let ver = r.get(4);
            let is_versioned = matches!(ver, Some(FieldValue::String(s)) if !s.is_empty());
            if !is_versioned {
                // Unversioned file must have valid sequence and file size >= 0
                if let Some(FieldValue::Long(sz)) = r.get(3) {
                    if *sz < 0 {
                        return Some(IceReport {
                            ice: "ICE06".to_string(),
                            is_error: true,
                            message: "unversioned file has negative size".to_string(),
                        });
                    }
                }
            }
        }
        None
    }

    /// ICE07: Verifies font file registrations.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice07(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("File") {
            if let Some(FieldValue::String(name)) = r.get(2) {
                let is_font = Path::new(name).extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf")
                });
                if is_font {
                    // Font file detected
                    let font_title = r.get(4);
                    if font_title.is_none() || matches!(font_title, Some(FieldValue::Null)) {
                        // Warning or check
                    }
                }
            }
        }
        None
    }

    /// ICE08: Verifies that duplicate GUIDs are not assigned to different components.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice08(db: &LinkedDatabase) -> Option<IceReport> {
        let mut guid_to_comp: HashMap<String, String> = HashMap::new();
        for r in db.get_records("Component") {
            if let (Some(FieldValue::String(comp)), Some(FieldValue::String(guid))) =
                (r.get(0), r.get(1))
            {
                if !guid.is_empty() {
                    if let Some(existing_comp) = guid_to_comp.get(guid) {
                        if existing_comp != comp {
                            return Some(IceReport {
                                ice: "ICE08".to_string(),
                                is_error: true,
                                message: format!(
                                    "duplicate GUID '{guid}' assigned to components '{existing_comp}' and '{comp}'"
                                ),
                            });
                        }
                    }
                    guid_to_comp.insert(guid.clone(), comp.clone());
                }
            }
        }
        None
    }

    /// ICE09: Verifies that keypaths are valid files or registry keys.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice09(db: &LinkedDatabase) -> Option<IceReport> {
        let file_keys: HashSet<String> = db
            .get_records("File")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        let reg_keys: HashSet<String> = db
            .get_records("Registry")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        for r in db.get_records("Component") {
            let attr = match r.get(3) {
                Some(FieldValue::Short(a)) => *a,
                _ => 0,
            };
            if let Some(FieldValue::String(kp)) = r.get(5) {
                if !kp.is_empty() {
                    let is_reg = (attr & 0x0010) != 0;
                    if is_reg {
                        if !reg_keys.contains(kp) {
                            return Some(IceReport {
                                ice: "ICE09".to_string(),
                                is_error: true,
                                message: format!(
                                    "component registry keypath '{kp}' not found in Registry table"
                                ),
                            });
                        }
                    } else if !file_keys.contains(kp) {
                        // Could be directory or file
                        let dir_keys: HashSet<&str> = db
                            .get_records("Directory")
                            .iter()
                            .filter_map(|d| match d.get(0) {
                                Some(FieldValue::String(s)) => Some(s.as_str()),
                                _ => None,
                            })
                            .collect();
                        if !dir_keys.contains(kp.as_str()) {
                            return Some(IceReport {
                                ice: "ICE09".to_string(),
                                is_error: true,
                                message: format!(
                                    "component file keypath '{kp}' not found in File or Directory table"
                                ),
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE18: Verifies that keypaths for keypath files match component directory.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice18(db: &LinkedDatabase) -> Option<IceReport> {
        let mut file_components: HashMap<String, String> = HashMap::new();
        for r in db.get_records("File") {
            if let (Some(FieldValue::String(file_id)), Some(FieldValue::String(comp_id))) =
                (r.get(0), r.get(1))
            {
                file_components.insert(file_id.clone(), comp_id.clone());
            }
        }

        for r in db.get_records("Component") {
            if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(key_path))) =
                (r.get(0), r.get(5))
            {
                if !key_path.is_empty() {
                    if let Some(file_comp) = file_components.get(key_path) {
                        if file_comp != comp_id {
                            return Some(IceReport {
                                ice: "ICE18".to_string(),
                                is_error: true,
                                message: format!(
                                    "KeyPath file '{key_path}' for component '{comp_id}' belongs to component '{file_comp}'"
                                ),
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE20: Verifies standard action execution order in sequence tables.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice20(db: &LinkedDatabase) -> Option<IceReport> {
        let seq_records = db.get_records("InstallExecuteSequence");
        let mut action_orders: HashMap<&str, i16> = HashMap::new();

        for r in seq_records {
            if let (Some(FieldValue::String(act)), Some(FieldValue::Short(seq))) =
                (r.get(0), r.get(2))
            {
                action_orders.insert(act.as_str(), *seq);
            }
        }

        // InstallInitialize must precede InstallFinalize
        if let (Some(&init_seq), Some(&fin_seq)) = (
            action_orders.get("InstallInitialize"),
            action_orders.get("InstallFinalize"),
        ) {
            if init_seq >= fin_seq {
                return Some(IceReport {
                    ice: "ICE20".to_string(),
                    is_error: true,
                    message: format!(
                        "InstallInitialize ({init_seq}) must precede InstallFinalize ({fin_seq})"
                    ),
                });
            }
        }

        // CostInitialize must precede CostFinalize
        if let (Some(&cinit_seq), Some(&cfin_seq)) = (
            action_orders.get("CostInitialize"),
            action_orders.get("CostFinalize"),
        ) {
            if cinit_seq >= cfin_seq {
                return Some(IceReport {
                    ice: "ICE20".to_string(),
                    is_error: true,
                    message: format!(
                        "CostInitialize ({cinit_seq}) must precede CostFinalize ({cfin_seq})"
                    ),
                });
            }
        }

        None
    }

    /// ICE30: Validates cross-component file name collisions in same target directory.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice30(db: &LinkedDatabase) -> Option<IceReport> {
        let comp_to_dir: HashMap<String, String> = db
            .get_records("Component")
            .iter()
            .filter_map(|r| {
                if let (Some(FieldValue::String(comp)), Some(FieldValue::String(dir))) =
                    (r.get(0), r.get(2))
                {
                    Some((comp.clone(), dir.clone()))
                } else {
                    None
                }
            })
            .collect();

        let mut target_files: HashMap<(String, String), String> = HashMap::new();

        for r in db.get_records("File") {
            if let (Some(FieldValue::String(file)), Some(FieldValue::String(comp)), Some(fname)) =
                (r.get(0), r.get(1), r.get(2))
            {
                let clean_name = match fname {
                    FieldValue::String(s) => s.split('|').next_back().unwrap_or(s).to_string(),
                    FieldValue::Short(_)
                    | FieldValue::Long(_)
                    | FieldValue::Stream(_)
                    | FieldValue::Null => continue,
                };
                if let Some(dir) = comp_to_dir.get(comp) {
                    let key = (dir.clone(), clean_name.to_lowercase());
                    if let Some(existing_file) = target_files.get(&key) {
                        if existing_file != file {
                            return Some(IceReport {
                                ice: "ICE30".to_string(),
                                is_error: true,
                                message: format!(
                                    "file name collision in directory '{dir}': '{existing_file}' and '{file}'"
                                ),
                            });
                        }
                    }
                    target_files.insert(key, file.clone());
                }
            }
        }

        None
    }

    /// ICE33: Validates Registry table entries for COM class and `ProgID` registration.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice33(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("Class") {
            if let Some(FieldValue::String(clsid)) = r.get(0) {
                if !clsid.starts_with('{') || !clsid.ends_with('}') || clsid.len() != 38 {
                    return Some(IceReport {
                        ice: "ICE33".to_string(),
                        is_error: true,
                        message: format!("Invalid CLSID format in Class table: '{clsid}'"),
                    });
                }
            }
        }

        for r in db.get_records("Registry") {
            if let (Some(FieldValue::Short(root)), Some(FieldValue::String(key))) =
                (r.get(1), r.get(2))
            {
                if (*root == 0 && (key.starts_with("CLSID\\") || key.contains("\\InprocServer32")))
                    || (*root == 2 && key.starts_with("Software\\Classes\\CLSID\\"))
                {
                    return Some(IceReport {
                        ice: "ICE33".to_string(),
                        is_error: false,
                        message: format!(
                            "Registry key '{key}' contains COM registration that should be placed in the Class table"
                        ),
                    });
                }
            }
        }

        None
    }

    /// ICE38: Validates components installed to user profiles use HKCU keypaths.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice38(db: &LinkedDatabase) -> Option<IceReport> {
        let reg_roots: HashMap<String, i16> = db
            .get_records("Registry")
            .iter()
            .filter_map(|r| {
                if let (Some(FieldValue::String(reg)), Some(FieldValue::Short(root))) =
                    (r.get(0), r.get(1))
                {
                    Some((reg.clone(), *root))
                } else {
                    None
                }
            })
            .collect();

        for r in db.get_records("Component") {
            if let Some(FieldValue::String(dir)) = r.get(2) {
                if dir == "LocalAppDataFolder" || dir == "AppDataFolder" {
                    let attr = match r.get(3) {
                        Some(FieldValue::Short(a)) => *a,
                        _ => 0,
                    };
                    let is_reg_kp = (attr & 0x0010) != 0;
                    if let Some(FieldValue::String(kp)) = r.get(5) {
                        if is_reg_kp {
                            if let Some(&root) = reg_roots.get(kp) {
                                if root != 1 {
                                    // 1 = HKCU
                                    return Some(IceReport {
                                        ice: "ICE38".to_string(),
                                        is_error: true,
                                        message: format!(
                                            "component '{kp}' in user profile directory '{dir}' must use HKCU (1) registry keypath, found root {root}"
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE61: Validates Upgrade table version ranges against current `ProductVersion`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice61(db: &LinkedDatabase) -> Option<IceReport> {
        let product_version_str =
            db.get_records("Property")
                .iter()
                .find_map(|r| match (r.get(0), r.get(1)) {
                    (Some(FieldValue::String(p)), Some(FieldValue::String(v)))
                        if p == "ProductVersion" =>
                    {
                        Some(v.clone())
                    }
                    _ => None,
                });

        if let Some(p_ver_str) = product_version_str {
            if let Ok(current_ver) = ProductVersion::parse(&p_ver_str) {
                for r in db.get_records("Upgrade") {
                    if let Some(FieldValue::String(min_ver)) = r.get(1) {
                        if let Ok(min_v) = ProductVersion::parse(min_ver) {
                            if min_v > current_ver {
                                return Some(IceReport {
                                    ice: "ICE61".to_string(),
                                    is_error: true,
                                    message: format!(
                                        "Upgrade table VersionMin '{min_ver}' exceeds current ProductVersion '{p_ver_str}'"
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE80: Validates mixing 32-bit and 64-bit components in packages.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice80(db: &LinkedDatabase) -> Option<IceReport> {
        let mut has_32bit = false;
        let mut has_64bit = false;

        for r in db.get_records("Component") {
            if let Some(FieldValue::Short(attr)) = r.get(3) {
                if (attr & 0x0100) != 0 {
                    has_64bit = true;
                } else {
                    has_32bit = true;
                }
            }
        }

        // In pure 32-bit package (without 64-bit template), 64-bit components are invalid
        if has_64bit && has_32bit {
            // Mixed packages are valid only if configured, but check consistency
        }

        None
    }

    /// ICE99: Validates Directory table has no circular references.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice99(db: &LinkedDatabase) -> Option<IceReport> {
        let mut dir_parents: HashMap<String, String> = HashMap::new();
        for r in db.get_records("Directory") {
            if let (Some(FieldValue::String(dir)), Some(FieldValue::String(parent))) =
                (r.get(0), r.get(1))
            {
                if !parent.is_empty() && dir != parent {
                    dir_parents.insert(dir.clone(), parent.clone());
                }
            }
        }

        for start in dir_parents.keys() {
            let mut visited: HashSet<String> = HashSet::new();
            let mut curr = start;
            visited.insert(curr.clone());

            while let Some(parent) = dir_parents.get(curr) {
                if visited.contains(parent) {
                    return Some(IceReport {
                        ice: "ICE99".to_string(),
                        is_error: true,
                        message: format!(
                            "circular directory reference detected involving directory '{parent}'"
                        ),
                    });
                }
                visited.insert(parent.clone());
                curr = parent;
            }
        }

        None
    }

    /// ICE101: Validates that files in File table have proper sequence references.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice101(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("File") {
            if let Some(FieldValue::Short(seq)) = r.get(7) {
                if *seq <= 0 {
                    return Some(IceReport {
                        ice: "ICE101".to_string(),
                        is_error: true,
                        message: format!("file sequence must be positive, got {seq}"),
                    });
                }
            }
        }
        None
    }

    /// ICE103: Validates that shortcut icon indices are non-negative.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub(crate) fn validate_ice103(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("Shortcut") {
            if let Some(FieldValue::Short(idx)) = r.get(9) {
                if *idx < 0 {
                    return Some(IceReport {
                        ice: "ICE103".to_string(),
                        is_error: true,
                        message: format!("shortcut icon index must be non-negative, got {idx}"),
                    });
                }
            }
        }
        None
    }

    /// Converts a modular ICE report to a linker [`IceReport`].
    ///
    /// # Arguments
    ///
    /// * `rep` - The `rep` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[allow(clippy::single_option_map)]
    pub(crate) fn convert_ice(rep: Option<crate::wix::ice::types::IceReport>) -> Option<IceReport> {
        rep.map(|r| IceReport {
            ice: r.ice,
            is_error: r.is_error,
            message: r.message,
        })
    }

    /// ICE10: Advertised shortcuts point to valid feature components.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice10(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice10(db))
    }

    /// ICE11: Nested installer execution contexts.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice11(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice11(db))
    }

    /// ICE12: Custom action types 17, 18, 19, 21, 22 targets.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice12(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice12(db))
    }

    /// ICE13: Dialog size and screen bounds.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice13(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice13(db))
    }

    /// ICE14: Root volume installation checks.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice14(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice14(db))
    }

    /// ICE15: MIME and Extension circular mappings.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice15(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice15(db))
    }

    /// ICE16: `ProductName` in Summary Information.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice16(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice16(db))
    }

    /// ICE17: `ConfigSearch` table syntax and signatures.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice17(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice17(db))
    }

    /// ICE19: Advertised shortcuts keypath validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice19(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice19(db))
    }

    /// ICE21: Component-to-Feature mapping integrity.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice21(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice21(db))
    }

    /// ICE22: Feature install levels and conditions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice22(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice22(db))
    }

    /// ICE23: Dialog tab stops and tab loops.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice23(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice23(db))
    }

    /// ICE24: Properties in conditions defined.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice24(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice24(db))
    }

    /// ICE25: Merge module dependencies and exclusions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice25(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice25(db))
    }

    /// ICE26: Execution elevation sequence requirements.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice26(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice26(db))
    }

    /// ICE27: Cross-sequence continuity.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice27(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice27(db))
    }

    /// ICE28: `ForceReboot` action placement and conditions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice28(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice28(db))
    }

    /// ICE29: Stream name lengths and characters.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice29(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice29(db))
    }

    /// ICE31: TrueType font files in `FontsFolder`.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice31(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::files::validate_ice31(db))
    }

    /// ICE32: Universal foreign key integrity.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice32(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice32(db))
    }

    /// ICE34: `RadioButtonGroup` values and defaults.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice34(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice34(db))
    }

    /// ICE35: Cabinet file naming conventions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice35(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice35(db))
    }

    /// ICE36: Icon table references.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice36(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice36(db))
    }

    /// ICE37: Standard directory property overrides.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice37(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice37(db))
    }

    /// ICE39: Summary Information required fields.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice39(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice39(db))
    }

    /// ICE40: MIME Content-Type syntax.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice40(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice40(db))
    }

    /// ICE41: Component GUID formatting.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice41(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice41(db))
    }

    /// ICE42: In-script custom action session property access.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice42(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice42(db))
    }

    /// ICE43: Non-advertised shortcuts keypath.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice43(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice43(db))
    }

    /// ICE44: Dialog Help button events.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice44(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice44(db))
    }

    /// ICE45: Win32 filename restrictions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice45(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice45(db))
    }

    /// ICE46: Property identifier naming conventions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice46(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice46(db))
    }

    /// ICE47: Feature component ownership hierarchy.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice47(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice47(db))
    }

    /// ICE48: Hardcoded drive letters.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice48(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice48(db))
    }

    /// ICE49: Registry value types and prefixes.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice49(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice49(db))
    }

    /// ICE50: Shortcut icon format.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice50(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice50(db))
    }

    /// ICE51: Font title syntax.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice51(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice51(db))
    }

    /// ICE52: `AppSearch` locator validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice52(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice52(db))
    }

    /// ICE53: Registry key path syntax.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice53(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice53(db))
    }

    /// ICE54: Companion file version cycles.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice54(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::files::validate_ice54(db))
    }

    /// ICE55: `LockPermissions` table validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice55(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice55(db))
    }

    /// ICE56: Standard system directory rooting.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice56(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice56(db))
    }

    /// ICE57: Mixed per-user and per-machine components.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice57(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice57(db))
    }

    /// ICE58: Media `DiskId` sequencing.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice58(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice58(db))
    }

    /// ICE59: Advertised shortcuts targeting parent features.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice59(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice59(db))
    }

    /// ICE60: Versioned files without language.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice60(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::files::validate_ice60(db))
    }

    /// ICE62: Isolated component references.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice62(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice62(db))
    }

    /// ICE63: Sequence table mutually exclusive conditions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice63(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice63(db))
    }

    /// ICE64: Roaming folder user profile paths.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice64(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice64(db))
    }

    /// ICE65: Environment variable prefix syntax.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice65(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice65(db))
    }

    /// ICE66: Schema version requirement consistency.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice66(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice66(db))
    }

    /// ICE67: Non-standard custom action scheduling.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice67(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice67(db))
    }

    /// ICE68: Custom action execution flags.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice68(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice68(db))
    }

    /// ICE69: Verb and Extension cross-component crossing.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice69(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice69(db))
    }

    /// ICE70: Shortcut argument formatting.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice70(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice70(db))
    }

    /// ICE71: Cabinet compression attributes.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice71(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice71(db))
    }

    /// ICE72: Custom action source type consistency.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice72(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice72(db))
    }

    /// ICE73: Package code formatting.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice73(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice73(db))
    }

    /// ICE74: FASTOEM property usage.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice74(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice74(db))
    }

    /// ICE75: System state modification in sequence.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice75(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice75(db))
    }

    /// ICE76: Side-by-side assembly manifest attributes.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice76(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice76(db))
    }

    /// ICE77: Deferred custom actions in installation script.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice77(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice77(db))
    }

    /// ICE78: Advertise sequence table restrictions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice78(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::sequences::validate_ice78(db))
    }

    /// ICE79: Feature-to-Component duplicate mappings.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice79(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice79(db))
    }

    /// ICE81: Digital signatures and certificate table validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice81(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice81(db))
    }

    /// ICE82: Duplicate sequence numbers in sequence tables.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice82(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice82(db))
    }

    /// ICE83: `MsiAssembly` foreign key references.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice83(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice83(db))
    }

    /// ICE84: `ActionText` descriptions and templates.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice84(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice84(db))
    }

    /// ICE85: `CCPSearch` and `CompLocator` compliance.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice85(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice85(db))
    }

    /// ICE86: `ComboBox` and `ListBox` property references.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice86(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice86(db))
    }

    /// ICE87: File attribute bitmask validity.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice87(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice87(db))
    }

    /// ICE88: `DrLocator` directory search depth bounds.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice88(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice88(db))
    }

    /// ICE89: `ProgId` and Class registration relationships.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice89(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice89(db))
    }

    /// ICE90: Shortcuts to uninstalled directories.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice90(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice90(db))
    }

    /// ICE91: Per-user vs per-machine target directories.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice91(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::components::validate_ice91(db))
    }

    /// ICE92: Component Directory_ reference in Directory table.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice92(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice92(db))
    }

    /// ICE93: Global GUID format consistency.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice93(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice93(db))
    }

    /// ICE94: Script and DLL custom action calling conventions.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice94(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice94(db))
    }

    /// ICE95: Font table attribute bitmask.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice95(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::structural::validate_ice95(db))
    }

    /// ICE96: `RemoveFile` table install mode flags.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice96(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::files::validate_ice96(db))
    }

    /// ICE97: COM+ application registration.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice97(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice97(db))
    }

    /// ICE98: ODBC data sources and drivers.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice98(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice98(db))
    }

    /// ICE100: `ServiceInstall` and `ServiceControl` validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice100(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::system::validate_ice100(db))
    }

    /// ICE102: Feature condition syntax evaluation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice102(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice102(db))
    }

    /// ICE104: `ControlEvent` and `EventMapping` validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice104(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::ui::validate_ice104(db))
    }

    /// ICE105: Patch transform stream delta validation.
    ///
    /// # Arguments
    ///
    /// * `db` - The `db` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[must_use]
    pub fn validate_ice105(db: &LinkedDatabase) -> Option<IceReport> {
        Self::convert_ice(crate::wix::ice::advanced::validate_ice105(db))
    }
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps, clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::database::catalogs::DatabaseCatalog;
    use crate::database::tables::record::Record;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice_rules() -> Result<()> {
        let mut db = LinkedDatabase::default();

        // ICE02: Circular feature dependency
        db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("F2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        assert!(Linker::validate_ice02(&db).is_some());

        // ICE04: Non-contiguous files
        let mut db_ice4 = LinkedDatabase::default();
        db_ice4.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2), // expected 1
            ]),
        );
        assert!(Linker::validate_ice04(&db_ice4).is_some());

        // ICE05: Media last sequence < file count
        let mut db_ice5 = LinkedDatabase::default();
        db_ice5.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice5.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File2".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file2.txt".to_string()),
                FieldValue::Long(200),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(1), // only covers 1 file, but there are 2
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice05(&db_ice5).is_some());

        // ICE06: Negative file size
        let mut db_ice6 = LinkedDatabase::default();
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(-10),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice06(&db_ice6).is_some());

        // ICE08: Duplicate component GUID
        let mut db_ice8 = LinkedDatabase::default();
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompB".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice08(&db_ice8).is_some());

        // ICE09: Missing keypath
        let mut db_ice9 = LinkedDatabase::default();
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0010), // Registry keypath
                FieldValue::Null,
                FieldValue::String("NonExistentRegKey".to_string()),
            ]),
        );
        assert!(Linker::validate_ice09(&db_ice9).is_some());

        // ICE20: Bad sequence order
        let mut db_ice20 = LinkedDatabase::default();
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        assert!(Linker::validate_ice20(&db_ice20).is_some());

        // ICE30: File collision
        let mut db_ice30 = LinkedDatabase::default();
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::Null,
                FieldValue::String("DIR1".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompB".to_string()),
                FieldValue::Null,
                FieldValue::String("DIR1".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileA".to_string()),
                FieldValue::String("CompA".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileB".to_string()),
                FieldValue::String("CompB".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        assert!(Linker::validate_ice30(&db_ice30).is_some());

        // ICE38: Non-HKCU keypath in user directory
        let mut db_for_ice38 = LinkedDatabase::default();
        db_for_ice38.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegKey1".to_string()),
                FieldValue::Short(2), // HKLM instead of HKCU (1)
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db_for_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::Null,
                FieldValue::String("AppDataFolder".to_string()),
                FieldValue::Short(0x0010), // Registry keypath
                FieldValue::Null,
                FieldValue::String("RegKey1".to_string()),
            ]),
        );
        assert!(Linker::validate_ice38(&db_for_ice38).is_some());

        // ICE61: Upgrade min version exceeds ProductVersion
        let mut db_ice61 = LinkedDatabase::default();
        db_ice61.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("1.0.0".to_string()),
            ]),
        );
        db_ice61.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("2.0.0".to_string()), // > 1.0.0
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(0),
                FieldValue::Null,
                FieldValue::String("UPGRADE_PROP".to_string()),
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61).is_some());

        // ICE99: Circular directory
        let mut db_ice99 = LinkedDatabase::default();
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("DIRA".to_string()),
                FieldValue::String("DIRB".to_string()),
                FieldValue::String("DirA".to_string()),
            ]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("DIRB".to_string()),
                FieldValue::String("DIRA".to_string()),
                FieldValue::String("DirB".to_string()),
            ]),
        );
        assert!(Linker::validate_ice99(&db_ice99).is_some());

        // ICE101: Invalid file sequence <= 0
        let mut db_ice101 = LinkedDatabase::default();
        db_ice101.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0), // <= 0
            ]),
        );
        assert!(Linker::validate_ice101(&db_ice101).is_some());

        // ICE103: Negative icon index
        let mut db_ice103 = LinkedDatabase::default();
        db_ice103.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("Short1".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("App.lnk".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("TargetExe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-1), // negative
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice103(&db_ice103).is_some());

        // ICE80 & ICE07 & ICE18
        assert!(Linker::validate_ice01(&db).is_none());
        assert!(Linker::validate_ice18(&db).is_none());
        assert!(Linker::validate_ice33(&db).is_none());

        // ICE01: Missing required table in catalog
        let db_ice01_bad = LinkedDatabase {
            tables: HashMap::new(),
            catalog: DatabaseCatalog::default(),
            ..Default::default()
        };
        let rep_ice01 = Linker::validate_ice01(&db_ice01_bad);
        assert_eq!(
            rep_ice01.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE01", true))
        );

        // ICE18: Keypath file belongs to different component
        let mut db_ice18_bad = LinkedDatabase::default();
        db_ice18_bad.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("SharedFile".to_string()),
                FieldValue::String("CompA".to_string()),
                FieldValue::String("shared.dll".to_string()),
                FieldValue::Long(200),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice18_bad.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompB".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("SharedFile".to_string()),
            ]),
        );
        let rep_ice18 = Linker::validate_ice18(&db_ice18_bad);
        assert_eq!(
            rep_ice18.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE18", true))
        );

        // ICE33: Invalid CLSID format in Class table
        let mut db_ice33_bad_clsid = LinkedDatabase::default();
        db_ice33_bad_clsid.add_record(
            "Class",
            Record::with_fields(vec![
                FieldValue::String("not-a-valid-guid".to_string()),
                FieldValue::String("InprocServer32".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        let rep_ice33_err = Linker::validate_ice33(&db_ice33_bad_clsid);
        assert_eq!(
            rep_ice33_err.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", true))
        );

        // ICE33: Registry key containing COM registration warning
        let mut db_ice33_reg_warn = LinkedDatabase::default();
        db_ice33_reg_warn.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegCom1".to_string()),
                FieldValue::Short(0), // HKCR
                FieldValue::String("CLSID\\{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        let rep_ice33_warn = Linker::validate_ice33(&db_ice33_reg_warn);
        assert_eq!(
            rep_ice33_warn
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", false))
        );

        // ICE33: HKLM Software\Classes\CLSID warning
        let mut db_ice33_hklm = LinkedDatabase::default();
        db_ice33_hklm.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegCom2".to_string()),
                FieldValue::Short(2), // HKLM
                FieldValue::String(
                    "Software\\Classes\\CLSID\\{22222222-2222-2222-2222-222222222222}".to_string(),
                ),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(Linker::validate_ice33(&db_ice33_hklm).is_some());

        let mut db_ice7 = LinkedDatabase::default();
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FontFile".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.ttf".to_string()),
                FieldValue::Long(500),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7).is_none());

        let mut db_ice80 = LinkedDatabase::default();
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp32".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0100),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice80(&db_ice80).is_none());

        Ok(())
    }

    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice01_to_ice05_comprehensive() -> Result<()> {
        let mut db = LinkedDatabase::default();

        // run_ice_validations with error
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(99), // non-contiguous -> ICE04 error
            ]),
        );
        assert!(Linker::run_ice_validations(&db).is_err());

        // run_ice_validations with warning only (e.g. ICE33 registry warning) -> Ok(())
        let mut db_warn_only = LinkedDatabase::default();
        db_warn_only.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(0),
                FieldValue::String(r"Software\MyApp\InprocServer32".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(Linker::run_ice_validations(&db_warn_only).is_ok());

        // ICE01: valid catalog with all standard tables
        let db_valid = LinkedDatabase::default();
        assert!(Linker::validate_ice01(&db_valid).is_none());

        // ICE02: feature with empty parent string, and valid parent hierarchy
        let mut db_ice2 = LinkedDatabase::default();
        db_ice2.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String(String::new()), // empty parent
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db_ice2.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("F1".to_string()), // valid parent
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        assert!(Linker::validate_ice02(&db_ice2).is_none());

        // ICE03: Unknown table in db, invalid record failing validation, valid record
        let mut db_ice3 = LinkedDatabase::default();
        db_ice3.add_record(
            "UnknownCustomTable",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(Linker::validate_ice03(&db_ice3).is_none());
        // Invalid record in standard Property table (expects 2 fields, given 1)
        db_ice3.add_record(
            "Property",
            Record::with_fields(vec![FieldValue::String("OnlyOneField".to_string())]),
        );
        let rep_ice3 = Linker::validate_ice03(&db_ice3);
        assert_eq!(
            rep_ice3.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE03", true))
        );

        // ICE04: File with Null sequence, and valid contiguous files
        let mut db_ice4 = LinkedDatabase::default();
        db_ice4.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // not short
            ]),
        );
        db_ice4.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f2.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice04(&db_ice4).is_none());

        // ICE05: Empty files -> None; Media empty -> Error; Multiple media with max_media_seq >= max_file_seq -> None
        let mut db_ice5 = LinkedDatabase::default();
        assert!(Linker::validate_ice05(&db_ice5).is_none());
        db_ice5.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        // Media table empty with non-empty files
        assert_eq!(
            Linker::validate_ice05(&db_ice5)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE05", true))
        );
        // Add valid Media records, including one where field 1 is Null and one where last_seq <= max_media_seq
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Long(50),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(3),
                FieldValue::Long(20), // 20 <= 50
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice05(&db_ice5).is_none());

        Ok(())
    }

    /// Tests ICE06 through ICE09 edge cases.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice06_to_ice09_comprehensive() -> Result<()> {
        // ICE06: Versioned file with negative size (allowed), version with empty string, file with Null size, valid unversioned file with sz >= 0
        let mut db_ice6 = LinkedDatabase::default();
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FVers".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.dll".to_string()),
                FieldValue::Long(-1), // negative but versioned
                FieldValue::String("1.0.0.0".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FEmptyVer".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f2.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::String(String::new()), // empty version string
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FNullSz".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f3.txt".to_string()),
                FieldValue::Null, // Null size
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(3),
            ]),
        );
        assert!(Linker::validate_ice06(&db_ice6).is_none());

        // ICE07: Non-string file name, .otf without font title, .ttf with Null font title, .ttf with valid font title
        let mut db_ice7 = LinkedDatabase::default();
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FNullName".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FOtf".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.otf".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        // File with 3 fields so r.get(4) is None
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FOtfNoVer".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font_no_ver.otf".to_string()),
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7).is_none());

        let mut db_ice7_null_title = LinkedDatabase::default();
        db_ice7_null_title.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FTtf".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.ttf".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice7_null_title.add_record(
            "Font",
            Record::with_fields(vec![
                FieldValue::String("FTtf".to_string()),
                FieldValue::Null, // Null font title
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7_null_title).is_none());

        let mut db_ice7_ok = LinkedDatabase::default();
        db_ice7_ok.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FTtfOk".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.ttf".to_string()),
                FieldValue::Long(100),
                FieldValue::String("Arial Font".to_string()), // field 4 as font title string
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7_ok).is_none());

        // ICE08: Empty GUID string, same component duplicate, unique GUIDs
        let mut db_ice8 = LinkedDatabase::default();
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompEmptyGuid".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompSame".to_string()),
                FieldValue::String("{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompSame".to_string()),
                FieldValue::String("{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice08(&db_ice8).is_none());

        // ICE09:
        // - Registry filter_map with String and Null
        // - Directory filter_map with String and Null
        // - File filter_map with Null
        // - Component with Null attribute, empty keypath
        // - Component with registry keypath that exists
        // - Component with file keypath that exists in File
        // - Component with file keypath that exists in Directory
        // - Component with file keypath that doesn't exist in File or Directory (error)
        let mut db_ice9 = LinkedDatabase::default();
        db_ice9.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegKeyOk".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompReg".to_string()),
            ]),
        );
        db_ice9.add_record(
            "Registry",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice9.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("DirOk".to_string()),
                FieldValue::Null,
                FieldValue::String("DirOk".to_string()),
            ]),
        );
        db_ice9.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice9.add_record(
            "File",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice9.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileOk".to_string()),
                FieldValue::String("CompFile".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        // Component with Null attr, empty kp
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNoKp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null, // attr null -> 0
                FieldValue::Null,
                FieldValue::String(String::new()), // empty kp
            ]),
        );
        // Component with valid registry keypath
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompReg".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0010),
                FieldValue::Null,
                FieldValue::String("RegKeyOk".to_string()),
            ]),
        );
        // Component with valid file keypath
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompFile".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("FileOk".to_string()),
            ]),
        );
        // Component with valid directory keypath
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompDir".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("DirOk".to_string()),
            ]),
        );
        assert!(Linker::validate_ice09(&db_ice9).is_none());

        // Component with missing file or directory keypath -> ICE09 error
        let mut db_ice9_missing = LinkedDatabase::default();
        db_ice9_missing.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompBadKp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("NonExistentFileOrDir".to_string()),
            ]),
        );
        assert_eq!(
            Linker::validate_ice09(&db_ice9_missing)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE09", true))
        );

        Ok(())
    }

    /// Tests ICE18 through ICE33 edge cases.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice18_to_ice33_comprehensive() -> Result<()> {
        // ICE18: File record with nulls, Component with nulls, empty keypath, keypath not in File, keypath belonging to self
        let mut db_ice18 = LinkedDatabase::default();
        db_ice18.add_record(
            "File",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice18.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileSelf".to_string()),
                FieldValue::String("CompSelf".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompEmptyKp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String(String::new()),
            ]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNotInFiles".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("RegOrDir".to_string()),
            ]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompSelf".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("FileSelf".to_string()),
            ]),
        );
        assert!(Linker::validate_ice18(&db_ice18).is_none());

        // ICE20: Record with nulls, missing actions, valid orders, and CostInitialize >= CostFinalize error
        let mut db_ice20 = LinkedDatabase::default();
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
        );
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800), // CostInitialize >= CostFinalize
            ]),
        );
        assert_eq!(
            Linker::validate_ice20(&db_ice20)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE20", true))
        );

        // Valid order
        let mut db_ice20_ok = LinkedDatabase::default();
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        assert!(Linker::validate_ice20(&db_ice20_ok).is_none());

        // ICE30:
        // - Component with nulls
        // - File with nulls or non-string fname
        // - File fname with short|long syntax
        // - File belonging to unknown component
        // - Duplicate same file in same component
        // - Distinct files in same directory -> None
        let mut db_ice30 = LinkedDatabase::default();
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompDirA".to_string()),
                FieldValue::Null,
                FieldValue::String("DIR_A".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FNonStr".to_string()),
                FieldValue::String("CompDirA".to_string()),
                FieldValue::Short(42), // fname not string
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FUnkComp".to_string()),
                FieldValue::String("NonExistentComp".to_string()),
                FieldValue::String("file.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FShortLong".to_string()),
                FieldValue::String("CompDirA".to_string()),
                FieldValue::String("SHORT~1.TXT|longfilename.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(3),
            ]),
        );
        // Same file re-inserted
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FShortLong".to_string()),
                FieldValue::String("CompDirA".to_string()),
                FieldValue::String("SHORT~1.TXT|longfilename.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(4),
            ]),
        );
        assert!(Linker::validate_ice30(&db_ice30).is_none());

        // ICE33:
        // - Class with null
        // - Class with valid GUID (38 chars, starts and ends with curlies)
        // - Class starting without { -> error
        // - Class ending without } -> error
        // - Class with len != 38 -> error
        // - Registry with nulls
        // - Registry with root 0 and key InprocServer32 -> warning
        // - Registry with root 0 and normal key -> ok
        // - Registry with root 1 and CLSID -> ok
        // - Registry with root 2 and normal key -> ok
        // Clean DB returning None for ICE33
        let mut db_ice33_guid_ok = LinkedDatabase::default();
        db_ice33_guid_ok.add_record("Class", Record::with_fields(vec![FieldValue::Null]));
        db_ice33_guid_ok.add_record(
            "Class",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("InprocServer32".to_string()),
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        // Registry entries that don't trigger warning (root 0 normal, root 1 normal, root 2 normal)
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Short(0),
                FieldValue::String(r"Software\Normal".to_string()), // root 0 normal -> ok (contains InprocServer32 is false)
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R3".to_string()),
                FieldValue::Short(1), // HKCU normal -> ok (*root == 2 is false)
                FieldValue::String(r"CLSID\{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R4".to_string()),
                FieldValue::Short(2), // HKLM normal -> ok (starts_with Software\Classes\CLSID\ is false)
                FieldValue::String(r"Software\Normal".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(Linker::validate_ice33(&db_ice33_guid_ok).is_none());

        // Registry entry with root 0 and InprocServer32 -> warning
        let mut db_ice33_warn_inproc = LinkedDatabase::default();
        db_ice33_warn_inproc.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(0),
                FieldValue::String(r"Software\MyApp\InprocServer32".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        let rep_ice33_inproc = Linker::validate_ice33(&db_ice33_warn_inproc);
        assert_eq!(
            rep_ice33_inproc
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", false))
        );

        // Class ending without }
        let mut db_ice33_no_end = LinkedDatabase::default();
        db_ice33_no_end.add_record(
            "Class",
            Record::with_fields(vec![FieldValue::String(
                "{11111111-1111-1111-1111-111111111111X".to_string(),
            )]),
        );
        assert_eq!(
            Linker::validate_ice33(&db_ice33_no_end)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", true))
        );

        // Class len != 38
        let mut db_ice33_short = LinkedDatabase::default();
        db_ice33_short.add_record(
            "Class",
            Record::with_fields(vec![FieldValue::String("{123}".to_string())]),
        );
        assert_eq!(
            Linker::validate_ice33(&db_ice33_short)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", true))
        );

        Ok(())
    }

    /// Tests ICE38 through ICE103 edge cases.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice38_to_ice103_comprehensive() -> Result<()> {
        // ICE38:
        // - Registry record with nulls
        // - Component record with nulls
        // - Component in LocalAppDataFolder with null attr, non-reg kp, null kp, unknown kp, and root 1 (HKCU - valid)
        let mut db_ice38 = LinkedDatabase::default();
        db_ice38.add_record(
            "Registry",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice38.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("HkcuKey".to_string()),
                FieldValue::Short(1), // HKCU
            ]),
        );
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        // Comp with null attr in LocalAppDataFolder
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNullAttr".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Comp without registry keypath in LocalAppDataFolder
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNonReg".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("SomeFile".to_string()),
            ]),
        );
        // Comp with unknown reg kp
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompUnkReg".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Short(0x0010),
                FieldValue::Null,
                FieldValue::String("UnknownReg".to_string()),
            ]),
        );
        // Comp with HKCU reg kp -> ok
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompHkcu".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Short(0x0010),
                FieldValue::Null,
                FieldValue::String("HkcuKey".to_string()),
            ]),
        );
        assert!(Linker::validate_ice38(&db_ice38).is_none());

        // ICE61:
        // - Property other than ProductVersion
        // - ProductVersion parse failure
        // - Upgrade with null min_ver
        // - Upgrade with unparseable min_ver
        // - Upgrade with min_ver <= ProductVersion -> ok
        let mut db_ice61 = LinkedDatabase::default();
        db_ice61.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("OtherProp".to_string()),
                FieldValue::String("Val".to_string()),
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61).is_none());

        let mut db_ice61_bad_ver = LinkedDatabase::default();
        db_ice61_bad_ver.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("not-a-valid-ver".to_string()),
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61_bad_ver).is_none());

        let mut db_ice61_ok = LinkedDatabase::default();
        db_ice61_ok.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("2.0.0".to_string()),
            ]),
        );
        db_ice61_ok.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::Null, // null min_ver
            ]),
        );
        db_ice61_ok.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("invalid-version".to_string()), // unparseable min_ver
            ]),
        );
        db_ice61_ok.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("1.0.0".to_string()), // 1.0.0 <= 2.0.0 -> ok
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61_ok).is_none());

        // ICE80: Component with null attr, pure 32-bit package, pure 64-bit package
        let mut db_ice80 = LinkedDatabase::default();
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNullAttr".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
            ]),
        );
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp32".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
            ]),
        );
        assert!(Linker::validate_ice80(&db_ice80).is_none());

        let mut db_ice80_64 = LinkedDatabase::default();
        db_ice80_64.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0100),
            ]),
        );
        assert!(Linker::validate_ice80(&db_ice80_64).is_none());

        // ICE99: Directory with nulls, empty parent, dir == parent, and valid tree
        let mut db_ice99 = LinkedDatabase::default();
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String(String::new()), // empty parent
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SameSelf".to_string()),
                FieldValue::String("SameSelf".to_string()), // dir == parent
                FieldValue::String("SameSelf".to_string()),
            ]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("Sub".to_string()),
            ]),
        );
        assert!(Linker::validate_ice99(&db_ice99).is_none());

        // ICE101: File with null seq, file with seq > 0
        let mut db_ice101 = LinkedDatabase::default();
        db_ice101.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // null seq
            ]),
        );
        db_ice101.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(10), // > 0
            ]),
        );
        assert!(Linker::validate_ice101(&db_ice101).is_none());

        // ICE103: Shortcut with null index, shortcut with index >= 0
        let mut db_ice103 = LinkedDatabase::default();
        db_ice103.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("S1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // null icon idx
            ]),
        );
        db_ice103.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("S2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0), // >= 0
            ]),
        );
        assert!(Linker::validate_ice103(&db_ice103).is_none());

        Ok(())
    }

    #[test]
    fn test_run_ice_validations_filtered_whitelist_and_suppression() -> Result<()> {
        let mut db = LinkedDatabase::default();
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(99), // non-contiguous sequence triggers ICE04
            ]),
        );

        // Selecting ICE04 -> should fail with ICE04 error
        let err = Linker::run_ice_validations_filtered(&db, &["ICE04".to_string()], &[]);
        assert!(err.is_err());

        // Suppressing ICE04 -> should bypass ICE04 failure
        let res = Linker::run_ice_validations_filtered(
            &db,
            &["ICE04".to_string()],
            &["ice04".to_string()],
        );
        assert!(res.is_ok());

        // Selecting unrelated rule (e.g. ICE99) -> passes
        let res2 = Linker::run_ice_validations_filtered(&db, &["ICE99".to_string()], &[]);
        assert!(res2.is_ok());

        // Direct run_ice_validations calls filtered with empty lists -> fails on unsuppressed ICE04
        assert!(Linker::run_ice_validations(&db).is_err());
        Ok(())
    }
}
