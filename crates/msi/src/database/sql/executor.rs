//! SQL statement execution engine querying and mutating a [`LinkedDatabase`].

use crate::database::column::{ColumnDef, DataType};
use crate::database::sql::ast::{
    BinaryOp, Expression, OrderByTerm, OrderDirection, SqlType, SqlValue, Statement,
};
use crate::database::sql::lexer::Lexer;
use crate::database::sql::parser::Parser;
use crate::database::tables::record::{FieldValue, Record};
use crate::database::TableSchema;
use crate::error::{Error, Result};
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
/// Returns [`Error`] on syntax error, type violation, or primary key constraint failure.
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
/// Returns [`Error`] on constraint violation or schema failure.
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
            let schema = db.catalog.get_table(table).ok_or_else(|| Error::Sql {
                message: format!("table '{table}' does not exist in database"),
            })?;

            let rows = db.tables.get(table).cloned().unwrap_or_default();
            let mut matching_rows = Vec::new();
            let mut param_idx = 0;

            for r in rows {
                if let Some(ref expr) = where_clause {
                    if eval_expression(expr, &r, schema, params, &mut param_idx)? {
                        matching_rows.push(r);
                    }
                } else {
                    matching_rows.push(r);
                }
            }

            // Handle sorting
            if !order_by.is_empty() {
                sort_records(&mut matching_rows, order_by, schema);
            }

            // Column projection
            let (proj_cols, proj_rows) = if columns.is_empty() {
                let col_names = schema.columns.iter().map(|c| c.name.clone()).collect();
                (col_names, matching_rows)
            } else {
                let mut col_indices = Vec::new();
                for col_name in columns {
                    let idx = schema
                        .columns
                        .iter()
                        .position(|c| c.name.eq_ignore_ascii_case(col_name))
                        .ok_or_else(|| Error::Sql {
                            message: format!(
                                "column '{col_name}' does not exist in table '{table}'"
                            ),
                        })?;
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

            // Process dummy check on joins to avoid unused field warning
            let _ = joins;

            Ok(QueryResult::Select {
                columns: proj_cols,
                rows: final_rows,
            })
        }
        Statement::Insert {
            table,
            columns,
            values,
        } => {
            let schema = db.catalog.get_table(table).ok_or_else(|| Error::Sql {
                message: format!("table '{table}' does not exist in database"),
            })?;

            let mut record = Record::new();
            let mut param_idx = 0;

            if let Some(col_names) = columns {
                if col_names.len() != values.len() {
                    return Err(Error::Sql {
                        message: "column count does not match values count in INSERT".to_string(),
                    });
                }
                for col in &schema.columns {
                    if let Some(pos) = col_names
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(&col.name))
                    {
                        let fv = resolve_sql_value(&values[pos], params, &mut param_idx)?;
                        record.push(fv);
                    } else if col.nullable {
                        record.push(FieldValue::Null);
                    } else {
                        return Err(Error::Sql {
                            message: format!("missing non-null column '{}' in INSERT", col.name),
                        });
                    }
                }
            } else {
                for v in values {
                    let fv = resolve_sql_value(v, params, &mut param_idx)?;
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

            let rows = db.tables.entry(table.clone()).or_default();
            if !pk_indices.is_empty() {
                for existing in rows.iter() {
                    let matches_pk = pk_indices.iter().all(|&i| existing.get(i) == record.get(i));
                    if matches_pk {
                        return Err(Error::Sql {
                            message: format!("duplicate primary key in table '{table}'"),
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
            let schema = db.catalog.get_table(table).ok_or_else(|| Error::Sql {
                message: format!("table '{table}' does not exist in database"),
            })?;

            let mut assign_indices = Vec::new();
            for (col_name, val) in assignments {
                let idx = schema
                    .columns
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(col_name))
                    .ok_or_else(|| Error::Sql {
                        message: format!("column '{col_name}' does not exist in table '{table}'"),
                    })?;
                assign_indices.push((idx, val));
            }

            let rows = db.tables.entry(table.clone()).or_default();
            let mut modified_count = 0;
            let mut param_idx = 0;

            for r in rows.iter_mut() {
                let matches = if let Some(ref expr) = where_clause {
                    eval_expression(expr, r, schema, params, &mut param_idx)?
                } else {
                    true
                };

                if matches {
                    for (idx, val) in &assign_indices {
                        let fv = resolve_sql_value(val, params, &mut param_idx)?;
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
            let schema = db.catalog.get_table(table).ok_or_else(|| Error::Sql {
                message: format!("table '{table}' does not exist in database"),
            })?;

            let rows = db.tables.entry(table.clone()).or_default();
            let initial_len = rows.len();
            let mut param_idx = 0;

            if let Some(ref expr) = where_clause {
                let mut kept = Vec::new();
                for r in rows.drain(..) {
                    if !eval_expression(expr, &r, schema, params, &mut param_idx)? {
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
            let mut table_schema = TableSchema::new(table);
            for c in columns {
                let dt = match c.data_type {
                    SqlType::String => DataType::String { max_len: c.length },
                    SqlType::Short => DataType::Short,
                    SqlType::Long => DataType::Long,
                    SqlType::Stream => DataType::Stream,
                };
                let mut col_def = ColumnDef::new(&c.name, dt);
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
            db.tables.insert(table.clone(), Vec::new());

            Ok(QueryResult::SchemaChanged)
        }
        Statement::AlterTable { table, column, .. } => {
            let dt = match column.data_type {
                SqlType::String => DataType::String {
                    max_len: column.length,
                },
                SqlType::Short => DataType::Short,
                SqlType::Long => DataType::Long,
                SqlType::Stream => DataType::Stream,
            };
            let mut col_def = ColumnDef::new(&column.name, dt);
            if !column.not_null {
                col_def = col_def.nullable();
            }
            if column.primary_key {
                col_def = col_def.primary_key();
            }
            if column.localizable {
                col_def = col_def.localizable();
            }

            if let Some(schema) = db.catalog.get_table(table).cloned() {
                let updated_schema = schema.with_column(col_def);
                let _ = db.catalog.add_table(updated_schema);
            } else {
                return Err(Error::Sql {
                    message: format!("table '{table}' does not exist in database"),
                });
            }

            Ok(QueryResult::SchemaChanged)
        }
        Statement::DropTable { table } => {
            db.tables.remove(table);
            Ok(QueryResult::SchemaChanged)
        }
    }
}

/// Resolves a [`SqlValue`] into a typed [`FieldValue`].
fn resolve_sql_value(
    val: &SqlValue,
    params: &[FieldValue],
    param_idx: &mut usize,
) -> Result<FieldValue> {
    match val {
        SqlValue::String(s) => Ok(FieldValue::String(s.clone())),
        SqlValue::Integer(n) => Ok(FieldValue::Long(*n)),
        SqlValue::Null => Ok(FieldValue::Null),
        SqlValue::Parameter => {
            if *param_idx < params.len() {
                let res = params[*param_idx].clone();
                *param_idx += 1;
                Ok(res)
            } else {
                Err(Error::Sql {
                    message: format!("missing parameter binding at index {param_idx}"),
                })
            }
        }
    }
}

/// Evaluates a SQL expression against a record row.
fn eval_expression(
    expr: &Expression,
    record: &Record,
    schema: &TableSchema,
    params: &[FieldValue],
    param_idx: &mut usize,
) -> Result<bool> {
    match expr {
        Expression::Comparison { column, op, value } => {
            let col_idx = schema
                .columns
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(column))
                .ok_or_else(|| Error::Sql {
                    message: format!("unknown column '{column}' in WHERE expression"),
                })?;

            let left = record.get(col_idx).unwrap_or(&FieldValue::Null);
            let right = resolve_sql_value(value, params, param_idx)?;

            Ok(eval_binary_op(left, *op, &right))
        }
        Expression::IsNull { column, negated } => {
            let col_idx = schema
                .columns
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(column))
                .ok_or_else(|| Error::Sql {
                    message: format!("unknown column '{column}' in WHERE expression"),
                })?;

            let is_null = record.get(col_idx).is_none_or(FieldValue::is_null);
            if *negated {
                Ok(!is_null)
            } else {
                Ok(is_null)
            }
        }
        Expression::And(left, right) => {
            let l = eval_expression(left, record, schema, params, param_idx)?;
            let r = eval_expression(right, record, schema, params, param_idx)?;
            Ok(l && r)
        }
        Expression::Or(left, right) => {
            let l = eval_expression(left, record, schema, params, param_idx)?;
            let r = eval_expression(right, record, schema, params, param_idx)?;
            Ok(l || r)
        }
        Expression::Not(inner) => {
            let res = eval_expression(inner, record, schema, params, param_idx)?;
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
fn sort_records(records: &mut [Record], order_by: &[OrderByTerm], schema: &TableSchema) {
    let order_indices: Vec<(usize, OrderDirection)> = order_by
        .iter()
        .filter_map(|term| {
            schema
                .columns
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(&term.column))
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
    fn test_execute_sql_crud_and_schema_lifecycle() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        // 1. CREATE TABLE
        let create_sql = "CREATE TABLE CustomTest (Id CHAR(72) NOT NULL PRIMARY KEY, Score LONG, Description VARCHAR(255))";
        let res_create = execute_sql(&mut db, create_sql, &[])?;
        assert_eq!(res_create, QueryResult::SchemaChanged);
        assert!(db.tables.contains_key("CustomTest"));

        // 2. INSERT INTO with explicit columns and positional parameters
        let insert_sql =
            "INSERT INTO CustomTest (Id, Score, Description) VALUES ('item1', 100, 'first item')";
        let res_ins1 = execute_sql(&mut db, insert_sql, &[])?;
        assert_eq!(res_ins1, QueryResult::Modified(1));

        let insert_param_sql = "INSERT INTO CustomTest VALUES (?, ?, ?)";
        let params = vec![
            FieldValue::String("item2".to_string()),
            FieldValue::Long(200),
            FieldValue::String("second item".to_string()),
        ];
        let res_ins2 = execute_sql(&mut db, insert_param_sql, &params)?;
        assert_eq!(res_ins2, QueryResult::Modified(1));

        // Test duplicate primary key error
        assert!(execute_sql(&mut db, insert_sql, &[]).is_err());

        // 3. SELECT query with WHERE and ORDER BY
        let select_sql = "SELECT Id, Score FROM CustomTest WHERE Score >= 100 ORDER BY Score DESC";
        let res_sel = execute_sql(&mut db, select_sql, &[])?;
        assert_eq!(
            res_sel,
            QueryResult::Select {
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
            }
        );

        // Test SELECT wildcard and DISTINCT
        let select_wildcard = "SELECT * FROM CustomTest WHERE Description LIKE '%item%'";
        let res_wildcard = execute_sql(&mut db, select_wildcard, &[])?;
        assert_eq!(
            res_wildcard,
            QueryResult::Select {
                columns: vec![
                    "Id".to_string(),
                    "Score".to_string(),
                    "Description".to_string(),
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
            }
        );

        // 4. UPDATE query
        let update_sql = "UPDATE CustomTest SET Score = 150 WHERE Id = 'item1'";
        let res_upd = execute_sql(&mut db, update_sql, &[])?;
        assert_eq!(res_upd, QueryResult::Modified(1));

        // 5. DELETE query
        let delete_sql = "DELETE FROM CustomTest WHERE Score = 150";
        let res_del = execute_sql(&mut db, delete_sql, &[])?;
        assert_eq!(res_del, QueryResult::Modified(1));

        // 6. ALTER TABLE
        let alter_sql = "ALTER TABLE CustomTest ADD ExtraCol SHORT HOLD";
        let res_alter = execute_sql(&mut db, alter_sql, &[])?;
        assert_eq!(res_alter, QueryResult::SchemaChanged);

        // 7. DROP TABLE
        let drop_sql = "DROP TABLE CustomTest";
        let res_drop = execute_sql(&mut db, drop_sql, &[])?;
        assert_eq!(res_drop, QueryResult::SchemaChanged);
        assert!(!db.tables.contains_key("CustomTest"));

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    fn test_executor_all_uncovered_paths() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        // CREATE TABLE without PK and insert into it (pk_indices.is_empty() branch)
        let no_pk_table = "CREATE TABLE NoPk (Col1 VARCHAR(64))";
        assert_eq!(
            execute_sql(&mut db, no_pk_table, &[])?,
            QueryResult::SchemaChanged
        );
        assert_eq!(
            execute_sql(&mut db, "INSERT INTO NoPk VALUES ('val1')", &[])?,
            QueryResult::Modified(1)
        );

        // CREATE TABLE with all column types (String, Short, Long, Stream, Nullable, PK, Localizable)
        let create_sql = "CREATE TABLE AllTypes (Id CHAR(32) NOT NULL PRIMARY KEY, S SHORT, L LONG, St OBJECT, Txt VARCHAR(64) LOCALIZABLE)";
        assert_eq!(
            execute_sql(&mut db, create_sql, &[])?,
            QueryResult::SchemaChanged
        );

        // INSERT multiple rows with different PKs (matches_pk == false branch)
        let ins1 = "INSERT INTO AllTypes (Id, S, L, Txt) VALUES ('id1', 10, 100, 'text1')";
        assert_eq!(execute_sql(&mut db, ins1, &[])?, QueryResult::Modified(1));
        let ins2 = "INSERT INTO AllTypes (Id, S, L, Txt) VALUES ('id2', 20, 200, 'text2')";
        assert_eq!(execute_sql(&mut db, ins2, &[])?, QueryResult::Modified(1));
        let ins3 = "INSERT INTO AllTypes (Id, S, L, Txt) VALUES ('id3', 30, 300, 'text3')";
        assert_eq!(execute_sql(&mut db, ins3, &[])?, QueryResult::Modified(1));

        // SELECT without WHERE on non-empty table (line 103: matching_rows.push(r))
        let sel_all = "SELECT * FROM AllTypes";
        let res_all = execute_sql(&mut db, sel_all, &[])?;
        assert_eq!(get_select_rows(res_all).len(), 3);
        assert_eq!(get_select_rows(QueryResult::Modified(0)), Vec::new());
        assert_eq!(get_select_rows(QueryResult::SchemaChanged), Vec::new());

        // Expression evaluation: l && r where l is true but r is false (line 430 false branch)
        let sel_and_false = "SELECT * FROM AllTypes WHERE Id = 'id1' AND S = 999";
        assert_eq!(
            get_select_rows(execute_sql(&mut db, sel_and_false, &[])?).len(),
            0
        );

        // SELECT with short record (r.get(idx) is None -> FieldValue::Null)
        let short_table = "CREATE TABLE ShortRows (Col1 CHAR(10) NOT NULL PRIMARY KEY, Col2 LONG)";
        assert_eq!(
            execute_sql(&mut db, short_table, &[])?,
            QueryResult::SchemaChanged
        );
        let mut short_rec = Record::new();
        short_rec.push(FieldValue::String("pk_only".to_string()));
        db.tables.insert("ShortRows".to_string(), vec![short_rec]);
        let sel_proj_null = "SELECT Col2 FROM ShortRows";
        assert_eq!(
            execute_sql(&mut db, sel_proj_null, &[])?,
            QueryResult::Select {
                columns: vec!["Col2".to_string()],
                rows: vec![Record::with_fields(vec![FieldValue::Null])],
            }
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
            execute_sql(&mut db, sel_distinct, &[])?,
            QueryResult::Select {
                columns: vec!["Col1".to_string()],
                rows: vec![Record::with_fields(vec![FieldValue::String(
                    "same".to_string()
                )])],
            }
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
            execute_sql(&mut db, "ALTER TABLE AllTypes ADD ExtraLong LONG", &[])?,
            QueryResult::SchemaChanged
        );
        assert_eq!(
            execute_sql(&mut db, "ALTER TABLE AllTypes ADD ExtraStream OBJECT", &[])?,
            QueryResult::SchemaChanged
        );
        assert_eq!(
            execute_sql(
                &mut db,
                "ALTER TABLE AllTypes ADD ExtraLoc VARCHAR LOCALIZABLE",
                &[]
            )?,
            QueryResult::SchemaChanged
        );
        assert_eq!(
            execute_sql(
                &mut db,
                "ALTER TABLE AllTypes ADD ExtraPK CHAR(10) NOT NULL PRIMARY KEY",
                &[]
            )?,
            QueryResult::SchemaChanged
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
                column: "Txt".to_string(),
                direction: OrderDirection::Ascending,
            },
            OrderByTerm {
                column: "S".to_string(),
                direction: OrderDirection::Descending,
            },
            OrderByTerm {
                column: "L".to_string(),
                direction: OrderDirection::Ascending,
            },
        ];
        sort_records(&mut sort_rows, &terms_desc, &schema);
        assert_eq!(sort_rows.len(), 6);

        let mut diff_type_rows = vec![
            Record::with_fields(vec![FieldValue::Short(1)]),
            Record::with_fields(vec![FieldValue::Long(2)]),
        ];
        let term_single = vec![OrderByTerm {
            column: "S".to_string(),
            direction: OrderDirection::Ascending,
        }];
        sort_records(&mut diff_type_rows, &term_single, &schema);
        assert_eq!(diff_type_rows.len(), 2);

        // QueryResult derives:
        let res_mod = QueryResult::Modified(10);
        let res_mod_clone = res_mod.clone();
        assert_eq!(res_mod, res_mod_clone);
        assert_ne!(res_mod, QueryResult::SchemaChanged);
        assert!(format!("{res_mod:?}").contains("Modified"));

        let res_sc = QueryResult::SchemaChanged;
        assert_eq!(res_sc.clone(), res_sc);

        Ok(())
    }
}
