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
        if action_name == "LaunchConditions" {
            let lc_records = self.transaction.database().get_records("LaunchCondition");
            for rec in lc_records {
                if let Some(FieldValue::String(cond)) = rec.get(0) {
                    if !self.transaction.context().evaluate_condition(cond)? {
                        return Ok(ActionStatus::Failure);
                    }
                }
            }
            return Ok(ActionStatus::Success);
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
            } else if base_type == 51 && !source.is_empty() {
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
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::database::tables::record::{FieldValue, Record};
    use crate::execution::properties::EvaluationContext;
    use crate::wix::linker::LinkedDatabase;

    #[test]
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

    #[test]
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

    #[test]
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

    #[test]
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
}
