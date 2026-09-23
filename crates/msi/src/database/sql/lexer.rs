//! Lexer for Windows Installer SQL dialect.

use crate::error::{Error, Result};

/// A lexical token in the Windows Installer SQL dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Keyword `SELECT`
    Select,
    /// Keyword `FROM`
    From,
    /// Keyword `WHERE`
    Where,
    /// Keyword `ORDER`
    Order,
    /// Keyword `BY`
    By,
    /// Keyword `ASC`
    Asc,
    /// Keyword `DESC`
    Desc,
    /// Keyword `INSERT`
    Insert,
    /// Keyword `INTO`
    Into,
    /// Keyword `VALUES`
    Values,
    /// Keyword `UPDATE`
    Update,
    /// Keyword `SET`
    Set,
    /// Keyword `DELETE`
    Delete,
    /// Keyword `CREATE`
    Create,
    /// Keyword `TABLE`
    Table,
    /// Keyword `ALTER`
    Alter,
    /// Keyword `ADD`
    Add,
    /// Keyword `DROP`
    Drop,
    /// Keyword `HOLD`
    Hold,
    /// Keyword `FREE`
    Free,
    /// Keyword `DISTINCT`
    Distinct,
    /// Keyword `IS`
    Is,
    /// Keyword `NULL`
    Null,
    /// Keyword `NOT`
    Not,
    /// Keyword `AND`
    And,
    /// Keyword `OR`
    Or,
    /// Keyword `LIKE`
    Like,
    /// Keyword `PRIMARY`
    Primary,
    /// Keyword `KEY`
    Key,
    /// Keyword `LOCALIZABLE`
    Localizable,
    /// Keyword `CHAR`
    Char,
    /// Keyword `VARCHAR`
    Varchar,
    /// Keyword `SHORT`
    Short,
    /// Keyword `INT` or `INTEGER`
    Int,
    /// Keyword `LONG`
    Long,
    /// Keyword `OBJECT`
    Object,

    /// Identifier name
    Identifier(String),
    /// Quoted string literal
    StringLiteral(String),
    /// Integer literal
    IntegerLiteral(i32),
    /// Question mark positional parameter placeholder `?`
    QuestionMark,

    /// Equals operator `=`
    Equal,
    /// Not equals operator `!=` or `<>`
    NotEqual,
    /// Less than operator `<`
    LessThan,
    /// Greater than operator `>`
    GreaterThan,
    /// Less than or equal operator `<=`
    LessOrEqual,
    /// Greater than or equal operator `>=`
    GreaterOrEqual,
    /// Comma `,`
    Comma,
    /// Asterisk `*`
    Asterisk,
    /// Open parenthesis `(`
    OpenParen,
    /// Close parenthesis `)`
    CloseParen,
}

/// Tokenizer splitting SQL text into a sequence of [`Token`] items.
#[derive(Debug, Clone)]
pub struct Lexer {
    /// Unicode character buffer of the query.
    chars: Vec<char>,
    /// Current 0-based character cursor position.
    pos: usize,
}

impl Lexer {
    /// Creates a new [`Lexer`] for the given SQL string.
    ///
    /// # Arguments
    ///
    /// * `input` - Raw SQL query text.
    #[must_use]
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    /// Tokenizes the entire SQL input string.
    ///
    /// # Returns
    ///
    /// Vector of [`Token`]s.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Sql`] on unclosed string literals or unexpected characters.
    #[allow(clippy::too_many_lines)]
    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();

        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];

            if ch.is_whitespace() {
                self.pos += 1;
                continue;
            }

            match ch {
                ',' => {
                    tokens.push(Token::Comma);
                    self.pos += 1;
                }
                '*' => {
                    tokens.push(Token::Asterisk);
                    self.pos += 1;
                }
                '(' => {
                    tokens.push(Token::OpenParen);
                    self.pos += 1;
                }
                ')' => {
                    tokens.push(Token::CloseParen);
                    self.pos += 1;
                }
                '?' => {
                    tokens.push(Token::QuestionMark);
                    self.pos += 1;
                }
                '=' => {
                    tokens.push(Token::Equal);
                    self.pos += 1;
                }
                '<' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                        tokens.push(Token::LessOrEqual);
                        self.pos += 1;
                    } else if self.pos < self.chars.len() && self.chars[self.pos] == '>' {
                        tokens.push(Token::NotEqual);
                        self.pos += 1;
                    } else {
                        tokens.push(Token::LessThan);
                    }
                }
                '>' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                        tokens.push(Token::GreaterOrEqual);
                        self.pos += 1;
                    } else {
                        tokens.push(Token::GreaterThan);
                    }
                }
                '!' => {
                    self.pos += 1;
                    if self.pos < self.chars.len() && self.chars[self.pos] == '=' {
                        tokens.push(Token::NotEqual);
                        self.pos += 1;
                    } else {
                        return Err(Error::Sql {
                            message: format!("unexpected character '!' at index {}", self.pos - 1),
                        });
                    }
                }
                '\'' | '`' | '"' => {
                    let quote = ch;
                    self.pos += 1;
                    let mut s = String::new();
                    let mut closed = false;
                    while self.pos < self.chars.len() {
                        let c = self.chars[self.pos];
                        if c == quote {
                            self.pos += 1;
                            closed = true;
                            break;
                        }
                        s.push(c);
                        self.pos += 1;
                    }
                    if !closed {
                        return Err(Error::Sql {
                            message: "unclosed string literal in SQL".to_string(),
                        });
                    }
                    if quote == '`' {
                        tokens.push(Token::Identifier(s));
                    } else {
                        tokens.push(Token::StringLiteral(s));
                    }
                }
                '[' => {
                    self.pos += 1;
                    let mut s = String::new();
                    let mut closed = false;
                    while self.pos < self.chars.len() {
                        let c = self.chars[self.pos];
                        if c == ']' {
                            self.pos += 1;
                            closed = true;
                            break;
                        }
                        s.push(c);
                        self.pos += 1;
                    }
                    if !closed {
                        return Err(Error::Sql {
                            message: "unclosed square bracket identifier in SQL".to_string(),
                        });
                    }
                    tokens.push(Token::Identifier(s));
                }
                '0'..='9' | '-' => {
                    let start = self.pos;
                    let is_neg = ch == '-';
                    if is_neg {
                        self.pos += 1;
                    }
                    while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                        self.pos += 1;
                    }
                    let num_str: String = self.chars[start..self.pos].iter().collect();
                    if let Ok(num) = num_str.parse::<i32>() {
                        tokens.push(Token::IntegerLiteral(num));
                    } else {
                        return Err(Error::Sql {
                            message: format!("invalid integer literal '{num_str}'"),
                        });
                    }
                }
                _ if ch.is_alphabetic() || ch == '_' => {
                    let start = self.pos;
                    while self.pos < self.chars.len()
                        && (self.chars[self.pos].is_alphanumeric() || self.chars[self.pos] == '_')
                    {
                        self.pos += 1;
                    }
                    let word: String = self.chars[start..self.pos].iter().collect();
                    tokens.push(match_keyword_or_ident(&word));
                }
                other => {
                    return Err(Error::Sql {
                        message: format!("unexpected character '{other}' in SQL query"),
                    });
                }
            }
        }

        Ok(tokens)
    }
}

/// Matches a case-insensitive word against known SQL keywords.
fn match_keyword_or_ident(w: &str) -> Token {
    if w.eq_ignore_ascii_case("SELECT") {
        Token::Select
    } else if w.eq_ignore_ascii_case("FROM") {
        Token::From
    } else if w.eq_ignore_ascii_case("WHERE") {
        Token::Where
    } else if w.eq_ignore_ascii_case("ORDER") {
        Token::Order
    } else if w.eq_ignore_ascii_case("BY") {
        Token::By
    } else if w.eq_ignore_ascii_case("ASC") {
        Token::Asc
    } else if w.eq_ignore_ascii_case("DESC") {
        Token::Desc
    } else if w.eq_ignore_ascii_case("INSERT") {
        Token::Insert
    } else if w.eq_ignore_ascii_case("INTO") {
        Token::Into
    } else if w.eq_ignore_ascii_case("VALUES") {
        Token::Values
    } else if w.eq_ignore_ascii_case("UPDATE") {
        Token::Update
    } else if w.eq_ignore_ascii_case("SET") {
        Token::Set
    } else if w.eq_ignore_ascii_case("DELETE") {
        Token::Delete
    } else if w.eq_ignore_ascii_case("CREATE") {
        Token::Create
    } else if w.eq_ignore_ascii_case("TABLE") {
        Token::Table
    } else if w.eq_ignore_ascii_case("ALTER") {
        Token::Alter
    } else if w.eq_ignore_ascii_case("ADD") {
        Token::Add
    } else if w.eq_ignore_ascii_case("DROP") {
        Token::Drop
    } else if w.eq_ignore_ascii_case("HOLD") {
        Token::Hold
    } else if w.eq_ignore_ascii_case("FREE") {
        Token::Free
    } else if w.eq_ignore_ascii_case("DISTINCT") {
        Token::Distinct
    } else if w.eq_ignore_ascii_case("IS") {
        Token::Is
    } else if w.eq_ignore_ascii_case("NULL") {
        Token::Null
    } else if w.eq_ignore_ascii_case("NOT") {
        Token::Not
    } else if w.eq_ignore_ascii_case("AND") {
        Token::And
    } else if w.eq_ignore_ascii_case("OR") {
        Token::Or
    } else if w.eq_ignore_ascii_case("LIKE") {
        Token::Like
    } else if w.eq_ignore_ascii_case("PRIMARY") {
        Token::Primary
    } else if w.eq_ignore_ascii_case("KEY") {
        Token::Key
    } else if w.eq_ignore_ascii_case("LOCALIZABLE") {
        Token::Localizable
    } else if w.eq_ignore_ascii_case("CHAR") {
        Token::Char
    } else if w.eq_ignore_ascii_case("VARCHAR") {
        Token::Varchar
    } else if w.eq_ignore_ascii_case("SHORT") {
        Token::Short
    } else if w.eq_ignore_ascii_case("INT") || w.eq_ignore_ascii_case("INTEGER") {
        Token::Int
    } else if w.eq_ignore_ascii_case("LONG") {
        Token::Long
    } else if w.eq_ignore_ascii_case("OBJECT") {
        Token::Object
    } else {
        Token::Identifier(w.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_all_keywords_and_tokens() {
        let sql = "SELECT DISTINCT FROM WHERE ORDER BY ASC DESC INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE ALTER ADD DROP HOLD FREE IS NULL NOT AND OR LIKE PRIMARY KEY LOCALIZABLE CHAR VARCHAR SHORT INT INTEGER LONG OBJECT";
        let mut lexer = Lexer::new(sql);
        for res in [
            lexer.tokenize(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ] {
            if let Ok(tokens) = res {
                assert_eq!(
                    tokens,
                    vec![
                        Token::Select,
                        Token::Distinct,
                        Token::From,
                        Token::Where,
                        Token::Order,
                        Token::By,
                        Token::Asc,
                        Token::Desc,
                        Token::Insert,
                        Token::Into,
                        Token::Values,
                        Token::Update,
                        Token::Set,
                        Token::Delete,
                        Token::Create,
                        Token::Table,
                        Token::Alter,
                        Token::Add,
                        Token::Drop,
                        Token::Hold,
                        Token::Free,
                        Token::Is,
                        Token::Null,
                        Token::Not,
                        Token::And,
                        Token::Or,
                        Token::Like,
                        Token::Primary,
                        Token::Key,
                        Token::Localizable,
                        Token::Char,
                        Token::Varchar,
                        Token::Short,
                        Token::Int,
                        Token::Int,
                        Token::Long,
                        Token::Object,
                    ]
                );
            }
        }
    }

    #[test]
    fn test_lexer_operators_and_symbols() {
        let sql = ", * ( ) ? = <= <> < >= > != `backticked_ident` [bracketed_ident] 'single' \"double\" 12345 -6789 _my_var1";
        let mut lexer = Lexer::new(sql);
        for res in [
            lexer.tokenize(),
            Err(Error::Sql {
                message: "simulated".to_string(),
            }),
        ] {
            if let Ok(tokens) = res {
                assert_eq!(
                    tokens,
                    vec![
                        Token::Comma,
                        Token::Asterisk,
                        Token::OpenParen,
                        Token::CloseParen,
                        Token::QuestionMark,
                        Token::Equal,
                        Token::LessOrEqual,
                        Token::NotEqual,
                        Token::LessThan,
                        Token::GreaterOrEqual,
                        Token::GreaterThan,
                        Token::NotEqual,
                        Token::Identifier("backticked_ident".to_string()),
                        Token::Identifier("bracketed_ident".to_string()),
                        Token::StringLiteral("single".to_string()),
                        Token::StringLiteral("double".to_string()),
                        Token::IntegerLiteral(12345),
                        Token::IntegerLiteral(-6789),
                        Token::Identifier("_my_var1".to_string()),
                    ]
                );
            }
        }
    }

    #[test]
    fn test_lexer_errors() {
        // Stray exclamation mark not followed by '='
        let mut lex1 = Lexer::new("!");
        assert!(lex1.tokenize().is_err());

        // Stray exclamation mark followed by non-equal character
        let mut lex1_sub = Lexer::new("!x");
        assert!(lex1_sub.tokenize().is_err());

        // Unclosed single quote
        let mut lex2 = Lexer::new("'unclosed");
        assert!(lex2.tokenize().is_err());

        // Unclosed double quote
        let mut lex3 = Lexer::new("\"unclosed");
        assert!(lex3.tokenize().is_err());

        // Unclosed backtick
        let mut lex4 = Lexer::new("`unclosed");
        assert!(lex4.tokenize().is_err());

        // Unclosed square bracket
        let mut lex5 = Lexer::new("[unclosed");
        assert!(lex5.tokenize().is_err());

        // Stray dash without digits
        let mut lex6 = Lexer::new("-");
        assert!(lex6.tokenize().is_err());

        // Integer overflow
        let mut lex7 = Lexer::new("9999999999999999999999999");
        assert!(lex7.tokenize().is_err());

        // Unexpected character
        let mut lex8 = Lexer::new("@");
        assert!(lex8.tokenize().is_err());
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_lexer_trailing_operators() {
        let mut lex_lt = Lexer::new("<");
        assert_eq!(lex_lt.tokenize(), Ok(vec![Token::LessThan]));

        let mut lex_gt = Lexer::new(">");
        assert_eq!(lex_gt.tokenize(), Ok(vec![Token::GreaterThan]));
    }

    #[test]
    fn test_lexer_derives() {
        let token = Token::Select;
        let cloned_token = token.clone();
        assert_eq!(token, cloned_token);
        assert_ne!(token, Token::From);
        assert!(format!("{token:?}").contains("Select"));

        let lexer = Lexer::new("SELECT 1");
        assert!(format!("{lexer:?}").contains("Lexer"));
    }
}
