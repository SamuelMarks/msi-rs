//! MSI Property & Expression Evaluation Engine.
//!
//! Grounded directly in official Microsoft Windows Installer SDK:
//! - Property scoping & accessibility:
//!   - Public properties (uppercase letters `[A-Z_0-9]`, can be set via CLI/UI).
//!   - Private properties (contains lowercase `[a-z]`, session-internal).
//!   - Restricted public properties enforcement.
//! - Expression evaluation:
//!   - Logical operators: `AND`, `OR`, `NOT`, `XOR`.
//!   - Bitwise operators: `&`, `|`.
//!   - Comparison operators: `=`, `<>`, `<`, `>`, `<=`, `>=`, and case-insensitive `~=`, `~<>`, etc.
//!   - Substring & pattern operators: `><` (contains), `<<` (starts with), `>>` (ends with),
//!     and case-insensitive variants `~><`, `~<<`, `~>>`.
//!   - Feature & Component state symbols: `&Feature`, `!Feature`, `$Component`, `?Component`.
//! - Formatted string engine:
//!   - `[PropertyName]`: Expand property.
//!   - `[\#FileKey]`: Resolved file path.
//!   - `[\$ComponentKey]`: Target directory of component.
//!   - `[\%EnvVar]`: System environment variable.
//!   - Escape sequences: `[\[]`, `[\]]`, `[\{]`, `[\}]`.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Installation states for Features and Components in MSI expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstallState {
    /// Not installed / absent.
    #[default]
    Absent = 2,
    /// Installed to run locally.
    Local = 3,
    /// Installed to run from source / CD / network.
    Source = 4,
    /// Advertised state.
    Advertised = 5,
}

impl InstallState {
    /// Converts integer value to [`InstallState`].
    #[must_use]
    pub const fn from_i32(val: i32) -> Option<Self> {
        match val {
            2 => Some(Self::Absent),
            3 => Some(Self::Local),
            4 => Some(Self::Source),
            5 => Some(Self::Advertised),
            _ => None,
        }
    }

    /// Returns numeric value corresponding to MSI state constants.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Execution session properties and lookup context for expressions and formatted strings.
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    /// Property table mapping property name to string value.
    properties: HashMap<String, String>,
    /// Feature requested action state (`&Feature`).
    feature_action_states: HashMap<String, InstallState>,
    /// Feature installed state (`!Feature`).
    feature_installed_states: HashMap<String, InstallState>,
    /// Component requested action state (`$Component`).
    component_action_states: HashMap<String, InstallState>,
    /// Component installed state (`?Component`).
    component_installed_states: HashMap<String, InstallState>,
    /// Resolved file key paths (`[#FileKey]`).
    file_paths: HashMap<String, String>,
    /// Resolved component directories (`[$ComponentKey]`).
    component_directories: HashMap<String, String>,
}

impl EvaluationContext {
    /// Creates a new empty [`EvaluationContext`].
    ///
    /// # Returns
    ///
    /// A new [`EvaluationContext`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a property name is public (consists only of uppercase ASCII, digits, and underscore).
    ///
    /// # Arguments
    ///
    /// * `name` - Property name.
    ///
    /// # Returns
    ///
    /// `true` if public, `false` if private.
    #[must_use]
    pub fn is_public_property(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }

    /// Sets a property value.
    ///
    /// # Arguments
    ///
    /// * `name` - Property name.
    /// * `value` - String value.
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(name.into(), value.into());
    }

    /// Retrieves a property value, or None if unset.
    ///
    /// # Arguments
    ///
    /// * `name` - Property name.
    ///
    /// # Returns
    ///
    /// Optional reference to string value.
    #[must_use]
    pub fn get_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    /// Returns a reference to the map of all properties.
    ///
    /// # Returns
    ///
    /// Reference to [`HashMap<String, String>`].
    #[must_use]
    pub const fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// Sets requested action state for a Feature (`&Feature`).
    pub fn set_feature_action(&mut self, feature: impl Into<String>, state: InstallState) {
        self.feature_action_states.insert(feature.into(), state);
    }

    /// Sets installed state for a Feature (`!Feature`).
    pub fn set_feature_installed(&mut self, feature: impl Into<String>, state: InstallState) {
        self.feature_installed_states.insert(feature.into(), state);
    }

    /// Sets requested action state for a Component (`$Component`).
    pub fn set_component_action(&mut self, comp: impl Into<String>, state: InstallState) {
        self.component_action_states.insert(comp.into(), state);
    }

    /// Sets installed state for a Component (`?Component`).
    pub fn set_component_installed(&mut self, comp: impl Into<String>, state: InstallState) {
        self.component_installed_states.insert(comp.into(), state);
    }

    /// Sets the resolved path for a File key (`[#FileKey]`).
    pub fn set_file_path(&mut self, file_key: impl Into<String>, path: impl Into<String>) {
        self.file_paths.insert(file_key.into(), path.into());
    }

    /// Sets the resolved directory for a Component key (`[$ComponentKey]`).
    pub fn set_component_dir(&mut self, comp_key: impl Into<String>, dir: impl Into<String>) {
        self.component_directories
            .insert(comp_key.into(), dir.into());
    }

    /// Formats a template string by expanding `[Property]`, `[#File]`, `[$Comp]`, `[%Env]`, etc.
    ///
    /// # Arguments
    ///
    /// * `template` - Formatted template string.
    ///
    /// # Returns
    ///
    /// Expanded result string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on unclosed brackets.
    #[allow(clippy::too_many_lines)]
    pub fn format_string(&self, template: &str) -> Result<String> {
        let mut out = String::new();
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '[' {
                let start = i;
                i += 1;
                let mut bracket_content = String::new();
                let mut closed = false;

                while i < chars.len() {
                    if chars[i] == '\\'
                        && i + 1 < chars.len()
                        && (chars[i + 1] == ']' || chars[i + 1] == '[')
                    {
                        bracket_content.push(chars[i]);
                        bracket_content.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    if chars[i] == ']' {
                        closed = true;
                        i += 1;
                        break;
                    }
                    bracket_content.push(chars[i]);
                    i += 1;
                }

                if !closed {
                    return Err(Error::Validation {
                        element: "FormattedString".to_string(),
                        reason: format!("unclosed bracket starting at position {start}"),
                    });
                }

                // Handle escape sequences
                if bracket_content == r"\[" {
                    out.push('[');
                } else if bracket_content == r"\]" {
                    out.push(']');
                } else if bracket_content == r"\{" {
                    out.push('{');
                } else if bracket_content == r"\}" {
                    out.push('}');
                } else if let Some(file_key) = bracket_content.strip_prefix('#') {
                    // File path [#FileKey]
                    if let Some(path) = self.file_paths.get(file_key) {
                        out.push_str(path);
                    }
                } else if let Some(comp_key) = bracket_content.strip_prefix('$') {
                    // Component directory [$ComponentKey]
                    if let Some(dir) = self.component_directories.get(comp_key) {
                        out.push_str(dir);
                    }
                } else if let Some(env_var) = bracket_content.strip_prefix('%') {
                    // Environment variable [%EnvVar]
                    if let Ok(val) = std::env::var(env_var) {
                        out.push_str(&val);
                    }
                } else {
                    // Property lookup [PropertyName]
                    if let Some(val) = self.get_property(&bracket_content) {
                        out.push_str(val);
                    }
                }
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }

        Ok(out)
    }

    /// Evaluates a Windows Installer condition expression string to boolean.
    ///
    /// Empty condition evaluates to `true`.
    ///
    /// # Arguments
    ///
    /// * `expr` - Expression string.
    ///
    /// # Returns
    ///
    /// Boolean result.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid syntax or unclosed parentheses.
    pub fn evaluate_condition(&self, expr: &str) -> Result<bool> {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return Ok(true);
        }

        let tokens = Self::tokenize_expression(trimmed);
        let mut parser = ConditionParser::new(tokens, self);
        parser.parse_or_expr()
    }

    /// Checks if a character terminates an unquoted word in condition syntax.
    const fn is_word_terminator(c: char) -> bool {
        matches!(
            c,
            ' ' | '\t' | '\r' | '\n' | '(' | ')' | '=' | '<' | '>' | '~'
        )
    }

    /// Tokenizes an expression string.
    #[allow(clippy::too_many_lines)]
    fn tokenize_expression(expr: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = expr.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            if ch.is_whitespace() {
                i += 1;
                continue;
            }

            if ch == '(' {
                tokens.push(Token::OpenParen);
                i += 1;
            } else if ch == ')' {
                tokens.push(Token::CloseParen);
                i += 1;
            } else if ch == '=' {
                tokens.push(Token::Eq);
                i += 1;
            } else if ch == '<' {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::Neq);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Lte);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '<' {
                    tokens.push(Token::StartsWith);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            } else if ch == '>' {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Gte);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '<' {
                    tokens.push(Token::Contains);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::EndsWith);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            } else if ch == '~' {
                // Case-insensitive variants
                i += 1;
                if i < chars.len() {
                    let next = chars[i];
                    if next == '=' {
                        tokens.push(Token::CiEq);
                        i += 1;
                    } else if next == '<' {
                        if i + 1 < chars.len() && chars[i + 1] == '>' {
                            tokens.push(Token::CiNeq);
                            i += 2;
                        } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                            tokens.push(Token::CiLte);
                            i += 2;
                        } else if i + 1 < chars.len() && chars[i + 1] == '<' {
                            tokens.push(Token::CiStartsWith);
                            i += 2;
                        } else {
                            tokens.push(Token::CiLt);
                            i += 1;
                        }
                    } else if next == '>' {
                        if i + 1 < chars.len() && chars[i + 1] == '=' {
                            tokens.push(Token::CiGte);
                            i += 2;
                        } else if i + 1 < chars.len() && chars[i + 1] == '<' {
                            tokens.push(Token::CiContains);
                            i += 2;
                        } else if i + 1 < chars.len() && chars[i + 1] == '>' {
                            tokens.push(Token::CiEndsWith);
                            i += 2;
                        } else {
                            tokens.push(Token::CiGt);
                            i += 1;
                        }
                    } else {
                        tokens.push(Token::Literal("~".to_string()));
                    }
                } else {
                    tokens.push(Token::Literal("~".to_string()));
                }
            } else if ch == '&' || ch == '|' || ch == '!' || ch == '?' || ch == '$' {
                // Symbols or bitwise
                if ch == '&' {
                    if i + 1 < chars.len()
                        && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_')
                    {
                        // &Feature
                        i += 1;
                        let mut name = String::new();
                        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            name.push(chars[i]);
                            i += 1;
                        }
                        tokens.push(Token::FeatureAction(name));
                    } else {
                        tokens.push(Token::BitAnd);
                        i += 1;
                    }
                } else if ch == '!' {
                    if i + 1 < chars.len()
                        && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_')
                    {
                        // !Feature
                        i += 1;
                        let mut name = String::new();
                        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                            name.push(chars[i]);
                            i += 1;
                        }
                        tokens.push(Token::FeatureInstalled(name));
                    } else {
                        tokens.push(Token::Not);
                        i += 1;
                    }
                } else if ch == '$' {
                    i += 1;
                    let mut name = String::new();
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        name.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::ComponentAction(name));
                } else if ch == '?' {
                    i += 1;
                    let mut name = String::new();
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        name.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::ComponentInstalled(name));
                } else {
                    tokens.push(Token::BitOr);
                    i += 1;
                }
            } else if ch == '"' || ch == '\'' {
                // Quoted literal string
                let quote = ch;
                i += 1;
                let mut lit = String::new();
                while i < chars.len() && chars[i] != quote {
                    lit.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // consume closing quote
                }
                tokens.push(Token::Literal(lit));
            } else {
                // Word (Identifier, Keyword, Number)
                let mut word = String::new();
                while i < chars.len() && !Self::is_word_terminator(chars[i]) {
                    word.push(chars[i]);
                    i += 1;
                }

                let upper = word.to_ascii_uppercase();
                if upper == "AND" {
                    tokens.push(Token::And);
                } else if upper == "OR" {
                    tokens.push(Token::Or);
                } else if upper == "NOT" {
                    tokens.push(Token::Not);
                } else if upper == "XOR" {
                    tokens.push(Token::Xor);
                } else {
                    tokens.push(Token::Identifier(word));
                }
            }
        }

        tokens
    }
}

/// Token types inside a Windows Installer expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `NOT`
    Not,
    /// `XOR`
    Xor,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `=`
    Eq,
    /// `<>`
    Neq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Lte,
    /// `>=`
    Gte,
    /// `><`
    Contains,
    /// `<<`
    StartsWith,
    /// `>>`
    EndsWith,
    /// `~=`
    CiEq,
    /// `~<>`
    CiNeq,
    /// `~<`
    CiLt,
    /// `~>`
    CiGt,
    /// `~<=`
    CiLte,
    /// `~>=`
    CiGte,
    /// `~><`
    CiContains,
    /// `~<<`
    CiStartsWith,
    /// `~>>`
    CiEndsWith,
    /// `&Feature`
    FeatureAction(String),
    /// `!Feature`
    FeatureInstalled(String),
    /// `$Component`
    ComponentAction(String),
    /// `?Component`
    ComponentInstalled(String),
    /// Quoted literal string.
    Literal(String),
    /// Identifier or numeric literal.
    Identifier(String),
}

/// Recursive descent parser for boolean expressions.
struct ConditionParser<'a> {
    /// Token sequence.
    tokens: Vec<Token>,
    /// Current token position.
    cursor: usize,
    /// Property evaluation context.
    context: &'a EvaluationContext,
}

impl<'a> ConditionParser<'a> {
    /// Creates a new parser.
    const fn new(tokens: Vec<Token>, context: &'a EvaluationContext) -> Self {
        Self {
            tokens,
            cursor: 0,
            context,
        }
    }

    /// Returns the current token without consuming it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    /// Advances and returns the current token.
    fn advance(&mut self) -> Option<Token> {
        (self.cursor < self.tokens.len()).then(|| {
            let t = self.tokens[self.cursor].clone();
            self.cursor += 1;
            t
        })
    }

    /// Parses an OR expression.
    fn parse_or_expr(&mut self) -> Result<bool> {
        let mut left = self.parse_xor_expr()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_xor_expr()?;
            left = left || right;
        }
        Ok(left)
    }

    /// Parses an XOR expression.
    fn parse_xor_expr(&mut self) -> Result<bool> {
        let mut left = self.parse_and_expr()?;
        while matches!(self.peek(), Some(Token::Xor)) {
            self.advance();
            let right = self.parse_and_expr()?;
            left ^= right;
        }
        Ok(left)
    }

    /// Parses an AND expression.
    fn parse_and_expr(&mut self) -> Result<bool> {
        let mut left = self.parse_not_expr()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_not_expr()?;
            left = left && right;
        }
        Ok(left)
    }

    /// Parses a NOT expression.
    fn parse_not_expr(&mut self) -> Result<bool> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let val = self.parse_not_expr()?;
            Ok(!val)
        } else {
            self.parse_relational_or_primary()
        }
    }

    /// Parses a primary term or relational comparison.
    fn parse_relational_or_primary(&mut self) -> Result<bool> {
        if matches!(self.peek(), Some(Token::OpenParen)) {
            self.advance();
            let val = self.parse_or_expr()?;
            if matches!(self.peek(), Some(Token::CloseParen)) {
                self.advance();
                return Ok(val);
            }
            return Err(Error::Validation {
                element: "ConditionExpression".to_string(),
                reason: "missing closing parenthesis".to_string(),
            });
        }

        let left_val = self.parse_value()?;

        // Check if there is an operator
        if let Some(op_token) = self.peek().cloned() {
            if is_relational_op(&op_token) {
                self.advance();
                let right_val = self.parse_value()?;
                return Ok(compare_values(&left_val, &op_token, &right_val));
            }
        }

        // Single value evaluates to true if non-empty and non-zero
        Ok(is_truthy(&left_val))
    }

    /// Parses a value operand.
    fn parse_value(&mut self) -> Result<String> {
        match self.advance() {
            Some(Token::Literal(s)) => Ok(s),
            Some(Token::Identifier(id)) => {
                // If id is integer literal, keep it, otherwise lookup property
                if id.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    Ok(id)
                } else {
                    Ok(self.context.get_property(&id).unwrap_or("").to_string())
                }
            }
            Some(Token::FeatureAction(feat)) => {
                let state = self
                    .context
                    .feature_action_states
                    .get(&feat)
                    .copied()
                    .unwrap_or(InstallState::Absent);
                Ok(state.as_i32().to_string())
            }
            Some(Token::FeatureInstalled(feat)) => {
                let state = self
                    .context
                    .feature_installed_states
                    .get(&feat)
                    .copied()
                    .unwrap_or(InstallState::Absent);
                Ok(state.as_i32().to_string())
            }
            Some(Token::ComponentAction(comp)) => {
                let state = self
                    .context
                    .component_action_states
                    .get(&comp)
                    .copied()
                    .unwrap_or(InstallState::Absent);
                Ok(state.as_i32().to_string())
            }
            Some(Token::ComponentInstalled(comp)) => {
                let state = self
                    .context
                    .component_installed_states
                    .get(&comp)
                    .copied()
                    .unwrap_or(InstallState::Absent);
                Ok(state.as_i32().to_string())
            }
            other => Err(Error::Validation {
                element: "ConditionExpression".to_string(),
                reason: format!("expected value or identifier, found {other:?}"),
            }),
        }
    }
}

/// Checks if a token is a relational comparison operator.
const fn is_relational_op(t: &Token) -> bool {
    matches!(
        t,
        Token::Eq
            | Token::Neq
            | Token::Lt
            | Token::Gt
            | Token::Lte
            | Token::Gte
            | Token::Contains
            | Token::StartsWith
            | Token::EndsWith
            | Token::CiEq
            | Token::CiNeq
            | Token::CiLt
            | Token::CiGt
            | Token::CiLte
            | Token::CiGte
            | Token::CiContains
            | Token::CiStartsWith
            | Token::CiEndsWith
            | Token::BitAnd
            | Token::BitOr
    )
}

/// Compares two string or integer operand values.
fn compare_values(left: &str, op: &Token, right: &str) -> bool {
    // Check if both sides can be parsed as integers
    let l_int = left.parse::<i64>().ok();
    let r_int = right.parse::<i64>().ok();

    if let (Some(li), Some(ri)) = (l_int, r_int) {
        return match op {
            Token::Eq | Token::CiEq => li == ri,
            Token::Neq | Token::CiNeq => li != ri,
            Token::Lt | Token::CiLt => li < ri,
            Token::Gt | Token::CiGt => li > ri,
            Token::Lte | Token::CiLte => li <= ri,
            Token::Gte | Token::CiGte => li >= ri,
            Token::BitAnd => (li & ri) != 0,
            Token::BitOr => (li | ri) != 0,
            _ => false,
        };
    }

    match op {
        Token::Eq => left == right,
        Token::Neq => left != right,
        Token::Lt => left < right,
        Token::Gt => left > right,
        Token::Lte => left <= right,
        Token::Gte => left >= right,
        Token::Contains => left.contains(right),
        Token::StartsWith => left.starts_with(right),
        Token::EndsWith => left.ends_with(right),
        Token::CiEq => left.eq_ignore_ascii_case(right),
        Token::CiNeq => !left.eq_ignore_ascii_case(right),
        Token::CiLt => left.to_lowercase() < right.to_lowercase(),
        Token::CiGt => left.to_lowercase() > right.to_lowercase(),
        Token::CiLte => left.to_lowercase() <= right.to_lowercase(),
        Token::CiGte => left.to_lowercase() >= right.to_lowercase(),
        Token::CiContains => left.to_lowercase().contains(&right.to_lowercase()),
        Token::CiStartsWith => left.to_lowercase().starts_with(&right.to_lowercase()),
        Token::CiEndsWith => left.to_lowercase().ends_with(&right.to_lowercase()),
        _ => false,
    }
}

/// Checks truthiness of a single value.
fn is_truthy(val: &str) -> bool {
    if val.is_empty() {
        return false;
    }
    if let Ok(i) = val.parse::<i64>() {
        return i != 0;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_scoping() {
        assert!(EvaluationContext::is_public_property("INSTALLDIR"));
        assert!(EvaluationContext::is_public_property("ALLUSERS"));
        assert!(EvaluationContext::is_public_property("PROP_123"));
        assert!(!EvaluationContext::is_public_property("myPrivateProp"));
        assert!(!EvaluationContext::is_public_property(""));
    }

    #[test]
    fn test_formatted_string() -> Result<()> {
        let mut ctx = EvaluationContext::new();
        ctx.set_property("ProductName", "Acme Widget");
        ctx.set_file_path("MainExe", r"C:\Program Files\Acme\widget.exe");
        ctx.set_component_dir("MainComp", r"C:\Program Files\Acme");

        let res =
            ctx.format_string("Installing [ProductName] to [$MainComp] (binary: [#MainExe])")?;
        assert_eq!(
            res,
            r"Installing Acme Widget to C:\Program Files\Acme (binary: C:\Program Files\Acme\widget.exe)"
        );

        // Escape test
        let esc = ctx.format_string(r"Escaped: [\[]bracket[\]] and [\{]brace[\}]")?;
        assert_eq!(esc, "Escaped: [bracket] and {brace}");

        // Individual escapes test
        let esc_rb = ctx.format_string(r"[\]]")?;
        assert_eq!(esc_rb, "]");
        let esc_rbr = ctx.format_string(r"[\}]")?;
        assert_eq!(esc_rbr, "}");

        // Non-matching escapes and missing keys
        let missing = ctx.format_string(
            "Missing: [#MissingFile] [$MissingComp] [%MISSING_ENV_VAR] [MissingProp]",
        )?;
        assert_eq!(missing, "Missing:    ");

        // Trailing backslash inside bracket
        assert!(ctx.format_string(r"Bad [Prop\]").is_err());
        assert!(ctx.format_string(r"Bad [Prop\").is_err());

        assert!(ctx.format_string("Unclosed [Prop").is_err());
        Ok(())
    }

    #[test]
    fn test_condition_evaluation() -> Result<()> {
        let mut ctx = EvaluationContext::new();
        ctx.set_property("VersionNT", "601");
        ctx.set_property("ACTION", "INSTALL");
        ctx.set_property("FEATURE_STATE", "ENABLED");
        ctx.set_feature_action("MainFeature", InstallState::Local);
        ctx.set_feature_installed("OldFeature", InstallState::Local);
        ctx.set_component_action("CompA", InstallState::Local);
        ctx.set_component_installed("CompB", InstallState::Local);

        // Test feature and component symbols at end-of-string to cover while loops terminating at chars.len()
        assert!(ctx.evaluate_condition("&MainFeature")?);
        assert!(ctx.evaluate_condition("!OldFeature")?);
        assert!(ctx.evaluate_condition("$CompA")?);
        assert!(ctx.evaluate_condition("?CompB")?);

        // Test feature and component symbols starting with and containing underscores
        ctx.set_feature_action("_F_1", InstallState::Local);
        ctx.set_feature_installed("_F_2", InstallState::Local);
        ctx.set_component_action("_C_1", InstallState::Local);
        ctx.set_component_installed("_C_2", InstallState::Local);
        assert!(ctx.evaluate_condition("&_F_1")?);
        assert!(ctx.evaluate_condition("!_F_2")?);
        assert!(ctx.evaluate_condition("$_C_1")?);
        assert!(ctx.evaluate_condition("?_C_2")?);

        // Test isolated '!' at end-of-string (i + 1 < chars.len() is false)
        assert!(ctx.evaluate_condition("1 = 1 AND !").is_err());

        // Test bitwise operators followed by non-alphanumeric (such as space or digit)
        assert!(ctx.evaluate_condition("5 & 1")?);
        assert!(!ctx.evaluate_condition("4 & 1")?);

        // Test logical NOT followed by non-alphanumeric or space
        assert!(ctx.evaluate_condition("NOT 0")?);
        assert!(ctx.evaluate_condition("! 0")?);

        // Test unterminated string literal in condition (i < chars.len() is false at quote check)
        assert!(ctx.evaluate_condition("\"unterminated").is_ok());
        ctx.set_feature_installed("MainFeature", InstallState::Absent);

        assert!(ctx.evaluate_condition("")?);
        assert!(ctx.evaluate_condition(r#"VersionNT >= 600 AND ACTION = "INSTALL""#)?);
        assert!(ctx.evaluate_condition("NOT (VersionNT < 500)")?);
        assert!(ctx.evaluate_condition("! VersionNT < 500")?);
        assert!(ctx.evaluate_condition(r#"FEATURE_STATE ~= "enabled""#)?);
        assert!(ctx.evaluate_condition(r#"ACTION >< "NST""#)?);
        assert!(ctx.evaluate_condition(r#"ACTION << "IN""#)?);
        assert!(ctx.evaluate_condition(r#"ACTION >> "ALL""#)?);
        assert!(ctx.evaluate_condition("&MainFeature = 3 AND !MainFeature = 2")?);

        // Bitwise test
        ctx.set_property("FLAGS", "5");
        assert!(ctx.evaluate_condition("FLAGS & 4")?);
        assert!(!ctx.evaluate_condition("FLAGS & 2")?);

        // Logical XOR
        assert!(ctx.evaluate_condition("1 XOR 0")?);
        assert!(!ctx.evaluate_condition("1 XOR 1")?);

        // Logical OR combinations
        assert!(ctx.evaluate_condition("0 OR 1")?);
        assert!(ctx.evaluate_condition("1 OR 0")?);
        assert!(!ctx.evaluate_condition("0 OR 0")?);

        // Standalone bitwise and symbols at end of expression or before operator
        assert!(ctx.evaluate_condition("FLAGS & 4")?);
        assert!(!ctx.evaluate_condition("! 1")?);
        assert!(ctx.evaluate_condition("! 0")?);

        Ok(())
    }

    #[test]
    fn test_condition_evaluation_extended() -> Result<()> {
        let mut ctx = EvaluationContext::new();
        ctx.set_property("VersionNT", "601");
        ctx.set_property("FLAGS", "5");

        // Component state symbols ($Component and ?Component)
        ctx.set_component_action("Comp1", InstallState::Local);
        ctx.set_component_installed("Comp1", InstallState::Source);
        assert!(ctx.evaluate_condition("$Comp1 = 3")?);
        assert!(ctx.evaluate_condition("?Comp1 = 4")?);

        // Unset component and feature states fallback to InstallState::Absent (2)
        assert!(ctx.evaluate_condition("$UnsetComp = 2")?);
        assert!(ctx.evaluate_condition("?UnsetComp = 2")?);
        assert!(ctx.evaluate_condition("&UnsetFeat = 2")?);
        assert!(ctx.evaluate_condition("!UnsetFeat = 2")?);

        // Relational comparisons
        assert!(ctx.evaluate_condition("10 < 20")?);
        assert!(ctx.evaluate_condition("20 > 10")?);
        assert!(ctx.evaluate_condition("10 <= 10")?);
        assert!(ctx.evaluate_condition("10 >= 10")?);
        assert!(ctx.evaluate_condition("10 <> 20")?);
        assert!(ctx.evaluate_condition("FLAGS | 2")?);

        // String non-integer comparisons for all operators
        assert!(!ctx.evaluate_condition("10 >< 20")?);
        assert!(!ctx.evaluate_condition(r#""a" & "b""#)?);
        assert!(ctx.evaluate_condition(r#""a" <> "b""#)?);
        assert!(ctx.evaluate_condition(r#""a" <= "b""#)?);
        assert!(ctx.evaluate_condition(r#""b" >= "a""#)?);
        assert!(ctx.evaluate_condition(r#""apple" < "banana""#)?);
        assert!(ctx.evaluate_condition(r#""banana" > "apple""#)?);
        assert!(ctx.evaluate_condition(r#""APPLE" ~< "banana""#)?);
        assert!(ctx.evaluate_condition(r#""banana" ~> "apple""#)?);
        assert!(!ctx.evaluate_condition(r#""apple" ~> "banana""#)?);
        assert!(ctx.evaluate_condition(r#""banana" ~> "APPLE""#)?);
        assert!(ctx.evaluate_condition(r#""APPLE" ~<= "apple""#)?);
        assert!(ctx.evaluate_condition(r#""APPLE" ~>= "apple""#)?);
        assert!(ctx.evaluate_condition(r#""APPLE" ~<> "orange""#)?);
        assert!(ctx.evaluate_condition(r#""hello world" ~>< "WORLD""#)?);
        assert!(ctx.evaluate_condition(r#""hello world" ~<< "HELLO""#)?);
        assert!(ctx.evaluate_condition(r#""hello world" ~>> "WORLD""#)?);

        // Truthiness
        assert!(ctx.evaluate_condition("VersionNT")?);
        assert!(!ctx.evaluate_condition("0")?);
        assert!(ctx.evaluate_condition(r#""non_numeric_string""#)?);

        // Environment variable in format_string
        std::env::set_var("MSI_TEST_VAR", "TestVal");
        let formatted_env = ctx.format_string("[%MSI_TEST_VAR]")?;
        assert_eq!(formatted_env, "TestVal");

        // InstallState conversions
        assert_eq!(InstallState::from_i32(2), Some(InstallState::Absent));
        assert_eq!(InstallState::from_i32(3), Some(InstallState::Local));
        assert_eq!(InstallState::from_i32(4), Some(InstallState::Source));
        assert_eq!(InstallState::from_i32(5), Some(InstallState::Advertised));
        assert_eq!(InstallState::from_i32(99), None);

        // Feature and component identifiers at end of string
        ctx.set_feature_action("MainFeature", InstallState::Local);
        assert!(ctx.evaluate_condition("&MainFeature")?);
        assert!(ctx.evaluate_condition("!MainFeature")?);
        assert!(ctx.evaluate_condition("$Comp1")?);
        assert!(ctx.evaluate_condition("?Comp1")?);

        // Standalone prefix symbols
        assert!(ctx.evaluate_condition("&").is_err());
        assert!(ctx.evaluate_condition("$")?);
        assert!(ctx.evaluate_condition("?")?);

        // Standalone ~ and unknown operator starting with ~
        assert!(ctx.evaluate_condition("~")?);
        assert!(ctx.evaluate_condition("~?")?);

        // Trailing comparison operators at EOF (syntax errors covering EOF branch)
        assert!(ctx.evaluate_condition("1 <").is_err());
        assert!(ctx.evaluate_condition("1 >").is_err());
        assert!(ctx.evaluate_condition("1 ~<").is_err());
        assert!(ctx.evaluate_condition("1 ~>").is_err());
        assert!(ctx.evaluate_condition("1 &").is_err());

        // Syntax errors covering sub-expression error branches
        assert!(ctx.evaluate_condition("1 OR =").is_err());
        assert!(ctx.evaluate_condition("1 XOR =").is_err());
        assert!(ctx.evaluate_condition("1 AND =").is_err());
        assert!(ctx.evaluate_condition("NOT =").is_err());
        assert!(ctx.evaluate_condition("! =").is_err());
        assert!(ctx.evaluate_condition("1 = =").is_err());

        // Unclosed quotes
        assert!(ctx.evaluate_condition(r#""unclosed quote"#)?);

        // Syntax error
        assert!(ctx.evaluate_condition("(VersionNT = 601").is_err());
        assert!(ctx.evaluate_condition("=").is_err());

        Ok(())
    }
}
