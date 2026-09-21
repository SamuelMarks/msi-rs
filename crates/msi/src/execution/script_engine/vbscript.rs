//! `VBScript` Pure-Rust Sandboxed Interpreter.
//!
//! Implements Microsoft Windows Installer `VBScript` custom action runtime specifications:
//! - Complete case-insensitive lexer, AST parser, and runtime evaluator.
//! - Subroutines (`Sub ... End Sub`), functions (`Function ... End Function`), control flow
//!   (`If ... Then ... ElseIf ... Else ... End If`, `While ... Wend`, `For ... To ... Next`).
//! - Variable model supporting COM `Variant` types: `Empty`, `Null`, `Integer`, `String`, `Boolean`.
//! - Standard COM `Session` automation object model bound into global namespace.
//! - Full `VBScript` error handling constructs: `On Error Resume Next`, `On Error Goto 0`,
//!   and the built-in `Err` object (`Err.Number`, `Err.Description`, `Err.Clear`, `Err.Raise`).

use crate::error::{Error, Result};
use crate::execution::script_engine::session::ScriptSession;
use std::collections::HashMap;

/// COM `Variant` representation for `VBScript` data types.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Variant {
    /// Uninitialized variant (`Empty`).
    #[default]
    Empty,
    /// Explicit null value (`Null`).
    Null,
    /// 64-bit signed integer.
    Integer(i64),
    /// String text value.
    String(String),
    /// Boolean flag (`True` or `False`).
    Boolean(bool),
    /// COM Automation Object identifier (e.g. "Session", "Err").
    Object(String),
}

impl Variant {
    /// Evaluates whether the variant is truthy in a boolean condition.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Empty | Self::Null => false,
            Self::Boolean(b) => *b,
            Self::Integer(i) => *i != 0,
            Self::String(s) => !s.is_empty() && s != "0",
            Self::Object(_) => true,
        }
    }

    /// Converts the variant to an integer.
    #[must_use]
    pub fn to_integer(&self) -> i64 {
        match self {
            Self::Boolean(b) => {
                if *b {
                    -1
                } else {
                    0
                }
            }
            Self::Integer(i) => *i,
            Self::String(s) => s.trim().parse::<i64>().unwrap_or(0),
            Self::Empty | Self::Null | Self::Object(_) => 0,
        }
    }

    /// Converts the variant to a string representation.
    #[must_use]
    pub fn to_string_value(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Null => "Null".to_string(),
            Self::Boolean(b) => {
                if *b {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            }
            Self::Integer(i) => i.to_string(),
            Self::String(s) => s.clone(),
            Self::Object(name) => format!("[object {name}]"),
        }
    }
}

/// Token kinds for `VBScript`.
#[derive(Debug, Clone, PartialEq)]
enum VbTokenKind {
    /// Identifier name.
    Identifier(String),
    /// Integer literal.
    Integer(i64),
    /// String literal.
    StringLiteral(String),
    /// `Dim` keyword.
    KeywordDim,
    /// `Set` keyword.
    KeywordSet,
    /// `Sub` keyword.
    KeywordSub,
    /// `End` keyword.
    KeywordEnd,
    /// `Function` keyword.
    KeywordFunction,
    /// `Call` keyword.
    KeywordCall,
    /// `If` keyword.
    KeywordIf,
    /// `Then` keyword.
    KeywordThen,
    /// `ElseIf` keyword.
    KeywordElseIf,
    /// `Else` keyword.
    KeywordElse,
    /// `While` keyword.
    KeywordWhile,
    /// `Wend` keyword.
    KeywordWend,
    /// `Do` keyword.
    KeywordDo,
    /// `Loop` keyword.
    KeywordLoop,
    /// `Until` keyword.
    KeywordUntil,
    /// `For` keyword.
    KeywordFor,
    /// `To` keyword.
    KeywordTo,
    /// `Step` keyword.
    KeywordStep,
    /// `Next` keyword.
    KeywordNext,
    /// `Exit` keyword.
    KeywordExit,
    /// `True` keyword.
    KeywordTrue,
    /// `False` keyword.
    KeywordFalse,
    /// `Empty` keyword.
    KeywordEmpty,
    /// `Null` keyword.
    KeywordNull,
    /// `Nothing` keyword.
    KeywordNothing,
    /// `On` keyword.
    KeywordOn,
    /// `Error` keyword.
    KeywordError,
    /// `Resume` keyword.
    KeywordResume,
    /// `Goto` keyword.
    KeywordGoto,
    /// `Not` keyword.
    KeywordNot,
    /// `And` keyword.
    KeywordAnd,
    /// `Or` keyword.
    KeywordOr,
    /// `Xor` keyword.
    KeywordXor,
    /// `Mod` keyword.
    KeywordMod,
    /// `+` operator.
    Plus,
    /// `-` operator.
    Minus,
    /// `*` operator.
    Star,
    /// `/` operator.
    Slash,
    /// Integer division backslash operator.
    Backslash,
    /// `&` string concatenation operator.
    Ampersand,
    /// `=` equality / assignment operator.
    Equal,
    /// `<>` inequality operator.
    NotEqual,
    /// `<` less-than operator.
    Less,
    /// `<=` less-than-or-equal operator.
    LessEqual,
    /// `>` greater-than operator.
    Greater,
    /// `>=` greater-than-or-equal operator.
    GreaterEqual,
    /// `(` opening parenthesis.
    LeftParen,
    /// `)` closing parenthesis.
    RightParen,
    /// `,` comma separator.
    Comma,
    /// `.` dot member access operator.
    Dot,
    /// `:` colon statement separator.
    Colon,
    /// Newline statement separator.
    Newline,
    /// End-of-file sentinel.
    Eof,
}

/// Token tagged with line and column.
#[derive(Debug, Clone, PartialEq)]
struct VbToken {
    /// Token kind.
    kind: VbTokenKind,
    /// 1-based line number.
    line: usize,
    /// 1-based column number.
    col: usize,
}

/// Lexer for scanning `VBScript` source text.
struct VbLexer<'a> {
    /// Character buffer.
    chars: Vec<char>,
    /// Current position index.
    pos: usize,
    /// 1-based line number.
    line: usize,
    /// 1-based column number.
    col: usize,
    /// Phantom data lifetime marker.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> VbLexer<'a> {
    /// Creates a new [`VbLexer`].
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            _marker: std::marker::PhantomData,
        }
    }

    /// Peeks at the current character without advancing.
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Advances by one character.
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

    /// Skips inline whitespace and comments.
    fn skip_whitespace_inline(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else if ch == '_' && self.chars.get(self.pos + 1) == Some(&'\n') {
                self.advance();
                self.advance();
            } else if ch == '\'' {
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        self.pos -= 1;
                        self.col = 1;
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Reads a string literal bounded by quotes.
    fn read_string(&mut self, start_line: usize, start_col: usize) -> Result<VbToken> {
        let mut text = String::new();
        while let Some(ch) = self.advance() {
            if ch == '"' {
                if self.peek() == Some('"') {
                    self.advance();
                    text.push('"');
                } else {
                    return Ok(VbToken {
                        kind: VbTokenKind::StringLiteral(text),
                        line: start_line,
                        col: start_col,
                    });
                }
            } else {
                text.push(ch);
            }
        }
        Err(Error::ScriptRuntimeError {
            line: start_line,
            col: start_col,
            message: "Unterminated string literal in VBScript".to_string(),
        })
    }

    /// Scans the next token.
    #[allow(clippy::too_many_lines)]
    fn next_token(&mut self) -> Result<VbToken> {
        self.skip_whitespace_inline();
        let start_line = self.line;
        let start_col = self.col;

        let Some(ch) = self.peek() else {
            return Ok(VbToken {
                kind: VbTokenKind::Eof,
                line: start_line,
                col: start_col,
            });
        };

        if ch == '\n' {
            self.advance();
            return Ok(VbToken {
                kind: VbTokenKind::Newline,
                line: start_line,
                col: start_col,
            });
        }

        if ch == '"' {
            self.advance();
            return self.read_string(start_line, start_col);
        }

        if ch.is_ascii_digit() {
            let mut num_str = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    num_str.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let n = num_str
                .parse::<i64>()
                .map_err(|_| Error::ScriptRuntimeError {
                    line: start_line,
                    col: start_col,
                    message: format!("Invalid integer: {num_str}"),
                })?;
            return Ok(VbToken {
                kind: VbTokenKind::Integer(n),
                line: start_line,
                col: start_col,
            });
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut ident = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    ident.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            let lower = ident.to_ascii_lowercase();
            if lower == "rem" {
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        self.pos -= 1;
                        self.col = 1;
                        break;
                    }
                }
                return self.next_token();
            }

            let kind = match lower.as_str() {
                "dim" => VbTokenKind::KeywordDim,
                "set" => VbTokenKind::KeywordSet,
                "sub" => VbTokenKind::KeywordSub,
                "end" => VbTokenKind::KeywordEnd,
                "function" => VbTokenKind::KeywordFunction,
                "call" => VbTokenKind::KeywordCall,
                "if" => VbTokenKind::KeywordIf,
                "then" => VbTokenKind::KeywordThen,
                "elseif" => VbTokenKind::KeywordElseIf,
                "else" => VbTokenKind::KeywordElse,
                "while" => VbTokenKind::KeywordWhile,
                "wend" => VbTokenKind::KeywordWend,
                "do" => VbTokenKind::KeywordDo,
                "loop" => VbTokenKind::KeywordLoop,
                "until" => VbTokenKind::KeywordUntil,
                "for" => VbTokenKind::KeywordFor,
                "to" => VbTokenKind::KeywordTo,
                "step" => VbTokenKind::KeywordStep,
                "next" => VbTokenKind::KeywordNext,
                "exit" => VbTokenKind::KeywordExit,
                "true" => VbTokenKind::KeywordTrue,
                "false" => VbTokenKind::KeywordFalse,
                "empty" => VbTokenKind::KeywordEmpty,
                "null" => VbTokenKind::KeywordNull,
                "nothing" => VbTokenKind::KeywordNothing,
                "on" => VbTokenKind::KeywordOn,
                "error" => VbTokenKind::KeywordError,
                "resume" => VbTokenKind::KeywordResume,
                "goto" => VbTokenKind::KeywordGoto,
                "not" => VbTokenKind::KeywordNot,
                "and" => VbTokenKind::KeywordAnd,
                "or" => VbTokenKind::KeywordOr,
                "xor" => VbTokenKind::KeywordXor,
                "mod" => VbTokenKind::KeywordMod,
                _ => VbTokenKind::Identifier(ident),
            };
            return Ok(VbToken {
                kind,
                line: start_line,
                col: start_col,
            });
        }

        self.advance();
        let kind = match ch {
            '+' => VbTokenKind::Plus,
            '-' => VbTokenKind::Minus,
            '*' => VbTokenKind::Star,
            '/' => VbTokenKind::Slash,
            '\\' => VbTokenKind::Backslash,
            '&' => VbTokenKind::Ampersand,
            '=' => VbTokenKind::Equal,
            '<' => {
                if self.peek() == Some('>') {
                    self.advance();
                    VbTokenKind::NotEqual
                } else if self.peek() == Some('=') {
                    self.advance();
                    VbTokenKind::LessEqual
                } else {
                    VbTokenKind::Less
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    VbTokenKind::GreaterEqual
                } else {
                    VbTokenKind::Greater
                }
            }
            '(' => VbTokenKind::LeftParen,
            ')' => VbTokenKind::RightParen,
            ',' => VbTokenKind::Comma,
            '.' => VbTokenKind::Dot,
            ':' => VbTokenKind::Colon,
            other => {
                return Err(Error::ScriptRuntimeError {
                    line: start_line,
                    col: start_col,
                    message: format!("Unexpected character in VBScript: '{other}'"),
                });
            }
        };

        Ok(VbToken {
            kind,
            line: start_line,
            col: start_col,
        })
    }

    /// Tokenizes the entire input.
    fn tokenize_all(&mut self) -> Result<Vec<VbToken>> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == VbTokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}

/// Binary operator for `VBScript`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VbBinaryOp {
    /// Add `+`.
    Add,
    /// Subtract `-`.
    Subtract,
    /// Multiply `*`.
    Multiply,
    /// Divide `/`.
    Divide,
    /// Modulo `Mod`.
    Mod,
    /// Concatenation `&`.
    Concat,
    /// Equal `=`.
    Equal,
    /// Not equal `<>`.
    NotEqual,
    /// Less `<`.
    Less,
    /// Less or equal `<=`.
    LessEqual,
    /// Greater `>`.
    Greater,
    /// Greater or equal `>=`.
    GreaterEqual,
    /// Logical `And`.
    And,
    /// Logical `Or`.
    Or,
    /// Logical `Xor`.
    Xor,
}

/// Unary operator for `VBScript`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VbUnaryOp {
    /// Negation `-`.
    Negate,
    /// Logical `Not`.
    Not,
}

/// Expression node for `VBScript`.
#[derive(Debug, Clone)]
enum VbExpr {
    /// Literal variant.
    Literal(Variant, usize, usize),
    /// Identifier name.
    Identifier(String, usize, usize),
    /// Member access `object.member`.
    MemberAccess {
        /// Target object expression.
        object: Box<Self>,
        /// Member name.
        member: String,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// Function or method invocation.
    Call {
        /// Callee expression.
        callee: Box<Self>,
        /// Arguments.
        arguments: Vec<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// Unary operation.
    Unary {
        /// Operator.
        op: VbUnaryOp,
        /// Operand expression.
        expr: Box<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// Binary operation.
    Binary {
        /// Operator.
        op: VbBinaryOp,
        /// Left operand.
        left: Box<Self>,
        /// Right operand.
        right: Box<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
}

/// Statement node for `VBScript`.
#[derive(Debug, Clone)]
enum VbStmt {
    /// Variable declaration `Dim x, y`.
    Dim(Vec<String>, usize, usize),
    /// Assignment `target = value`.
    Assign {
        /// Target expression.
        target: VbExpr,
        /// Value expression.
        value: VbExpr,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// Invocation statement `Call expr`.
    Call(VbExpr),
    /// If conditional statement.
    If {
        /// Condition expression.
        condition: VbExpr,
        /// Then branch statements.
        then_branch: Vec<Self>,
        /// `ElseIf` branches list.
        elseif_branches: Vec<(VbExpr, Vec<Self>)>,
        /// Optional else branch statements.
        else_branch: Option<Vec<Self>>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// While-Wend loop.
    While {
        /// Loop condition.
        condition: VbExpr,
        /// Loop body.
        body: Vec<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// For-Next loop.
    For {
        /// Variable name.
        var_name: String,
        /// Start expression.
        start: VbExpr,
        /// End expression.
        end: VbExpr,
        /// Optional step expression.
        step: Option<VbExpr>,
        /// Loop body.
        body: Vec<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// Subroutine declaration `Sub Name(...)`.
    SubDecl {
        /// Subroutine name.
        name: String,
        /// Parameter names.
        parameters: Vec<String>,
        /// Body statements.
        body: Vec<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// Function declaration `Function Name(...)`.
    FunctionDecl {
        /// Function name.
        name: String,
        /// Parameter names.
        parameters: Vec<String>,
        /// Body statements.
        body: Vec<Self>,
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// `On Error Resume Next`.
    OnErrorResumeNext {
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// `On Error Goto 0`.
    OnErrorGotoZero {
        /// Line.
        line: usize,
        /// Column.
        col: usize,
    },
    /// `Exit Sub`.
    ExitSub,
    /// `Exit Function`.
    ExitFunction,
}

/// Parser for `VBScript`.
struct VbParser {
    /// Token stream.
    tokens: Vec<VbToken>,
    /// Current position.
    pos: usize,
}

impl VbParser {
    /// Creates a new [`VbParser`].
    const fn new(tokens: Vec<VbToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Peeks at current token.
    fn peek(&self) -> &VbToken {
        self.tokens.get(self.pos).unwrap_or(&VbToken {
            kind: VbTokenKind::Eof,
            line: 0,
            col: 0,
        })
    }

    /// Advances and returns the current token.
    fn advance(&mut self) -> VbToken {
        let tok = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    /// Checks and consumes token kind if matching.
    fn match_token(&mut self, kind: &VbTokenKind) -> bool {
        if &self.peek().kind == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Skips newline and colon statement separators.
    fn skip_newlines(&mut self) {
        while self.peek().kind == VbTokenKind::Newline || self.peek().kind == VbTokenKind::Colon {
            self.advance();
        }
    }

    /// Expects specific token kind or returns parse error.
    fn expect(&mut self, kind: &VbTokenKind, msg: &str) -> Result<VbToken> {
        let tok = self.peek().clone();
        if &tok.kind == kind {
            Ok(self.advance())
        } else {
            Err(Error::ScriptRuntimeError {
                line: tok.line,
                col: tok.col,
                message: format!("VBScript parse error: expected {msg}, found {:?}", tok.kind),
            })
        }
    }

    /// Parses the entire program into statements.
    fn parse_program(&mut self) -> Result<Vec<VbStmt>> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while self.peek().kind != VbTokenKind::Eof {
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    /// Parses a single statement.
    #[allow(clippy::too_many_lines)]
    fn parse_statement(&mut self) -> Result<VbStmt> {
        self.skip_newlines();
        let tok = self.peek().clone();
        match tok.kind {
            VbTokenKind::KeywordDim => {
                self.advance();
                let mut vars = Vec::new();
                loop {
                    let next_tok = self.advance();
                    if let VbTokenKind::Identifier(id) = next_tok.kind {
                        vars.push(id);
                    } else {
                        return Err(Error::ScriptRuntimeError {
                            line: next_tok.line,
                            col: next_tok.col,
                            message: "Expected variable name after Dim".to_string(),
                        });
                    }
                    if !self.match_token(&VbTokenKind::Comma) {
                        break;
                    }
                }
                Ok(VbStmt::Dim(vars, tok.line, tok.col))
            }
            VbTokenKind::KeywordOn => {
                self.advance();
                self.expect(&VbTokenKind::KeywordError, "'Error' after 'On'")?;
                if self.match_token(&VbTokenKind::KeywordResume) {
                    if self.match_token(&VbTokenKind::KeywordNext) {
                        return Ok(VbStmt::OnErrorResumeNext {
                            line: tok.line,
                            col: tok.col,
                        });
                    }
                } else if self.match_token(&VbTokenKind::KeywordGoto) {
                    let zero_tok = self.advance();
                    if zero_tok.kind == VbTokenKind::Integer(0) {
                        return Ok(VbStmt::OnErrorGotoZero {
                            line: tok.line,
                            col: tok.col,
                        });
                    }
                }
                Err(Error::ScriptRuntimeError {
                    line: tok.line,
                    col: tok.col,
                    message: "Unsupported 'On Error' directive".to_string(),
                })
            }
            VbTokenKind::KeywordSub => {
                self.advance();
                let next_tok = self.advance();
                let name = match next_tok.kind {
                    VbTokenKind::Identifier(s) => s,
                    other => {
                        return Err(Error::ScriptRuntimeError {
                            line: next_tok.line,
                            col: next_tok.col,
                            message: format!("Expected subroutine name, found {other:?}"),
                        });
                    }
                };
                let parameters = self.parse_parameter_list()?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.is_end_sub() && self.peek().kind != VbTokenKind::Eof {
                    body.push(self.parse_statement()?);
                    self.skip_newlines();
                }
                self.expect_end_sub()?;
                Ok(VbStmt::SubDecl {
                    name,
                    parameters,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            VbTokenKind::KeywordFunction => {
                self.advance();
                let next_tok = self.advance();
                let name = match next_tok.kind {
                    VbTokenKind::Identifier(s) => s,
                    other => {
                        return Err(Error::ScriptRuntimeError {
                            line: next_tok.line,
                            col: next_tok.col,
                            message: format!("Expected function name, found {other:?}"),
                        });
                    }
                };
                let parameters = self.parse_parameter_list()?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.is_end_function() && self.peek().kind != VbTokenKind::Eof {
                    body.push(self.parse_statement()?);
                    self.skip_newlines();
                }
                self.expect_end_function()?;
                Ok(VbStmt::FunctionDecl {
                    name,
                    parameters,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            VbTokenKind::KeywordIf => {
                self.advance();
                let condition = self.parse_expression()?;
                self.expect(&VbTokenKind::KeywordThen, "'Then' after 'If' condition")?;
                self.skip_newlines();

                let mut then_branch = Vec::new();
                let mut elseif_branches = Vec::new();
                let mut else_branch = None;

                while !self.is_if_boundary() && self.peek().kind != VbTokenKind::Eof {
                    then_branch.push(self.parse_statement()?);
                    self.skip_newlines();
                }

                while self.match_token(&VbTokenKind::KeywordElseIf) {
                    let elif_cond = self.parse_expression()?;
                    self.expect(&VbTokenKind::KeywordThen, "'Then' after 'ElseIf'")?;
                    self.skip_newlines();
                    let mut elif_body = Vec::new();
                    while !self.is_if_boundary() && self.peek().kind != VbTokenKind::Eof {
                        elif_body.push(self.parse_statement()?);
                        self.skip_newlines();
                    }
                    elseif_branches.push((elif_cond, elif_body));
                }

                if self.match_token(&VbTokenKind::KeywordElse) {
                    self.skip_newlines();
                    let mut else_body = Vec::new();
                    while !self.is_end_if() && self.peek().kind != VbTokenKind::Eof {
                        else_body.push(self.parse_statement()?);
                        self.skip_newlines();
                    }
                    else_branch = Some(else_body);
                }

                self.expect_end_if()?;
                Ok(VbStmt::If {
                    condition,
                    then_branch,
                    elseif_branches,
                    else_branch,
                    line: tok.line,
                    col: tok.col,
                })
            }
            VbTokenKind::KeywordWhile => {
                self.advance();
                let condition = self.parse_expression()?;
                self.skip_newlines();
                let mut body = Vec::new();
                while self.peek().kind != VbTokenKind::KeywordWend
                    && self.peek().kind != VbTokenKind::Eof
                {
                    body.push(self.parse_statement()?);
                    self.skip_newlines();
                }
                self.expect(&VbTokenKind::KeywordWend, "'Wend' ending 'While' loop")?;
                Ok(VbStmt::While {
                    condition,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            VbTokenKind::KeywordFor => {
                self.advance();
                let next_tok = self.advance();
                let var_name = match next_tok.kind {
                    VbTokenKind::Identifier(s) => s,
                    other => {
                        return Err(Error::ScriptRuntimeError {
                            line: next_tok.line,
                            col: next_tok.col,
                            message: format!("Expected for loop variable, found {other:?}"),
                        });
                    }
                };
                self.expect(&VbTokenKind::Equal, "'=' in For loop")?;
                let start = self.parse_expression()?;
                self.expect(&VbTokenKind::KeywordTo, "'To' in For loop")?;
                let end = self.parse_expression()?;
                let step = if self.match_token(&VbTokenKind::KeywordStep) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.skip_newlines();
                let mut body = Vec::new();
                while self.peek().kind != VbTokenKind::KeywordNext
                    && self.peek().kind != VbTokenKind::Eof
                {
                    body.push(self.parse_statement()?);
                    self.skip_newlines();
                }
                self.expect(&VbTokenKind::KeywordNext, "'Next' ending 'For' loop")?;
                if let VbTokenKind::Identifier(_) = self.peek().kind {
                    self.advance();
                }
                Ok(VbStmt::For {
                    var_name,
                    start,
                    end,
                    step,
                    body,
                    line: tok.line,
                    col: tok.col,
                })
            }
            VbTokenKind::KeywordExit => {
                self.advance();
                if self.match_token(&VbTokenKind::KeywordSub) {
                    Ok(VbStmt::ExitSub)
                } else if self.match_token(&VbTokenKind::KeywordFunction) {
                    Ok(VbStmt::ExitFunction)
                } else {
                    Err(Error::ScriptRuntimeError {
                        line: tok.line,
                        col: tok.col,
                        message: "Unsupported 'Exit' statement in VBScript".to_string(),
                    })
                }
            }
            VbTokenKind::KeywordCall => {
                self.advance();
                let expr = self.parse_expression()?;
                Ok(VbStmt::Call(expr))
            }
            VbTokenKind::KeywordSet => {
                self.advance();
                let target = self.parse_postfix()?;
                self.expect(&VbTokenKind::Equal, "'=' in Set statement")?;
                let value = self.parse_expression()?;
                Ok(VbStmt::Assign {
                    target,
                    value,
                    line: tok.line,
                    col: tok.col,
                })
            }
            _ => {
                let expr = self.parse_postfix()?;
                if self.match_token(&VbTokenKind::Equal) {
                    let value = self.parse_expression()?;
                    Ok(VbStmt::Assign {
                        target: expr,
                        value,
                        line: tok.line,
                        col: tok.col,
                    })
                } else {
                    let mut args = Vec::new();
                    if self.peek().kind != VbTokenKind::Newline
                        && self.peek().kind != VbTokenKind::Colon
                        && self.peek().kind != VbTokenKind::Eof
                    {
                        loop {
                            args.push(self.parse_expression()?);
                            if !self.match_token(&VbTokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    if args.is_empty() {
                        Ok(VbStmt::Call(expr))
                    } else {
                        Ok(VbStmt::Call(VbExpr::Call {
                            callee: Box::new(expr),
                            arguments: args,
                            line: tok.line,
                            col: tok.col,
                        }))
                    }
                }
            }
        }
    }

    /// Parses parameter list for Sub or Function.
    fn parse_parameter_list(&mut self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        if self.match_token(&VbTokenKind::LeftParen) {
            if self.peek().kind != VbTokenKind::RightParen {
                loop {
                    let next_tok = self.advance();
                    if let VbTokenKind::Identifier(id) = next_tok.kind {
                        params.push(id);
                    }
                    if !self.match_token(&VbTokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(&VbTokenKind::RightParen, "')' closing parameter list")?;
        }
        Ok(params)
    }

    /// Checks if current token sequence is `End Sub`.
    fn is_end_sub(&self) -> bool {
        self.peek().kind == VbTokenKind::KeywordEnd
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|next| next.kind == VbTokenKind::KeywordSub)
    }

    /// Consumes `End Sub`.
    fn expect_end_sub(&mut self) -> Result<()> {
        self.expect(&VbTokenKind::KeywordEnd, "'End'")?;
        self.expect(&VbTokenKind::KeywordSub, "'Sub'")?;
        Ok(())
    }

    /// Checks if current token sequence is `End Function`.
    fn is_end_function(&self) -> bool {
        self.peek().kind == VbTokenKind::KeywordEnd
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|next| next.kind == VbTokenKind::KeywordFunction)
    }

    /// Consumes `End Function`.
    fn expect_end_function(&mut self) -> Result<()> {
        self.expect(&VbTokenKind::KeywordEnd, "'End'")?;
        self.expect(&VbTokenKind::KeywordFunction, "'Function'")?;
        Ok(())
    }

    /// Checks whether an If block boundary is reached.
    fn is_if_boundary(&self) -> bool {
        match self.peek().kind {
            VbTokenKind::KeywordElseIf | VbTokenKind::KeywordElse => true,
            _ => self.is_end_if(),
        }
    }

    /// Checks whether current token sequence is `End If`.
    fn is_end_if(&self) -> bool {
        self.peek().kind == VbTokenKind::KeywordEnd
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|next| next.kind == VbTokenKind::KeywordIf)
    }

    /// Consumes `End If`.
    fn expect_end_if(&mut self) -> Result<()> {
        self.expect(&VbTokenKind::KeywordEnd, "'End'")?;
        self.expect(&VbTokenKind::KeywordIf, "'If'")?;
        Ok(())
    }

    /// Parses expression.
    fn parse_expression(&mut self) -> Result<VbExpr> {
        self.parse_logical_or()
    }

    /// Parses logical OR and XOR operators.
    fn parse_logical_or(&mut self) -> Result<VbExpr> {
        let mut left = self.parse_logical_and()?;
        loop {
            let op = if self.match_token(&VbTokenKind::KeywordOr) {
                VbBinaryOp::Or
            } else if self.match_token(&VbTokenKind::KeywordXor) {
                VbBinaryOp::Xor
            } else {
                break;
            };
            let right = self.parse_logical_and()?;
            left = VbExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: self.peek().line,
                col: self.peek().col,
            };
        }
        Ok(left)
    }

    /// Parses logical AND operator.
    fn parse_logical_and(&mut self) -> Result<VbExpr> {
        let mut left = self.parse_equality()?;
        while self.match_token(&VbTokenKind::KeywordAnd) {
            let right = self.parse_equality()?;
            left = VbExpr::Binary {
                op: VbBinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
                line: self.peek().line,
                col: self.peek().col,
            };
        }
        Ok(left)
    }

    /// Parses relational equality and comparison operators.
    fn parse_equality(&mut self) -> Result<VbExpr> {
        let mut left = self.parse_addition()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                VbTokenKind::Equal => VbBinaryOp::Equal,
                VbTokenKind::NotEqual => VbBinaryOp::NotEqual,
                VbTokenKind::Less => VbBinaryOp::Less,
                VbTokenKind::LessEqual => VbBinaryOp::LessEqual,
                VbTokenKind::Greater => VbBinaryOp::Greater,
                VbTokenKind::GreaterEqual => VbBinaryOp::GreaterEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = VbExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses addition, subtraction, string concatenation `+`, `-`, `&`.
    fn parse_addition(&mut self) -> Result<VbExpr> {
        let mut left = self.parse_multiplication()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                VbTokenKind::Plus => VbBinaryOp::Add,
                VbTokenKind::Minus => VbBinaryOp::Subtract,
                VbTokenKind::Ampersand => VbBinaryOp::Concat,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = VbExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses multiplication, division, modulo `*`, `/`, `Mod`.
    fn parse_multiplication(&mut self) -> Result<VbExpr> {
        let mut left = self.parse_unary()?;
        loop {
            let tok = self.peek().clone();
            let op = match tok.kind {
                VbTokenKind::Star => VbBinaryOp::Multiply,
                VbTokenKind::Slash | VbTokenKind::Backslash => VbBinaryOp::Divide,
                VbTokenKind::KeywordMod => VbBinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = VbExpr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                line: tok.line,
                col: tok.col,
            };
        }
        Ok(left)
    }

    /// Parses unary prefix operators `-`, `Not`.
    fn parse_unary(&mut self) -> Result<VbExpr> {
        let tok = self.peek().clone();
        match tok.kind {
            VbTokenKind::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(VbExpr::Unary {
                    op: VbUnaryOp::Negate,
                    expr: Box::new(expr),
                    line: tok.line,
                    col: tok.col,
                })
            }
            VbTokenKind::KeywordNot => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(VbExpr::Unary {
                    op: VbUnaryOp::Not,
                    expr: Box::new(expr),
                    line: tok.line,
                    col: tok.col,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// Parses postfix operations (calls and member access).
    fn parse_postfix(&mut self) -> Result<VbExpr> {
        let mut expr = self.parse_primary()?;
        loop {
            let tok = self.peek().clone();
            match tok.kind {
                VbTokenKind::Dot => {
                    self.advance();
                    let next_tok = self.advance();
                    let member = match next_tok.kind {
                        VbTokenKind::Identifier(s) => s,
                        other => {
                            return Err(Error::ScriptRuntimeError {
                                line: next_tok.line,
                                col: next_tok.col,
                                message: format!("Expected member name after '.', found {other:?}"),
                            });
                        }
                    };
                    expr = VbExpr::MemberAccess {
                        object: Box::new(expr),
                        member,
                        line: tok.line,
                        col: tok.col,
                    };
                }
                VbTokenKind::LeftParen => {
                    self.advance();
                    let mut arguments = Vec::new();
                    if self.peek().kind != VbTokenKind::RightParen {
                        loop {
                            arguments.push(self.parse_expression()?);
                            if !self.match_token(&VbTokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&VbTokenKind::RightParen, "')' closing argument list")?;
                    expr = VbExpr::Call {
                        callee: Box::new(expr),
                        arguments,
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
    fn parse_primary(&mut self) -> Result<VbExpr> {
        let tok = self.advance();
        match tok.kind {
            VbTokenKind::Integer(n) => Ok(VbExpr::Literal(Variant::Integer(n), tok.line, tok.col)),
            VbTokenKind::StringLiteral(s) => {
                Ok(VbExpr::Literal(Variant::String(s), tok.line, tok.col))
            }
            VbTokenKind::KeywordTrue => {
                Ok(VbExpr::Literal(Variant::Boolean(true), tok.line, tok.col))
            }
            VbTokenKind::KeywordFalse => {
                Ok(VbExpr::Literal(Variant::Boolean(false), tok.line, tok.col))
            }
            VbTokenKind::KeywordEmpty | VbTokenKind::KeywordNothing => {
                Ok(VbExpr::Literal(Variant::Empty, tok.line, tok.col))
            }
            VbTokenKind::KeywordNull => Ok(VbExpr::Literal(Variant::Null, tok.line, tok.col)),
            VbTokenKind::Identifier(id) => Ok(VbExpr::Identifier(id, tok.line, tok.col)),
            VbTokenKind::LeftParen => {
                let expr = self.parse_expression()?;
                self.expect(&VbTokenKind::RightParen, "')' closing parenthesis")?;
                Ok(expr)
            }
            _ => Err(Error::ScriptRuntimeError {
                line: tok.line,
                col: tok.col,
                message: format!("Unexpected token in VBScript expression: {:?}", tok.kind),
            }),
        }
    }
}

/// Active error object in `VBScript` scope (`Err`).
#[derive(Debug, Clone, Default)]
pub struct VbErr {
    /// Numeric error code (`Err.Number`).
    pub number: i64,
    /// Error description (`Err.Description`).
    pub description: String,
}

impl VbErr {
    /// Resets the error status (`Err.Clear`).
    pub fn clear(&mut self) {
        self.number = 0;
        self.description.clear();
    }
}

/// Function or Subroutine in `VBScript`.
#[derive(Debug, Clone)]
struct VbRoutine {
    /// Parameter names.
    parameters: Vec<String>,
    /// Body statements.
    body: Vec<VbStmt>,
    /// Whether this routine returns a value (Function vs Sub).
    is_function: bool,
}

/// The `VBScript` execution engine.
#[derive(Debug, Default)]
pub struct VBScriptEngine {
    /// Variables map.
    variables: HashMap<String, Variant>,
    /// Routines map.
    routines: HashMap<String, VbRoutine>,
    /// Active Err object.
    err: VbErr,
    /// Whether On Error Resume Next is active.
    resume_next: bool,
    /// Whether an early exit from the active routine was requested.
    exit_routine: bool,
}

impl VBScriptEngine {
    /// Creates a new [`VBScriptEngine`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluates `VBScript` source code within the given [`ScriptSession`].
    ///
    /// # Arguments
    ///
    /// * `script` - The source text.
    /// * `session` - Active installer session automation object.
    ///
    /// # Returns
    ///
    /// Final evaluated [`Variant`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptRuntimeError`] on syntax or execution failure.
    pub fn execute(&mut self, script: &str, session: &mut ScriptSession) -> Result<Variant> {
        let mut lexer = VbLexer::new(script);
        let tokens = lexer.tokenize_all()?;
        let mut parser = VbParser::new(tokens);
        let statements = parser.parse_program()?;

        let mut last_val = Variant::Empty;
        for stmt in &statements {
            if let Some(val) = self.eval_statement(stmt, session)? {
                last_val = val;
            }
        }
        Ok(last_val)
    }

    /// Retrieves variable value from local scope or session properties.
    fn get_var(&self, name: &str, session: &ScriptSession) -> Variant {
        let lower = name.to_ascii_lowercase();
        if lower == "session" {
            return Variant::Object("Session".to_string());
        }
        if lower == "err" {
            return Variant::Object("Err".to_string());
        }
        if let Some(v) = self.variables.get(&lower) {
            return v.clone();
        }
        let prop_val = session.property(name);
        if !prop_val.is_empty() {
            return Variant::String(prop_val);
        }
        Variant::Empty
    }

    /// Assigns variable value in local scope and session property if public uppercase identifier.
    fn set_var(&mut self, name: &str, val: &Variant, session: &mut ScriptSession) {
        let lower = name.to_ascii_lowercase();
        self.variables.insert(lower, val.clone());
        if name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            session.set_property(name, &val.to_string_value());
        }
    }

    /// Evaluates a single statement.
    #[allow(clippy::too_many_lines)]
    fn eval_statement(
        &mut self,
        stmt: &VbStmt,
        session: &mut ScriptSession,
    ) -> Result<Option<Variant>> {
        match stmt {
            VbStmt::Dim(vars, line, col) => {
                session.consume_fuel(*line, *col)?;
                for v in vars {
                    self.set_var(v, &Variant::Empty, session);
                }
                Ok(None)
            }
            VbStmt::OnErrorResumeNext { line, col } => {
                session.consume_fuel(*line, *col)?;
                self.resume_next = true;
                Ok(None)
            }
            VbStmt::OnErrorGotoZero { line, col } => {
                session.consume_fuel(*line, *col)?;
                self.resume_next = false;
                Ok(None)
            }
            VbStmt::Assign {
                target,
                value,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let val = self.eval_expression(value, session)?;
                match target {
                    VbExpr::Identifier(id, _, _) => {
                        self.set_var(id, &val, session);
                        Ok(Some(val))
                    }
                    VbExpr::Call {
                        callee, arguments, ..
                    } => {
                        if let VbExpr::MemberAccess { object, member, .. } = &**callee {
                            if let VbExpr::Identifier(obj_name, _, _) = &**object {
                                if obj_name.eq_ignore_ascii_case("session")
                                    && member.eq_ignore_ascii_case("property")
                                    && !arguments.is_empty()
                                {
                                    let prop_name = self
                                        .eval_expression(&arguments[0], session)?
                                        .to_string_value();
                                    session.set_property(&prop_name, &val.to_string_value());
                                    return Ok(Some(val));
                                }
                            }
                        }
                        self.raise_error(*line, *col, "Invalid assignment target")?;
                        Ok(None)
                    }
                    _ => {
                        self.raise_error(*line, *col, "Invalid assignment target")?;
                        Ok(None)
                    }
                }
            }
            VbStmt::Call(expr) => {
                let val = self.eval_expression(expr, session)?;
                Ok(Some(val))
            }
            VbStmt::SubDecl {
                name,
                parameters,
                body,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                self.routines.insert(
                    name.to_ascii_lowercase(),
                    VbRoutine {
                        parameters: parameters.clone(),
                        body: body.clone(),
                        is_function: false,
                    },
                );
                Ok(None)
            }
            VbStmt::FunctionDecl {
                name,
                parameters,
                body,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                self.routines.insert(
                    name.to_ascii_lowercase(),
                    VbRoutine {
                        parameters: parameters.clone(),
                        body: body.clone(),
                        is_function: true,
                    },
                );
                Ok(None)
            }
            VbStmt::If {
                condition,
                then_branch,
                elseif_branches,
                else_branch,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let cond_val = self.eval_expression(condition, session)?;
                if cond_val.is_truthy() {
                    for s in then_branch {
                        if self.exit_routine {
                            break;
                        }
                        self.eval_statement(s, session)?;
                    }
                    return Ok(None);
                }
                for (elif_cond, elif_body) in elseif_branches {
                    if self.eval_expression(elif_cond, session)?.is_truthy() {
                        for s in elif_body {
                            if self.exit_routine {
                                break;
                            }
                            self.eval_statement(s, session)?;
                        }
                        return Ok(None);
                    }
                }
                if let Some(else_stmts) = else_branch {
                    for s in else_stmts {
                        if self.exit_routine {
                            break;
                        }
                        self.eval_statement(s, session)?;
                    }
                }
                Ok(None)
            }
            VbStmt::While {
                condition,
                body,
                line,
                col,
            } => {
                while {
                    session.consume_fuel(*line, *col)?;
                    self.eval_expression(condition, session)?.is_truthy()
                } {
                    for s in body {
                        if self.exit_routine {
                            break;
                        }
                        self.eval_statement(s, session)?;
                    }
                    if self.exit_routine {
                        break;
                    }
                }
                Ok(None)
            }
            VbStmt::For {
                var_name,
                start,
                end,
                step,
                body,
                line,
                col,
            } => {
                let start_val = self.eval_expression(start, session)?.to_integer();
                let end_val = self.eval_expression(end, session)?.to_integer();
                let step_val = if let Some(st) = step {
                    self.eval_expression(st, session)?.to_integer()
                } else {
                    1
                };

                let mut current = start_val;
                while if step_val >= 0 {
                    current <= end_val
                } else {
                    current >= end_val
                } {
                    session.consume_fuel(*line, *col)?;
                    self.set_var(var_name, &Variant::Integer(current), session);
                    for s in body {
                        if self.exit_routine {
                            break;
                        }
                        self.eval_statement(s, session)?;
                    }
                    if self.exit_routine {
                        break;
                    }
                    current += step_val;
                }
                Ok(None)
            }
            VbStmt::ExitSub | VbStmt::ExitFunction => {
                self.exit_routine = true;
                Ok(None)
            }
        }
    }

    /// Evaluates an expression.
    #[allow(clippy::too_many_lines)]
    fn eval_expression(&mut self, expr: &VbExpr, session: &mut ScriptSession) -> Result<Variant> {
        match expr {
            VbExpr::Literal(v, line, col) => {
                session.consume_fuel(*line, *col)?;
                Ok(v.clone())
            }
            VbExpr::Identifier(id, line, col) => {
                session.consume_fuel(*line, *col)?;
                Ok(self.get_var(id, session))
            }
            VbExpr::MemberAccess {
                object,
                member,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let obj_val = self.eval_expression(object, session)?;
                if let Variant::Object(name) = obj_val {
                    if name.eq_ignore_ascii_case("session") {
                        if member.eq_ignore_ascii_case("database") {
                            return Ok(Variant::Object("Database".to_string()));
                        }
                        let prop_val = session.property(member);
                        return Ok(Variant::String(prop_val));
                    }
                    if name.eq_ignore_ascii_case("err") {
                        if member.eq_ignore_ascii_case("number") {
                            return Ok(Variant::Integer(self.err.number));
                        }
                        if member.eq_ignore_ascii_case("description") {
                            return Ok(Variant::String(self.err.description.clone()));
                        }
                        if member.eq_ignore_ascii_case("clear") {
                            self.err.clear();
                            return Ok(Variant::Empty);
                        }
                    }
                }
                Ok(Variant::Empty)
            }
            VbExpr::Call {
                callee,
                arguments,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                if let VbExpr::MemberAccess { object, member, .. } = &**callee {
                    let obj_val = self.eval_expression(object, session)?;
                    return self.call_method(obj_val, member, arguments, *line, *col, session);
                }
                if let VbExpr::Identifier(fn_name, _, _) = &**callee {
                    return self.call_function(fn_name, arguments, *line, *col, session);
                }
                Ok(Variant::Empty)
            }
            VbExpr::Unary {
                op,
                expr,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let val = self.eval_expression(expr, session)?;
                match op {
                    VbUnaryOp::Negate => Ok(Variant::Integer(-val.to_integer())),
                    VbUnaryOp::Not => Ok(Variant::Boolean(!val.is_truthy())),
                }
            }
            VbExpr::Binary {
                op,
                left,
                right,
                line,
                col,
            } => {
                session.consume_fuel(*line, *col)?;
                let l_val = self.eval_expression(left, session)?;
                match op {
                    VbBinaryOp::And => {
                        if l_val.is_truthy() {
                            self.eval_expression(right, session)
                        } else {
                            Ok(l_val)
                        }
                    }
                    VbBinaryOp::Or => {
                        if l_val.is_truthy() {
                            Ok(l_val)
                        } else {
                            self.eval_expression(right, session)
                        }
                    }
                    VbBinaryOp::Add => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Integer(l_val.to_integer() + r_val.to_integer()))
                    }
                    VbBinaryOp::Subtract => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Integer(l_val.to_integer() - r_val.to_integer()))
                    }
                    VbBinaryOp::Multiply => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Integer(l_val.to_integer() * r_val.to_integer()))
                    }
                    VbBinaryOp::Divide => {
                        let r_val = self.eval_expression(right, session)?;
                        let r_int = r_val.to_integer();
                        if r_int == 0 {
                            self.raise_error(*line, *col, "Division by zero")?;
                            Ok(Variant::Integer(0))
                        } else {
                            Ok(Variant::Integer(l_val.to_integer() / r_int))
                        }
                    }
                    VbBinaryOp::Mod => {
                        let r_val = self.eval_expression(right, session)?;
                        let r_int = r_val.to_integer();
                        if r_int == 0 {
                            self.raise_error(*line, *col, "Division by zero")?;
                            Ok(Variant::Integer(0))
                        } else {
                            Ok(Variant::Integer(l_val.to_integer() % r_int))
                        }
                    }
                    VbBinaryOp::Concat => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::String(format!(
                            "{}{}",
                            l_val.to_string_value(),
                            r_val.to_string_value()
                        )))
                    }
                    VbBinaryOp::Equal => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(
                            l_val.to_string_value() == r_val.to_string_value(),
                        ))
                    }
                    VbBinaryOp::NotEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(
                            l_val.to_string_value() != r_val.to_string_value(),
                        ))
                    }
                    VbBinaryOp::Less => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(l_val.to_integer() < r_val.to_integer()))
                    }
                    VbBinaryOp::LessEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(l_val.to_integer() <= r_val.to_integer()))
                    }
                    VbBinaryOp::Greater => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(l_val.to_integer() > r_val.to_integer()))
                    }
                    VbBinaryOp::GreaterEqual => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(l_val.to_integer() >= r_val.to_integer()))
                    }
                    VbBinaryOp::Xor => {
                        let r_val = self.eval_expression(right, session)?;
                        Ok(Variant::Boolean(l_val.is_truthy() ^ r_val.is_truthy()))
                    }
                }
            }
        }
    }

    /// Invokes a method on an automation object.
    fn call_method(
        &mut self,
        target: Variant,
        method: &str,
        arguments: &[VbExpr],
        line: usize,
        col: usize,
        session: &mut ScriptSession,
    ) -> Result<Variant> {
        if let Variant::Object(obj_name) = target {
            let mut evaluated_args = Vec::new();
            for arg in arguments {
                evaluated_args.push(self.eval_expression(arg, session)?);
            }
            if obj_name.eq_ignore_ascii_case("session") {
                if method.eq_ignore_ascii_case("property") {
                    let prop = evaluated_args.first().map_or("", |v| match v {
                        Variant::String(s) => s.as_str(),
                        _ => "",
                    });
                    if evaluated_args.len() >= 2 {
                        let val = evaluated_args[1].to_string_value();
                        session.set_property(prop, &val);
                        return Ok(Variant::String(val));
                    }
                    return Ok(Variant::String(session.property(prop)));
                }
                if method.eq_ignore_ascii_case("evaluatecondition") {
                    let expr_str = evaluated_args
                        .first()
                        .map_or(String::new(), Variant::to_string_value);
                    let res = session.evaluate_condition(&expr_str);
                    return Ok(Variant::Integer(i64::from(res)));
                }
                if method.eq_ignore_ascii_case("message") {
                    #[allow(clippy::cast_possible_truncation)]
                    let kind = evaluated_args.first().map_or(0, Variant::to_integer) as i32;
                    let text = evaluated_args
                        .get(1)
                        .map_or(String::new(), Variant::to_string_value);
                    let ret = session.message(kind, &text);
                    return Ok(Variant::Integer(i64::from(ret)));
                }
                if method.eq_ignore_ascii_case("mode") {
                    #[allow(clippy::cast_possible_truncation)]
                    let mode_id = evaluated_args.first().map_or(0, Variant::to_integer) as i32;
                    return Ok(Variant::Boolean(session.mode(mode_id)));
                }
                if method.eq_ignore_ascii_case("doaction") {
                    let action = evaluated_args
                        .first()
                        .map_or(String::new(), Variant::to_string_value);
                    let ret = session.do_action(&action);
                    return Ok(Variant::Integer(i64::from(ret)));
                }
            } else if obj_name.eq_ignore_ascii_case("database") {
                if method.eq_ignore_ascii_case("tableexists") {
                    let table = evaluated_args
                        .first()
                        .map_or(String::new(), Variant::to_string_value);
                    return Ok(Variant::Boolean(session.database().table_exists(&table)));
                }
                if method.eq_ignore_ascii_case("rowcount") {
                    let table = evaluated_args
                        .first()
                        .map_or(String::new(), Variant::to_string_value);
                    #[allow(clippy::cast_possible_wrap)]
                    return Ok(Variant::Integer(session.database().row_count(&table) as i64));
                }
            } else if obj_name.eq_ignore_ascii_case("err") {
                if method.eq_ignore_ascii_case("clear") {
                    self.err.clear();
                    return Ok(Variant::Empty);
                }
                if method.eq_ignore_ascii_case("raise") {
                    let num = evaluated_args.first().map_or(1, Variant::to_integer);
                    let desc = evaluated_args
                        .get(1)
                        .map_or(String::new(), Variant::to_string_value);
                    self.err.number = num;
                    self.err.description.clone_from(&desc);
                    if !self.resume_next {
                        return Err(Error::ScriptRuntimeError {
                            line,
                            col,
                            message: format!("VBScript Err.Raise #{num}: {desc}"),
                        });
                    }
                    return Ok(Variant::Empty);
                }
            }
        }
        self.raise_error(line, col, &format!("Method '{method}' not found"))?;
        Ok(Variant::Empty)
    }

    /// Invokes a built-in or user-defined function.
    fn call_function(
        &mut self,
        name: &str,
        arguments: &[VbExpr],
        line: usize,
        col: usize,
        session: &mut ScriptSession,
    ) -> Result<Variant> {
        let mut evaluated_args = Vec::new();
        for arg in arguments {
            evaluated_args.push(self.eval_expression(arg, session)?);
        }

        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "cint" | "clng" => {
                let n = evaluated_args.first().map_or(0, Variant::to_integer);
                Ok(Variant::Integer(n))
            }
            "cstr" => {
                let s = evaluated_args
                    .first()
                    .map_or(String::new(), Variant::to_string_value);
                Ok(Variant::String(s))
            }
            "cbool" => {
                let b = evaluated_args.first().is_some_and(Variant::is_truthy);
                Ok(Variant::Boolean(b))
            }
            "len" => {
                let s = evaluated_args
                    .first()
                    .map_or(String::new(), Variant::to_string_value);
                #[allow(clippy::cast_possible_wrap)]
                Ok(Variant::Integer(s.len() as i64))
            }
            "ucase" => {
                let s = evaluated_args
                    .first()
                    .map_or(String::new(), Variant::to_string_value);
                Ok(Variant::String(s.to_uppercase()))
            }
            "lcase" => {
                let s = evaluated_args
                    .first()
                    .map_or(String::new(), Variant::to_string_value);
                Ok(Variant::String(s.to_lowercase()))
            }
            "trim" => {
                let s = evaluated_args
                    .first()
                    .map_or(String::new(), Variant::to_string_value);
                Ok(Variant::String(s.trim().to_string()))
            }
            "msgbox" => {
                let s = evaluated_args
                    .first()
                    .map_or(String::new(), Variant::to_string_value);
                session.message(0, &s);
                Ok(Variant::Integer(1))
            }
            _ => {
                if let Some(routine) = self.routines.get(&lower).cloned() {
                    session.push_call(name);
                    let mut saved_vars = HashMap::new();
                    for (i, param) in routine.parameters.iter().enumerate() {
                        let p_lower = param.to_ascii_lowercase();
                        let arg_val = evaluated_args.get(i).cloned().unwrap_or(Variant::Empty);
                        if let Some(old) = self.variables.insert(p_lower.clone(), arg_val) {
                            saved_vars.insert(p_lower, old);
                        }
                    }
                    if routine.is_function {
                        self.variables.insert(lower.clone(), Variant::Empty);
                    }
                    for stmt in &routine.body {
                        self.eval_statement(stmt, session)?;
                        if self.exit_routine {
                            break;
                        }
                    }
                    self.exit_routine = false;
                    let ret = if routine.is_function {
                        self.variables
                            .get(&lower)
                            .cloned()
                            .unwrap_or(Variant::Empty)
                    } else {
                        Variant::Empty
                    };
                    for (k, v) in saved_vars {
                        self.variables.insert(k, v);
                    }
                    session.pop_call();
                    Ok(ret)
                } else {
                    self.raise_error(line, col, &format!("Undefined VBScript function '{name}'"))?;
                    Ok(Variant::Empty)
                }
            }
        }
    }

    /// Raises an error adhering to active On Error Resume Next status.
    fn raise_error(&mut self, line: usize, col: usize, msg: &str) -> Result<()> {
        self.err.number = 5;
        self.err.description = msg.to_string();
        if self.resume_next {
            Ok(())
        } else {
            Err(Error::ScriptRuntimeError {
                line,
                col,
                message: msg.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vbscript_basic_execution_and_variables() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        let script = "
            Dim x, y, z
            x = 15
            y = 25
            z = x + y
        ";
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok());
        assert_eq!(engine.get_var("z", &session), Variant::Integer(40));
    }

    #[test]
    fn test_vbscript_session_properties_and_actions() {
        let mut session = ScriptSession::default();
        session.set_property("ProductName", "InstallerApp");
        let mut row = HashMap::new();
        row.insert("Property".to_string(), "TestProp".to_string());
        session.database_mut().insert_row("Property", row);

        let mut engine = VBScriptEngine::new();
        let script = r#"
            Dim p
            p = Session.Property("ProductName")
            Session.Property("ProductName") = p & " 2026"
            Session.DoAction "InitAction"
            FLAG_SET = "YES"
            Dim hasProp, propCount
            hasProp = Session.Database.TableExists("Property")
            propCount = Session.Database.RowCount("Property")
        "#;
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok());
        assert_eq!(session.property("ProductName"), "InstallerApp 2026");
        assert_eq!(session.property("FLAG_SET"), "YES");
        assert_eq!(session.executed_actions(), &["InitAction".to_string()]);
        assert_eq!(engine.get_var("hasProp", &session), Variant::Boolean(true));
        assert_eq!(engine.get_var("propCount", &session), Variant::Integer(1));

        // Test unsupported method on Database
        let err_script = r#"Session.Database.UnknownMethod("Property")"#;
        assert!(engine.execute(err_script, &mut session).is_err());
    }

    #[test]
    fn test_vbscript_functions_and_loops() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        let script = "
            Function AddNumbers(a, b)
                AddNumbers = a + b
            End Function

            Dim total, i
            total = 0
            For i = 1 To 5
                total = AddNumbers(total, i)
            Next
        ";
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok());
        assert_eq!(engine.get_var("total", &session), Variant::Integer(15));
    }

    #[test]
    fn test_vbscript_error_handling_resume_next() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        let script = r#"
            On Error Resume Next
            Err.Raise 101, "Simulated Error"
            Dim errNum
            errNum = Err.Number
            Err.Clear
        "#;
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok());
        assert_eq!(engine.get_var("errNum", &session), Variant::Integer(101));
        assert_eq!(engine.err.number, 0);
    }

    #[test]
    fn test_vbscript_while_and_elseif_and_strings() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        let script = r#"
            Dim counter, acc
            counter = 1
            acc = 0
            While counter <= 3
                acc = acc + counter
                counter = counter + 1
            Wend

            Dim grade
            If acc > 10 Then
                grade = "A"
            ElseIf acc > 4 Then
                grade = "B"
            Else
                grade = "C"
            End If

            Dim textVal
            textVal = Trim(UCase("  hello  "))
            Dim lengthVal
            lengthVal = Len(textVal)
            Dim boolVal
            boolVal = CBool(acc)
            MsgBox "Calculated: " & textVal
        "#;
        let res = engine.execute(script, &mut session);
        assert!(res.is_ok());
        assert_eq!(engine.get_var("acc", &session), Variant::Integer(6));
        assert_eq!(
            engine.get_var("grade", &session),
            Variant::String("B".to_string())
        );
        assert_eq!(
            engine.get_var("textVal", &session),
            Variant::String("HELLO".to_string())
        );
        assert_eq!(engine.get_var("lengthVal", &session), Variant::Integer(5));
        assert_eq!(engine.get_var("boolVal", &session), Variant::Boolean(true));
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn test_vbscript_err_raise_unhandled() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        let script = r#"
            Err.Raise 500, "Fatal error occurred"
        "#;
        let res = engine.execute(script, &mut session);
        assert_eq!(
            res,
            Err(Error::ScriptRuntimeError {
                line: 2,
                col: 13,
                message: "VBScript Err.Raise #500: Fatal error occurred".to_string(),
            })
        );
    }

    /// Tests `Variant` truthiness, integer, and string representations across all variants.
    #[test]
    fn test_vbscript_variant_methods() {
        // is_truthy
        assert!(!Variant::Empty.is_truthy());
        assert!(!Variant::Null.is_truthy());
        assert!(!Variant::Boolean(false).is_truthy());
        assert!(Variant::Boolean(true).is_truthy());
        assert!(!Variant::Integer(0).is_truthy());
        assert!(Variant::Integer(1).is_truthy());
        assert!(!Variant::String(String::new()).is_truthy());
        assert!(!Variant::String("0".to_string()).is_truthy());
        assert!(Variant::String("1".to_string()).is_truthy());
        assert!(Variant::Object("Session".to_string()).is_truthy());

        // to_integer
        assert_eq!(Variant::Boolean(true).to_integer(), -1);
        assert_eq!(Variant::Boolean(false).to_integer(), 0);
        assert_eq!(Variant::Integer(42).to_integer(), 42);
        assert_eq!(Variant::String(" 100 ".to_string()).to_integer(), 100);
        assert_eq!(Variant::String("abc".to_string()).to_integer(), 0);
        assert_eq!(Variant::Empty.to_integer(), 0);
        assert_eq!(Variant::Null.to_integer(), 0);
        assert_eq!(Variant::Object("Session".to_string()).to_integer(), 0);

        // to_string_value
        assert_eq!(Variant::Empty.to_string_value(), "");
        assert_eq!(Variant::Null.to_string_value(), "Null");
        assert_eq!(Variant::Boolean(true).to_string_value(), "True");
        assert_eq!(Variant::Boolean(false).to_string_value(), "False");
        assert_eq!(Variant::Integer(42).to_string_value(), "42");
        assert_eq!(
            Variant::String("hello".to_string()).to_string_value(),
            "hello"
        );
        assert_eq!(
            Variant::Object("Session".to_string()).to_string_value(),
            "[object Session]"
        );
    }

    /// Tests lexer line continuation, comments, quotes, integer errors, and keywords.
    #[test]
    fn test_vbscript_lexer_tokens_and_errors() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        // Line continuation and comments
        let script = "
            ' single line comment
            rem another comment
            Dim a _
                , b
            a = 10
            b = 20
            Dim c : c = a + b
        ";
        assert!(engine.execute(script, &mut session).is_ok());

        // Tabs, carriage returns, and leading underscore identifiers
        assert!(engine
            .execute(
                "\t\r Dim _leading_under \r\n _leading_under = 10",
                &mut session
            )
            .is_ok());

        // Trailing comments without newline at EOF
        assert!(engine
            .execute("' trailing comment without newline", &mut session)
            .is_ok());
        assert!(engine
            .execute("rem trailing rem comment", &mut session)
            .is_ok());

        // Escaped quotes inside strings
        let quote_script = r#"
            Dim s
            s = "He said ""Hello"""
            s
        "#;
        assert!(engine.execute(quote_script, &mut session).is_ok());

        // Lexer errors
        assert!(engine
            .execute("Dim s : s = \"unterminated", &mut session)
            .is_err());
        assert!(engine
            .execute(
                "Dim n : n = 99999999999999999999999999999999999999999999999999999999999999999",
                &mut session
            )
            .is_err());
        assert!(engine.execute("Dim x : x = @", &mut session).is_err());

        // All operators: <>, <=, <, >=, >, =, +, -, *, /, \, &, (, ), ,, ., :
        let op_script = "
            Dim v1, v2, v3, v4, v5, v6, v7
            v1 = (10 <> 20)
            v2 = (10 <= 10)
            v3 = (5 < 10)
            v4 = (20 >= 20)
            v5 = (20 > 10)
            v6 = 10 \\ 3
            v7 = 10 Mod 3
        ";
        assert!(engine.execute(op_script, &mut session).is_ok());
    }

    /// Tests parser error handling and diagnostic paths.
    #[test]
    fn test_vbscript_parser_errors() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        // Advance past EOF in Parser
        let mut p = VbParser::new(vec![]);
        let tok1 = p.advance();
        assert_eq!(tok1.kind, VbTokenKind::Eof);
        let tok2 = p.advance();
        assert_eq!(tok2.kind, VbTokenKind::Eof);

        // Parser errors
        assert!(engine.execute("Dim 123", &mut session).is_err());
        assert!(engine.execute("On Error Foo", &mut session).is_err());
        assert!(engine.execute("On Error Goto 1", &mut session).is_err());
        assert!(engine.execute("On Error Resume Foo", &mut session).is_err());
        assert!(engine.execute("Sub 123", &mut session).is_err());
        assert!(engine.execute("Sub Foo(a, b", &mut session).is_err());
        assert!(engine
            .execute("Sub Foo(a, b) : Dim x", &mut session)
            .is_err());
        assert!(engine.execute("Function 123", &mut session).is_err());
        assert!(engine.execute("Function Foo(a, b", &mut session).is_err());
        assert!(engine
            .execute("Function Foo(a, b) : Dim x", &mut session)
            .is_err());
        assert!(engine.execute("If 1 = 1", &mut session).is_err());
        assert!(engine
            .execute("If 1 = 1 Then : Dim x", &mut session)
            .is_err());
        assert!(engine
            .execute("If False Then ElseIf True Then Dim x", &mut session)
            .is_err());
        assert!(engine
            .execute("If False Then Else Dim x", &mut session)
            .is_err());
        assert!(engine.execute("While 1 = 1", &mut session).is_err());
        assert!(engine.execute("For 123", &mut session).is_err());
        assert!(engine.execute("For i 10", &mut session).is_err());
        assert!(engine.execute("For i = 1 10", &mut session).is_err());
        assert!(engine
            .execute("For i = 1 To 10 : Dim x", &mut session)
            .is_err());
        assert!(engine.execute("Exit While", &mut session).is_err());
        assert!(engine.execute("Set x 1", &mut session).is_err());
        assert!(engine.execute("Dim x : x = obj.123", &mut session).is_err());
        assert!(engine.execute("Dim x : x = (1 + ", &mut session).is_err());
        assert!(engine.execute("Dim x : x = 1 + +", &mut session).is_err());
    }

    /// Tests subroutines, Exit Sub/Function, On Error Goto 0, and For loops.
    #[test]
    fn test_vbscript_statements_and_control_flow() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        // Subroutine declaration and execution via statement and Call
        let sub_script = "
            Dim globalVal
            Sub SetVal(n)
                globalVal = n
                Exit Sub
                globalVal = 999
            End Sub

            SetVal 42
            Call SetVal(100)
        ";
        assert!(engine.execute(sub_script, &mut session).is_ok());
        assert_eq!(engine.get_var("globalVal", &session), Variant::Integer(100));

        // Function with Exit Function
        let fn_script = "
            Function EarlyExit(flag)
                EarlyExit = 10
                If flag Then
                    Exit Function
                End If
                EarlyExit = 20
            End Function

            Dim r1, r2
            r1 = EarlyExit(True)
            r2 = EarlyExit(False)
        ";
        assert!(engine.execute(fn_script, &mut session).is_ok());
        assert_eq!(engine.get_var("r1", &session), Variant::Integer(10));
        assert_eq!(engine.get_var("r2", &session), Variant::Integer(20));

        // On Error Goto 0 resets resume_next
        let err_script = r#"
            On Error Resume Next
            Err.Raise 1, "Handled"
            On Error Goto 0
            Err.Raise 2, "Unhandled"
        "#;
        assert!(engine.execute(err_script, &mut session).is_err());

        // For loop with step (positive, negative, and with variable on Next)
        let for_script = "
            Dim sumPos, sumNeg, i, j
            sumPos = 0
            For i = 1 To 5 Step 2
                sumPos = sumPos + i
            Next i

            sumNeg = 0
            For j = 5 To 1 Step -2
                sumNeg = sumNeg + j
            Next
        ";
        assert!(engine.execute(for_script, &mut session).is_ok());
        assert_eq!(
            engine.get_var("sumPos", &session),
            Variant::Integer(1 + 3 + 5)
        );
        assert_eq!(
            engine.get_var("sumNeg", &session),
            Variant::Integer(5 + 3 + 1)
        );
    }

    /// Tests operators, logical short-circuiting, division by zero, and assignment targets.
    #[test]
    fn test_vbscript_expressions_and_operators() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        // Binary operations and short-circuiting
        let expr_script = "
            Dim a, b, c, d, e, f
            a = True And 42
            b = False And (1 / 0)
            c = True Or (1 / 0)
            d = False Or 99
            e = True Xor False
            f = False Xor False
        ";
        assert!(engine.execute(expr_script, &mut session).is_ok());

        // Unary Negate and Not
        let unary_script = "
            Dim n, b
            n = -(-10)
            b = Not (Not True)
        ";
        assert!(engine.execute(unary_script, &mut session).is_ok());
        assert_eq!(engine.get_var("n", &session), Variant::Integer(10));
        assert_eq!(engine.get_var("b", &session), Variant::Boolean(true));

        // Division by zero without Resume Next and with Resume Next
        assert!(engine.execute("Dim d : d = 10 / 0", &mut session).is_err());
        assert!(engine
            .execute("Dim m : m = 10 Mod 0", &mut session)
            .is_err());
        let resume_div = "
            On Error Resume Next
            Dim d, m
            d = 10 / 0
            m = 10 Mod 0
        ";
        assert!(engine.execute(resume_div, &mut session).is_ok());

        // Invalid assignment targets
        let mut strict_engine = VBScriptEngine::new();
        assert!(strict_engine.execute("(1 + 2) = 3", &mut session).is_err());
        assert!(strict_engine.execute("Foo(1) = 2", &mut session).is_err());
    }

    /// Tests built-in functions, Automation methods, Err object, and fuel limits.
    #[test]
    fn test_vbscript_builtins_and_automation() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        let builtins_script = "
            Dim i1, i2, s, b1, b2, l, uc, lc, tr
            i1 = CInt(\"123\")
            i2 = CLng(\"456\")
            s = CStr(789)
            b1 = CBool(1)
            b2 = CBool(0)
            l = Len(\"hello\")
            uc = UCase(\"hello\")
            lc = LCase(\"WORLD\")
            tr = Trim(\"  spaced  \")
        ";
        assert!(engine.execute(builtins_script, &mut session).is_ok());

        // Session automation methods
        let session_script = r#"
            Dim condRes, msgRes, modeRes, actRes
            condRes = Session.EvaluateCondition("1 = 1")
            msgRes = Session.Message(1, "Test")
            modeRes = Session.Mode(2)
            actRes = Session.DoAction("CustomAction")
            Dim emptyProp
            emptyProp = Session.Property()
        "#;
        assert!(engine.execute(session_script, &mut session).is_ok());

        // Unknown methods on Session and Err
        assert!(engine
            .execute("Session.NonExistentMethod()", &mut session)
            .is_err());
        assert!(engine
            .execute("Dim e : e = Err.UnknownMethod()", &mut session)
            .is_err());
        assert!(engine
            .execute("Dim m : m = (123).Unknown()", &mut session)
            .is_err());

        // Undefined function with and without Resume Next
        assert!(engine
            .execute("NonExistentFunction()", &mut session)
            .is_err());
        assert!(engine
            .execute("On Error Resume Next : NonExistentFunction()", &mut session)
            .is_ok());

        // Fuel exhaustion
        let mut fuel_session =
            ScriptSession::with_fuel(crate::execution::properties::EvaluationContext::new(), 10);
        assert!(engine
            .execute("While True : Wend", &mut fuel_session)
            .is_err());
    }

    /// Tests remaining edge-case branches in `VBScript` evaluation, lexing, and parsing.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_vbscript_uncovered_branches() {
        let mut session = ScriptSession::default();
        let mut engine = VBScriptEngine::new();

        // 1. Tokens: nothing, set, empty, null, do, loop, until
        let tok_script = "
            Dim obj, e, n
            Set obj = Session
            Set obj = Nothing
            e = Empty
            n = Null
            Dim do_words : do_words = 1
        ";
        assert!(engine.execute(tok_script, &mut session).is_ok());

        // Lexer words do, loop, until
        let mut lex = VbLexer::new("Do Loop Until");
        assert!(lex.tokenize_all().is_ok());

        // 2. Subroutines without parens, empty parens, and Exit Sub inside structures with trailing stmts
        let exit_sub_script = "
            Sub NoParens
                Dim a : a = 1
            End Sub

            Sub EmptyParens()
                Dim b : b = 2
            End Sub

            Sub ParamNotIdent(123)
                Dim p : p = 3
            End Sub

            Sub InIfThen()
                If True Then
                    Exit Sub
                    Dim dead1 : dead1 = 1
                End If
            End Sub

            Sub InIfElseIf()
                If False Then
                    Dim dead2 : dead2 = 1
                ElseIf True Then
                    Exit Sub
                    Dim dead3 : dead3 = 1
                End If
            End Sub

            Sub InIfElse()
                If False Then
                    Dim dead4 : dead4 = 1
                Else
                    Exit Sub
                    Dim dead5 : dead5 = 1
                End If
            End Sub

            Sub InWhile()
                While True
                    Exit Sub
                    Dim dead6 : dead6 = 1
                Wend
            End Sub

            Sub InFor()
                Dim i
                For i = 1 To 10
                    Exit Sub
                    Dim dead7 : dead7 = 1
                Next
            End Sub

            Sub NormalElse()
                Dim normal
                If False Then
                    normal = 0
                ElseIf False Then
                    normal = 1
                Else
                    normal = 42
                End If
            End Sub

            NoParens()
            EmptyParens()
            ParamNotIdent 99
            InIfThen()
            InIfElseIf()
            InIfElse()
            InWhile()
            InFor()
            NormalElse()
        ";
        assert!(engine.execute(exit_sub_script, &mut session).is_ok());

        // 3. Binary operations: subtract, multiply, equal
        let bin_script = "
            Dim subRes, mulRes, eqRes
            subRes = 20 - 5
            mulRes = 10 * 5
            eqRes = (\"test\" = \"test\")
        ";
        assert!(engine.execute(bin_script, &mut session).is_ok());
        assert_eq!(engine.get_var("subRes", &session), Variant::Integer(15));
        assert_eq!(engine.get_var("mulRes", &session), Variant::Integer(50));
        assert_eq!(engine.get_var("eqRes", &session), Variant::Boolean(true));

        // 4. Session.Property setter with 2 arguments, non-string argument, and direct member access
        session.set_property("PROP_EXISTS", "OldValue");
        let sess_prop_script = "
            Session.Property \"PROP_EXISTS\", \"NewValue\"
            Dim directRead, nonStr
            directRead = Session.PROP_EXISTS
            nonStr = Session.Property(123)
        ";
        assert!(engine.execute(sess_prop_script, &mut session).is_ok());
        assert_eq!(session.property("PROP_EXISTS"), "NewValue");

        // 5. Err object direct member access and method calls
        let err_script = "
            On Error Resume Next
            Err.Raise 999, \"Custom Err\"
            Dim desc, unkErr, numVal, nonObjMem
            desc = Err.Description
            numVal = Err.Number
            unkErr = Err.NonExistentField
            nonObjMem = (42).SomeMember
            Err.Clear
            Err.Clear()
        ";
        assert!(engine.execute(err_script, &mut session).is_ok());
        assert_eq!(
            engine.get_var("desc", &session),
            Variant::String("Custom Err".to_string())
        );

        // 6. Resume Next on method not found and invalid assignments
        let resume_errors = "
            On Error Resume Next
            Session.NonExistentMethod()
            (1 + 2) = 3
            Session.OtherMethod(\"a\") = 1
            Session.Property() = 1
            (42).Property(\"a\") = 1
            otherObj.Property(\"a\") = 1
        ";
        assert!(engine.execute(resume_errors, &mut session).is_ok());

        // Call without arguments followed by colon statement separator
        assert!(engine
            .execute(
                "Sub TestColon : End Sub\n TestColon : Dim nextVar : nextVar = 1",
                &mut session
            )
            .is_ok());

        // 7. Non-identifier callee in Call expression and custom object call
        let non_ident_call = VbExpr::Call {
            callee: Box::new(VbExpr::Literal(Variant::Integer(42), 1, 1)),
            arguments: vec![],
            line: 1,
            col: 1,
        };
        assert_eq!(
            engine.eval_expression(&non_ident_call, &mut session),
            Ok(Variant::Empty)
        );
        let custom_obj = Variant::Object("OtherObject".to_string());
        assert!(engine
            .call_method(custom_obj.clone(), "Foo", &[], 1, 1, &mut session)
            .is_ok());
        let non_standard_mem = VbExpr::MemberAccess {
            object: Box::new(VbExpr::Literal(custom_obj, 1, 1)),
            member: "Prop".to_string(),
            line: 1,
            col: 1,
        };
        assert_eq!(
            engine.eval_expression(&non_standard_mem, &mut session),
            Ok(Variant::Empty)
        );

        // 8. Public property reading from Session vs unbound variable
        session.set_property("PUB_VAR_VBS", "PropVal");
        let pub_script = "
            Dim p, unb
            p = PUB_VAR_VBS
            unb = COMPLETELY_UNKNOWN_VAR
        ";
        assert!(engine.execute(pub_script, &mut session).is_ok());
        assert_eq!(
            engine.get_var("p", &session),
            Variant::String("PropVal".to_string())
        );
        assert_eq!(engine.get_var("unb", &session), Variant::Empty);

        // 9. Boundary checks for End Sub/Function/If with unexpected next token
        let parser = VbParser::new(vec![
            VbToken {
                kind: VbTokenKind::KeywordEnd,
                line: 1,
                col: 1,
            },
            VbToken {
                kind: VbTokenKind::KeywordDim,
                line: 1,
                col: 5,
            },
        ]);
        assert!(!parser.is_end_sub());
        assert!(!parser.is_end_function());
        assert!(!parser.is_end_if());
    }
}
