//! ECMAScript / `JScript` Pure-Rust Sandboxed Interpreter.
//!
//! Grounded in Microsoft Windows Installer specifications for `JScript` custom actions:
//! - Complete AST evaluator supporting variables, functions, control structures, and expressions.
//! - Binds the MSI `Session` automation object model directly into script execution scope:
//!   - `Session.Property(name)` / `Session.Property("NAME") = value`
//!   - `Session.EvaluateCondition(expr)`
//!   - `Session.Message(kind, text)`
//!   - `Session.Mode(modeId)`
//!   - `Session.DoAction(actionName)`
//!   - `Session.Database.TableExists(tableName)` / `Session.Database.RowCount(tableName)`
//! - Sandboxed execution: strictly in-memory, bounded recursion and execution fuel,
//!   and detailed line/column runtime error diagnostics with call stack tracking.

use crate::error::{Error, Result};
use crate::execution::script_engine::session::ScriptSession;
use std::collections::HashMap;

/// Representation of values in the `JScript` execution environment.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum JsValue {
    /// JavaScript `undefined` value.
    #[default]
    Undefined,
    /// JavaScript `null` value.
    Null,
    /// Boolean value (`true` or `false`).
    Boolean(bool),
    /// Number value (64-bit float).
    Number(f64),
    /// String primitive value.
    String(String),
    /// Object value mapping string keys to values.
    Object(HashMap<String, Self>),
    /// Special sentinel representing the `Session` object.
    SessionObject,
    /// Special sentinel representing the `Session.Database` object.
    DatabaseObject,
    /// Special sentinel representing the `Math` built-in object.
    MathObject,
}

impl JsValue {
    /// Determines whether the value is truthy according to JavaScript semantics.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(b) => *b,
            Self::Number(n) => *n != 0.0 && !n.is_nan(),
            Self::String(s) => !s.is_empty(),
            Self::Object(_) | Self::SessionObject | Self::DatabaseObject | Self::MathObject => true,
        }
    }

    /// Converts the JavaScript value to a string representation.
    #[must_use]
    pub fn to_string_value(&self) -> String {
        match self {
            Self::Undefined => "undefined".to_string(),
            Self::Null => "null".to_string(),
            Self::Boolean(b) => b.to_string(),
            Self::Number(n) => {
                if n.is_finite() && n.fract() == 0.0 {
                    format!("{n:.0}")
                } else {
                    n.to_string()
                }
            }
            Self::String(s) => s.clone(),
            Self::Object(_) => "[object Object]".to_string(),
            Self::SessionObject => "[object Session]".to_string(),
            Self::DatabaseObject => "[object Database]".to_string(),
            Self::MathObject => "[object Math]".to_string(),
        }
    }

    /// Converts the JavaScript value to a number.
    #[must_use]
    pub fn to_number(&self) -> f64 {
        match self {
            Self::Undefined => f64::NAN,
            Self::Null => 0.0,
            Self::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Number(n) => *n,
            Self::String(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
            Self::Object(_) | Self::SessionObject | Self::DatabaseObject | Self::MathObject => {
                f64::NAN
            }
        }
    }
}

/// Token types produced by the `JScript` lexer.
#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    /// Identifier name.
    Identifier(String),
    /// Numeric literal.
    Number(f64),
    /// String literal literal.
    StringLiteral(String),
    /// `var` keyword.
    KeywordVar,
    /// `let` keyword.
    KeywordLet,
    /// `if` keyword.
    KeywordIf,
    /// `else` keyword.
    KeywordElse,
    /// `while` keyword.
    KeywordWhile,
    /// `for` keyword.
    KeywordFor,
    /// `function` keyword.
    KeywordFunction,
    /// `return` keyword.
    KeywordReturn,
    /// `true` keyword.
    KeywordTrue,
    /// `false` keyword.
    KeywordFalse,
    /// `null` keyword.
    KeywordNull,
    /// `undefined` keyword.
    KeywordUndefined,
    /// `+` operator.
    Plus,
    /// `-` operator.
    Minus,
    /// `*` operator.
    Star,
    /// `/` operator.
    Slash,
    /// `%` operator.
    Percent,
    /// `=` assignment operator.
    Equal,
    /// `==` equality operator.
    EqualEqual,
    /// `===` strict equality operator.
    EqualEqualEqual,
    /// `!=` inequality operator.
    NotEqual,
    /// `!==` strict inequality operator.
    NotEqualEqual,
    /// `<` less-than operator.
    Less,
    /// `<=` less-than-or-equal operator.
    LessEqual,
    /// `>` greater-than operator.
    Greater,
    /// `>=` greater-than-or-equal operator.
    GreaterEqual,
    /// `&&` logical AND operator.
    AmpAmp,
    /// `||` logical OR operator.
    PipePipe,
    /// `!` logical NOT operator.
    Bang,
    /// `+=` compound add operator.
    PlusEqual,
    /// `-=` compound subtract operator.
    MinusEqual,
    /// `++` increment operator.
    PlusPlus,
    /// `--` decrement operator.
    MinusMinus,
    /// `?` ternary conditional operator.
    Question,
    /// `:` colon punctuation.
    Colon,
    /// `(` opening parenthesis.
    LeftParen,
    /// `)` closing parenthesis.
    RightParen,
    /// `{` opening brace.
    LeftBrace,
    /// `}` closing brace.
    RightBrace,
    /// `[` opening bracket.
    LeftBracket,
    /// `]` closing bracket.
    RightBracket,
    /// `;` semicolon punctuation.
    Semicolon,
    /// `,` comma punctuation.
    Comma,
    /// `.` dot member access operator.
    Dot,
    /// End-of-file sentinel.
    Eof,
}

/// Positioned token with 1-based line and column information.
#[derive(Debug, Clone, PartialEq)]
struct Token {
    /// Token classification.
    kind: TokenKind,
    /// 1-based line number.
    line: usize,
    /// 1-based column number.
    col: usize,
}

/// Lexer for scanning ECMAScript / `JScript` source code into tokens.
struct Lexer<'a> {
    /// Characters in source string.
    chars: Vec<char>,
    /// Current character index.
    pos: usize,
    /// Current 1-based line number.
    line: usize,
    /// Current 1-based column number.
    col: usize,
    /// Phantom data binding lifetime.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> Lexer<'a> {
    /// Creates a new [`Lexer`] instance.
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            _marker: std::marker::PhantomData,
        }
    }

    /// Peeks at the current character without consuming it.
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Consumes and returns the current character, advancing position.
    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    /// Skips whitespace characters and line/block comments.
    fn skip_whitespace_and_comments(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else if ch == '/' && self.chars.get(self.pos + 1) == Some(&'/') {
                // Single-line comment
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        break;
                    }
                }
            } else if ch == '/' && self.chars.get(self.pos + 1) == Some(&'*') {
                // Multi-line comment
                self.advance();
                self.advance();
                while let Some(c) = self.advance() {
                    if c == '*' && self.peek() == Some('/') {
                        self.advance();
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Reads a string literal bounded by the given quote character.
    fn read_string(&mut self, quote: char, start_line: usize, start_col: usize) -> Result<Token> {
        let mut text = String::new();
        while let Some(ch) = self.advance() {
            if ch == quote {
                return Ok(Token {
                    kind: TokenKind::StringLiteral(text),
                    line: start_line,
                    col: start_col,
                });
            }
            if ch == '\\' {
                match self.advance() {
                    Some('n') => text.push('\n'),
                    Some('t') => text.push('\t'),
                    Some('r') => text.push('\r'),
                    Some('"') => text.push('"'),
                    Some('\'') => text.push('\''),
                    Some('\\') => text.push('\\'),
                    Some(c) => text.push(c),
                    None => {
                        return Err(Error::ScriptRuntimeError {
                            line: self.line,
                            col: self.col,
                            message: "Unterminated string escape sequence".to_string(),
                        });
                    }
                }
            } else {
                text.push(ch);
            }
        }
        Err(Error::ScriptRuntimeError {
            line: start_line,
            col: start_col,
            message: "Unterminated string literal in script".to_string(),
        })
    }

    /// Scans the next token from input.
    #[allow(clippy::too_many_lines)]
    fn next_token(&mut self) -> Result<Token> {
        self.skip_whitespace_and_comments();
        let start_line = self.line;
        let start_col = self.col;

        let Some(ch) = self.peek() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                line: start_line,
                col: start_col,
            });
        };

        if ch == '"' || ch == '\'' {
            self.advance();
            return self.read_string(ch, start_line, start_col);
        }

        if ch.is_ascii_digit() {
            let mut num_str = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() || c == '.' {
                    num_str.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let n = num_str
                .parse::<f64>()
                .map_err(|_| Error::ScriptRuntimeError {
                    line: start_line,
                    col: start_col,
                    message: format!("Invalid numeric literal: {num_str}"),
                })?;
            return Ok(Token {
                kind: TokenKind::Number(n),
                line: start_line,
                col: start_col,
            });
        }

        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            let mut ident = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                    ident.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let kind = match ident.as_str() {
                "var" => TokenKind::KeywordVar,
                "let" => TokenKind::KeywordLet,
                "if" => TokenKind::KeywordIf,
                "else" => TokenKind::KeywordElse,
                "while" => TokenKind::KeywordWhile,
                "for" => TokenKind::KeywordFor,
                "function" => TokenKind::KeywordFunction,
                "return" => TokenKind::KeywordReturn,
                "true" => TokenKind::KeywordTrue,
                "false" => TokenKind::KeywordFalse,
                "null" => TokenKind::KeywordNull,
                "undefined" => TokenKind::KeywordUndefined,
                _ => TokenKind::Identifier(ident),
            };
            return Ok(Token {
                kind,
                line: start_line,
                col: start_col,
            });
        }

        self.advance();
        let kind = match ch {
            '+' => {
                if self.peek() == Some('+') {
                    self.advance();
                    TokenKind::PlusPlus
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if self.peek() == Some('-') {
                    self.advance();
                    TokenKind::MinusMinus
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::MinusEqual
                } else {
                    TokenKind::Minus
                }
            }
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::EqualEqualEqual
                    } else {
                        TokenKind::EqualEqual
                    }
                } else {
                    TokenKind::Equal
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::NotEqualEqual
                    } else {
                        TokenKind::NotEqual
                    }
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    TokenKind::AmpAmp
                } else {
                    return Err(Error::ScriptRuntimeError {
                        line: start_line,
                        col: start_col,
                        message: "Unexpected single '&' operator".to_string(),
                    });
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    TokenKind::PipePipe
                } else {
                    return Err(Error::ScriptRuntimeError {
                        line: start_line,
                        col: start_col,
                        message: "Unexpected single '|' operator".to_string(),
                    });
                }
            }
            '?' => TokenKind::Question,
            ':' => TokenKind::Colon,
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            ';' => TokenKind::Semicolon,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            other => {
                return Err(Error::ScriptRuntimeError {
                    line: start_line,
                    col: start_col,
                    message: format!("Unexpected character: '{other}'"),
                });
            }
        };

        Ok(Token {
            kind,
            line: start_line,
            col: start_col,
        })
    }

    /// Tokenizes the entire input into a list of tokens.
    fn tokenize_all(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}

/// Binary operation operators supported in expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOp {
    /// Addition operator `+`.
    Add,
    /// Subtraction operator `-`.
    Subtract,
    /// Multiplication operator `*`.
    Multiply,
    /// Division operator `/`.
    Divide,
    /// Modulo operator `%`.
    Modulo,
    /// Equality operator `==`.
    Equal,
    /// Inequality operator `!=`.
    NotEqual,
    /// Strict equality operator `===`.
    StrictEqual,
    /// Strict inequality operator `!==`.
    StrictNotEqual,
    /// Less than operator `<`.
    Less,
    /// Less than or equal operator `<=`.
    LessEqual,
    /// Greater than operator `>`.
    Greater,
    /// Greater than or equal operator `>=`.
    GreaterEqual,
    /// Logical AND operator `&&`.
    And,
    /// Logical OR operator `||`.
    Or,
}

/// Unary operators supported in expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnaryOp {
    /// Numeric negation `-`.
    Negate,
    /// Logical NOT `!`.
    Not,
    /// Pre-increment `++x`.
    PreIncrement,
    /// Pre-decrement `--x`.
    PreDecrement,
    /// Post-increment `x++`.
    PostIncrement,
    /// Post-decrement `x--`.
    PostDecrement,
}

/// AST expression node.
#[derive(Debug, Clone)]
enum Expr {
    /// Literal constant value.
    Literal(JsValue, usize, usize),
    /// Identifier lookup.
    Identifier(String, usize, usize),
    /// Dot member access (`object.property`).
    MemberAccess {
        /// Target object.
        object: Box<Self>,
        /// Property identifier name.
        property: String,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Index bracket access (`object[index]`).
    IndexAccess {
        /// Target object.
        object: Box<Self>,
        /// Key index expression.
        index: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Function or method invocation.
    Call {
        /// Callee expression.
        callee: Box<Self>,
        /// Argument expressions.
        arguments: Vec<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Unary operation.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand expression.
        expr: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Binary operation.
    Binary {
        /// Operator.
        op: BinaryOp,
        /// Left operand.
        left: Box<Self>,
        /// Right operand.
        right: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Assignment expression.
    Assign {
        /// Target assignment target.
        target: Box<Self>,
        /// Assigned value expression.
        value: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Ternary conditional expression (`cond ? true_expr : false_expr`).
    Ternary {
        /// Condition expression.
        condition: Box<Self>,
        /// Expression evaluated if truthy.
        true_expr: Box<Self>,
        /// Expression evaluated if falsy.
        false_expr: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
}

/// AST statement node.
#[derive(Debug, Clone)]
enum Stmt {
    /// Variable declaration (`var x = 1;`).
    VarDecl {
        /// Variable name.
        name: String,
        /// Optional initializer expression.
        init: Option<Expr>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Expression statement.
    Expr(Expr),
    /// Statement block `{ ... }`.
    Block(Vec<Self>),
    /// If-else conditional statement.
    If {
        /// Condition expression.
        condition: Expr,
        /// Then branch statement.
        then_branch: Box<Self>,
        /// Optional else branch statement.
        else_branch: Option<Box<Self>>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// While loop statement.
    While {
        /// Condition expression.
        condition: Expr,
        /// Loop body statement.
        body: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// For loop statement.
    For {
        /// Optional initialization statement.
        init: Option<Box<Self>>,
        /// Optional condition expression.
        condition: Option<Expr>,
        /// Optional step expression.
        step: Option<Expr>,
        /// Loop body statement.
        body: Box<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Function declaration statement.
    FunctionDecl {
        /// Function name.
        name: String,
        /// Parameter identifier list.
        parameters: Vec<String>,
        /// Function body statements.
        body: Vec<Self>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
    /// Return statement.
    Return {
        /// Optional returned expression.
        value: Option<Expr>,
        /// 1-based line number.
        line: usize,
        /// 1-based column number.
        col: usize,
    },
}

/// Recursive descent parser for `JScript` programs.
struct Parser {
    /// Input tokens.
    tokens: Vec<Token>,
    /// Current token position.
    pos: usize,
}

impl Parser {
    /// Creates a new [`Parser`].
    const fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Peeks at current token.
    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token {
            kind: TokenKind::Eof,
            line: 0,
            col: 0,
        })
    }

    /// Advances and returns the current token.
    fn advance(&mut self) -> Token {
        let tok = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    /// Matches and consumes the expected token kind if present.
    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if &self.peek().kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Expects a specific token kind or returns a descriptive parse error.
    fn expect(&mut self, kind: &TokenKind, msg: &str) -> Result<Token> {
        let tok = self.peek().clone();
        if &tok.kind == kind {
            Ok(self.advance())
        } else {
            Err(Error::ScriptRuntimeError {
                line: tok.line,
                col: tok.col,
                message: format!("Parse error: expected {msg}, found {:?}", tok.kind),
            })
        }
    }

    /// Parses the entire token stream as a list of statements.
    fn parse_program(&mut self) -> Result<Vec<Stmt>> {
        let mut statements = Vec::new();
        while self.peek().kind != TokenKind::Eof {
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    /// Parses a single statement.
    #[allow(clippy::too_many_lines)]
    fn parse_statement(&mut self) -> Result<Stmt> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::KeywordVar | TokenKind::KeywordLet => {
                self.advance();
                let next_tok = self.advance();
                let name = match next_tok.kind {
                    TokenKind::Identifier(s) => s,
                    other => {
                        return Err(Error::ScriptRuntimeError {
                            line: next_tok.line,
                            col: next_tok.col,
                            message: format!("Expected variable identifier, found {other:?}"),
                        });
                    }
                };
                let init = if self.match_token(&TokenKind::Equal) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                let _ = self.match_token(&TokenKind::Semicolon);
                Ok(Stmt::VarDecl {
                    name,
                    init,
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::KeywordIf => {
                self.advance();
                self.expect(&TokenKind::LeftParen, "'(' after 'if'")?;
                let condition = self.parse_expression()?;
                self.expect(&TokenKind::RightParen, "')' after if condition")?;
                let then_branch = Box::new(self.parse_statement()?);
                let else_branch = if self.match_token(&TokenKind::KeywordElse) {
                    Some(Box::new(self.parse_statement()?))
                } else {
                    None
                };
                Ok(Stmt::If {
                    condition,
                    then_branch,
                    else_branch,
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::KeywordWhile => {
                self.advance();
                self.expect(&TokenKind::LeftParen, "'(' after 'while'")?;
                let condition = self.parse_expression()?;
                self.expect(&TokenKind::RightParen, "')' after while condition")?;
                let body = Box::new(self.parse_statement()?);
                Ok(Stmt::While {
                    condition,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::KeywordFor => {
                self.advance();
                self.expect(&TokenKind::LeftParen, "'(' after 'for'")?;
                let init = if self.match_token(&TokenKind::Semicolon) {
                    None
                } else {
                    let s = self.parse_statement()?;
                    Some(Box::new(s))
                };
                let condition = if self.peek().kind == TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expression()?)
                };
                self.expect(&TokenKind::Semicolon, "';' in for loop")?;
                let step = if self.peek().kind == TokenKind::RightParen {
                    None
                } else {
                    Some(self.parse_expression()?)
                };
                self.expect(&TokenKind::RightParen, "')' after for clauses")?;
                let body = Box::new(self.parse_statement()?);
                Ok(Stmt::For {
                    init,
                    condition,
                    step,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::KeywordFunction => {
                self.advance();
                let next_tok = self.advance();
                let name = match next_tok.kind {
                    TokenKind::Identifier(s) => s,
                    other => {
                        return Err(Error::ScriptRuntimeError {
                            line: next_tok.line,
                            col: next_tok.col,
                            message: format!("Expected function name, found {other:?}"),
                        });
                    }
                };
                self.expect(&TokenKind::LeftParen, "'(' after function name")?;
                let mut parameters = Vec::new();
                if self.peek().kind != TokenKind::RightParen {
                    loop {
                        let param_tok = self.advance();
                        if let TokenKind::Identifier(p) = param_tok.kind {
                            parameters.push(p);
                        }
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RightParen, "')' after function parameters")?;
                self.expect(&TokenKind::LeftBrace, "'{' at function body start")?;
                let mut body = Vec::new();
                while self.peek().kind != TokenKind::RightBrace
                    && self.peek().kind != TokenKind::Eof
                {
                    body.push(self.parse_statement()?);
                }
                self.expect(&TokenKind::RightBrace, "'}' at function body end")?;
                Ok(Stmt::FunctionDecl {
                    name,
                    parameters,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::KeywordReturn => {
                self.advance();
                let value = if self.peek().kind == TokenKind::Semicolon {
                    None
                } else {
                    Some(self.parse_expression()?)
                };
                let _ = self.match_token(&TokenKind::Semicolon);
                Ok(Stmt::Return {
                    value,
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::LeftBrace => {
                self.advance();
                let mut block_stmts = Vec::new();
                while self.peek().kind != TokenKind::RightBrace
                    && self.peek().kind != TokenKind::Eof
                {
                    block_stmts.push(self.parse_statement()?);
                }
                self.expect(&TokenKind::RightBrace, "'}' at block end")?;
                Ok(Stmt::Block(block_stmts))
            }
            _ => {
                let expr = self.parse_expression()?;
                let _ = self.match_token(&TokenKind::Semicolon);
                Ok(Stmt::Expr(expr))
            }
        }
    }

    /// Parses an expression.
    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_assignment()
    }

    /// Parses an assignment expression.
    fn parse_assignment(&mut self) -> Result<Expr> {
        let expr = self.parse_ternary()?;
        if self.match_token(&TokenKind::Equal) {
            let val = self.parse_assignment()?;
            return Ok(Expr::Assign {
                target: Box::new(expr),
                value: Box::new(val),
                line: self.peek().line,
                col: self.peek().col,
            });
        }
        if self.match_token(&TokenKind::PlusEqual) {
            let val = self.parse_assignment()?;
            let line = self.peek().line;
            let col = self.peek().col;
            let add_expr = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(expr.clone()),
                right: Box::new(val),
                line,
                col,
            };
            return Ok(Expr::Assign {
                target: Box::new(expr),
                value: Box::new(add_expr),
                line,
                col,
            });
        }
        if self.match_token(&TokenKind::MinusEqual) {
            let val = self.parse_assignment()?;
            let line = self.peek().line;
            let col = self.peek().col;
            let sub_expr = Expr::Binary {
                op: BinaryOp::Subtract,
                left: Box::new(expr.clone()),
                right: Box::new(val),
                line,
                col,
            };
            return Ok(Expr::Assign {
                target: Box::new(expr),
                value: Box::new(sub_expr),
                line,
                col,
            });
        }
        Ok(expr)
    }

    /// Parses ternary conditional operator `? :`.
    fn parse_ternary(&mut self) -> Result<Expr> {
        let mut expr = self.parse_logical_or()?;
        if self.match_token(&TokenKind::Question) {
            let true_expr = self.parse_expression()?;
            self.expect(&TokenKind::Colon, "':' in ternary conditional")?;
            let false_expr = self.parse_expression()?;
            expr = Expr::Ternary {
                condition: Box::new(expr),
                true_expr: Box::new(true_expr),
                false_expr: Box::new(false_expr),
                line: self.peek().line,
                col: self.peek().col,
            };
        }
        Ok(expr)
    }

    /// Parses logical OR `||`.
    fn parse_logical_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_logical_and()?;
        while self.match_token(&TokenKind::PipePipe) {
            let right = self.parse_logical_and()?;
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                line: self.peek().line,
                col: self.peek().col,
            };
        }
        Ok(left)
    }

    /// Parses logical AND `&&`.
    fn parse_logical_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;
        while self.match_token(&TokenKind::AmpAmp) {
            let right = self.parse_equality()?;
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
                line: self.peek().line,
                col: self.peek().col,
            };
        }
        Ok(left)
    }

    /// Parses equality operators `==`, `!=`, `===`, `!==`.
    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                TokenKind::EqualEqual => BinaryOp::Equal,
                TokenKind::NotEqual => BinaryOp::NotEqual,
                TokenKind::EqualEqualEqual => BinaryOp::StrictEqual,
                TokenKind::NotEqualEqual => BinaryOp::StrictNotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses relational comparison operators `<`, `<=`, `>`, `>=`.
    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_addition()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses addition and subtraction `+`, `-`.
    fn parse_addition(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplication()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses multiplication, division, modulo `*`, `/`, `%`.
    fn parse_multiplication(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Modulo,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses unary prefix operators `-`, `!`, `++`, `--`.
    fn parse_unary(&mut self) -> Result<Expr> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Negate,
                    expr: Box::new(expr),
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::PlusPlus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::PreIncrement,
                    expr: Box::new(expr),
                    line: tok.line,
                    col: tok.col,
                })
            }
            TokenKind::MinusMinus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::PreDecrement,
                    expr: Box::new(expr),
                    line: tok.line,
                    col: tok.col,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// Parses postfix operations (calls, index access, member access, `++`, `--`).
    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            let tok = self.peek().clone();
            match tok.kind {
                TokenKind::Dot => {
                    self.advance();
                    let prop_tok = self.advance();
                    let prop = match prop_tok.kind {
                        TokenKind::Identifier(s) => s,
                        other => {
                            return Err(Error::ScriptRuntimeError {
                                line: prop_tok.line,
                                col: prop_tok.col,
                                message: format!("Expected member name after '.', found {other:?}"),
                            });
                        }
                    };
                    expr = Expr::MemberAccess {
                        object: Box::new(expr),
                        property: prop,
                        line: tok.line,
                        col: tok.col,
                    };
                }
                TokenKind::LeftBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&TokenKind::RightBracket, "']' after index expression")?;
                    expr = Expr::IndexAccess {
                        object: Box::new(expr),
                        index: Box::new(index),
                        line: tok.line,
                        col: tok.col,
                    };
                }
                TokenKind::LeftParen => {
                    self.advance();
                    let mut arguments = Vec::new();
                    if self.peek().kind != TokenKind::RightParen {
                        loop {
                            arguments.push(self.parse_expression()?);
                            if !self.match_token(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RightParen, "')' after call arguments")?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        arguments,
                        line: tok.line,
                        col: tok.col,
                    };
                }
                TokenKind::PlusPlus => {
                    self.advance();
                    expr = Expr::Unary {
                        op: UnaryOp::PostIncrement,
                        expr: Box::new(expr),
                        line: tok.line,
                        col: tok.col,
                    };
                }
                TokenKind::MinusMinus => {
                    self.advance();
                    expr = Expr::Unary {
                        op: UnaryOp::PostDecrement,
                        expr: Box::new(expr),
                        line: tok.line,
                        col: tok.col,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    /// Parses primary tokens (literals, identifiers, parentheses).
    fn parse_primary(&mut self) -> Result<Expr> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::Number(n) => Ok(Expr::Literal(JsValue::Number(n), tok.line, tok.col)),
            TokenKind::StringLiteral(s) => Ok(Expr::Literal(JsValue::String(s), tok.line, tok.col)),
            TokenKind::KeywordTrue => Ok(Expr::Literal(JsValue::Boolean(true), tok.line, tok.col)),
            TokenKind::KeywordFalse => {
                Ok(Expr::Literal(JsValue::Boolean(false), tok.line, tok.col))
            }
            TokenKind::KeywordNull => Ok(Expr::Literal(JsValue::Null, tok.line, tok.col)),
            TokenKind::KeywordUndefined => Ok(Expr::Literal(JsValue::Undefined, tok.line, tok.col)),
            TokenKind::Identifier(id) => Ok(Expr::Identifier(id, tok.line, tok.col)),
            TokenKind::LeftParen => {
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RightParen, "')' closing parenthesis")?;
                Ok(expr)
            }
            _ => Err(Error::ScriptRuntimeError {
                line: tok.line,
                col: tok.col,
                message: format!("Unexpected token in expression: {:?}", tok.kind),
            }),
        }
    }
}

/// Function definition stored in execution environment.
#[derive(Debug, Clone)]
struct FunctionDef {
    /// Parameter names.
    parameters: Vec<String>,
    /// Body statements.
    body: Vec<Stmt>,
}

/// Environment frame storing variables and functions.
#[derive(Debug, Clone, Default)]
struct Environment {
    /// Local variables map.
    variables: HashMap<String, JsValue>,
    /// Function definitions map.
    functions: HashMap<String, FunctionDef>,
}

/// The `JScript` / ECMAScript execution engine.
#[derive(Debug, Default)]
pub struct JScriptEngine {
    /// Environment stack for scoping.
    env_stack: Vec<Environment>,
}

impl JScriptEngine {
    /// Creates a new [`JScriptEngine`].
    #[must_use]
    pub fn new() -> Self {
        let mut engine = Self {
            env_stack: Vec::new(),
        };
        engine.env_stack.push(Environment::default());
        engine
    }

    /// Evaluates `JScript` code within the given [`ScriptSession`].
    ///
    /// # Arguments
    ///
    /// * `script` - The source code to execute.
    /// * `session` - The active installer session object.
    ///
    /// # Returns
    ///
    /// The final result [`JsValue`] of the script.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptRuntimeError`] on syntax or execution failure.
    pub fn execute(&mut self, script: &str, session: &mut ScriptSession) -> Result<JsValue> {
        let mut lexer = Lexer::new(script);
        let tokens = lexer.tokenize_all()?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program()?;

        let mut last_val = JsValue::Undefined;
        for stmt in &program {
            if let Some(val) = self.eval_statement(stmt, session)? {
                last_val = val;
            }
        }
        Ok(last_val)
    }

    /// Looks up variable by name in current scope chain.
    fn lookup_var(&self, name: &str, session: &ScriptSession) -> Option<JsValue> {
        if name == "Session" {
            return Some(JsValue::SessionObject);
        }
        if name == "Math" {
            return Some(JsValue::MathObject);
        }
        for frame in self.env_stack.iter().rev() {
            if let Some(val) = frame.variables.get(name) {
                return Some(val.clone());
            }
        }
        // Mirror session property if public uppercase identifier
        let prop_val = session.property(name);
        if !prop_val.is_empty() {
            return Some(JsValue::String(prop_val));
        }
        None
    }

    /// Sets variable in current scope chain.
    fn set_var(&mut self, name: &str, value: &JsValue, session: &mut ScriptSession) {
        // Check local frames first
        for frame in self.env_stack.iter_mut().rev() {
            if frame.variables.contains_key(name) {
                frame.variables.insert(name.to_string(), value.clone());
                if name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
                {
                    session.set_property(name, &value.to_string_value());
                }
                return;
            }
        }
        // Otherwise set in global frame
        if let Some(global) = self.env_stack.first_mut() {
            global.variables.insert(name.to_string(), value.clone());
        }
        // Mirror to session properties if uppercase public property name
        if name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            session.set_property(name, &value.to_string_value());
        }
    }

    /// Evaluates a statement.
    #[allow(clippy::too_many_lines)]
    fn eval_statement(
        &mut self,
        stmt: &Stmt,
        session: &mut ScriptSession,
    ) -> Result<Option<JsValue>> {
        match stmt {
            Stmt::VarDecl {
                name,
                init,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let val = if let Some(expr) = init {
                    self.eval_expression(expr, session)?
                } else {
                    JsValue::Undefined
                };
                if let Some(current) = self.env_stack.last_mut() {
                    current.variables.insert(name.clone(), val.clone());
                }
                if name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
                {
                    session.set_property(name, &val.to_string_value());
                }
                Ok(None)
            }
            Stmt::Expr(expr) => {
                let val = self.eval_expression(expr, session)?;
                Ok(Some(val))
            }
            Stmt::Block(stmts) => {
                self.env_stack.push(Environment::default());
                let mut ret = None;
                for s in stmts {
                    if let Some(val) = self.eval_statement(s, session)? {
                        ret = Some(val);
                    }
                }
                self.env_stack.pop();
                Ok(ret)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let cond_val = self.eval_expression(condition, session)?;
                if cond_val.is_truthy() {
                    self.eval_statement(then_branch, session)
                } else if let Some(else_b) = else_branch {
                    self.eval_statement(else_b, session)
                } else {
                    Ok(None)
                }
            }
            Stmt::While {
                condition,
                body,
                line,
                col,
            } => {
                while {
                    session.consume_fuel(*line, *col)?;
                    self.eval_expression(condition, session)?.is_truthy()
                } {
                    if let Some(val) = self.eval_statement(body, session)? {
                        let _ = val;
                    }
                }
                Ok(None)
            }
            Stmt::For {
                init,
                condition,
                step,
                body,
                line,
                col,
            } => {
                if let Some(init_stmt) = init {
                    self.eval_statement(init_stmt, session)?;
                }
                loop {
                    session.consume_fuel(*line, *col)?;
                    if let Some(cond_expr) = condition {
                        if !self.eval_expression(cond_expr, session)?.is_truthy() {
                            break;
                        }
                    }
                    self.eval_statement(body, session)?;
                    if let Some(step_expr) = step {
                        self.eval_expression(step_expr, session)?;
                    }
                }
                Ok(None)
            }
            Stmt::FunctionDecl {
                name,
                parameters,
                body,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                if let Some(global) = self.env_stack.first_mut() {
                    global.functions.insert(
                        name.clone(),
                        FunctionDef {
                            parameters: parameters.clone(),
                            body: body.clone(),
                        },
                    );
                }
                Ok(None)
            }
            Stmt::Return { value, line, col } => {
                session.consume_fuel(*line, *col)?;
                let ret_val = if let Some(expr) = value {
                    self.eval_expression(expr, session)?
                } else {
                    JsValue::Undefined
                };
                Ok(Some(ret_val))
            }
        }
    }

    /// Evaluates an expression.
    #[allow(clippy::too_many_lines)]
    fn eval_expression(&mut self, expr: &Expr, session: &mut ScriptSession) -> Result<JsValue> {
        match expr {
            Expr::Literal(val, line, col) => {
                session.consume_fuel(*line, *col)?;
                Ok(val.clone())
            }
            Expr::Identifier(id, line, col) => {
                session.consume_fuel(*line, *col)?;
                Ok(self.lookup_var(id, session).unwrap_or(JsValue::Undefined))
            }
            Expr::MemberAccess {
                object,
                property,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let obj_val = self.eval_expression(object, session)?;
                match obj_val {
                    JsValue::SessionObject => {
                        if property == "Database" {
                            Ok(JsValue::DatabaseObject)
                        } else {
                            let val = session.property(property);
                            Ok(JsValue::String(val))
                        }
                    }
                    JsValue::DatabaseObject | JsValue::MathObject => {
                        Ok(JsValue::String(property.clone()))
                    }
                    JsValue::String(s) => {
                        if property == "length" {
                            #[allow(clippy::cast_precision_loss)]
                            Ok(JsValue::Number(s.len() as f64))
                        } else {
                            Ok(JsValue::Undefined)
                        }
                    }
                    JsValue::Object(map) => {
                        Ok(map.get(property).cloned().unwrap_or(JsValue::Undefined))
                    }
                    _ => Ok(JsValue::Undefined),
                }
            }
            Expr::IndexAccess {
                object,
                index,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let obj_val = self.eval_expression(object, session)?;
                let idx_val = self.eval_expression(index, session)?;
                match obj_val {
                    JsValue::SessionObject => {
                        let prop_name = idx_val.to_string_value();
                        Ok(JsValue::String(session.property(&prop_name)))
                    }
                    JsValue::Object(map) => {
                        let key = idx_val.to_string_value();
                        Ok(map.get(&key).cloned().unwrap_or(JsValue::Undefined))
                    }
                    _ => Ok(JsValue::Undefined),
                }
            }
            Expr::Call {
                callee,
                arguments,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                if let Expr::MemberAccess {
                    object, property, ..
                } = &**callee
                {
                    let obj_val = self.eval_expression(object, session)?;
                    return self.call_method(&obj_val, property, arguments, *line, *col, session);
                }

                if let Expr::Identifier(fn_name, _, _) = &**callee {
                    let mut evaluated_args = Vec::new();
                    for arg in arguments {
                        evaluated_args.push(self.eval_expression(arg, session)?);
                    }
                    return self.call_function(fn_name, &evaluated_args, *line, *col, session);
                }

                Err(Error::ScriptRuntimeError {
                    line: *line,
                    col: *col,
                    message: "Invalid callee in function call".to_string(),
                })
            }
            Expr::Unary {
                op,
                expr,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let val = self.eval_expression(expr, session)?;
                match op {
                    UnaryOp::Negate => Ok(JsValue::Number(-val.to_number())),
                    UnaryOp::Not => Ok(JsValue::Boolean(!val.is_truthy())),
                    UnaryOp::PreIncrement => {
                        let new_num = val.to_number() + 1.0;
                        if let Expr::Identifier(id, _, _) = &**expr {
                            self.set_var(id, &JsValue::Number(new_num), session);
                        }
                        Ok(JsValue::Number(new_num))
                    }
                    UnaryOp::PreDecrement => {
                        let new_num = val.to_number() - 1.0;
                        if let Expr::Identifier(id, _, _) = &**expr {
                            self.set_var(id, &JsValue::Number(new_num), session);
                        }
                        Ok(JsValue::Number(new_num))
                    }
                    UnaryOp::PostIncrement => {
                        let old_num = val.to_number();
                        if let Expr::Identifier(id, _, _) = &**expr {
                            self.set_var(id, &JsValue::Number(old_num + 1.0), session);
                        }
                        Ok(JsValue::Number(old_num))
                    }
                    UnaryOp::PostDecrement => {
                        let old_num = val.to_number();
                        if let Expr::Identifier(id, _, _) = &**expr {
                            self.set_var(id, &JsValue::Number(old_num - 1.0), session);
                        }
                        Ok(JsValue::Number(old_num))
                    }
                }
            }
            Expr::Binary {
                op,
                left,
                right,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let l_val = self.eval_expression(left, session)?;
                match op {
                    BinaryOp::And => {
                        if l_val.is_truthy() {
                            self.eval_expression(right, session)
                        } else {
                            Ok(l_val)
                        }
                    }
                    BinaryOp::Or => {
                        if l_val.is_truthy() {
                            Ok(l_val)
                        } else {
                            self.eval_expression(right, session)
                        }
                    }
                    BinaryOp::Add => {
                        let r_val = self.eval_expression(right, session)?;
                        if matches!(l_val, JsValue::String(_))
                            || matches!(r_val, JsValue::String(_))
                        {
                            Ok(JsValue::String(format!(
                                "{}{}",
                                l_val.to_string_value(),
                                r_val.to_string_value()
                            )))
                        } else {
                            Ok(JsValue::Number(l_val.to_number() + r_val.to_number()))
                        }
                    }
                    BinaryOp::Subtract => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Number(l_val.to_number() - r_val.to_number()))
                    }
                    BinaryOp::Multiply => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Number(l_val.to_number() * r_val.to_number()))
                    }
                    BinaryOp::Divide => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Number(l_val.to_number() / r_val.to_number()))
                    }
                    BinaryOp::Modulo => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Number(l_val.to_number() % r_val.to_number()))
                    }
                    BinaryOp::Equal | BinaryOp::StrictEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Boolean(
                            l_val.to_string_value() == r_val.to_string_value(),
                        ))
                    }
                    BinaryOp::NotEqual | BinaryOp::StrictNotEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Boolean(
                            l_val.to_string_value() != r_val.to_string_value(),
                        ))
                    }
                    BinaryOp::Less => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Boolean(l_val.to_number() < r_val.to_number()))
                    }
                    BinaryOp::LessEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Boolean(l_val.to_number() <= r_val.to_number()))
                    }
                    BinaryOp::Greater => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Boolean(l_val.to_number() > r_val.to_number()))
                    }
                    BinaryOp::GreaterEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(JsValue::Boolean(l_val.to_number() >= r_val.to_number()))
                    }
                }
            }
            Expr::Assign {
                target,
                value,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let eval_val = self.eval_expression(value, session)?;
                match &**target {
                    Expr::Identifier(id, _, _) => {
                        self.set_var(id, &eval_val, session);
                        Ok(eval_val)
                    }
                    Expr::MemberAccess {
                        object, property, ..
                    } => {
                        let obj_val = self.eval_expression(object, session)?;
                        if matches!(obj_val, JsValue::SessionObject) {
                            session.set_property(property, &eval_val.to_string_value());
                        }
                        Ok(eval_val)
                    }
                    Expr::Call {
                        callee, arguments, ..
                    } => {
                        if let Expr::MemberAccess {
                            object, property, ..
                        } = &**callee
                        {
                            let obj_val = self.eval_expression(object, session)?;
                            if matches!(obj_val, JsValue::SessionObject)
                                && property == "Property"
                                && !arguments.is_empty()
                            {
                                let prop_name = self
                                    .eval_expression(&arguments[0], session)?
                                    .to_string_value();
                                session.set_property(&prop_name, &eval_val.to_string_value());
                                return Ok(eval_val);
                            }
                        }
                        Err(Error::ScriptRuntimeError {
                            line: *line,
                            col: *col,
                            message: "Invalid left-hand side assignment target".to_string(),
                        })
                    }
                    Expr::IndexAccess { object, index, .. } => {
                        let obj_val = self.eval_expression(object, session)?;
                        let idx_val = self.eval_expression(index, session)?;
                        if matches!(obj_val, JsValue::SessionObject) {
                            session.set_property(
                                &idx_val.to_string_value(),
                                &eval_val.to_string_value(),
                            );
                        }
                        Ok(eval_val)
                    }
                    _ => Err(Error::ScriptRuntimeError {
                        line: *line,
                        col: *col,
                        message: "Invalid left-hand side assignment target".to_string(),
                    }),
                }
            }
            Expr::Ternary {
                condition,
                true_expr,
                false_expr,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let cond_val = self.eval_expression(condition, session)?;
                if cond_val.is_truthy() {
                    self.eval_expression(true_expr, session)
                } else {
                    self.eval_expression(false_expr, session)
                }
            }
        }
    }

    /// Invokes a method on a built-in or automation object.
    #[allow(clippy::too_many_lines)]
    fn call_method(
        &mut self,
        target: &JsValue,
        method: &str,
        arguments: &[Expr],
        line: usize,
        col: usize,
        session: &mut ScriptSession,
    ) -> Result<JsValue> {
        match target {
            JsValue::SessionObject => {
                let mut evaluated_args = Vec::new();
                for arg in arguments {
                    evaluated_args.push(self.eval_expression(arg, session)?);
                }
                match method {
                    "Property" => {
                        let prop_name = evaluated_args.first().map_or("", |v| match v {
                            JsValue::String(s) => s.as_str(),
                            _ => "",
                        });
                        if evaluated_args.len() >= 2 {
                            let prop_val = evaluated_args[1].to_string_value();
                            session.set_property(prop_name, &prop_val);
                            Ok(JsValue::String(prop_val))
                        } else {
                            Ok(JsValue::String(session.property(prop_name)))
                        }
                    }
                    "EvaluateCondition" => {
                        let expr_str = evaluated_args
                            .first()
                            .map_or(String::new(), JsValue::to_string_value);
                        let res = session.evaluate_condition(&expr_str);
                        Ok(JsValue::Number(f64::from(res)))
                    }
                    "Message" => {
                        #[allow(clippy::cast_possible_truncation)]
                        let kind = evaluated_args.first().map_or(0.0, JsValue::to_number) as i32;
                        let text = evaluated_args
                            .get(1)
                            .map_or(String::new(), JsValue::to_string_value);
                        let ret = session.message(kind, &text);
                        Ok(JsValue::Number(f64::from(ret)))
                    }
                    "Mode" => {
                        #[allow(clippy::cast_possible_truncation)]
                        let mode_id = evaluated_args.first().map_or(0.0, JsValue::to_number) as i32;
                        Ok(JsValue::Boolean(session.mode(mode_id)))
                    }
                    "DoAction" => {
                        let action_name = evaluated_args
                            .first()
                            .map_or(String::new(), JsValue::to_string_value);
                        let ret = session.do_action(&action_name);
                        Ok(JsValue::Number(f64::from(ret)))
                    }
                    _ => Err(Error::ScriptRuntimeError {
                        line,
                        col,
                        message: format!("Unknown method '{method}' on Session object"),
                    }),
                }
            }
            JsValue::DatabaseObject => {
                let mut evaluated_args = Vec::new();
                for arg in arguments {
                    evaluated_args.push(self.eval_expression(arg, session)?);
                }
                let table_name = evaluated_args
                    .first()
                    .map_or(String::new(), JsValue::to_string_value);
                match method {
                    "TableExists" => Ok(JsValue::Boolean(
                        session.database().table_exists(&table_name),
                    )),
                    #[allow(clippy::cast_precision_loss)]
                    "RowCount" => Ok(JsValue::Number(
                        session.database().row_count(&table_name) as f64
                    )),
                    _ => Err(Error::ScriptRuntimeError {
                        line,
                        col,
                        message: format!("Unknown method '{method}' on Database object"),
                    }),
                }
            }
            JsValue::MathObject => {
                let mut evaluated_args = Vec::new();
                for arg in arguments {
                    evaluated_args.push(self.eval_expression(arg, session)?);
                }
                let first_num = evaluated_args.first().map_or(0.0, JsValue::to_number);
                match method {
                    "floor" => Ok(JsValue::Number(first_num.floor())),
                    "ceil" => Ok(JsValue::Number(first_num.ceil())),
                    "round" => Ok(JsValue::Number(first_num.round())),
                    "abs" => Ok(JsValue::Number(first_num.abs())),
                    "max" => {
                        let second_num = evaluated_args.get(1).map_or(0.0, JsValue::to_number);
                        Ok(JsValue::Number(first_num.max(second_num)))
                    }
                    "min" => {
                        let second_num = evaluated_args.get(1).map_or(0.0, JsValue::to_number);
                        Ok(JsValue::Number(first_num.min(second_num)))
                    }
                    _ => Err(Error::ScriptRuntimeError {
                        line,
                        col,
                        message: format!("Unknown method '{method}' on Math object"),
                    }),
                }
            }
            _ => Err(Error::ScriptRuntimeError {
                line,
                col,
                message: format!("Cannot call method '{method}' on non-object"),
            }),
        }
    }

    /// Invokes a global function or user-defined function.
    fn call_function(
        &mut self,
        name: &str,
        args: &[JsValue],
        line: usize,
        col: usize,
        session: &mut ScriptSession,
    ) -> Result<JsValue> {
        match name {
            "parseInt" => {
                let s = args.first().map_or(String::new(), JsValue::to_string_value);
                #[allow(clippy::cast_precision_loss)]
                let val = s.trim().parse::<i64>().map_or(f64::NAN, |i| i as f64);
                Ok(JsValue::Number(val))
            }
            "parseFloat" => {
                let s = args.first().map_or(String::new(), JsValue::to_string_value);
                let val = s.trim().parse::<f64>().unwrap_or(f64::NAN);
                Ok(JsValue::Number(val))
            }
            "String" => Ok(JsValue::String(
                args.first().map_or(String::new(), JsValue::to_string_value),
            )),
            "isNaN" => {
                let n = args.first().map_or(f64::NAN, JsValue::to_number);
                Ok(JsValue::Boolean(n.is_nan()))
            }
            _ => {
                let fn_def = self
                    .env_stack
                    .first()
                    .and_then(|g| g.functions.get(name).cloned());
                if let Some(def) = fn_def {
                    session.push_call(name);
                    let mut call_env = Environment::default();
                    for (i, param) in def.parameters.iter().enumerate() {
                        let arg_val = args.get(i).cloned().unwrap_or(JsValue::Undefined);
                        call_env.variables.insert(param.clone(), arg_val);
                    }
                    self.env_stack.push(call_env);
                    let mut ret = JsValue::Undefined;
                    for stmt in &def.body {
                        if let Some(val) = self.eval_statement(stmt, session)? {
                            ret = val;
                            break;
                        }
                    }
                    self.env_stack.pop();
                    session.pop_call();
                    Ok(ret)
                } else {
                    Err(Error::ScriptRuntimeError {
                        line,
                        col,
                        message: format!("Undefined function '{name}'"),
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jscript_basic_arithmetic_and_variables() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        let script = "
            var x = 10;
            var y = 20;
            var z = x + y * 2;
            z;
        ";
        let res = engine.execute(script, &mut session);
        assert_eq!(res, Ok(JsValue::Number(50.0)));
    }

    #[test]
    fn test_jscript_session_property_access_and_mutation() {
        let mut session = ScriptSession::default();
        session.set_property("PRODUCTNAME", "TestProduct");

        let mut engine = JScriptEngine::new();
        let script = r#"
            var current = Session.Property("PRODUCTNAME");
            Session.Property("PRODUCTNAME") = current + " v2";
            MY_FLAG = "ENABLED";
        "#;
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok());
        assert_eq!(session.property("PRODUCTNAME"), "TestProduct v2");
        assert_eq!(session.property("MY_FLAG"), "ENABLED");
    }

    #[test]
    fn test_jscript_control_structures_and_functions() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        let script = "
            function factorial(n) {
                if (n <= 1) {
                    return 1;
                }
                return n * factorial(n - 1);
            }

            var sum = 0;
            for (var i = 1; i <= 4; i = i + 1) {
                sum += factorial(i);
            }
            sum;
        ";
        let res = engine.execute(script, &mut session);
        assert_eq!(res, Ok(JsValue::Number(1.0 + 2.0 + 6.0 + 24.0)));
    }

    #[test]
    fn test_jscript_session_automation_methods() {
        let mut session = ScriptSession::default();
        session.set_property("VersionNT", "600");
        let mut engine = JScriptEngine::new();

        let script = r#"
            var isWin = Session.EvaluateCondition("VersionNT >= 500");
            Session.Message(1, "Condition is: " + isWin);
            Session.DoAction("DoSomething");
            var sched = Session.Mode(16);
            var mathVal = Math.floor(4.9);
            mathVal;
        "#;
        let res = engine.execute(script, &mut session);
        assert_eq!(res, Ok(JsValue::Number(4.0)));
        assert_eq!(session.messages().len(), 1);
        assert_eq!(session.messages()[0].text, "Condition is: 1");
        assert_eq!(session.executed_actions(), &["DoSomething".to_string()]);
    }

    #[test]
    fn test_jscript_syntax_error_reporting() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        let script = "var x = ;";
        let res = engine.execute(script, &mut session);
        assert!(matches!(
            res,
            Err(Error::ScriptRuntimeError { line: 1, .. })
        ));
    }

    #[test]
    fn test_jscript_math_and_builtins() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        let script = r#"
            // Single line comment
            /* Multi-line
               comment */
            var parsed = parseInt("42");
            var floatVal = parseFloat("3.14");
            var strVal = String(100);
            var isNotNum = isNaN("abc");
            var c = Math.ceil(4.1);
            var r = Math.round(4.6);
            var a = Math.abs(-10);
            var mx = Math.max(5, 10);
            var mn = Math.min(5, 10);
            c + r + a + mx + mn;
        "#;
        let res = engine.execute(script, &mut session);
        assert_eq!(res, Ok(JsValue::Number(5.0 + 5.0 + 10.0 + 10.0 + 5.0)));
    }

    #[test]
    fn test_jscript_while_and_unary_and_ternary() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        let script = r#"
            var i = 0;
            var acc = 0;
            while (i < 5) {
                acc += i;
                i++;
            }
            var cond = (acc > 5) ? "HIGH" : "LOW";
            var negated = !false;
            Session.Property("LEVEL") = cond;
            acc;
        "#;
        let res = engine.execute(script, &mut session);
        assert_eq!(res, Ok(JsValue::Number(10.0)));
        assert_eq!(session.property("LEVEL"), "HIGH");
    }

    #[test]
    fn test_jscript_database_inspection() {
        let mut session = ScriptSession::default();
        let mut row = HashMap::new();
        row.insert("Property".to_string(), "TestProp".to_string());
        session.database_mut().insert_row("Property", row);

        let mut engine = JScriptEngine::new();
        let script = r#"
            var exists = Session.Database.TableExists("Property");
            var count = Session.Database.RowCount("Property");
            count;
        "#;
        let res = engine.execute(script, &mut session);
        assert_eq!(res, Ok(JsValue::Number(1.0)));
    }

    /// Tests `JsValue` truthiness, string formatting, and numeric conversions across all variants.
    #[test]
    fn test_jscript_jsvalue_methods_and_derives() {
        // is_truthy
        assert!(!JsValue::Undefined.is_truthy());
        assert!(!JsValue::Null.is_truthy());
        assert!(!JsValue::Boolean(false).is_truthy());
        assert!(JsValue::Boolean(true).is_truthy());
        assert!(!JsValue::Number(0.0).is_truthy());
        assert!(!JsValue::Number(f64::NAN).is_truthy());
        assert!(JsValue::Number(1.0).is_truthy());
        assert!(!JsValue::String(String::new()).is_truthy());
        assert!(JsValue::String("hello".to_string()).is_truthy());
        assert!(JsValue::Object(HashMap::new()).is_truthy());
        assert!(JsValue::SessionObject.is_truthy());
        assert!(JsValue::DatabaseObject.is_truthy());
        assert!(JsValue::MathObject.is_truthy());

        // to_string_value
        assert_eq!(JsValue::Undefined.to_string_value(), "undefined");
        assert_eq!(JsValue::Null.to_string_value(), "null");
        assert_eq!(JsValue::Boolean(true).to_string_value(), "true");
        assert_eq!(JsValue::Boolean(false).to_string_value(), "false");
        assert_eq!(JsValue::Number(42.0).to_string_value(), "42");
        assert_eq!(JsValue::Number(42.5).to_string_value(), "42.5");
        assert_eq!(JsValue::Number(f64::NAN).to_string_value(), "NaN");
        assert_eq!(JsValue::Number(f64::INFINITY).to_string_value(), "inf");
        assert_eq!(JsValue::String("val".to_string()).to_string_value(), "val");
        assert_eq!(
            JsValue::Object(HashMap::new()).to_string_value(),
            "[object Object]"
        );
        assert_eq!(JsValue::SessionObject.to_string_value(), "[object Session]");
        assert_eq!(
            JsValue::DatabaseObject.to_string_value(),
            "[object Database]"
        );
        assert_eq!(JsValue::MathObject.to_string_value(), "[object Math]");

        // to_number
        assert!(JsValue::Undefined.to_number().is_nan());
        assert_eq!(JsValue::Null.to_number(), 0.0);
        assert_eq!(JsValue::Boolean(true).to_number(), 1.0);
        assert_eq!(JsValue::Boolean(false).to_number(), 0.0);
        assert_eq!(JsValue::Number(12.5).to_number(), 12.5);
        assert_eq!(JsValue::String(" 123 ".to_string()).to_number(), 123.0);
        assert!(JsValue::String("abc".to_string()).to_number().is_nan());
        assert!(JsValue::Object(HashMap::new()).to_number().is_nan());
        assert!(JsValue::SessionObject.to_number().is_nan());
        assert!(JsValue::DatabaseObject.to_number().is_nan());
        assert!(JsValue::MathObject.to_number().is_nan());
    }

    /// Tests lexer string escapes, errors, operators, and keywords.
    #[test]
    fn test_jscript_lexer_tokens_and_errors() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        // Valid escape sequences: \n, \t, \r, \", \', \\, and other (\x)
        let script = r#"
            var s = "a\nb\tc\rd\"e\'f\\g\xh";
            s;
        "#;
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok_and(|v| v.to_string_value() == "a\nb\tc\rd\"e'f\\gxh"));

        // Single quote string with escapes
        assert!(engine
            .execute("var s = 'single quote'; s;", &mut session)
            .is_ok());

        // Lexer errors
        assert!(engine
            .execute("var s = \"unterminated", &mut session)
            .is_err());
        assert!(engine
            .execute("var s = \"unterminated escape \\", &mut session)
            .is_err());
        assert!(engine.execute("var n = 12.34.56;", &mut session).is_err());
        assert!(engine.execute("var a = 1 & 2;", &mut session).is_err());
        assert!(engine.execute("var a = 1 | 2;", &mut session).is_err());
        assert!(engine.execute("var a = @;", &mut session).is_err());

        // Operators: ===, !==, ==, !=, <=, >=, %, -=, +=
        let op_script = "
            let x = 10;
            x += 5;
            x -= 3;
            var isStrictEq = (x === 12);
            var isStrictNeq = (x !== 13);
            var isLooseEq = (x == 12);
            var isLooseNeq = (x != 13);
            var modVal = x % 5;
            var isLe = (x <= 12);
            var isGe = (x >= 12);
            var isLt = (x < 20);
            var isGt = (x > 5);
            var n = null;
            x;
        ";
        assert_eq!(
            engine.execute(op_script, &mut session),
            Ok(JsValue::Number(12.0))
        );
    }

    /// Tests parser error branches and unexpected token diagnostics.
    #[test]
    fn test_jscript_parser_errors() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        assert!(engine.execute("var 123 = 4;", &mut session).is_err());
        assert!(engine.execute("if x > 0 { }", &mut session).is_err());
        assert!(engine.execute("if (x > 0 { }", &mut session).is_err());
        assert!(engine.execute("while x > 0 { }", &mut session).is_err());
        assert!(engine.execute("while (x > 0 { }", &mut session).is_err());
        assert!(engine.execute("for x; y; z) { }", &mut session).is_err());
        assert!(engine
            .execute("for (var i = 0 i < 10) { }", &mut session)
            .is_err());
        assert!(engine
            .execute("for (var i = 0; i < 10; i++ { }", &mut session)
            .is_err());
        assert!(engine.execute("function 123() { }", &mut session).is_err());
        assert!(engine.execute("function foo { }", &mut session).is_err());
        assert!(engine
            .execute("function foo(a, b { }", &mut session)
            .is_err());
        assert!(engine
            .execute("function foo(a, b) ;", &mut session)
            .is_err());
        assert!(engine.execute("var x = obj.123;", &mut session).is_err());
        assert!(engine.execute("var x = obj[1;", &mut session).is_err());
        assert!(engine.execute("foo(1, 2;", &mut session).is_err());
        assert!(engine.execute("var x = (1 + 2;", &mut session).is_err());
        assert!(engine.execute("var x = 1 ? 2 ;", &mut session).is_err());
        assert!(engine.execute("var x = 1 + ;", &mut session).is_err());
    }

    /// Tests statements, scoping, loops, and function returns.
    #[test]
    fn test_jscript_statements_and_loops() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        // Uninitialized variable
        assert_eq!(
            engine.execute("var uninit; uninit;", &mut session),
            Ok(JsValue::Undefined)
        );

        // Block scope
        let block_script = "
            var outer = 1;
            {
                var inner = 2;
                outer = outer + inner;
            }
            outer;
        ";
        assert_eq!(
            engine.execute(block_script, &mut session),
            Ok(JsValue::Number(3.0))
        );

        // If with else and if without else
        let if_script = "
            var v = 0;
            if (false) {
                v = 1;
            } else {
                v = 2;
            }
            if (false) {
                v = 3;
            }
            v;
        ";
        assert_eq!(
            engine.execute(if_script, &mut session),
            Ok(JsValue::Number(2.0))
        );

        // For loops with omitted clauses
        let for_omitted = "
            var a = 0;
            for (; a < 3; ) {
                a++;
            }
            a;
        ";
        assert_eq!(
            engine.execute(for_omitted, &mut session),
            Ok(JsValue::Number(3.0))
        );

        // For loop with omitted condition runs until fuel exhaustion
        let mut loop_session =
            ScriptSession::with_fuel(crate::execution::properties::EvaluationContext::new(), 10);
        assert!(engine
            .execute("for (var b = 0; ; b++) {}", &mut loop_session)
            .is_err());

        // Function returns without expression and implicit return
        let fn_script = "
            function earlyRet() {
                var x = 10;
                return;
            }
            function noRet() {
                var y = 20;
            }
            var r1 = earlyRet();
            var r2 = noRet();
            (r1 === undefined) && (r2 === undefined);
        ";
        assert_eq!(
            engine.execute(fn_script, &mut session),
            Ok(JsValue::Boolean(true))
        );
    }

    /// Tests unary prefix/postfix operators, binary short-circuiting, member/index access, and assignments.
    #[test]
    fn test_jscript_expressions_and_operators() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        // Pre/post increment/decrement
        let inc_dec = "
            var x = 5;
            var pre = ++x;
            var post = x++;
            var pre_d = --x;
            var post_d = x--;
            pre + post + pre_d + post_d + x;
        ";
        assert!(engine.execute(inc_dec, &mut session).is_ok());

        // Unary minus and not
        let unary_script = "
            var n = -(-10);
            var b = !(!true);
            (n === 10) && (b === true);
        ";
        assert_eq!(
            engine.execute(unary_script, &mut session),
            Ok(JsValue::Boolean(true))
        );

        // Binary operations and string length
        let bin_script = r#"
            var s = "hello";
            var len = s.length;
            var unk = s.unknown;
            var concat1 = 10 + " apples";
            var concat2 = "apples: " + 10;
            var div = 10 / 2;
            var sub = 10 - 2;
            var mul = 10 * 2;
            (len === 5) && (unk === undefined) && (div === 5) && (sub === 8) && (mul === 20);
        "#;
        assert_eq!(
            engine.execute(bin_script, &mut session),
            Ok(JsValue::Boolean(true))
        );

        // Logical short-circuiting and ternary
        let logic_script = r#"
            var falseAnd = false && (1 / 0);
            var trueOr = true || (1 / 0);
            var ternTrue = true ? "yes" : "no";
            var ternFalse = false ? "yes" : "no";
            (ternTrue == "yes") && (ternFalse == "no");
        "#;
        assert_eq!(
            engine.execute(logic_script, &mut session),
            Ok(JsValue::Boolean(true))
        );

        // Index access and assignment on Session
        let session_index = r#"
            Session["MY_INDEX_PROP"] = "IndexVal";
            var readBack = Session["MY_INDEX_PROP"];
            readBack;
        "#;
        assert_eq!(
            engine.execute(session_index, &mut session),
            Ok(JsValue::String("IndexVal".to_string()))
        );

        // Invalid assignment targets and callee errors
        assert!(engine.execute("10 = 20;", &mut session).is_err());
        assert!(engine.execute("var f = 1; f();", &mut session).is_err());
        assert!(engine.execute("(123)(456);", &mut session).is_err());
    }

    /// Tests method calls on built-in automation objects, Math functions, and global functions.
    #[test]
    fn test_jscript_methods_and_builtins_extended() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        let script = r#"
            var p1 = parseInt("invalid");
            var p2 = parseFloat("invalid");
            var nanCheck = isNaN(10);
            var m_unk = isNaN(p1);
            m_unk;
        "#;
        assert_eq!(
            engine.execute(script, &mut session),
            Ok(JsValue::Boolean(true))
        );

        // Unknown methods on objects
        assert!(engine
            .execute("Session.UnknownMethod();", &mut session)
            .is_err());
        assert!(engine
            .execute("Session.Database.UnknownMethod();", &mut session)
            .is_err());
        assert!(engine
            .execute("Math.unknownMethod();", &mut session)
            .is_err());
        assert!(engine.execute("(123).someMethod();", &mut session).is_err());

        // Math functions
        let math_script = "
            var fl = Math.floor(1.9);
            var ce = Math.ceil(1.1);
            var rd = Math.round(1.5);
            var ab = Math.abs(-3.5);
            var mx = Math.max(1, 2);
            var mn = Math.min(1, 2);
            fl + ce + rd + ab + mx + mn;
        ";
        assert!(engine.execute(math_script, &mut session).is_ok());
    }

    /// Tests call stack overflow detection and fuel exhaustion.
    #[test]
    fn test_jscript_recursion_and_fuel() {
        use crate::execution::properties::EvaluationContext;

        // Fuel exhaustion in while loop
        let mut fuel_session = ScriptSession::with_fuel(EvaluationContext::new(), 10);
        let mut engine = JScriptEngine::new();
        let while_infinite = "while (true) { }";
        assert!(matches!(
            engine.execute(while_infinite, &mut fuel_session),
            Err(Error::ScriptRuntimeError { .. })
        ));
    }

    /// Tests remaining uncovered branches in property mirroring, assignments, and expressions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_jscript_uncovered_branches() {
        let mut session = ScriptSession::default();
        let mut engine = JScriptEngine::new();

        // 1. Advance past EOF in Parser
        let mut p = Parser::new(vec![]);
        let tok1 = p.advance();
        assert_eq!(tok1.kind, TokenKind::Eof);
        let tok2 = p.advance();
        assert_eq!(tok2.kind, TokenKind::Eof);

        // 2. Public uppercase property mirroring and reading
        session.set_property("PUB_PROP", "InitialValue");
        let script = r#"
            var readBack = PUB_PROP;
            {
                var LOCAL_PUB = "Local1";
                LOCAL_PUB = "Local2";
            }
            PUB_PROP = "ModifiedValue";
            var non_pub = "local";
            non_pub = "updated";
            IMPLICIT_UPPER = "ImpVal";
            implicit_lower = "ImpLower";
        "#;
        assert!(engine.execute(script, &mut session).is_ok());
        assert_eq!(session.property("PUB_PROP"), "ModifiedValue");
        assert_eq!(session.property("IMPLICIT_UPPER"), "ImpVal");

        // 3. Member and index access on non-objects, database/math objects, and Session
        let access_script = r#"
            Session.DIRECT_MEMBER_SET = "DirectMemberVal";
            var direct_read = Session.DIRECT_MEMBER_SET;
            var d_prop = Session.Database.SomeProperty;
            var m_prop = Math.PI;
            var n_prop = (123).foo;
            var idx_num = (123)[0];
            var s_prop = "abc"[0];
        "#;
        assert!(engine.execute(access_script, &mut session).is_ok());
        assert_eq!(session.property("DIRECT_MEMBER_SET"), "DirectMemberVal");

        // 4. Session.Property with 0 arguments and non-string argument
        assert!(engine.execute("Session.Property();", &mut session).is_ok());
        assert!(engine
            .execute("Session.Property(123);", &mut session)
            .is_ok());
        assert!(engine
            .execute("Session.Property(123, 'val');", &mut session)
            .is_ok());

        // 5. Invalid call assignments and non-session assignments
        assert!(engine
            .execute("Session.OtherMethod('arg') = 1;", &mut session)
            .is_err());
        assert!(engine
            .execute("Session.Property() = 1;", &mut session)
            .is_err());
        assert!(engine
            .execute("(42).Property('p') = 1;", &mut session)
            .is_err());
        assert!(engine
            .execute("var m = Math; m.prop = 1;", &mut session)
            .is_ok());
        assert!(engine.execute("(42).prop = 1;", &mut session).is_ok());
        assert!(engine
            .execute("var s = 'str'; s[0] = 1;", &mut session)
            .is_ok());
        assert!(engine.execute("(42)[0] = 1;", &mut session).is_ok());

        // 6. Pre/post increment/decrement on non-identifiers
        assert!(engine
            .execute("++(5); (5)++; --(5); (5)--;", &mut session)
            .is_ok());

        // 7. Function parameter that is not an identifier is skipped without error
        assert!(engine.execute("function f(123) {}", &mut session).is_ok());

        // 8. Lookup unbound variable returns None
        assert_eq!(engine.lookup_var("UNBOUND_VARIABLE_XYZ", &session), None);

        // 9. Object map member and index access
        let mut map = HashMap::new();
        map.insert("test_key".to_string(), JsValue::Number(42.0));
        let obj = JsValue::Object(map);
        let expr_member = Expr::MemberAccess {
            object: Box::new(Expr::Literal(obj.clone(), 1, 1)),
            property: "test_key".to_string(),
            line: 1,
            col: 1,
        };
        assert_eq!(
            engine.eval_expression(&expr_member, &mut session),
            Ok(JsValue::Number(42.0))
        );
        let expr_index = Expr::IndexAccess {
            object: Box::new(Expr::Literal(obj, 1, 1)),
            index: Box::new(Expr::Literal(JsValue::String("test_key".to_string()), 1, 1)),
            line: 1,
            col: 1,
        };
        assert_eq!(
            engine.eval_expression(&expr_index, &mut session),
            Ok(JsValue::Number(42.0))
        );

        // 10. Empty env_stack robustness
        let mut empty_engine = JScriptEngine {
            env_stack: Vec::new(),
        };
        empty_engine.set_var("ANY_VAR", &JsValue::Null, &mut session);
        assert!(empty_engine
            .eval_statement(
                &Stmt::VarDecl {
                    name: "DUMMY".to_string(),
                    init: None,
                    line: 1,
                    col: 1,
                },
                &mut session,
            )
            .is_ok());
        assert!(empty_engine
            .eval_statement(
                &Stmt::FunctionDecl {
                    name: "DUMMY_FN".to_string(),
                    parameters: vec![],
                    body: vec![],
                    line: 1,
                    col: 1,
                },
                &mut session,
            )
            .is_ok());
        // 11. Logical operators both truthy and falsy branches
        assert!(engine
            .execute(
                "var andT = true && 42; var orF = false || 99;",
                &mut session
            )
            .is_ok());

        // 12. Invalid LHS assignment on non-member call
        assert!(engine.execute("foo() = 1;", &mut session).is_err());

        // 13. Comments edge cases: comment at EOF, unclosed multi-line, asterisk in comment
        assert!(engine
            .execute("// trailing comment without newline", &mut session)
            .is_ok());
        assert!(engine
            .execute("/* unclosed comment at EOF", &mut session)
            .is_ok());
        assert!(engine
            .execute("/* comment with * and then * / */", &mut session)
            .is_ok());

        // 14. Number and identifier at exact EOF
        assert_eq!(
            engine.execute("42", &mut session),
            Ok(JsValue::Number(42.0))
        );
        assert_eq!(
            engine.execute("var _under$score = 10; _under$score", &mut session),
            Ok(JsValue::Number(10.0))
        );
        assert_eq!(
            engine.execute("var $dollar_var = 20; $dollar_var", &mut session),
            Ok(JsValue::Number(20.0))
        );

        // 15. Unclosed function body and unclosed block at EOF
        assert!(engine
            .execute("function unclosed() { var a = 1;", &mut session)
            .is_err());
        assert!(engine.execute("{ var b = 2;", &mut session).is_err());
    }
}
