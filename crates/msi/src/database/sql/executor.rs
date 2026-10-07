//! SQL statement execution engine querying and mutating a [`LinkedDatabase`].

use crate::database::column::{ColumnDef, DataType};
use crate::database::sql::ast::{
    BinaryOp, Expression, OrderByTerm, OrderDirection, SqlType, SqlValue, Statement,
};
use crate::database::sql::lexer::Lexer;
use crate::database::sql::parser::Parser;
use crate::database::tables::record::{FieldValue, Record};
use crate::database::TableSchema;
use crate::error::{MsiError, Result};
use crate::wix::LinkedDatabase;
use std::cmp::Ordering;

/// Result of executing a SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResult {
    /// Select query returning projected column names and matching records.
    Select {
        /// Projected column names.
        columns: Vec<String>,
        /// Matching records.
        rows: Vec<Record>,
    },
    /// Data modification query returning number of affected rows.
    Modified(usize),
    /// Schema definition query (e.g. `CREATE TABLE`, `DROP TABLE`).
    SchemaChanged,
}

/// Executes a SQL query against a [`LinkedDatabase`] with optional parameterized bindings.
///
/// # Arguments
///
/// * `db` - Target database to query or modify.
/// * `sql` - Raw SQL query text.
/// * `params` - Optional positional parameter bindings for `?` placeholders.
///
/// # Returns
///
/// Query execution result.
///
/// # Errors
///
/// Returns [`crate::MsiError`] on syntax error, type violation, or primary key constraint failure.
pub fn execute_sql(
    db: &mut LinkedDatabase,
    sql: &str,
    params: &[FieldValue],
) -> Result<QueryResult> {
    let mut lexer = Lexer::new(sql);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let stmt = parser.parse()?;

    execute_statement(db, &stmt, params)
}
#[allow(clippy::cognitive_complexity)]
/// Executes a parsed [`Statement`] against a [`LinkedDatabase`].
///
/// # Arguments
///
/// * `db` - Target database.
/// * `stmt` - Parsed SQL statement AST.
/// * `params` - Parameter bindings for `?` placeholders.
///
/// # Returns
///
/// Query execution result.
///
/// # Errors
///
/// Returns [`crate::MsiError`] on constraint violation or schema failure.
#[allow(clippy::too_many_lines)]
pub fn execute_statement(
    db: &mut LinkedDatabase,
    stmt: &Statement,
    params: &[FieldValue],
) -> Result<QueryResult> {
    match stmt {
        Statement::Select {
            distinct,
            columns,
            table,
            joins,
            where_clause,
            order_by,
        } => {
            let mut schemas = Vec::new();

            let primary_schema =
                db.catalog
                    .get_table(table.as_str())
                    .ok_or_else(|| MsiError::Sql {
                        message: format!("table '{}' does not exist in database", table.as_str()),
                    })?;
            schemas.push(primary_schema);

            for j in joins {
                let s = db
                    .catalog
                    .get_table(j.as_str())
                    .ok_or_else(|| MsiError::Sql {
                        message: format!("table '{}' does not exist in database", j.as_str()),
                    })?;
                schemas.push(s);
            }

            // Cartesian product of all rows
            let mut all_rows = vec![Record::new()];
            for schema_ref in &schemas {
                let table_rows = db.tables.get(&schema_ref.name).cloned().unwrap_or_default();
                let mut new_rows = Vec::new();
                for existing in &all_rows {
                    for r in &table_rows {
                        let mut combined = existing.fields().to_vec();
                        combined.extend(r.fields().iter().cloned());
                        new_rows.push(Record::with_fields(combined));
                    }
                }
                all_rows = new_rows;
            }

            let mut matching_rows = Vec::new();
            let mut param_idx = 0;

            for r in all_rows {
                if let Some(ref expr) = where_clause {
                    if eval_expression(expr, &r, &schemas, params, &mut param_idx)? {
                        matching_rows.push(r);
                    }
                } else {
                    matching_rows.push(r);
                }
            }

            // Handle sorting
            if !order_by.is_empty() {
                sort_records(&mut matching_rows, order_by, &schemas);
            }

            // Column projection
            let (proj_cols, proj_rows): (Vec<crate::database::sql::ast::ColumnName>, _) =
                if columns.is_empty() {
                    let mut col_names = Vec::new();
                    for s in &schemas {
                        for c in &s.columns {
                            col_names.push(crate::database::sql::ast::ColumnName(c.name.clone()));
                        }
                    }
                    (col_names, matching_rows)
                } else {
                    let mut col_indices = Vec::new();
                    for col_name in columns {
                        let idx = find_column_index(&schemas, col_name.as_str())?;
                        col_indices.push(idx);
                    }

                    let mut projected = Vec::new();
                    for r in matching_rows {
                        let mut new_r = Record::new();
                        for &idx in &col_indices {
                            if let Some(val) = r.get(idx) {
                                new_r.push(val.clone());
                            } else {
                                new_r.push(FieldValue::Null);
                            }
                        }
                        projected.push(new_r);
                    }
                    (columns.clone(), projected)
                };

            let final_rows = if *distinct {
                let mut seen = Vec::new();
                for r in proj_rows {
                    if !seen.contains(&r) {
                        seen.push(r);
                    }
                }
                seen
            } else {
                proj_rows
            };

            Ok(QueryResult::Select {
                columns: proj_cols.into_iter().map(|c| c.0).collect(),
                rows: final_rows,
            })
        }
        Statement::Insert {
            table,
            columns,
            values,
        } => {
            let schema = db
                .catalog
                .get_table(table.as_str())
                .ok_or_else(|| MsiError::Sql {
                    message: format!("table '{}' does not exist in database", table.as_str()),
                })?;

            let mut record = Record::new();
            let mut param_idx = 0;

            if let Some(col_names) = columns {
                if col_names.len() != values.len() {
                    return Err(MsiError::Sql {
                        message: "column count does not match values count in INSERT".to_string(),
                    });
                }
                for col in &schema.columns {
                    if let Some(pos) = col_names
                        .iter()
                        .position(|c| c.0.eq_ignore_ascii_case(&col.name))
                    {
                        let fv =
                            resolve_sql_value(&values[pos], params, &mut param_idx, None, None)?;
                        record.push(fv);
                    } else if col.nullable {
                        record.push(FieldValue::Null);
                    } else {
                        return Err(MsiError::Sql {
                            message: format!("missing non-null column '{}' in INSERT", col.name),
                        });
                    }
                }
            } else {
                for v in values {
                    let fv = resolve_sql_value(v, params, &mut param_idx, None, None)?;
                    record.push(fv);
                }
            }

            // Enforce primary key constraint
            let pk_indices: Vec<usize> = schema
                .columns
                .iter()
                .enumerate()
                .filter_map(|(idx, c)| c.primary_key.then_some(idx))
                .collect();

            let rows = db.tables.entry(table.as_str().to_string()).or_default();
            if !pk_indices.is_empty() {
                for existing in rows.iter() {
                    let matches_pk = pk_indices.iter().all(|&i| existing.get(i) == record.get(i));
                    if matches_pk {
                        return Err(MsiError::Sql {
                            message: format!(
                                "duplicate primary key in table '{table_str}'",
                                table_str = table.as_str()
                            ),
                        });
                    }
                }
            }

            rows.push(record);
            Ok(QueryResult::Modified(1))
        }
        Statement::Update {
            table,
            assignments,
            where_clause,
        } => {
            let schema = db
                .catalog
                .get_table(table.as_str())
                .ok_or_else(|| MsiError::Sql {
                    message: format!("table '{}' does not exist in database", table.as_str()),
                })?;

            let mut assign_indices = Vec::new();
            for (col_name, val) in assignments {
                let idx = schema
                    .columns
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(col_name.as_str()))
                    .ok_or_else(|| MsiError::Sql {
                        message: format!(
                            "column '{}' does not exist in table '{}'",
                            col_name.as_str(),
                            table.as_str()
                        ),
                    })?;
                assign_indices.push((idx, val));
            }

            let rows = db.tables.entry(table.as_str().to_string()).or_default();
            let mut modified_count = 0;
            let mut param_idx = 0;

            for r in rows.iter_mut() {
                let matches = if let Some(ref expr) = where_clause {
                    eval_expression(expr, r, &[schema], params, &mut param_idx)?
                } else {
                    true
                };

                if matches {
                    for (idx, val) in &assign_indices {
                        let fv = resolve_sql_value(val, params, &mut param_idx, None, None)?;
                        r.set(*idx, fv);
                    }
                    modified_count += 1;
                }
            }

            Ok(QueryResult::Modified(modified_count))
        }
        Statement::Delete {
            table,
            where_clause,
        } => {
            let schema = db
                .catalog
                .get_table(table.as_str())
                .ok_or_else(|| MsiError::Sql {
                    message: format!("table '{}' does not exist in database", table.as_str()),
                })?;

            let rows = db.tables.entry(table.as_str().to_string()).or_default();
            let initial_len = rows.len();
            let mut param_idx = 0;

            if let Some(ref expr) = where_clause {
                let mut kept = Vec::new();
                for r in rows.drain(..) {
                    if !eval_expression(expr, &r, &[schema], params, &mut param_idx)? {
                        kept.push(r);
                    }
                }
                *rows = kept;
            } else {
                rows.clear();
            }

            let deleted_count = initial_len - rows.len();
            Ok(QueryResult::Modified(deleted_count))
        }
        Statement::CreateTable { table, columns } => {
            let mut table_schema = TableSchema::new(table.as_str());
            for c in columns {
                let dt = match c.data_type {
                    SqlType::String => DataType::String { max_len: c.length },
                    SqlType::Short => DataType::Short,
                    SqlType::Long => DataType::Long,
                    SqlType::Stream => DataType::Stream,
                };
                let mut col_def = ColumnDef::new(c.name.as_str(), dt);
                if !c.not_null {
                    col_def = col_def.nullable();
                }
                if c.primary_key {
                    col_def = col_def.primary_key();
                }
                if c.localizable {
                    col_def = col_def.localizable();
                }
                table_schema = table_schema.with_column(col_def);
            }

            let _ = db.catalog.add_table(table_schema);
            db.tables.insert(table.as_str().to_string(), Vec::new());

            Ok(QueryResult::SchemaChanged)
        }
        Statement::AlterTable { table, column, .. } => {
            // Check if table exists
            let mut schema = db
                .catalog
                .get_table(table.as_str())
                .cloned()
                .ok_or_else(|| MsiError::Sql {
                    message: format!("table '{}' does not exist in database", table.as_str()),
                })?;

            if let Some(column) = column.as_ref() {
                let dt = match column.data_type {
                    SqlType::String => DataType::String {
                        max_len: column.length,
                    },
                    SqlType::Short => DataType::Short,
                    SqlType::Long => DataType::Long,
                    SqlType::Stream => DataType::Stream,
                };
                let mut col_def = ColumnDef::new(column.name.as_str(), dt);
                if !column.not_null {
                    col_def = col_def.nullable();
                }
                if column.primary_key {
                    col_def = col_def.primary_key();
                }
                if column.localizable {
                    col_def = col_def.localizable();
                }

                schema = schema.with_column(col_def);
                let _ = db.catalog.add_table(schema);
            }

            // HOLD and FREE are memory management hints in MSI.
            // For `msi-rs`, we don't strictly need to do anything since our catalog is always in memory,
            // but returning SchemaChanged maintains expected behavior.

            Ok(QueryResult::SchemaChanged)
        }
        Statement::DropTable { table } => {
            let table_name = table.as_str();

            // Clean up streams associated with the table if any.
            if let Some(schema) = db.catalog.get_table(table_name) {
                let has_stream_col = schema
                    .columns
                    .iter()
                    .any(|c| matches!(c.data_type, DataType::Stream));
                if has_stream_col {
                    if let Some(streams) = db.tables.get_mut("_Streams") {
                        let prefix = format!("{table_name}.");
                        streams.retain(|r| {
                            if let Some(FieldValue::String(name)) = r.get(0) {
                                !name.starts_with(&prefix)
                            } else {
                                true
                            }
                        });
                    }
                }
            }

            db.tables.remove(table_name);
            db.catalog.remove_table(table_name);
            Ok(QueryResult::SchemaChanged)
        }
    }
}

/// Resolves a [`SqlValue`] into a typed [`FieldValue`].
fn resolve_sql_value(
    val: &SqlValue,
    params: &[FieldValue],
    param_idx: &mut usize,
    record: Option<&Record>,
    schemas: Option<&[&TableSchema]>,
) -> Result<FieldValue> {
    match val {
        SqlValue::Column(col_name) => {
            if let (Some(r), Some(s)) = (record, schemas) {
                let idx = find_column_index(s, col_name.as_str())?;
                Ok(r.get(idx).cloned().unwrap_or(FieldValue::Null))
            } else {
                Err(MsiError::Sql {
                    message: format!(
                        "Cannot evaluate column {} in this context",
                        col_name.as_str()
                    ),
                })
            }
        }
        SqlValue::String(s) => Ok(FieldValue::String(s.clone())),
        SqlValue::Integer(n) => Ok(FieldValue::Long(*n)),
        SqlValue::Null => Ok(FieldValue::Null),
        SqlValue::Parameter => {
            if *param_idx < params.len() {
                let res = params[*param_idx].clone();
                *param_idx += 1;
                Ok(res)
            } else {
                Err(MsiError::Sql {
                    message: format!("missing parameter binding at index {param_idx}"),
                })
            }
        }
    }
}

/// Evaluates a SQL expression against a record row.

/// Finds the index of a column in a list of schemas, handling optional table prefixes.
fn find_column_index(schemas: &[&TableSchema], col_ref: &str) -> Result<usize> {
    let mut match_idx = None;
    let mut current_offset = 0;

    let parts: Vec<&str> = col_ref.split('.').collect();
    let (target_table, target_col) = if parts.len() == 2 {
        (Some(parts[0]), parts[1])
    } else {
        (None, col_ref)
    };

    for schema in schemas {
        for col in &schema.columns {
            let table_matches = target_table.is_none_or(|t| t.eq_ignore_ascii_case(&schema.name));
            if table_matches && col.name.eq_ignore_ascii_case(target_col) {
                if match_idx.is_some() {
                    return Err(MsiError::Sql {
                        message: format!("ambiguous column reference '{col_ref}'"),
                    });
                }
                match_idx = Some(current_offset);
            }
            current_offset += 1;
        }
    }

    match_idx.ok_or_else(|| MsiError::Sql {
        message: format!("column '{col_ref}' not found in any of the joined tables"),
    })
}

fn eval_expression(
    expr: &Expression,
    record: &Record,
    schemas: &[&TableSchema],
    params: &[FieldValue],
    param_idx: &mut usize,
) -> Result<bool> {
    match expr {
        Expression::Comparison { column, op, value } => {
            let col_idx = find_column_index(schemas, column.as_str())?;

            let left = record.get(col_idx).unwrap_or(&FieldValue::Null);
            let right = resolve_sql_value(value, params, param_idx, Some(record), Some(schemas))?;

            Ok(eval_binary_op(left, *op, &right))
        }
        Expression::IsNull { column, negated } => {
            let col_idx = find_column_index(schemas, column.as_str())?;

            let is_null = record.get(col_idx).is_none_or(FieldValue::is_null);
            if *negated {
                Ok(!is_null)
            } else {
                Ok(is_null)
            }
        }
        Expression::And(left, right) => {
            let l = eval_expression(left, record, schemas, params, param_idx)?;
            let r = eval_expression(right, record, schemas, params, param_idx)?;
            Ok(l && r)
        }
        Expression::Or(left, right) => {
            let l = eval_expression(left, record, schemas, params, param_idx)?;
            let r = eval_expression(right, record, schemas, params, param_idx)?;
            Ok(l || r)
        }
        Expression::Not(inner) => {
            let res = eval_expression(inner, record, schemas, params, param_idx)?;
            Ok(!res)
        }
    }
}

/// Compares two field values according to a binary operator.
fn eval_binary_op(left: &FieldValue, op: BinaryOp, right: &FieldValue) -> bool {
    match (left, right) {
        (FieldValue::Null, FieldValue::Null) => op == BinaryOp::Equal,
        (FieldValue::Null, _) | (_, FieldValue::Null) => op == BinaryOp::NotEqual,
        (FieldValue::String(s1), FieldValue::String(s2)) => match op {
            BinaryOp::Equal => s1 == s2,
            BinaryOp::NotEqual => s1 != s2,
            BinaryOp::LessThan => s1 < s2,
            BinaryOp::GreaterThan => s1 > s2,
            BinaryOp::LessOrEqual => s1 <= s2,
            BinaryOp::GreaterOrEqual => s1 >= s2,
            BinaryOp::Like => match_like_pattern(s1, s2),
        },
        (FieldValue::Short(n1), FieldValue::Short(n2)) => {
            eval_num_op(i32::from(*n1), op, i32::from(*n2))
        }
        (FieldValue::Long(n1), FieldValue::Long(n2)) => eval_num_op(*n1, op, *n2),
        (FieldValue::Short(n1), FieldValue::Long(n2)) => eval_num_op(i32::from(*n1), op, *n2),
        (FieldValue::Long(n1), FieldValue::Short(n2)) => eval_num_op(*n1, op, i32::from(*n2)),
        _ => false,
    }
}

/// Compares two numeric values according to a binary operator.
const fn eval_num_op(n1: i32, op: BinaryOp, n2: i32) -> bool {
    match op {
        BinaryOp::Equal => n1 == n2,
        BinaryOp::NotEqual => n1 != n2,
        BinaryOp::LessThan => n1 < n2,
        BinaryOp::GreaterThan => n1 > n2,
        BinaryOp::LessOrEqual => n1 <= n2,
        BinaryOp::GreaterOrEqual => n1 >= n2,
        BinaryOp::Like => false,
    }
}

/// Matches simple SQL `LIKE` wildcard patterns (`%` for any string, `_` for single char).
fn match_like_pattern(text: &str, pattern: &str) -> bool {
    let mut text_chars = text.chars();
    let mut pattern_chars = pattern.chars().peekable();

    while let Some(pc) = pattern_chars.next() {
        match pc {
            '%' => {
                if pattern_chars.peek().is_none() {
                    return true;
                }
                let rest_pattern: String = pattern_chars.collect();
                while !text_chars.clone().collect::<String>().is_empty() {
                    if match_like_pattern(&text_chars.clone().collect::<String>(), &rest_pattern) {
                        return true;
                    }
                    text_chars.next();
                }
                return false;
            }
            '_' => {
                if text_chars.next().is_none() {
                    return false;
                }
            }
            literal => {
                if text_chars.next() != Some(literal) {
                    return false;
                }
            }
        }
    }

    text_chars.next().is_none()
}

/// Sorts records in-place by `ORDER BY` terms.
fn sort_records(records: &mut [Record], order_by: &[OrderByTerm], schemas: &[&TableSchema]) {
    let order_indices: Vec<(usize, OrderDirection)> = order_by
        .iter()
        .filter_map(|term| {
            find_column_index(schemas, term.column.as_str())
                .ok()
                .map(|idx| (idx, term.direction))
        })
        .collect();

    records.sort_by(|r1, r2| {
        for &(idx, dir) in &order_indices {
            let v1 = r1.get(idx);
            let v2 = r2.get(idx);
            let ord = match (v1, v2) {
                (Some(FieldValue::Short(s1)), Some(FieldValue::Short(s2))) => s1.cmp(s2),
                (Some(FieldValue::Long(l1)), Some(FieldValue::Long(l2))) => l1.cmp(l2),
                (Some(FieldValue::String(s1)), Some(FieldValue::String(s2))) => s1.cmp(s2),
                (Some(FieldValue::Null), Some(FieldValue::Null)) => Ordering::Equal,
                (Some(FieldValue::Null), _) => Ordering::Less,
                (_, Some(FieldValue::Null)) => Ordering::Greater,
                _ => Ordering::Equal,
            };
            if ord != Ordering::Equal {
                return if dir == OrderDirection::Descending {
                    ord.reverse()
                } else {
                    ord
                };
            }
        }
        Ordering::Equal
    });
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    include!("tests_extended.rs");

    fn get_select_rows(res: QueryResult) -> Vec<Record> {
        match res {
            QueryResult::Select { rows, .. } => rows,
            QueryResult::Modified(_) | QueryResult::SchemaChanged => Vec::new(),
        }
    }

    #[test]
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    fn test_execute_sql_crud_and_schema_lifecycle() {
        let mut db = LinkedDatabase::default();

        // 1. CREATE TABLE
        let create_sql = "CREATE TABLE CustomTest (Id CHAR(72) NOT NULL PRIMARY KEY, Score LONG, Description VARCHAR(255))";
        assert_eq!(
            execute_sql(&mut db, create_sql, &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        assert!(db.tables.contains_key("CustomTest"));

        // 2. INSERT INTO with explicit columns and positional parameters
        let insert_sql =
            "INSERT INTO CustomTest (Id, Score, Description) VALUES ('item1', 100, 'first item')";
        assert_eq!(
            execute_sql(&mut db, insert_sql, &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );

        let insert_param_sql = "INSERT INTO CustomTest VALUES (?, ?, ?)";
        let params = vec![
            FieldValue::String("item2".to_string()),
            FieldValue::Long(200),
            FieldValue::String("second item".to_string()),
        ];
        assert_eq!(
            execute_sql(&mut db, insert_param_sql, &params).as_ref(),
            Ok(&QueryResult::Modified(1))
        );

        // Test duplicate primary key error
        assert!(execute_sql(&mut db, insert_sql, &[]).is_err());

        // 3. SELECT query with WHERE and ORDER BY
        let select_sql = "SELECT Id, Score FROM CustomTest WHERE Score >= 100 ORDER BY Score DESC";
        assert_eq!(
            execute_sql(&mut db, select_sql, &[]).as_ref(),
            Ok(&QueryResult::Select {
                columns: vec!["Id".to_string(), "Score".to_string()],
                rows: vec![
                    Record::with_fields(vec![
                        FieldValue::String("item2".to_string()),
                        FieldValue::Long(200),
                    ]),
                    Record::with_fields(vec![
                        FieldValue::String("item1".to_string()),
                        FieldValue::Long(100),
                    ]),
                ],
            })
        );

        // Test SELECT wildcard and DISTINCT
        let select_wildcard = "SELECT * FROM CustomTest WHERE Description LIKE '%item%'";
        assert_eq!(
            execute_sql(&mut db, select_wildcard, &[]).as_ref(),
            Ok(&QueryResult::Select {
                columns: vec![
                    "Id".to_string(),
                    "Score".to_string(),
                    "Description".to_string()
                ],
                rows: vec![
                    Record::with_fields(vec![
                        FieldValue::String("item1".to_string()),
                        FieldValue::Long(100),
                        FieldValue::String("first item".to_string()),
                    ]),
                    Record::with_fields(vec![
                        FieldValue::String("item2".to_string()),
                        FieldValue::Long(200),
                        FieldValue::String("second item".to_string()),
                    ]),
                ],
            })
        );

        // 4. UPDATE query
        let update_sql = "UPDATE CustomTest SET Score = 150 WHERE Id = 'item1'";
        assert_eq!(
            execute_sql(&mut db, update_sql, &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );

        // 5. DELETE query
        let delete_sql = "DELETE FROM CustomTest WHERE Score = 150";
        assert_eq!(
            execute_sql(&mut db, delete_sql, &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );

        // 6. ALTER TABLE
        let alter_sql = "ALTER TABLE CustomTest ADD ExtraCol SHORT HOLD";
        assert_eq!(
            execute_sql(&mut db, alter_sql, &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );

        // 7. DROP TABLE
        let drop_sql = "DROP TABLE CustomTest";
        assert_eq!(
            execute_sql(&mut db, drop_sql, &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        assert!(!db.tables.contains_key("CustomTest"));
    }
    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    fn test_executor_all_uncovered_paths() {
        let mut db = LinkedDatabase::default();

        // CREATE TABLE without PK and insert into it (pk_indices.is_empty() branch)
        let no_pk_table = "CREATE TABLE NoPk (Col1 VARCHAR(64))";
        assert_eq!(
            execute_sql(&mut db, no_pk_table, &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        assert_eq!(
            execute_sql(&mut db, "INSERT INTO NoPk VALUES ('val1')", &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );

        // CREATE TABLE with all column types (String, Short, Long, Stream, Nullable, PK, Localizable)
        let create_sql = "CREATE TABLE AllTypes (Id CHAR(32) NOT NULL PRIMARY KEY, S SHORT, L LONG, St OBJECT, Txt VARCHAR(64) LOCALIZABLE)";
        assert_eq!(
            execute_sql(&mut db, create_sql, &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );

        // INSERT multiple rows with different PKs (matches_pk == false branch)
        let ins1 = "INSERT INTO AllTypes (Id, S, L, Txt) VALUES ('id1', 10, 100, 'text1')";
        assert_eq!(
            execute_sql(&mut db, ins1, &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );
        let ins2 = "INSERT INTO AllTypes (Id, S, L, Txt) VALUES ('id2', 20, 200, 'text2')";
        assert_eq!(
            execute_sql(&mut db, ins2, &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );
        let ins3 = "INSERT INTO AllTypes (Id, S, L, Txt) VALUES ('id3', 30, 300, 'text3')";
        assert_eq!(
            execute_sql(&mut db, ins3, &[]).as_ref(),
            Ok(&QueryResult::Modified(1))
        );

        // SELECT without WHERE on non-empty table (line 103: matching_rows.push(r))
        let sel_all = "SELECT * FROM AllTypes";
        let res_all = execute_sql(&mut db, sel_all, &[]);
        assert_eq!(
            res_all.as_ref().map(|r| get_select_rows(r.clone()).len()),
            Ok(3)
        );
        assert_eq!(get_select_rows(QueryResult::Modified(0)), Vec::new());
        assert_eq!(get_select_rows(QueryResult::SchemaChanged), Vec::new());

        // Expression evaluation: l && r where l is true but r is false (line 430 false branch)
        let sel_and_false = "SELECT * FROM AllTypes WHERE Id = 'id1' AND S = 999";
        let res_and = execute_sql(&mut db, sel_and_false, &[]);
        assert_eq!(
            res_and.as_ref().map(|r| get_select_rows(r.clone()).len()),
            Ok(0)
        );

        // SELECT with short record (r.get(idx) is None -> FieldValue::Null)
        let short_table = "CREATE TABLE ShortRows (Col1 CHAR(10) NOT NULL PRIMARY KEY, Col2 LONG)";
        assert_eq!(
            execute_sql(&mut db, short_table, &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        let mut short_rec = Record::new();
        short_rec.push(FieldValue::String("pk_only".to_string()));
        db.tables.insert("ShortRows".to_string(), vec![short_rec]);
        let sel_proj_null = "SELECT Col2 FROM ShortRows";
        assert_eq!(
            execute_sql(&mut db, sel_proj_null, &[]).as_ref(),
            Ok(&QueryResult::Select {
                columns: vec!["Col2".to_string()],
                rows: vec![Record::with_fields(vec![FieldValue::Null])],
            })
        );

        // SELECT DISTINCT with identical rows (seen.contains(&r) == true)
        db.tables.insert(
            "ShortRows".to_string(),
            vec![
                Record::with_fields(vec![
                    FieldValue::String("same".to_string()),
                    FieldValue::Long(5),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("same".to_string()),
                    FieldValue::Long(5),
                ]),
            ],
        );
        let sel_distinct = "SELECT DISTINCT Col1 FROM ShortRows";
        assert_eq!(
            execute_sql(&mut db, sel_distinct, &[]).as_ref(),
            Ok(&QueryResult::Select {
                columns: vec!["Col1".to_string()],
                rows: vec![Record::with_fields(vec![FieldValue::String(
                    "same".to_string()
                )])],
            })
        );

        // INSERT errors: column count mismatch & missing non-null column
        let err_col_count = "INSERT INTO AllTypes (Id, S) VALUES ('bad_count')";
        assert!(execute_sql(&mut db, err_col_count, &[]).is_err());

        let err_missing_nonnull = "INSERT INTO AllTypes (S) VALUES (10)";
        assert!(execute_sql(&mut db, err_missing_nonnull, &[]).is_err());

        // UPDATE error: non-existent column
        let err_upd_col = "UPDATE AllTypes SET NonExistent = 1 WHERE Id = 'id1'";
        assert!(execute_sql(&mut db, err_upd_col, &[]).is_err());

        // ALTER TABLE: Long, Stream, Localizable, Primary Key, and Nullable
        assert_eq!(
            execute_sql(&mut db, "ALTER TABLE AllTypes ADD ExtraLong LONG", &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        assert_eq!(
            execute_sql(&mut db, "ALTER TABLE AllTypes ADD ExtraStream OBJECT", &[]).as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        assert_eq!(
            execute_sql(
                &mut db,
                "ALTER TABLE AllTypes ADD ExtraLoc VARCHAR LOCALIZABLE",
                &[]
            )
            .as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );
        assert_eq!(
            execute_sql(
                &mut db,
                "ALTER TABLE AllTypes ADD ExtraPK CHAR(10) NOT NULL PRIMARY KEY",
                &[]
            )
            .as_ref(),
            Ok(&QueryResult::SchemaChanged)
        );

        // Expression evaluation: missing parameter error
        assert!(execute_sql(&mut db, "SELECT * FROM AllTypes WHERE Id = ?", &[]).is_err());

        // Expression evaluation: unknown column in IsNull
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM AllTypes WHERE UnknownCol IS NULL",
            &[]
        )
        .is_err());
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM AllTypes WHERE UnknownCol IS NOT NULL",
            &[]
        )
        .is_err());

        // Expression evaluation and parameter resolution errors
        assert!(execute_sql(&mut db, "INSERT INTO NoPk (Col1) VALUES (?)", &[]).is_err());
        assert!(execute_sql(&mut db, "INSERT INTO NoPk VALUES (?)", &[]).is_err());
        assert!(execute_sql(
            &mut db,
            "UPDATE ShortRows SET Col2 = ? WHERE Col1 = 'same'",
            &[]
        )
        .is_err());
        assert!(execute_sql(
            &mut db,
            "UPDATE ShortRows SET Col2 = 1 WHERE UnknownCol = 1",
            &[]
        )
        .is_err());
        assert!(execute_sql(&mut db, "DELETE FROM ShortRows WHERE UnknownCol = 1", &[]).is_err());

        // Re-populate ShortRows after DELETE error test
        db.tables.insert(
            "ShortRows".to_string(),
            vec![Record::with_fields(vec![
                FieldValue::String("same".to_string()),
                FieldValue::Long(5),
            ])],
        );
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM ShortRows WHERE UnknownCol = 1 AND Col1 = 'same'",
            &[]
        )
        .is_err());
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM ShortRows WHERE Col1 = 'same' AND UnknownCol = 1",
            &[]
        )
        .is_err());
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM ShortRows WHERE UnknownCol = 1 OR Col1 = 'same'",
            &[]
        )
        .is_err());
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM ShortRows WHERE Col1 = 'same' OR UnknownCol = 1",
            &[]
        )
        .is_err());
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM ShortRows WHERE NOT UnknownCol = 1",
            &[]
        )
        .is_err());
        assert!(execute_sql(&mut db, "'unclosed string token", &[]).is_err());

        // Binary operations and comparisons:
        assert!(eval_binary_op(
            &FieldValue::Null,
            BinaryOp::Equal,
            &FieldValue::Null
        ));
        assert!(eval_binary_op(
            &FieldValue::Null,
            BinaryOp::NotEqual,
            &FieldValue::Long(10)
        ));
        assert!(eval_binary_op(
            &FieldValue::Long(10),
            BinaryOp::NotEqual,
            &FieldValue::Null
        ));
        assert!(!eval_binary_op(
            &FieldValue::Null,
            BinaryOp::Equal,
            &FieldValue::Long(10)
        ));

        // String binary operations:
        assert!(eval_binary_op(
            &FieldValue::String("a".to_string()),
            BinaryOp::LessThan,
            &FieldValue::String("b".to_string())
        ));
        assert!(eval_binary_op(
            &FieldValue::String("b".to_string()),
            BinaryOp::GreaterThan,
            &FieldValue::String("a".to_string())
        ));
        assert!(eval_binary_op(
            &FieldValue::String("a".to_string()),
            BinaryOp::LessOrEqual,
            &FieldValue::String("a".to_string())
        ));
        assert!(eval_binary_op(
            &FieldValue::String("b".to_string()),
            BinaryOp::GreaterOrEqual,
            &FieldValue::String("b".to_string())
        ));
        assert!(eval_binary_op(
            &FieldValue::String("a".to_string()),
            BinaryOp::NotEqual,
            &FieldValue::String("b".to_string())
        ));

        // Cross numeric comparisons (Short vs Long and Long vs Short, and Short vs Short):
        assert!(eval_binary_op(
            &FieldValue::Short(10),
            BinaryOp::Equal,
            &FieldValue::Short(10)
        ));
        assert!(eval_binary_op(
            &FieldValue::Short(10),
            BinaryOp::Equal,
            &FieldValue::Long(10)
        ));
        assert!(eval_binary_op(
            &FieldValue::Long(20),
            BinaryOp::GreaterThan,
            &FieldValue::Short(15)
        ));
        assert!(eval_binary_op(
            &FieldValue::Short(5),
            BinaryOp::LessThan,
            &FieldValue::Long(10)
        ));
        assert!(eval_binary_op(
            &FieldValue::Long(30),
            BinaryOp::Equal,
            &FieldValue::Short(30)
        ));

        // Incompatible types and eval_num_op Like & NotEqual:
        assert!(!eval_binary_op(
            &FieldValue::String("a".to_string()),
            BinaryOp::Equal,
            &FieldValue::Long(1)
        ));
        assert!(!eval_num_op(10, BinaryOp::Like, 10));
        assert!(eval_num_op(10, BinaryOp::NotEqual, 20));

        // match_like_pattern edge cases:
        assert!(match_like_pattern("abc", "a_c"));
        assert!(!match_like_pattern("a", "a_"));
        assert!(!match_like_pattern("ab", "ac"));
        assert!(match_like_pattern("hello", "hel%"));
        assert!(match_like_pattern("abcxyzdef", "abc%def"));
        assert!(!match_like_pattern("abcxyz", "abc%def"));

        // sort_records edge cases (Short, Long, String comparison, Null vs Null, and incompatible types):
        let schema = TableSchema::new("SortTest")
            .with_column(ColumnDef::new("S", DataType::Short))
            .with_column(ColumnDef::new("L", DataType::Long))
            .with_column(ColumnDef::new("Txt", DataType::String { max_len: 0 }));

        let mut sort_rows = vec![
            Record::with_fields(vec![
                FieldValue::Short(20),
                FieldValue::Null,
                FieldValue::String("b".to_string()),
            ]),
            Record::with_fields(vec![
                FieldValue::Short(10),
                FieldValue::Long(5),
                FieldValue::String("a".to_string()),
            ]),
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Long(1),
                FieldValue::String("a".to_string()),
            ]),
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
            Record::with_fields(vec![
                FieldValue::Short(10),
                FieldValue::Long(5),
                FieldValue::String("a".to_string()),
            ]),
            Record::with_fields(vec![
                FieldValue::String("incompatible".to_string()),
                FieldValue::Long(5),
                FieldValue::Null,
            ]),
        ];

        let terms_desc = vec![
            OrderByTerm {
                column: crate::database::sql::ast::ColumnName("Txt".to_string()),
                direction: OrderDirection::Ascending,
            },
            OrderByTerm {
                column: crate::database::sql::ast::ColumnName("S".to_string()),
                direction: OrderDirection::Descending,
            },
            OrderByTerm {
                column: crate::database::sql::ast::ColumnName("L".to_string()),
                direction: OrderDirection::Ascending,
            },
        ];
        sort_records(&mut sort_rows, &terms_desc, &[&schema]);
        assert_eq!(sort_rows.len(), 6);

        let mut diff_type_rows = vec![
            Record::with_fields(vec![FieldValue::Short(1)]),
            Record::with_fields(vec![FieldValue::Long(2)]),
        ];
        let term_single = vec![OrderByTerm {
            column: crate::database::sql::ast::ColumnName("S".to_string()),
            direction: OrderDirection::Ascending,
        }];
        sort_records(&mut diff_type_rows, &term_single, &[&schema]);
        assert_eq!(diff_type_rows.len(), 2);

        // QueryResult derives:
        let res_mod = QueryResult::Modified(10);
        #[allow(clippy::redundant_clone)]
        let res_mod_clone = res_mod.clone();
        assert_eq!(res_mod, res_mod_clone);
        assert_ne!(res_mod, QueryResult::SchemaChanged);
        assert!(format!("{res_mod:?}").contains("Modified"));

        let res_sc = QueryResult::SchemaChanged;
        assert_eq!(res_sc.clone(), res_sc);
    }

    #[test]
    fn test_execute_drop_table_with_streams() {
        use crate::database::catalogs::TableSchema;
        use crate::database::column::{ColumnDef, DataType};
        use crate::database::sql::ast::{Statement, TableName};
        use crate::database::tables::record::{FieldValue, Record};
        use crate::wix::linker::LinkedDatabase;

        let mut db = LinkedDatabase::default();

        let _ = db.catalog.add_table(TableSchema {
            name: "StreamTable".to_string(),
            columns: vec![
                ColumnDef {
                    name: "Id".to_string(),
                    data_type: DataType::Long,
                    nullable: false,
                    primary_key: true,
                    localizable: false,
                },
                ColumnDef {
                    name: "Data".to_string(),
                    data_type: DataType::Stream,
                    nullable: false,
                    primary_key: false,
                    localizable: false,
                },
            ],
        });

        db.tables.insert("StreamTable".to_string(), vec![]);

        let streams = Record::with_fields(vec![
            FieldValue::String("StreamTable.1".to_string()),
            FieldValue::Null,
        ]);
        db.tables
            .entry("_Streams".to_string())
            .or_default()
            .push(streams);

        let streams2 = Record::with_fields(vec![
            FieldValue::String("OtherTable.1".to_string()),
            FieldValue::Null,
        ]);
        db.tables
            .entry("_Streams".to_string())
            .or_default()
            .push(streams2);

        let streams3 = Record::with_fields(vec![FieldValue::Null, FieldValue::Null]);
        db.tables
            .entry("_Streams".to_string())
            .or_default()
            .push(streams3);

        let ast = Statement::DropTable {
            table: TableName("StreamTable".to_string()),
        };
        execute_statement(&mut db, &ast, &[]).unwrap();

        let s = &db.tables["_Streams"];
        assert_eq!(s.len(), 2);
    }
    #[test]
    fn test_resolve_sql_value_column_no_context() {
        use crate::database::sql::ast::ColumnName;
        use crate::database::sql::ast::SqlValue;
        use crate::error::MsiError;

        let val = SqlValue::Column(ColumnName("MyCol".to_string()));
        let mut idx = 0;
        let err = resolve_sql_value(&val, &[], &mut idx, None, None).unwrap_err();
        assert!(matches!(err, MsiError::Sql { .. }));
    }
}
