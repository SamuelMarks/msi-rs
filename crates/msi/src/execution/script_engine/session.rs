//! MSI Script Session Automation Object Model.
//!
//! Provides the automation session object exposed to embedded ECMAScript (`JScript`)
//! and `VBScript` custom action engines per the Windows Installer SDK specification:
//! - `Session.Property(name)`: Dynamic property reading and writing.
//! - `Session.EvaluateCondition(expr)`: Evaluates boolean condition expressions.
//! - `Session.Message(kind, record)`: Dispatches status and error messages.
//! - `Session.Mode(modeId)`: Queries transaction lifecycle modes (scheduled, rollback, commit).
//! - `Session.DoAction(actionName)`: Synchronously invokes another installer action.
//! - `Session.Database`: Inspects database tables, schemas, and records.
//! - Execution fuel limiter guaranteeing sandboxed termination without hangs.

use crate::database::catalogs::DatabaseCatalog;
use crate::error::{Error, Result};
use crate::execution::properties::EvaluationContext;
use std::collections::HashMap;

/// Standard MSI Run Mode constant: Administrative installation session.
pub const MSIRUNMODE_ADMIN: i32 = 0;
/// Standard MSI Run Mode constant: Advertise installation session.
pub const MSIRUNMODE_ADVERTISE: i32 = 1;
/// Standard MSI Run Mode constant: Maintenance installation session.
pub const MSIRUNMODE_MAINTENANCE: i32 = 2;
/// Standard MSI Run Mode constant: Rollback operations are enabled.
pub const MSIRUNMODE_ROLLBACKENABLED: i32 = 3;
/// Standard MSI Run Mode constant: Logging is enabled.
pub const MSIRUNMODE_LOGENABLED: i32 = 4;
/// Standard MSI Run Mode constant: Operations currently executing.
pub const MSIRUNMODE_OPERATIONS: i32 = 5;
/// Standard MSI Run Mode constant: Reboot at completion of installation is required.
pub const MSIRUNMODE_REBOOTATEND: i32 = 6;
/// Standard MSI Run Mode constant: Immediate reboot is required.
pub const MSIRUNMODE_REBOOTNOW: i32 = 7;
/// Standard MSI Run Mode constant: Cabinet path.
pub const MSIRUNMODE_CABPATH: i32 = 8;
/// Standard MSI Run Mode constant: Source short names.
pub const MSIRUNMODE_SOURCESHORTNAMES: i32 = 9;
/// Standard MSI Run Mode constant: Target short names.
pub const MSIRUNMODE_TARGETSHORTNAMES: i32 = 10;
/// Standard MSI Run Mode constant: Windows 9x platform.
pub const MSIRUNMODE_WINDOWS9X: i32 = 12;
/// Standard MSI Run Mode constant: ZAW enabled.
pub const MSIRUNMODE_ZAWENABLED: i32 = 13;
/// Standard MSI Run Mode constant: Custom action executing deferred within the execution script.
pub const MSIRUNMODE_SCHEDULED: i32 = 16;
/// Standard MSI Run Mode constant: Custom action executing rollback operations on failure.
pub const MSIRUNMODE_ROLLBACK: i32 = 17;
/// Standard MSI Run Mode constant: Custom action executing commit operations on success.
pub const MSIRUNMODE_COMMIT: i32 = 18;

/// Default execution fuel allocation (maximum allowed statement/loop evaluations).
pub const DEFAULT_SCRIPT_FUEL: usize = 100_000;

/// Return code for condition evaluating to False.
pub const MSICONDITION_FALSE_CODE: i32 = 0;
/// Return code for condition evaluating to True.
pub const MSICONDITION_TRUE_CODE: i32 = 1;
/// Return code for condition evaluating with an expression syntax error.
pub const MSICONDITION_ERROR_CODE: i32 = 2;
/// Return code for empty or absent condition expression.
pub const MSICONDITION_NONE_CODE: i32 = 3;

/// Message log entry recorded by `Session.Message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMessage {
    /// Message type / flag bitmask.
    pub kind: i32,
    /// Message text payload.
    pub text: String,
}

/// Automation database interface providing read-only inspection of database tables.
#[derive(Debug, Clone, Default)]
pub struct ScriptDatabase {
    /// Optional underlying database catalog schema.
    catalog: Option<DatabaseCatalog>,
    /// Simulated or extracted table rows (`table_name -> list of row key-value maps`).
    records: HashMap<String, Vec<HashMap<String, String>>>,
}

impl ScriptDatabase {
    /// Creates a new [`ScriptDatabase`] instance.
    ///
    /// # Arguments
    ///
    /// * `catalog` - Optional catalog metadata.
    ///
    /// # Returns
    ///
    /// A new [`ScriptDatabase`].
    #[must_use]
    pub fn new(catalog: Option<DatabaseCatalog>) -> Self {
        Self {
            catalog,
            records: HashMap::new(),
        }
    }

    /// Checks whether a table exists in the database schema.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name to check.
    ///
    /// # Returns
    ///
    /// `true` if the table is defined, otherwise `false`.
    #[must_use]
    pub fn table_exists(&self, table_name: &str) -> bool {
        if self.records.contains_key(table_name) {
            return true;
        }
        self.catalog
            .as_ref()
            .is_some_and(|cat| cat.get_table(table_name).is_some())
    }

    /// Returns the number of records present in a given table.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name to query.
    ///
    /// # Returns
    ///
    /// Row count for the table.
    #[must_use]
    pub fn row_count(&self, table_name: &str) -> usize {
        self.records.get(table_name).map_or(0, Vec::len)
    }

    /// Inserts a row record for testing or script simulation.
    ///
    /// # Arguments
    ///
    /// * `table_name` - Target table name.
    /// * `row` - Map of column names to string values.
    pub fn insert_row(&mut self, table_name: impl Into<String>, row: HashMap<String, String>) {
        self.records.entry(table_name.into()).or_default().push(row);
    }

    /// Fetches a row by 0-based index from the specified table.
    ///
    /// # Arguments
    ///
    /// * `table_name` - Target table name.
    /// * `index` - 0-based row index.
    ///
    /// # Returns
    ///
    /// Reference to row map if found, or `None`.
    #[must_use]
    pub fn get_row(&self, table_name: &str, index: usize) -> Option<&HashMap<String, String>> {
        self.records
            .get(table_name)
            .and_then(|rows| rows.get(index))
    }
}

/// The MSI Session automation object model bound into script engines.
#[derive(Debug, Clone)]
pub struct ScriptSession {
    /// Active evaluation context for property and condition handling.
    context: EvaluationContext,
    /// Run modes map (`mode_id -> boolean`).
    run_modes: HashMap<i32, bool>,
    /// Accumulated messages dispatched via `Session.Message`.
    messages: Vec<SessionMessage>,
    /// Chronological list of actions executed via `Session.DoAction`.
    executed_actions: Vec<String>,
    /// Database automation inspection engine.
    database: ScriptDatabase,
    /// Remaining execution fuel units preventing infinite loops.
    fuel: usize,
    /// Active call stack frames for error diagnostics.
    call_stack: Vec<String>,
}

impl Default for ScriptSession {
    fn default() -> Self {
        Self::new(EvaluationContext::new(), None)
    }
}

impl ScriptSession {
    /// Creates a new [`ScriptSession`] wrapping an [`EvaluationContext`] and optional catalog.
    ///
    /// # Arguments
    ///
    /// * `context` - Active installer property evaluation context.
    /// * `catalog` - Optional database catalog metadata.
    ///
    /// # Returns
    ///
    /// A configured [`ScriptSession`].
    #[must_use]
    pub fn new(context: EvaluationContext, catalog: Option<DatabaseCatalog>) -> Self {
        Self {
            context,
            run_modes: HashMap::new(),
            messages: Vec::new(),
            executed_actions: Vec::new(),
            database: ScriptDatabase::new(catalog),
            fuel: DEFAULT_SCRIPT_FUEL,
            call_stack: Vec::new(),
        }
    }

    /// Creates a [`ScriptSession`] with a custom fuel allotment.
    ///
    /// # Arguments
    ///
    /// * `context` - Property evaluation context.
    /// * `fuel` - Maximum number of script execution steps.
    ///
    /// # Returns
    ///
    /// A configured [`ScriptSession`].
    #[must_use]
    pub fn with_fuel(context: EvaluationContext, fuel: usize) -> Self {
        Self {
            context,
            run_modes: HashMap::new(),
            messages: Vec::new(),
            executed_actions: Vec::new(),
            database: ScriptDatabase::new(None),
            fuel,
            call_stack: Vec::new(),
        }
    }

    /// Creates a [`ScriptSession`] with a custom [`ScriptDatabase`].
    ///
    /// # Arguments
    ///
    /// * `context` - Property evaluation context.
    /// * `database` - Pre-populated script database.
    ///
    /// # Returns
    ///
    /// A configured [`ScriptSession`].
    #[must_use]
    pub fn with_database(context: EvaluationContext, database: ScriptDatabase) -> Self {
        Self {
            context,
            run_modes: HashMap::new(),
            messages: Vec::new(),
            executed_actions: Vec::new(),
            database,
            fuel: DEFAULT_SCRIPT_FUEL,
            call_stack: Vec::new(),
        }
    }

    /// Retrieves an installer property string value.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the property.
    ///
    /// # Returns
    ///
    /// Property value string, or empty string if unset.
    #[must_use]
    pub fn property(&self, name: &str) -> String {
        self.context.get_property(name).unwrap_or("").to_string()
    }

    /// Sets an installer property value.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the property.
    /// * `value` - New string value.
    pub fn set_property(&mut self, name: &str, value: &str) {
        self.context.set_property(name, value);
    }

    /// Returns a reference to the inner [`EvaluationContext`].
    ///
    /// # Returns
    ///
    /// Borrow of the evaluation context.
    #[must_use]
    pub const fn context(&self) -> &EvaluationContext {
        &self.context
    }

    /// Returns a mutable reference to the inner [`EvaluationContext`].
    ///
    /// # Returns
    ///
    /// Mutable borrow of the evaluation context.
    pub const fn context_mut(&mut self) -> &mut EvaluationContext {
        &mut self.context
    }

    /// Evaluates an MSI condition expression using the session properties.
    ///
    /// Returns integer condition codes matching MSI SDK:
    /// - `1` (`MSICONDITION_TRUE`)
    /// - `0` (`MSICONDITION_FALSE`)
    /// - `2` (`MSICONDITION_ERROR`)
    /// - `3` (`MSICONDITION_NONE`)
    ///
    /// # Arguments
    ///
    /// * `expr` - The MSI condition expression string.
    ///
    /// # Returns
    ///
    /// Integer condition evaluation status.
    #[must_use]
    pub fn evaluate_condition(&self, expr: &str) -> i32 {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return MSICONDITION_NONE_CODE;
        }

        match self.context.evaluate_condition(trimmed) {
            Ok(true) => MSICONDITION_TRUE_CODE,
            Ok(false) => MSICONDITION_FALSE_CODE,
            Err(_) => MSICONDITION_ERROR_CODE,
        }
    }

    /// Dispatches a status or error message record.
    ///
    /// # Arguments
    ///
    /// * `kind` - Bitmask flag identifying message classification.
    /// * `text` - Formatted message text.
    ///
    /// # Returns
    ///
    /// Standard Windows Installer return code (1 for OK/Success).
    pub fn message(&mut self, kind: i32, text: &str) -> i32 {
        self.messages.push(SessionMessage {
            kind,
            text: text.to_string(),
        });
        1
    }

    /// Returns the list of recorded messages.
    ///
    /// # Returns
    ///
    /// Slice of [`SessionMessage`].
    #[must_use]
    pub fn messages(&self) -> &[SessionMessage] {
        &self.messages
    }

    /// Queries whether a given transaction run mode is active.
    ///
    /// # Arguments
    ///
    /// * `mode_id` - Numeric run mode identifier (e.g. `MSIRUNMODE_SCHEDULED`).
    ///
    /// # Returns
    ///
    /// `true` if active, otherwise `false`.
    #[must_use]
    pub fn mode(&self, mode_id: i32) -> bool {
        self.run_modes.get(&mode_id).copied().unwrap_or(false)
    }

    /// Sets or clears an execution run mode.
    ///
    /// # Arguments
    ///
    /// * `mode_id` - Run mode identifier.
    /// * `enabled` - Boolean state.
    pub fn set_mode(&mut self, mode_id: i32, enabled: bool) {
        self.run_modes.insert(mode_id, enabled);
    }

    /// Synchronously invokes an action by name.
    ///
    /// # Arguments
    ///
    /// * `action_name` - Name of the sequence or custom action.
    ///
    /// # Returns
    ///
    /// Return code: `1` on success, `0` if empty action.
    pub fn do_action(&mut self, action_name: &str) -> i32 {
        if action_name.is_empty() {
            return 0;
        }
        self.executed_actions.push(action_name.to_string());
        1
    }

    /// Returns the chronological list of actions executed via `Session.DoAction`.
    ///
    /// # Returns
    ///
    /// Slice of action names.
    #[must_use]
    pub fn executed_actions(&self) -> &[String] {
        &self.executed_actions
    }

    /// Returns a reference to the [`ScriptDatabase`].
    ///
    /// # Returns
    ///
    /// Borrow of the database automation engine.
    #[must_use]
    pub const fn database(&self) -> &ScriptDatabase {
        &self.database
    }

    /// Returns a mutable reference to the [`ScriptDatabase`].
    ///
    /// # Returns
    ///
    /// Mutable borrow of the database automation engine.
    pub const fn database_mut(&mut self) -> &mut ScriptDatabase {
        &mut self.database
    }

    /// Consumes one unit of execution fuel, guarding against infinite loops.
    ///
    /// # Arguments
    ///
    /// * `line` - Current line number.
    /// * `col` - Current column number.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptRuntimeError`] if fuel is exhausted.
    pub fn consume_fuel(&mut self, line: usize, col: usize) -> Result<()> {
        if self.fuel == 0 {
            let stack_trace = if self.call_stack.is_empty() {
                String::new()
            } else {
                format!(" (stack: {})", self.call_stack.join(" -> "))
            };
            return Err(Error::ScriptRuntimeError {
                line,
                col,
                message: format!(
                    "Script execution step limit reached; sandbox aborted{stack_trace}"
                ),
            });
        }
        self.fuel -= 1;
        Ok(())
    }

    /// Pushes a function frame onto the active call stack.
    ///
    /// # Arguments
    ///
    /// * `name` - Function or subroutine name.
    pub fn push_call(&mut self, name: &str) {
        self.call_stack.push(name.to_string());
    }

    /// Pops a function frame from the call stack.
    pub fn pop_call(&mut self) {
        let _ = self.call_stack.pop();
    }

    /// Returns the active call stack representation.
    ///
    /// # Returns
    ///
    /// Slice of frame names.
    #[must_use]
    pub fn call_stack(&self) -> &[String] {
        &self.call_stack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_session_properties_and_modes() {
        let mut session = ScriptSession::default();
        assert_eq!(session.property("PRODUCTNAME"), "");

        session.set_property("PRODUCTNAME", "MyApplication");
        assert_eq!(session.property("PRODUCTNAME"), "MyApplication");

        assert!(!session.mode(MSIRUNMODE_SCHEDULED));
        session.set_mode(MSIRUNMODE_SCHEDULED, true);
        assert!(session.mode(MSIRUNMODE_SCHEDULED));

        // Test context_mut
        session.context_mut().set_property("CUSTOM_VAR", "VAL");
        assert_eq!(session.context().get_property("CUSTOM_VAR"), Some("VAL"));
    }

    #[test]
    fn test_script_session_condition_evaluation() {
        let mut session = ScriptSession::default();
        session.set_property("VersionNT", "600");

        assert_eq!(session.evaluate_condition(""), MSICONDITION_NONE_CODE);
        assert_eq!(
            session.evaluate_condition("VersionNT >= 500"),
            MSICONDITION_TRUE_CODE
        );
        assert_eq!(
            session.evaluate_condition("VersionNT < 500"),
            MSICONDITION_FALSE_CODE
        );
        assert_eq!(
            session.evaluate_condition("((unbalanced"),
            MSICONDITION_ERROR_CODE
        );
    }

    #[test]
    fn test_script_session_messages_and_actions() {
        let mut session = ScriptSession::default();
        assert_eq!(session.message(1, "Starting setup..."), 1);
        assert_eq!(session.messages().len(), 1);
        assert_eq!(session.messages()[0].text, "Starting setup...");

        assert_eq!(session.do_action("CustomInit"), 1);
        assert_eq!(session.do_action(""), 0);
        assert_eq!(session.executed_actions(), &["CustomInit".to_string()]);
    }

    #[test]
    fn test_script_database_operations() -> Result<()> {
        let mut session = ScriptSession::default();
        assert!(!session.database().table_exists("Property"));
        assert_eq!(session.database().row_count("Property"), 0);

        let mut row = HashMap::new();
        row.insert("Property".to_string(), "Manufacturer".to_string());
        row.insert("Value".to_string(), "Contoso Corp".to_string());
        session.database_mut().insert_row("Property", row);

        assert!(session.database().table_exists("Property"));
        assert_eq!(session.database().row_count("Property"), 1);
        let fetched = session.database().get_row("Property", 0);
        assert!(fetched.is_some());
        assert_eq!(
            fetched.and_then(|r| r.get("Value")),
            Some(&"Contoso Corp".to_string())
        );
        assert!(session.database().get_row("Property", 99).is_none());

        // Test table_exists fallback to DatabaseCatalog
        let mut cat = DatabaseCatalog::new();
        cat.add_table(crate::database::catalogs::TableSchema::new(
            "CatalogOnlyTable",
        ))?;
        let db_with_cat = ScriptDatabase::new(Some(cat));
        assert!(db_with_cat.table_exists("CatalogOnlyTable"));
        assert!(!db_with_cat.table_exists("NonExistentTable"));

        Ok(())
    }

    #[test]
    fn test_script_session_fuel_exhaustion() {
        let mut session = ScriptSession::with_fuel(EvaluationContext::new(), 2);
        session.push_call("main");
        session.push_call("compute");

        assert_eq!(session.consume_fuel(10, 5), Ok(()));
        assert_eq!(session.consume_fuel(11, 5), Ok(()));
        assert_eq!(
            session.consume_fuel(12, 5),
            Err(Error::ScriptRuntimeError {
                line: 12,
                col: 5,
                message:
                    "Script execution step limit reached; sandbox aborted (stack: main -> compute)"
                        .to_string(),
            })
        );

        session.pop_call();
        assert_eq!(session.call_stack(), &["main".to_string()]);

        // Test fuel exhaustion when call stack is empty
        let mut empty_session = ScriptSession::with_fuel(EvaluationContext::new(), 0);
        assert_eq!(
            empty_session.consume_fuel(1, 1),
            Err(Error::ScriptRuntimeError {
                line: 1,
                col: 1,
                message: "Script execution step limit reached; sandbox aborted".to_string(),
            })
        );
    }
}
