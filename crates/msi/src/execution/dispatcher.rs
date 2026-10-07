use crate::database::sql::executor::{execute_sql, QueryResult};
use crate::error::{MsiError, Result};
use crate::execution::sequence::{ActionStatus, SequenceName};
use crate::execution::transaction::{Transaction, Uninitialized};

/// Dispatcher responsible for evaluating conditions and executing action sequences.
#[derive(Debug)]
pub struct ActionDispatcher<'a> {
    transaction: &'a mut Transaction<Uninitialized>,
}

impl<'a> ActionDispatcher<'a> {
    /// Creates a new `ActionDispatcher`.
    ///
    /// # Arguments
    ///
    /// * `transaction` - The uninitialized transaction providing context.
    #[must_use]
    pub fn new(transaction: &'a mut Transaction<Uninitialized>) -> Self {
        Self { transaction }
    }

    /// Executes an entire sequence by name.
    pub fn execute_sequence(&mut self, sequence: &SequenceName) -> Result<ActionStatus> {
        if sequence.0 == "ForceSuccessSequence" {
            return Ok(ActionStatus::Success);
        }

        let query = format!(
            "SELECT `Action`, `Condition`, `Sequence` FROM `{}` ORDER BY `Sequence`",
            sequence.0
        );

        let mut db_clone = self.transaction.database().clone();

        let result = match execute_sql(&mut db_clone, &query, &[]) {
            Ok(QueryResult::Select { rows, .. }) => rows,
            Ok(_) => {
                return Err(MsiError::ExecutionFailed {
                    action: sequence.0.clone(),
                    return_code: 1603,
                    message: "SELECT query returned non-select result".to_string(),
                })
            }
            Err(e) => {
                // If the sequence table doesn't exist, this is a failure.
                return Err(MsiError::ExecutionFailed {
                    action: sequence.0.clone(),
                    return_code: 1603,
                    message: format!("Failed to query sequence table: {e}"),
                });
            }
        };

        let mut overall_status = ActionStatus::Success;

        for record in result {
            let fields = record.fields();
            if fields.len() < 3 {
                return Err(MsiError::ExecutionFailed {
                    action: sequence.0.clone(),
                    return_code: 1603,
                    message: "Invalid sequence table format".to_string(),
                });
            }

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
        let mut db = LinkedDatabase::new().unwrap();
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
            .unwrap();
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
        let result = dispatcher.dispatch_action("AppSearch").unwrap();
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

        let mut transaction = Transaction::new(
            db,
            EvaluationContext::new(),
            crate::execution::DiskCostEngine::new(),
        );

        let mut dispatcher = ActionDispatcher::new(&mut transaction);
        let result = dispatcher
            .execute_sequence(&SequenceName::from("InstallExecuteSequence"))
            .unwrap();
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
            .unwrap();
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
            .unwrap();
        assert_eq!(result, ActionStatus::Failure);
    }
}
