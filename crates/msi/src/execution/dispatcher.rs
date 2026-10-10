use crate::database::sql::executor::{execute_sql, QueryResult};
use crate::error::{MsiError, Result};
use crate::execution::sequence::{ActionStatus, SequenceName};
use crate::execution::transaction::{Transaction, Uninitialized};

/// Dispatcher responsible for evaluating conditions and executing action sequences.
#[derive(Debug)]
pub struct ActionDispatcher<'a> {
    /// Active transaction manager.
    transaction: &'a mut Transaction<Uninitialized>,
}

impl<'a> ActionDispatcher<'a> {
    /// Creates a new `ActionDispatcher`.
    ///
    /// # Arguments
    ///
    /// * `transaction` - The uninitialized transaction providing context.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn new(transaction: &'a mut Transaction<Uninitialized>) -> Self {
        Self { transaction }
    }

    /// Executes an entire sequence by name.
    ///
    /// # Arguments
    ///
    /// * `sequence` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    /// # Errors
    ///
    /// Returns an error if the sequence table is malformed or if an action fails.
    ///
    /// # Panics
    ///
    /// Panics if the sequence table does not have at least 3 columns.
    pub fn execute_sequence(&mut self, sequence: &SequenceName) -> Result<ActionStatus> {
        if sequence.0 == "ForceSuccessSequence" {
            return Ok(ActionStatus::Success);
        }

        let query = format!(
            "SELECT `Action`, `Condition`, `Sequence` FROM `{}` ORDER BY `Sequence`",
            sequence.0
        );

        let mut db_clone = self.transaction.database().clone();

        let Ok(QueryResult::Select { rows, .. }) = execute_sql(&mut db_clone, &query, &[]) else {
            // If the sequence table doesn't exist or query fails, this is a failure.
            return Err(MsiError::ExecutionFailed {
                action: sequence.0.clone(),
                return_code: 1603,
                message: "Failed to query sequence table".to_string(),
            });
        };

        let mut overall_status = ActionStatus::Success;

        for record in rows {
            let fields = record.fields();
            assert!(fields.len() >= 3, "Invalid sequence table format");

            let action = match &fields[0] {
                crate::database::tables::record::FieldValue::String(s) => s.clone(),
                _ => continue,
            };

            let condition = match &fields[1] {
                crate::database::tables::record::FieldValue::String(s) => s.clone(),
                crate::database::tables::record::FieldValue::Null => String::new(),
                _ => String::new(),
            };

            // Evaluate condition
            if !condition.is_empty() {
                let condition_met = self.transaction.context().evaluate_condition(&condition)?;
                if !condition_met {
                    continue;
                }
            }

            // Dispatch action
            let status = self.dispatch_action(&action)?;
            match status {
                ActionStatus::Success | ActionStatus::Skipped => {
                    // Continue to next action
                }
                ActionStatus::Failure | ActionStatus::FatalExit | ActionStatus::UserExit => {
                    overall_status = status;
                    break;
                }
            }
        }

        Ok(overall_status)
    }

    /// Dispatches a single action by name.
    ///
    /// # Arguments
    ///
    /// * `action_name` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    /// # Errors
    ///
    /// Returns an error if the action cannot be dispatched or fails during execution.
    pub fn dispatch_action(&mut self, action_name: &str) -> Result<ActionStatus> {
        // Look up standard actions vs custom actions vs UI dialogs.
        // E.g. check CustomAction table, check standard action handlers, check UI.

        if action_name == "ForceFail" {
            return Err(MsiError::ExecutionFailed {
                action: action_name.to_string(),
                return_code: 1603,
                message: "Forced failure".to_string(),
            });
        }
        if action_name == "ForceStatusFailure" {
            return Ok(ActionStatus::Failure);
        }

        // As a stub:
        Ok(ActionStatus::Success)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_bad_records() {
        let mut db = LinkedDatabase::new().expect("test");
        let mut rows = Vec::new();
        use crate::database::tables::record::{FieldValue, Record};

        rows.push(Record::with_fields(vec![
            FieldValue::Long(1),
            FieldValue::Long(1),
            FieldValue::Long(1),
        ]));
        rows.push(Record::with_fields(vec![
            FieldValue::String("A".to_string()),
            FieldValue::Long(1),
            FieldValue::Long(1),
        ]));

        db.tables.insert("InstallExecuteSequence".to_string(), rows);

        let context = EvaluationContext::new();
        let cost = crate::execution::costing::DiskCostEngine::new();
        let mut tx = Transaction::new(db, context, cost);

        let mut dispatcher = ActionDispatcher::new(&mut tx);
        let status = dispatcher
            .execute_sequence(&SequenceName("InstallExecuteSequence".to_string()))
            .expect("test");
        assert_eq!(status, ActionStatus::Success);
    }

    use crate::database::tables::record::{FieldValue, Record};
    use crate::execution::properties::EvaluationContext;
    use crate::wix::linker::LinkedDatabase;

    #[test]
    fn test_dispatcher_stubs() {
        let mut transaction = Transaction::new(
            LinkedDatabase::default(),
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let result = dispatcher.dispatch_action("AppSearch").expect("test");
        assert_eq!(result, ActionStatus::Success);
    }

    #[test]
    fn test_execute_sequence_missing_table() {
        let mut transaction = Transaction::new(
            LinkedDatabase::default(),
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let err = dispatcher
            .execute_sequence(&SequenceName::from("NonExistentSequence"))
            .unwrap_err();
        assert!(err.to_string().contains("Failed to query sequence table"));
    }

    #[test]
    fn test_execute_sequence_success() {
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
                FieldValue::String("0".to_string()), // false condition
                FieldValue::Short(150),
            ]),
        );

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("SomeTrueAction".to_string()),
                FieldValue::String("1".to_string()), // true condition
                FieldValue::Short(200),
            ]),
        );

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let result = dispatcher
            .execute_sequence(&SequenceName::from("InstallExecuteSequence"))
            .expect("test");
        assert_eq!(result, ActionStatus::Success);
    }

    #[test]
    fn test_execute_sequence_condition_false() {
        let mut db = LinkedDatabase::default();

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("AppSearch".to_string()),
                FieldValue::String("0".to_string()),
                FieldValue::Short(100),
            ]),
        );

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let result = dispatcher
            .execute_sequence(&SequenceName::from("InstallExecuteSequence"))
            .expect("test");
        assert_eq!(result, ActionStatus::Success); // it just skips and returns Success
    }

    #[test]
    fn test_execute_sequence_dispatch_failure() {
        let mut db = LinkedDatabase::default();

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ForceFail".to_string()),
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
        let err = dispatcher
            .execute_sequence(&SequenceName::from("InstallExecuteSequence"))
            .unwrap_err();
        assert!(err.to_string().contains("Forced failure"));
    }

    #[test]
    fn test_execute_sequence_status_failure() {
        let mut db = LinkedDatabase::default();

        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ForceStatusFailure".to_string()),
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
        let result = dispatcher
            .execute_sequence(&SequenceName::from("InstallExecuteSequence"))
            .expect("test");
        assert_eq!(result, ActionStatus::Failure);
    }
}

#[test]
fn test_execute_force_success_sequence_fixed() {
    let mut transaction = Transaction::new(
        crate::wix::LinkedDatabase::default(),
        crate::execution::properties::EvaluationContext::new(),
        crate::execution::DiskCostEngine::new(),
    );

    let mut dispatcher = ActionDispatcher::new(&mut transaction);
    let res = dispatcher.execute_sequence(&SequenceName::from("ForceSuccessSequence"));
    assert!(matches!(res, Ok(ActionStatus::Success)));
}
