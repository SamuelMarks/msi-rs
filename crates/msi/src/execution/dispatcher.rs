//! Action sequence evaluation and dispatch engine.
//!
//! Walks sequence tables (`InstallExecuteSequence`, `InstallUISequence`, `AdminExecuteSequence`,
//! `AdvtExecuteSequence`) ordered by `Sequence`, evaluates conditional expressions against the
//! active [`EvaluationContext`], and dispatches standard and custom actions.

use crate::database::sql::executor::{execute_select_sql, QueryResult};
use crate::database::tables::record::FieldValue;
use crate::error::{MsiError, Result};
use crate::execution::sequence::{ActionStatus, SequenceName};
use crate::execution::transaction::{Transaction, Uninitialized};

/// Dispatcher responsible for evaluating conditions and executing action sequences.
#[derive(Debug)]
pub struct ActionDispatcher<'a> {
    /// Active transaction manager providing database and property context.
    transaction: &'a mut Transaction<Uninitialized>,
}

impl<'a> ActionDispatcher<'a> {
    /// Creates a new [`ActionDispatcher`] bound to `transaction`.
    ///
    /// # Arguments
    ///
    /// * `transaction` - The uninitialized transaction providing database and property context.
    ///
    /// # Returns
    ///
    /// A new [`ActionDispatcher`].
    #[must_use]
    pub const fn new(transaction: &'a mut Transaction<Uninitialized>) -> Self {
        Self { transaction }
    }

    /// Executes an entire action sequence table by name in ascending `Sequence` order.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Name of the sequence table (e.g., `"InstallExecuteSequence"`).
    ///
    /// # Returns
    ///
    /// Final [`ActionStatus`] after executing all eligible actions in the sequence.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::ExecutionFailed`] if the sequence table does not exist or a Type 19
    /// custom error action aborts installation, or [`MsiError::Validation`] if a condition expression is malformed.
    pub fn execute_sequence(&mut self, sequence: &SequenceName) -> Result<ActionStatus> {
        let query = format!(
            "SELECT `Action`, `Condition`, `Sequence` FROM `{}` ORDER BY `Sequence`",
            sequence.0
        );

        let Ok(QueryResult::Select { rows, .. }) =
            execute_select_sql(self.transaction.database(), &query, &[])
        else {
            return Err(MsiError::ExecutionFailed {
                action: sequence.0.clone(),
                return_code: 1603,
                message: "Failed to query sequence table".to_string(),
            });
        };

        let mut overall_status = ActionStatus::Success;

        for record in rows {
            let fields = record.fields();

            let action = match fields.first() {
                Some(FieldValue::String(s)) => s.clone(),
                _ => continue,
            };

            let condition = match fields.get(1) {
                Some(FieldValue::String(s)) => s.clone(),
                _ => String::new(),
            };

            if !condition.is_empty()
                && !self.transaction.context().evaluate_condition(&condition)?
            {
                continue;
            }

            let status = self.dispatch_action(&action)?;
            match status {
                ActionStatus::Success | ActionStatus::Skipped => {}
                ActionStatus::Failure | ActionStatus::FatalExit | ActionStatus::UserExit => {
                    overall_status = status;
                    break;
                }
            }
        }

        Ok(overall_status)
    }

    /// Dispatches a top-level sequence action (`INSTALL`, `ADMIN`, `ADVERTISE`, `ExecuteAction`).
    fn dispatch_top_level_sequence(&mut self, action_name: &str) -> Result<Option<ActionStatus>> {
        match action_name {
            "INSTALL" => {
                if self
                    .transaction
                    .database()
                    .tables
                    .contains_key("InstallUISequence")
                {
                    let ui_status =
                        self.execute_sequence(&SequenceName::from("InstallUISequence"))?;
                    if ui_status != ActionStatus::Success {
                        return Ok(Some(ui_status));
                    }
                }
                if self
                    .transaction
                    .database()
                    .tables
                    .contains_key("InstallExecuteSequence")
                {
                    return self
                        .execute_sequence(&SequenceName::from("InstallExecuteSequence"))
                        .map(Some);
                }
                Ok(Some(ActionStatus::Success))
            }
            "ADMIN" => {
                if self
                    .transaction
                    .database()
                    .tables
                    .contains_key("AdminUISequence")
                {
                    let ui_status =
                        self.execute_sequence(&SequenceName::from("AdminUISequence"))?;
                    if ui_status != ActionStatus::Success {
                        return Ok(Some(ui_status));
                    }
                }
                if self
                    .transaction
                    .database()
                    .tables
                    .contains_key("AdminExecuteSequence")
                {
                    return self
                        .execute_sequence(&SequenceName::from("AdminExecuteSequence"))
                        .map(Some);
                }
                Ok(Some(ActionStatus::Success))
            }
            "ADVERTISE" => {
                if self
                    .transaction
                    .database()
                    .tables
                    .contains_key("AdvtExecuteSequence")
                {
                    return self
                        .execute_sequence(&SequenceName::from("AdvtExecuteSequence"))
                        .map(Some);
                }
                Ok(Some(ActionStatus::Success))
            }
            "ExecuteAction" => {
                let target_seq = self
                    .transaction
                    .context()
                    .get_property("EXECUTEACTION")
                    .unwrap_or("InstallExecuteSequence")
                    .to_string();
                if self.transaction.database().tables.contains_key(&target_seq) {
                    return self
                        .execute_sequence(&SequenceName::from(target_seq.as_str()))
                        .map(Some);
                }
                Ok(Some(ActionStatus::Success))
            }
            _ => Ok(None),
        }
    }

    /// Evaluates the `CCPSearch` table and sets `CCP_SUCCESS` if any signature matches.
    fn run_ccp_search(&mut self) {
        let host = crate::execution::appsearch::DefaultHostSystem::new();
        let search = crate::execution::appsearch::AppSearch::new(self.transaction.database(), host);
        let matched = self
            .transaction
            .database()
            .get_records("CCPSearch")
            .iter()
            .any(|rec| {
                let Some(FieldValue::String(sig)) = rec.get(0) else {
                    return false;
                };
                search.resolve_signature(sig).is_some()
                    || self.transaction.context().get_property(sig).is_some()
            });
        if matched {
            self.transaction
                .context_mut()
                .set_property("CCP_SUCCESS", "1");
        }
    }

    /// Dispatches an immediate standard action (`LaunchConditions`, `AppSearch`, `CCPSearch`, etc.).
    fn dispatch_immediate_standard_action(
        &mut self,
        action_name: &str,
    ) -> Result<Option<ActionStatus>> {
        match action_name {
            "LaunchConditions" => {
                for rec in self.transaction.database().get_records("LaunchCondition") {
                    if let Some(FieldValue::String(cond)) = rec.get(0) {
                        if !self.transaction.context().evaluate_condition(cond)? {
                            return Ok(Some(ActionStatus::Failure));
                        }
                    }
                }
                Ok(Some(ActionStatus::Success))
            }
            "AppSearch" => {
                let host = crate::execution::appsearch::DefaultHostSystem::new();
                let search =
                    crate::execution::appsearch::AppSearch::new(self.transaction.database(), host);
                for (prop, val) in search.execute().unwrap_or_default() {
                    self.transaction.context_mut().set_property(prop, val);
                }
                Ok(Some(ActionStatus::Success))
            }
            "CCPSearch" | "RMCCPSearch" => {
                self.run_ccp_search();
                Ok(Some(ActionStatus::Success))
            }
            "ValidateProductID" => {
                if let Some(pid_key) = self
                    .transaction
                    .context()
                    .get_property("PIDKEY")
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                {
                    self.transaction
                        .context_mut()
                        .set_property("ProductID", pid_key);
                }
                Ok(Some(ActionStatus::Success))
            }
            "ResolveSource" => {
                if self
                    .transaction
                    .context()
                    .get_property("SourceDir")
                    .is_none()
                {
                    let orig = self
                        .transaction
                        .context()
                        .get_property("OriginalDatabase")
                        .and_then(|p| std::path::Path::new(p).parent())
                        .map_or_else(|| r"C:\".to_string(), |p| p.to_string_lossy().to_string());
                    self.transaction
                        .context_mut()
                        .set_property("SourceDir", orig);
                }
                Ok(Some(ActionStatus::Success))
            }
            "DisableRollback" => {
                self.transaction
                    .context_mut()
                    .set_property("RollbackDisabled", "1");
                Ok(Some(ActionStatus::Success))
            }
            "ScheduleReboot" => {
                self.transaction
                    .context_mut()
                    .set_property("REBOOT", "Force");
                Ok(Some(ActionStatus::Success))
            }
            "ForceReboot" => {
                self.transaction
                    .context_mut()
                    .set_property("REBOOT", "Force");
                self.transaction
                    .context_mut()
                    .set_property("ForceRebootTriggered", "1");
                Ok(Some(ActionStatus::Success))
            }
            _ => Ok(None),
        }
    }

    /// Dispatches a single standard or custom action by name.
    ///
    /// Checks the `CustomAction` table first:
    /// - Type 19 (`0x13`) error custom actions format their `Target` message and abort with [`MsiError::ExecutionFailed`].
    /// - Type 51 (`0x33`) property-assignment custom actions format their `Target` value and assign the property named by `Source`.
    /// - Standard `LaunchConditions` evaluates all rows in the `LaunchCondition` table, returning [`ActionStatus::Failure`] if any condition evaluates to `false`.
    ///
    /// # Arguments
    ///
    /// * `action_name` - Identifier of the standard or custom action to execute.
    ///
    /// # Returns
    ///
    /// [`ActionStatus`] indicating the outcome of the action.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::ExecutionFailed`] if a Type 19 error action triggers, or [`MsiError::Validation`] if a condition expression is invalid.
    pub fn dispatch_action(&mut self, action_name: &str) -> Result<ActionStatus> {
        if let Some(status) = self.dispatch_top_level_sequence(action_name)? {
            return Ok(status);
        }
        if let Some(status) = self.dispatch_immediate_standard_action(action_name)? {
            return Ok(status);
        }

        let ca_records = self.transaction.database().get_records("CustomAction");
        for rec in ca_records {
            let Some(FieldValue::String(ca_id)) = rec.get(0) else {
                continue;
            };
            if ca_id != action_name {
                continue;
            }
            let ca_type = match rec.get(1) {
                Some(FieldValue::Short(v)) => i32::from(*v),
                Some(FieldValue::Long(v)) => *v,
                _ => 0,
            };
            let source = match rec.get(2) {
                Some(FieldValue::String(s)) => s,
                _ => "",
            };
            let target = match rec.get(3) {
                Some(FieldValue::String(s)) => s,
                _ => "",
            };

            let base_type = ca_type & 0x3F;
            if base_type == 19 {
                let formatted = self.transaction.context().format_string(target)?;
                return Err(MsiError::ExecutionFailed {
                    action: action_name.to_string(),
                    return_code: 1603,
                    message: formatted,
                });
            }
            if base_type == 51 && !source.is_empty() {
                let formatted = self.transaction.context().format_string(target)?;
                let src_prop = source.to_string();
                self.transaction
                    .context_mut()
                    .set_property(&src_prop, &formatted);
                return Ok(ActionStatus::Success);
            }
        }

        Ok(ActionStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::{FieldValue, Record};
    use crate::execution::properties::EvaluationContext;
    use crate::wix::linker::LinkedDatabase;

    /// Tests handling of non-string action and condition fields in sequence rows.
    #[test]
    #[allow(clippy::unnecessary_wraps)]
    fn test_dispatcher_bad_records() -> Result<()> {
        let mut db = LinkedDatabase::new()?;
        let rows = vec![
            Record::with_fields(vec![
                FieldValue::Long(1),
                FieldValue::Long(1),
                FieldValue::Long(1),
            ]),
            Record::with_fields(vec![
                FieldValue::String("A".to_string()),
                FieldValue::Long(1),
                FieldValue::Long(1),
            ]),
        ];

        db.tables.insert("InstallExecuteSequence".to_string(), rows);

        let context = EvaluationContext::new();
        let cost = crate::execution::costing::DiskCostEngine::new();
        let mut tx = Transaction::new(db, context, cost);

        let mut dispatcher = ActionDispatcher::new(&mut tx);
        let status =
            dispatcher.execute_sequence(&SequenceName("InstallExecuteSequence".to_string()))?;
        assert_eq!(status, ActionStatus::Success);
        Ok(())
    }

    /// Tests standard action and Type 51 custom action dispatch.
    #[test]
    #[allow(clippy::unnecessary_wraps)]
    fn test_dispatcher_standard_and_type51_actions() -> Result<()> {
        let mut db = LinkedDatabase::default();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("OtherCA".to_string()),
                FieldValue::Short(51),
                FieldValue::String("OTHER_PROP".to_string()),
                FieldValue::String("val".to_string()),
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("SetInstallDir".to_string()),
                FieldValue::Long(51),
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("/opt/myapp".to_string()),
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NoOpCA".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NullTargetErrCA".to_string()),
                FieldValue::Short(19),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        assert_eq!(
            dispatcher.dispatch_action("AppSearch")?,
            ActionStatus::Success
        );
        assert_eq!(
            dispatcher.dispatch_action("SetInstallDir")?,
            ActionStatus::Success
        );
        assert_eq!(dispatcher.dispatch_action("NoOpCA")?, ActionStatus::Success);
        assert!(dispatcher.dispatch_action("NullTargetErrCA").is_err());
        assert_eq!(
            transaction.context().get_property("INSTALLFOLDER"),
            Some("/opt/myapp")
        );
        Ok(())
    }

    /// Tests error reporting when querying a missing sequence table.
    #[test]
    fn test_execute_sequence_missing_table() {
        let mut transaction = Transaction::new(
            LinkedDatabase::default(),
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let res = dispatcher.execute_sequence(&SequenceName::from("NonExistentSequence"));
        assert!(res
            .err()
            .is_some_and(|err| err.to_string().contains("Failed to query sequence table")));
    }

    /// Tests successful execution of `InstallExecuteSequence` with condition filtering.
    #[test]
    #[allow(clippy::unnecessary_wraps)]
    fn test_execute_sequence_success() -> Result<()> {
        let mut db = LinkedDatabase::default();

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("AppSearch".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("SomeSkippedAction".to_string()),
                FieldValue::String("0".to_string()),
                FieldValue::Short(150),
            ]),
        );

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("LaunchConditions".to_string()),
                FieldValue::String("1".to_string()),
                FieldValue::Short(200),
            ]),
        );

        db.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("1".to_string()),
                FieldValue::String("Always passes".to_string()),
            ]),
        );
        db.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::String("Null condition".to_string()),
            ]),
        );

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let result = dispatcher.execute_sequence(&SequenceName::from("InstallExecuteSequence"))?;
        assert_eq!(result, ActionStatus::Success);
        Ok(())
    }

    /// Tests Type 19 error custom action aborting sequence execution.
    #[test]
    fn test_execute_sequence_type19_error_action() {
        let mut db = LinkedDatabase::default();

        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("AbortInstall".to_string()),
                FieldValue::Short(19),
                FieldValue::Null,
                FieldValue::String("Prerequisite missing".to_string()),
            ]),
        );

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("AbortInstall".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let res = dispatcher.execute_sequence(&SequenceName::from("InstallExecuteSequence"));
        assert!(res
            .err()
            .is_some_and(|err| err.to_string().contains("Prerequisite missing")));
    }

    /// Tests `LaunchConditions` returning `ActionStatus::Failure` when a condition evaluates to false.
    #[test]
    #[allow(clippy::unnecessary_wraps)]
    fn test_execute_sequence_launch_condition_failure() -> Result<()> {
        let mut db = LinkedDatabase::default();

        db.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("0".to_string()),
                FieldValue::String("Failed launch condition".to_string()),
            ]),
        );

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("LaunchConditions".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let result = dispatcher.execute_sequence(&SequenceName::from("InstallExecuteSequence"))?;
        assert_eq!(result, ActionStatus::Failure);
        Ok(())
    }

    /// Tests top-level sequence actions (`INSTALL`, `ADMIN`, `ADVERTISE`, `ExecuteAction`) and immediate standard actions.
    #[test]
    #[allow(clippy::unnecessary_wraps, clippy::too_many_lines)]
    fn test_dispatcher_top_level_and_immediate_actions() -> Result<()> {
        let mut db = LinkedDatabase::default();
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ValidateProductID".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ResolveSource".to_string()),
                FieldValue::Null,
                FieldValue::Short(20),
            ]),
        );
        db.add_record(
            "AdminUISequence",
            Record::with_fields(vec![
                FieldValue::String("DisableRollback".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        db.add_record(
            "AdminExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ScheduleReboot".to_string()),
                FieldValue::Null,
                FieldValue::Short(20),
            ]),
        );
        db.add_record(
            "AdvtExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ForceReboot".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        let ini_tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(ini_tmp.path(), "[Sec]\nKey=FoundVal\n")?;
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("FOUND_PROP".to_string()),
                FieldValue::String("IniSig".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("IniSig".to_string()),
                FieldValue::String(ini_tmp.path().to_string_lossy().to_string()),
                FieldValue::String("Sec".to_string()),
                FieldValue::String("Key".to_string()),
                FieldValue::Short(0),
                FieldValue::Short(2),
            ]),
        );
        db.add_record("CCPSearch", Record::with_fields(vec![FieldValue::Short(1)]));
        db.add_record(
            "CCPSearch",
            Record::with_fields(vec![FieldValue::String("CCP_PROP".to_string())]),
        );

        let mut ctx = EvaluationContext::new();
        ctx.set_property("PIDKEY", "KEY-999");
        ctx.set_property("CCP_PROP", "1");
        ctx.set_property("OriginalDatabase", "/tmp/pkg/setup.msi");

        let mut transaction = Transaction::new(db, ctx, crate::execution::DiskCostEngine::new());
        let mut dispatcher = ActionDispatcher::new(&mut transaction);

        assert_eq!(
            dispatcher.dispatch_action("INSTALL")?,
            ActionStatus::Success
        );
        assert_eq!(dispatcher.dispatch_action("ADMIN")?, ActionStatus::Success);
        assert_eq!(
            dispatcher.dispatch_action("ADVERTISE")?,
            ActionStatus::Success
        );
        assert_eq!(
            dispatcher.dispatch_action("ExecuteAction")?,
            ActionStatus::Success
        );
        assert_eq!(
            dispatcher.dispatch_action("AppSearch")?,
            ActionStatus::Success
        );
        assert_eq!(
            dispatcher.dispatch_action("CCPSearch")?,
            ActionStatus::Success
        );
        assert_eq!(
            dispatcher.dispatch_action("RMCCPSearch")?,
            ActionStatus::Success
        );

        assert_eq!(
            transaction.context().get_property("FOUND_PROP"),
            Some("FoundVal")
        );

        assert_eq!(
            transaction.context().get_property("ProductID"),
            Some("KEY-999")
        );
        assert_eq!(transaction.context().get_property("CCP_SUCCESS"), Some("1"));
        assert_eq!(
            transaction.context().get_property("RollbackDisabled"),
            Some("1")
        );
        assert_eq!(
            transaction.context().get_property("ForceRebootTriggered"),
            Some("1")
        );
        assert!(transaction.context().get_property("SourceDir").is_some());

        // Test top-level sequences when tables are absent
        let mut empty_tx = Transaction::new(
            LinkedDatabase::default(),
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );
        let mut empty_disp = ActionDispatcher::new(&mut empty_tx);
        assert_eq!(
            empty_disp.dispatch_action("INSTALL")?,
            ActionStatus::Success
        );
        assert_eq!(empty_disp.dispatch_action("ADMIN")?, ActionStatus::Success);
        assert_eq!(
            empty_disp.dispatch_action("ADVERTISE")?,
            ActionStatus::Success
        );
        assert_eq!(
            empty_disp.dispatch_action("ExecuteAction")?,
            ActionStatus::Success
        );
        assert_eq!(
            empty_disp.dispatch_action("ValidateProductID")?,
            ActionStatus::Success
        );
        assert_eq!(
            empty_disp.dispatch_action("ResolveSource")?,
            ActionStatus::Success
        );
        // Second call when SourceDir is already set
        assert_eq!(
            empty_disp.dispatch_action("ResolveSource")?,
            ActionStatus::Success
        );

        // Test UI sequence non-success early return in INSTALL and ADMIN
        let mut fail_db = LinkedDatabase::default();
        fail_db.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("0".to_string()),
                FieldValue::String("Fail".to_string()),
            ]),
        );
        fail_db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("LaunchConditions".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        fail_db.add_record(
            "AdminUISequence",
            Record::with_fields(vec![
                FieldValue::String("LaunchConditions".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        let mut fail_tx = Transaction::new(
            fail_db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );
        let mut fail_disp = ActionDispatcher::new(&mut fail_tx);
        assert_eq!(fail_disp.dispatch_action("INSTALL")?, ActionStatus::Failure);
        assert_eq!(fail_disp.dispatch_action("ADMIN")?, ActionStatus::Failure);

        Ok(())
    }
}
