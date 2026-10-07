use super::*;
use crate::wix::linker::LinkedDatabase;
use crate::execution::properties::EvaluationContext;
use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};

#[test]
fn test_dispatcher_stubs() {
    let mut transaction = Transaction::new(
        LinkedDatabase::default(),
        EvaluationContext::new(),
        crate::execution::costing::DiskCostEngine::new()
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
        crate::execution::costing::DiskCostEngine::new()
    );

    let mut dispatcher = ActionDispatcher::new(&mut transaction);
    let err = dispatcher.execute_sequence(&SequenceName::from("InstallExecuteSequence")).unwrap_err();
    assert!(err.to_string().contains("Failed to query sequence table"));
}

#[test]
fn test_execute_sequence_success() {
    let mut db = LinkedDatabase::default();
    
    // Create sequence table
    let schema = TableSchema::new("InstallExecuteSequence")
        .with_column(ColumnDef::new("Action", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Sequence", DataType::Short).nullable());
    db.catalog.add_table(schema).unwrap();

    db.add_record("InstallExecuteSequence", Record::with_fields(vec![
        FieldValue::String("AppSearch".to_string()),
        FieldValue::Null,
        FieldValue::Short(100),
    ]));

    let mut transaction = Transaction::new(
        db,
        EvaluationContext::new(),
        crate::execution::costing::DiskCostEngine::new()
    );

    let mut dispatcher = ActionDispatcher::new(&mut transaction);
    let result = dispatcher.execute_sequence(&SequenceName::from("InstallExecuteSequence")).unwrap();
    assert_eq!(result, ActionStatus::Success);
}

#[test]
fn test_execute_sequence_condition_false() {
    let mut db = LinkedDatabase::default();
    
    // Create sequence table
    let schema = TableSchema::new("InstallExecuteSequence")
        .with_column(ColumnDef::new("Action", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Sequence", DataType::Short).nullable());
    db.catalog.add_table(schema).unwrap();

    db.add_record("InstallExecuteSequence", Record::with_fields(vec![
        FieldValue::String("AppSearch".to_string()),
        FieldValue::String("0".to_string()), // Condition evaluates to false usually, but we need to ensure the evaluator handles it.
        FieldValue::Short(100),
    ]));

    let mut ctx = EvaluationContext::new();
    let mut transaction = Transaction::new(
        db,
        ctx,
        crate::execution::costing::DiskCostEngine::new()
    );

    let mut dispatcher = ActionDispatcher::new(&mut transaction);
    let result = dispatcher.execute_sequence(&SequenceName::from("InstallExecuteSequence")).unwrap();
    assert_eq!(result, ActionStatus::Success); // it just skips and returns Success
}
