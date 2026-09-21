//! Parser converting SQL tokens into abstract syntax tree [`Statement`] items.

use crate::database::sql::ast::{
    BinaryOp, Expression, OrderByTerm, OrderDirection, SqlColumnDef, SqlType, SqlValue, Statement,
};
use crate::database::sql::lexer::Token;
use crate::error::{Error, Result};

/// Parser for the Windows Installer SQL dialect.
#[derive(Debug, Clone)]
pub struct Parser {
    /// Token sequence to parse.
    tokens: Vec<Token>,
    /// Current token index.
    pos: usize,
}

impl Parser {
    /// Creates a new [`Parser`] from a sequence of tokens.
    ///
    /// # Arguments
    ///
    /// * `tokens` - Sequence of tokens produced by [`super::lexer::Lexer`].
    #[must_use]
    pub const fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parses a single SQL statement.
    ///
    /// # Returns
    ///
    /// Parsed [`Statement`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sql`] on syntax errors.
    pub fn parse(&mut self) -> Result<Statement> {
        let tok = self.peek().ok_or_else(|| Error::Sql {
            message: "unexpected end of SQL input".to_string(),
        })?;

        match tok {
            Token::Select => self.parse_select(),
            Token::Insert => self.parse_insert(),
            Token::Update => self.parse_update(),
            Token::Delete => self.parse_delete(),
            Token::Create => self.parse_create_table(),
            Token::Alter => self.parse_alter_table(),
            Token::Drop => self.parse_drop_table(),
            other => Err(Error::Sql {
                message: format!("unexpected leading token {other:?} in SQL query"),
            }),
        }
    }

    /// Peeks at the current token.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Advances and returns the next token.
    fn next_token(&mut self) -> Option<Token> {
        (self.pos < self.tokens.len()).then(|| {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            tok
        })
    }

    /// Verifies and consumes the expected token.
    fn expect(&mut self, expected: &Token) -> Result<()> {
        let tok = self.next_token().ok_or_else(|| Error::Sql {
            message: format!("expected token {expected:?}, but reached end of input"),
        })?;
        if &tok == expected {
            Ok(())
        } else {
            Err(Error::Sql {
                message: format!("expected token {expected:?}, found {tok:?}"),
            })
        }
    }

    /// Consumes and returns an identifier token.
    fn expect_ident(&mut self) -> Result<String> {
        let tok = self.next_token().ok_or_else(|| Error::Sql {
            message: "expected identifier, but reached end of input".to_string(),
        })?;
        match tok {
            Token::Identifier(s) => Ok(s),
            other => Err(Error::Sql {
                message: format!("expected identifier, found {other:?}"),
            }),
        }
    }

    /// Parses a `SELECT` statement.
    fn parse_select(&mut self) -> Result<Statement> {
        self.expect(&Token::Select)?;

        let distinct = if matches!(self.peek(), Some(Token::Distinct)) {
            self.next_token();
            true
        } else {
            false
        };

        let mut columns = Vec::new();
        if matches!(self.peek(), Some(Token::Asterisk)) {
            self.next_token();
        } else {
            loop {
                columns.push(self.expect_ident()?);
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.next_token();
                } else {
                    break;
                }
            }
        }

        self.expect(&Token::From)?;
        let table = self.expect_ident()?;

        let mut joins = Vec::new();
        while matches!(self.peek(), Some(Token::Comma)) {
            self.next_token();
            joins.push(self.expect_ident()?);
        }

        let where_clause = if matches!(self.peek(), Some(Token::Where)) {
            self.next_token();
            Some(self.parse_expression()?)
        } else {
            None
        };

        let mut order_by = Vec::new();
        if matches!(self.peek(), Some(Token::Order)) {
            self.next_token();
            self.expect(&Token::By)?;
            loop {
                let col = self.expect_ident()?;
                let dir = if matches!(self.peek(), Some(Token::Desc)) {
                    self.next_token();
                    OrderDirection::Descending
                } else {
                    if matches!(self.peek(), Some(Token::Asc)) {
                        self.next_token();
                    }
                    OrderDirection::Ascending
                };
                order_by.push(OrderByTerm {
                    column: col,
                    direction: dir,
                });
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.next_token();
                } else {
                    break;
                }
            }
        }

        Ok(Statement::Select {
            distinct,
            columns,
            table,
            joins,
            where_clause,
            order_by,
        })
    }

    /// Parses an `INSERT INTO` statement.
    fn parse_insert(&mut self) -> Result<Statement> {
        self.expect(&Token::Insert)?;
        self.expect(&Token::Into)?;
        let table = self.expect_ident()?;

        let columns = if matches!(self.peek(), Some(Token::OpenParen)) {
            self.next_token();
            let mut cols = Vec::new();
            loop {
                cols.push(self.expect_ident()?);
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.next_token();
                } else {
                    break;
                }
            }
            self.expect(&Token::CloseParen)?;
            Some(cols)
        } else {
            None
        };

        self.expect(&Token::Values)?;
        self.expect(&Token::OpenParen)?;
        let mut values = Vec::new();
        loop {
            values.push(self.parse_value()?);
            if matches!(self.peek(), Some(Token::Comma)) {
                self.next_token();
            } else {
                break;
            }
        }
        self.expect(&Token::CloseParen)?;

        Ok(Statement::Insert {
            table,
            columns,
            values,
        })
    }

    /// Parses an `UPDATE` statement.
    fn parse_update(&mut self) -> Result<Statement> {
        self.expect(&Token::Update)?;
        let table = self.expect_ident()?;
        self.expect(&Token::Set)?;

        let mut assignments = Vec::new();
        loop {
            let col = self.expect_ident()?;
            self.expect(&Token::Equal)?;
            let val = self.parse_value()?;
            assignments.push((col, val));
            if matches!(self.peek(), Some(Token::Comma)) {
                self.next_token();
            } else {
                break;
            }
        }

        let where_clause = if matches!(self.peek(), Some(Token::Where)) {
            self.next_token();
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(Statement::Update {
            table,
            assignments,
            where_clause,
        })
    }

    /// Parses a `DELETE FROM` statement.
    fn parse_delete(&mut self) -> Result<Statement> {
        self.expect(&Token::Delete)?;
        self.expect(&Token::From)?;
        let table = self.expect_ident()?;

        let where_clause = if matches!(self.peek(), Some(Token::Where)) {
            self.next_token();
            Some(self.parse_expression()?)
        } else {
            None
        };

        Ok(Statement::Delete {
            table,
            where_clause,
        })
    }

    /// Parses a `CREATE TABLE` statement.
    fn parse_create_table(&mut self) -> Result<Statement> {
        self.expect(&Token::Create)?;
        self.expect(&Token::Table)?;
        let table = self.expect_ident()?;
        self.expect(&Token::OpenParen)?;

        let mut columns = Vec::new();
        let mut primary_keys = Vec::new();

        loop {
            if matches!(self.peek(), Some(Token::Primary)) {
                self.next_token();
                self.expect(&Token::Key)?;
                if matches!(self.peek(), Some(Token::OpenParen)) {
                    self.next_token();
                    loop {
                        primary_keys.push(self.expect_ident()?);
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.next_token();
                        } else {
                            break;
                        }
                    }
                    self.expect(&Token::CloseParen)?;
                } else {
                    primary_keys.push(self.expect_ident()?);
                }
            } else {
                let col_def = self.parse_column_def()?;
                columns.push(col_def);
            }

            if matches!(self.peek(), Some(Token::Comma)) {
                self.next_token();
            } else {
                break;
            }
        }
        self.expect(&Token::CloseParen)?;

        for pk_name in primary_keys {
            for col in &mut columns {
                if col.name.eq_ignore_ascii_case(&pk_name) {
                    col.primary_key = true;
                }
            }
        }

        Ok(Statement::CreateTable { table, columns })
    }

    /// Parses an `ALTER TABLE` statement.
    fn parse_alter_table(&mut self) -> Result<Statement> {
        self.expect(&Token::Alter)?;
        self.expect(&Token::Table)?;
        let table = self.expect_ident()?;
        self.expect(&Token::Add)?;
        let column = self.parse_column_def()?;

        let hold = if matches!(self.peek(), Some(Token::Hold)) {
            self.next_token();
            true
        } else {
            false
        };

        Ok(Statement::AlterTable {
            table,
            column,
            hold,
        })
    }

    /// Parses a `DROP TABLE` statement.
    fn parse_drop_table(&mut self) -> Result<Statement> {
        self.expect(&Token::Drop)?;
        self.expect(&Token::Table)?;
        let table = self.expect_ident()?;
        Ok(Statement::DropTable { table })
    }

    /// Parses a single column definition in `CREATE TABLE` or `ALTER TABLE`.
    fn parse_column_def(&mut self) -> Result<SqlColumnDef> {
        let name = self.expect_ident()?;
        let type_tok = self.next_token().ok_or_else(|| Error::Sql {
            message: "unexpected end of column type definition".to_string(),
        })?;

        let mut length = 0;
        let data_type = match type_tok {
            Token::Char | Token::Varchar => {
                if matches!(self.peek(), Some(Token::OpenParen)) {
                    self.next_token();
                    if let Some(Token::IntegerLiteral(n)) = self.next_token() {
                        length = u8::try_from(n).unwrap_or(255);
                    }
                    self.expect(&Token::CloseParen)?;
                }
                SqlType::String
            }
            Token::Short | Token::Int => SqlType::Short,
            Token::Long => SqlType::Long,
            Token::Object => SqlType::Stream,
            other => {
                return Err(Error::Sql {
                    message: format!("unknown data type token {other:?} for column '{name}'"),
                });
            }
        };

        let mut not_null = false;
        let mut primary_key = false;
        let mut localizable = false;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Not => {
                    self.next_token();
                    self.expect(&Token::Null)?;
                    not_null = true;
                }
                Token::Primary => {
                    self.next_token();
                    self.expect(&Token::Key)?;
                    primary_key = true;
                }
                Token::Localizable => {
                    self.next_token();
                    localizable = true;
                }
                _ => break,
            }
        }

        Ok(SqlColumnDef {
            name,
            data_type,
            length,
            not_null,
            primary_key,
            localizable,
        })
    }

    /// Parses a literal or parameter placeholder value.
    fn parse_value(&mut self) -> Result<SqlValue> {
        let tok = self.next_token().ok_or_else(|| Error::Sql {
            message: "unexpected end of expression value".to_string(),
        })?;

        match tok {
            Token::StringLiteral(s) => Ok(SqlValue::String(s)),
            Token::IntegerLiteral(n) => Ok(SqlValue::Integer(n)),
            Token::QuestionMark => Ok(SqlValue::Parameter),
            Token::Null => Ok(SqlValue::Null),
            other => Err(Error::Sql {
                message: format!("expected literal value, found {other:?}"),
            }),
        }
    }

    /// Parses an expression tree.
    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_or()
    }

    /// Parses logical OR level.
    fn parse_or(&mut self) -> Result<Expression> {
        let mut expr = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.next_token();
            let right = self.parse_and()?;
            expr = Expression::Or(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    /// Parses logical AND level.
    fn parse_and(&mut self) -> Result<Expression> {
        let mut expr = self.parse_primary_expr()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.next_token();
            let right = self.parse_primary_expr()?;
            expr = Expression::And(Box::new(expr), Box::new(right));
        }
        Ok(expr)
    }

    /// Parses primary expression level.
    fn parse_primary_expr(&mut self) -> Result<Expression> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.next_token();
            let inner = self.parse_primary_expr()?;
            return Ok(Expression::Not(Box::new(inner)));
        }

        if matches!(self.peek(), Some(Token::OpenParen)) {
            self.next_token();
            let inner = self.parse_expression()?;
            self.expect(&Token::CloseParen)?;
            return Ok(inner);
        }

        let column = self.expect_ident()?;

        if matches!(self.peek(), Some(Token::Is)) {
            self.next_token();
            let negated = if matches!(self.peek(), Some(Token::Not)) {
                self.next_token();
                true
            } else {
                false
            };
            self.expect(&Token::Null)?;
            return Ok(Expression::IsNull { column, negated });
        }

        let op_tok = self.next_token().ok_or_else(|| Error::Sql {
            message: format!("expected comparison operator after column '{column}'"),
        })?;

        let op = match op_tok {
            Token::Equal => BinaryOp::Equal,
            Token::NotEqual => BinaryOp::NotEqual,
            Token::LessThan => BinaryOp::LessThan,
            Token::GreaterThan => BinaryOp::GreaterThan,
            Token::LessOrEqual => BinaryOp::LessOrEqual,
            Token::GreaterOrEqual => BinaryOp::GreaterOrEqual,
            Token::Like => BinaryOp::Like,
            other => {
                return Err(Error::Sql {
                    message: format!("unexpected comparison operator {other:?}"),
                });
            }
        };

        let value = self.parse_value()?;
        Ok(Expression::Comparison { column, op, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_query(sql: &str) -> Result<Statement> {
        let mut lexer = crate::database::sql::lexer::Lexer::new(sql);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_parser_select_statements() -> Result<()> {
        let sql1 = "SELECT DISTINCT Col1, Col2 FROM T1, T2 WHERE Col1 = 'val' ORDER BY Col1 ASC, Col2 DESC";
        let stmt1 = parse_query(sql1)?;
        assert_eq!(
            stmt1,
            Statement::Select {
                distinct: true,
                columns: vec!["Col1".to_string(), "Col2".to_string()],
                table: "T1".to_string(),
                joins: vec!["T2".to_string()],
                where_clause: Some(Expression::Comparison {
                    column: "Col1".to_string(),
                    op: BinaryOp::Equal,
                    value: SqlValue::String("val".to_string()),
                }),
                order_by: vec![
                    OrderByTerm {
                        column: "Col1".to_string(),
                        direction: OrderDirection::Ascending,
                    },
                    OrderByTerm {
                        column: "Col2".to_string(),
                        direction: OrderDirection::Descending,
                    },
                ],
            }
        );

        let sql2 = "SELECT * FROM T1";
        let stmt2 = parse_query(sql2)?;
        assert_eq!(
            stmt2,
            Statement::Select {
                distinct: false,
                columns: Vec::new(),
                table: "T1".to_string(),
                joins: Vec::new(),
                where_clause: None,
                order_by: Vec::new(),
            }
        );

        let sql_default_order = "SELECT * FROM T ORDER BY Col1";
        assert_eq!(
            parse_query(sql_default_order)?,
            Statement::Select {
                distinct: false,
                columns: Vec::new(),
                table: "T".to_string(),
                joins: Vec::new(),
                where_clause: None,
                order_by: vec![OrderByTerm {
                    column: "Col1".to_string(),
                    direction: OrderDirection::Ascending,
                }],
            }
        );

        let sql3 = "SELECT Col1 FROM T1 WHERE Col1 IS NULL";
        assert!(parse_query(sql3).is_ok());

        let sql4 = "SELECT Col1 FROM T1 WHERE Col1 IS NOT NULL";
        assert!(parse_query(sql4).is_ok());

        let sql5 = "SELECT Col1 FROM T1 WHERE NOT (Col1 < 10 AND Col2 > 20 OR Col3 <= 30 AND Col4 >= 40 OR Col5 != 50 OR Col6 LIKE 'abc%')";
        assert!(parse_query(sql5).is_ok());

        Ok(())
    }

    #[test]
    fn test_parser_dml_and_ddl_statements() {
        // INSERT
        let sql_ins1 = "INSERT INTO T (Col1, Col2) VALUES ('a', 1)";
        assert!(parse_query(sql_ins1).is_ok());

        let sql_ins2 = "INSERT INTO T VALUES ('a', 1, ?, NULL)";
        assert!(parse_query(sql_ins2).is_ok());

        // UPDATE
        let sql_upd1 = "UPDATE T SET Col1 = 'val', Col2 = 2 WHERE Col3 = 3";
        assert!(parse_query(sql_upd1).is_ok());

        let sql_upd2 = "UPDATE T SET Col1 = 'val'";
        assert!(parse_query(sql_upd2).is_ok());

        // DELETE
        let sql_del1 = "DELETE FROM T WHERE Col1 = 1";
        assert!(parse_query(sql_del1).is_ok());

        let sql_del2 = "DELETE FROM T";
        assert!(parse_query(sql_del2).is_ok());

        // CREATE TABLE with complex column definitions
        let sql_create1 = "CREATE TABLE T (Col1 CHAR(50) NOT NULL PRIMARY KEY, Col2 VARCHAR NOT NULL, Col3 SHORT, Col4 INT, Col5 LONG, Col6 OBJECT LOCALIZABLE, PRIMARY KEY (Col1))";
        assert!(parse_query(sql_create1).is_ok());

        let sql_create2 = "CREATE TABLE T (Col1 LONG, PRIMARY KEY Col1)";
        assert!(parse_query(sql_create2).is_ok());

        let sql_char_overflow = "CREATE TABLE T (Col1 CHAR(500), Col2 CHAR(xyz))";
        assert!(parse_query(sql_char_overflow).is_ok());

        // ALTER TABLE
        let sql_alter1 = "ALTER TABLE T ADD Col LONG HOLD";
        assert!(parse_query(sql_alter1).is_ok());

        let sql_alter2 = "ALTER TABLE T ADD Col SHORT";
        assert!(parse_query(sql_alter2).is_ok());

        // DROP TABLE
        let sql_drop = "DROP TABLE T";
        assert!(parse_query(sql_drop).is_ok());
    }

    #[test]
    fn test_parser_errors() {
        // Empty / leading token error
        assert!(parse_query("").is_err());
        assert!(parse_query("WHERE 1 = 1").is_err());

        // End of input errors
        assert!(parse_query("SELECT").is_err());
        assert!(parse_query("SELECT *").is_err());
        assert!(parse_query("SELECT * FROM").is_err());
        assert!(parse_query("SELECT * FROM T ORDER").is_err());
        assert!(parse_query("SELECT * FROM T ORDER BY").is_err());
        assert!(parse_query("INSERT INTO").is_err());
        assert!(parse_query("INSERT INTO T").is_err());
        assert!(parse_query("INSERT INTO T VALUES").is_err());
        assert!(parse_query("INSERT INTO T VALUES (").is_err());
        assert!(parse_query("UPDATE").is_err());
        assert!(parse_query("UPDATE T").is_err());
        assert!(parse_query("DELETE").is_err());
        assert!(parse_query("CREATE").is_err());
        assert!(parse_query("CREATE TABLE").is_err());
        assert!(parse_query("ALTER").is_err());
        assert!(parse_query("ALTER TABLE").is_err());
        assert!(parse_query("DROP").is_err());

        // Token mismatch errors
        assert!(parse_query("SELECT * WHERE").is_err());
        assert!(parse_query("SELECT * FROM T ORDER WHERE").is_err());
        assert!(parse_query("INSERT INTO T (Col1) FROM").is_err());
        assert!(parse_query("UPDATE T SET Col1 FROM").is_err());
        assert!(parse_query("DELETE WHERE").is_err());
        assert!(parse_query("CREATE TABLE T (Col1 NOT NULL)").is_err());
        assert!(parse_query("CREATE TABLE T (Col1 LONG PRIMARY)").is_err());
        assert!(parse_query("CREATE TABLE T (Col1 LONG NOT)").is_err());
        assert!(parse_query("INSERT INTO T VALUES (*)").is_err());
        assert!(parse_query("SELECT * FROM T WHERE Col1").is_err());
        assert!(parse_query("SELECT * FROM T WHERE Col1 FROM").is_err());
        assert!(parse_query("SELECT * FROM T WHERE Col1 IS").is_err());
        assert!(parse_query("SELECT * FROM T WHERE Col1 IS WHERE").is_err());
        assert!(parse_query("CREATE TABLE T (Col1)").is_err());
        assert!(parse_query("CREATE TABLE T (Col1").is_err());
        assert!(parse_query("CREATE TABLE T (Col1 CHAR(50)").is_err());
    }

    #[test]
    fn test_parser_and_ast_derives() {
        let parser = Parser::new(vec![Token::Select]);
        assert!(format!("{parser:?}").contains("Parser"));

        let op = BinaryOp::Equal;
        let cloned_op = op;
        assert_eq!(op, cloned_op);
        assert_ne!(op, BinaryOp::NotEqual);
        assert!(format!("{op:?}").contains("Equal"));

        let dir = OrderDirection::default();
        let cloned_dir = dir;
        assert_eq!(dir, cloned_dir);
        assert_ne!(dir, OrderDirection::Descending);
        assert!(format!("{dir:?}").contains("Ascending"));

        let val = SqlValue::Null;
        let cloned_val = val.clone();
        assert_eq!(val, cloned_val);
        assert_ne!(val, SqlValue::Parameter);
        assert!(format!("{val:?}").contains("Null"));

        let col_type = SqlType::Stream;
        let cloned_type = col_type;
        assert_eq!(col_type, cloned_type);
        assert!(format!("{col_type:?}").contains("Stream"));

        let term = OrderByTerm {
            column: "Col1".to_string(),
            direction: OrderDirection::Ascending,
        };
        let cloned_term = term.clone();
        assert_eq!(term, cloned_term);
        assert!(format!("{term:?}").contains("Col1"));

        let col_def = SqlColumnDef {
            name: "Col1".to_string(),
            data_type: SqlType::Short,
            length: 0,
            not_null: true,
            primary_key: true,
            localizable: false,
        };
        let cloned_col = col_def.clone();
        assert_eq!(col_def, cloned_col);
        assert!(format!("{col_def:?}").contains("Col1"));

        let expr = Expression::IsNull {
            column: "Col1".to_string(),
            negated: false,
        };
        let cloned_expr = expr.clone();
        assert_eq!(expr, cloned_expr);
        assert!(format!("{expr:?}").contains("IsNull"));

        let stmt = Statement::DropTable {
            table: "T".to_string(),
        };
        let cloned_stmt = stmt.clone();
        assert_eq!(stmt, cloned_stmt);
        assert!(format!("{stmt:?}").contains("DropTable"));
    }
}
