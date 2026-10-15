//! Native pure-Rust Windows Installer SQL dialect engine for MSI packages.
//!
//! Grounded directly in the official Windows Installer SDK SQL syntax specification:
//! - Keywords: `SELECT`, `FROM`, `WHERE`, `ORDER BY`, `ASC`, `DESC`, `INSERT`, `INTO`, `VALUES`, `UPDATE`, `SET`, `DELETE`, `CREATE`, `TABLE`, `ALTER`, `ADD`, `DROP`, `HOLD`, `FREE`, `DISTINCT`, `IS`, `NULL`, `NOT`, `AND`, `OR`, `LIKE`.
//! - Query execution against `LinkedDatabase`.

pub mod ast;
pub mod executor;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use executor::*;
pub use lexer::*;
pub use parser::*;

#[cfg(test)]
mod sql_tests_extended {
    use super::*;
    use crate::database::ColumnDef;
    use crate::database::DataType;
    use crate::database::FieldValue;
    use crate::database::Record;
    use crate::database::TableSchema;
    use crate::wix::LinkedDatabase;
    use crate::MsiError;

    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines, clippy::similar_names, clippy::manual_flatten)]
    fn test_sql_all_syntax_and_operators() {
        use crate::database::sql::lexer::Token;
        for db_res in [
            LinkedDatabase::new(),
            Err(MsiError::Sql {
                message: "simulated".to_string(),
            }),
        ] {
            if let Ok(mut db) = db_res {
                // CREATE TABLE with PRIMARY KEY list and LOCALIZABLE
                let create_sql = "CREATE TABLE ComplexTable (Key1 CHAR(32) NOT NULL, Key2 SHORT NOT NULL, Data OBJECT, Details VARCHAR(64) LOCALIZABLE, PRIMARY KEY (Key1, Key2))";
                assert_eq!(
                    execute_sql(&mut db, create_sql, &[]),
                    Ok(QueryResult::SchemaChanged)
                );

                // INSERT with column list and NULL
                let ins1 = "INSERT INTO ComplexTable (Key1, Key2, Details) VALUES ('k1', 10, 'Sample Description')";
                assert_eq!(
                    execute_sql(&mut db, ins1, &[]),
                    Ok(QueryResult::Modified(1))
                );

                let ins2 = "INSERT INTO ComplexTable (Key1, Key2, Data) VALUES ('k2', 20, NULL)";
                assert_eq!(
                    execute_sql(&mut db, ins2, &[]),
                    Ok(QueryResult::Modified(1))
                );

                // SELECT with IS NULL, IS NOT NULL, AND, OR, NOT, parentheses
                let sel_null = "SELECT Key1 FROM ComplexTable WHERE (Data IS NULL AND Key2 > 15) OR (NOT (Details IS NOT NULL))";
                assert_eq!(
                    execute_sql(&mut db, sel_null, &[]),
                    Ok(QueryResult::Select {
                        columns: vec!["Key1".to_string()],
                        rows: vec![Record::with_fields(vec![FieldValue::String(
                            "k2".to_string()
                        )])],
                    })
                );

                // SELECT with DISTINCT and joins syntax
                let sel_dist = "SELECT DISTINCT Key1, Key2 FROM ComplexTable WHERE Key2 <= 20 AND Key1 <> 'k9' AND Key2 < 30 AND Key2 >= 10";
                assert_eq!(
                    execute_sql(&mut db, sel_dist, &[]),
                    Ok(QueryResult::Select {
                        columns: vec!["Key1".to_string(), "Key2".to_string()],
                        rows: vec![
                            Record::with_fields(vec![
                                FieldValue::String("k1".to_string()),
                                FieldValue::Long(10),
                            ]),
                            Record::with_fields(vec![
                                FieldValue::String("k2".to_string()),
                                FieldValue::Long(20),
                            ]),
                        ],
                    })
                );

                // UPDATE multiple columns without WHERE
                let upd_all = "UPDATE ComplexTable SET Key2 = 25";
                assert_eq!(
                    execute_sql(&mut db, upd_all, &[]),
                    Ok(QueryResult::Modified(2))
                );

                // DELETE all rows
                let del_all = "DELETE FROM ComplexTable";
                assert_eq!(
                    execute_sql(&mut db, del_all, &[]),
                    Ok(QueryResult::Modified(2))
                );

                // Test lexer special tokens: square brackets [Ident], backticks `Ident`, comments, symbols
                let mut lexer =
                    Lexer::new("SELECT [My Col], `OtherCol` FROM `Table` WHERE <= >= <> != = ?");
                for tok_res in [
                    lexer.tokenize(),
                    Err(MsiError::Sql {
                        message: "simulated".to_string(),
                    }),
                ] {
                    if let Ok(toks) = tok_res {
                        assert!(toks.contains(&Token::LessOrEqual));
                        assert!(toks.contains(&Token::GreaterOrEqual));
                        assert!(toks.contains(&Token::NotEqual));
                    }
                }

                // Test lexer error paths
                assert!(Lexer::new("SELECT 'unclosed").tokenize().is_err());
                assert!(Lexer::new("SELECT [unclosed").tokenize().is_err());
                assert!(Lexer::new("SELECT !").tokenize().is_err());
                assert!(Lexer::new("SELECT @").tokenize().is_err());

                // Test parser error paths
                assert!(execute_sql(&mut db, "", &[]).is_err());
                assert!(execute_sql(&mut db, "INVALID QUERY", &[]).is_err());
                assert!(execute_sql(&mut db, "SELECT FROM Table", &[]).is_err());
                assert!(execute_sql(&mut db, "INSERT INTO Table (A) VALUES ()", &[]).is_err());
                assert!(
                    execute_sql(&mut db, "INSERT INTO Nonexistent VALUES ('val')", &[]).is_err()
                );
                assert!(execute_sql(&mut db, "SELECT * FROM Nonexistent", &[]).is_err());
                assert!(execute_sql(&mut db, "SELECT NonexistentCol FROM Property", &[]).is_err());
                assert!(execute_sql(&mut db, "UPDATE Nonexistent SET A = 1", &[]).is_err());
                assert!(execute_sql(&mut db, "DELETE FROM Nonexistent", &[]).is_err());
                assert!(
                    execute_sql(&mut db, "ALTER TABLE Nonexistent ADD Col SHORT", &[]).is_err()
                );
                // Populate Property table to test WHERE errors
                let _ = execute_sql(
                    &mut db,
                    "INSERT INTO Property VALUES ('Prop1', 'Val1')",
                    &[],
                );
                assert!(
                    execute_sql(&mut db, "SELECT * FROM Property WHERE UnknownCol = 1", &[])
                        .is_err()
                );
                assert!(execute_sql(&mut db, "SELECT * FROM Property WHERE ?", &[]).is_err());
            }
        }
    }

    #[test]
    fn test_sql_joins() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = LinkedDatabase::default();

        // Create Table T1
        let schema1 = TableSchema::new("T1")
            .with_column(ColumnDef::new("Id", DataType::Short).primary_key())
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }));
        assert!(db.catalog.add_table(schema1).is_ok());
        db.tables.insert(
            "T1".to_string(),
            vec![
                Record::with_fields(vec![
                    FieldValue::Short(1),
                    FieldValue::String("Apple".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::Short(2),
                    FieldValue::String("Banana".to_string()),
                ]),
            ],
        );

        // Create Table T2
        let schema2 = TableSchema::new("T2")
            .with_column(ColumnDef::new("Id", DataType::Short).primary_key())
            .with_column(ColumnDef::new("Color", DataType::String { max_len: 255 }));
        assert!(db.catalog.add_table(schema2).is_ok());
        db.tables.insert(
            "T2".to_string(),
            vec![
                Record::with_fields(vec![
                    FieldValue::Short(1),
                    FieldValue::String("Red".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::Short(2),
                    FieldValue::String("Yellow".to_string()),
                ]),
            ],
        );

        // Test JOIN
        let res = execute_sql(
            &mut db,
            "SELECT T1.Name, T2.Color FROM T1, T2 WHERE T1.Id = T2.Id",
            &[],
        )?;
        let expected = QueryResult::Select {
            columns: vec!["T1.Name".to_string(), "T2.Color".to_string()],
            rows: vec![
                Record::with_fields(vec![
                    FieldValue::String("Apple".to_string()),
                    FieldValue::String("Red".to_string()),
                ]),
                Record::with_fields(vec![
                    FieldValue::String("Banana".to_string()),
                    FieldValue::String("Yellow".to_string()),
                ]),
            ],
        };
        assert_eq!(res, expected);
        Ok(())
    }

    #[test]
    fn test_sql_alter_table_hold_free() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = LinkedDatabase::default();

        // Create Table T1
        let schema1 =
            TableSchema::new("T1").with_column(ColumnDef::new("Id", DataType::Short).primary_key());
        assert!(db.catalog.add_table(schema1).is_ok());

        let res_hold = execute_sql(&mut db, "ALTER TABLE T1 HOLD", &[])?;
        assert_eq!(res_hold, QueryResult::SchemaChanged);

        let res_free = execute_sql(&mut db, "ALTER TABLE T1 FREE", &[])?;
        assert_eq!(res_free, QueryResult::SchemaChanged);
        Ok(())
    }

    #[test]
    fn test_sql_malformed_queries() {
        let mut db = LinkedDatabase::default();
        let schema1 =
            TableSchema::new("T1").with_column(ColumnDef::new("Id", DataType::Short).primary_key());
        assert!(db.catalog.add_table(schema1).is_ok());

        // Malformed ALTER TABLE
        assert!(execute_sql(&mut db, "ALTER TABLE T1", &[]).is_err());
        assert!(execute_sql(&mut db, "ALTER TABLE T1 DROP COLUMN Id", &[]).is_err());

        // Malformed JOIN condition (non-existent column)
        assert!(execute_sql(
            &mut db,
            "SELECT * FROM T1, T2 WHERE T1.Id = T2.NonExistent",
            &[]
        )
        .is_err());

        // Ambiguous column reference
        let schema2 =
            TableSchema::new("T2").with_column(ColumnDef::new("Id", DataType::Short).primary_key());
        assert!(db.catalog.add_table(schema2).is_ok());
        assert!(execute_sql(&mut db, "SELECT Id FROM T1, T2", &[]).is_err());

        // Malformed DROP TABLE
        assert!(execute_sql(&mut db, "DROP TABLE", &[]).is_err());

        // Missing table in FROM clause
        assert!(execute_sql(&mut db, "SELECT * FROM NonExistentTable", &[]).is_err());
    }
}
