    #[test]
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    fn test_sql_all_syntax_and_operators() -> Result<()> {
        use crate::database::sql::lexer::Token;
        let mut db = LinkedDatabase::new()?;

        // CREATE TABLE with PRIMARY KEY list and LOCALIZABLE
        let create_sql = "CREATE TABLE ComplexTable (Key1 CHAR(32) NOT NULL, Key2 SHORT NOT NULL, Data OBJECT, Details VARCHAR(64) LOCALIZABLE, PRIMARY KEY (Key1, Key2))";
        let res_create = execute_sql(&mut db, create_sql, &[])?;
        assert_eq!(res_create, QueryResult::SchemaChanged);

        // INSERT with column list and NULL
        let ins1 = "INSERT INTO ComplexTable (Key1, Key2, Details) VALUES ('k1', 10, 'Sample Description')";
        assert_eq!(execute_sql(&mut db, ins1, &[])?, QueryResult::Modified(1));

        let ins2 = "INSERT INTO ComplexTable (Key1, Key2, Data) VALUES ('k2', 20, NULL)";
        assert_eq!(execute_sql(&mut db, ins2, &[])?, QueryResult::Modified(1));

        // SELECT with IS NULL, IS NOT NULL, AND, OR, NOT, parentheses
        let sel_null = "SELECT Key1 FROM ComplexTable WHERE (Data IS NULL AND Key2 > 15) OR (NOT (Details IS NOT NULL))";
        let res_null = execute_sql(&mut db, sel_null, &[])?;
        assert_eq!(
            res_null,
            QueryResult::Select {
                columns: vec!["Key1".to_string()],
                rows: vec![Record::with_fields(vec![FieldValue::String(
                    "k2".to_string()
                )])],
            }
        );

        // SELECT with DISTINCT and joins syntax
        let sel_dist = "SELECT DISTINCT Key1, Key2 FROM ComplexTable WHERE Key2 <= 20 AND Key1 <> 'k9' AND Key2 < 30 AND Key2 >= 10";
        let res_dist = execute_sql(&mut db, sel_dist, &[])?;
        assert_eq!(
            res_dist,
            QueryResult::Select {
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
            }
        );

        // UPDATE multiple columns without WHERE
        let upd_all = "UPDATE ComplexTable SET Key2 = 25";
        assert_eq!(execute_sql(&mut db, upd_all, &[])?, QueryResult::Modified(2));

        // DELETE all rows
        let del_all = "DELETE FROM ComplexTable";
        assert_eq!(execute_sql(&mut db, del_all, &[])?, QueryResult::Modified(2));

        // Test lexer special tokens: square brackets [Ident], backticks `Ident`, comments, symbols
        let mut lexer = Lexer::new("SELECT [My Col], `OtherCol` FROM `Table` WHERE <= >= <> != = ?");
        let toks = lexer.tokenize()?;
        assert!(toks.contains(&Token::LessOrEqual));
        assert!(toks.contains(&Token::GreaterOrEqual));
        assert!(toks.contains(&Token::NotEqual));

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
        assert!(execute_sql(&mut db, "INSERT INTO Nonexistent VALUES ('val')", &[]).is_err());
        assert!(execute_sql(&mut db, "SELECT * FROM Nonexistent", &[]).is_err());
        assert!(execute_sql(&mut db, "SELECT NonexistentCol FROM Property", &[]).is_err());
        assert!(execute_sql(&mut db, "UPDATE Nonexistent SET A = 1", &[]).is_err());
        assert!(execute_sql(&mut db, "DELETE FROM Nonexistent", &[]).is_err());
        assert!(execute_sql(&mut db, "ALTER TABLE Nonexistent ADD Col SHORT", &[]).is_err());
        // Populate Property table to test WHERE errors
        let _ = execute_sql(&mut db, "INSERT INTO Property VALUES ('Prop1', 'Val1')", &[]);
        assert!(execute_sql(&mut db, "SELECT * FROM Property WHERE UnknownCol = 1", &[]).is_err());
        assert!(execute_sql(&mut db, "SELECT * FROM Property WHERE ?", &[]).is_err());

        Ok(())
    }
