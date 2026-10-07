use super::*;
use crate::database::column::{ColumnDef, DataType, MSIDB_PRIMARY_KEY};
use crate::database::tables::record::{FieldValue, Record};

#[test]
fn test_merge_config_default() {
    let config = MergeConfig::default();
    assert_eq!(config.table_name, None);
}

// TODO: write more tests.
