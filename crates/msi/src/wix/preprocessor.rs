//! `WiX` source code preprocessor.
//!
//! Implements macro expansions, variable scoping stack, conditional blocks,
//! loop unrolling (`foreach`), and file inclusions per the `WiX` Preprocessor specification.

use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// System variable values for preprocessor macro resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemVariables {
    /// Working directory (`$(sys.CURRENTDIR)`).
    pub current_dir: PathBuf,
    /// Source file directory (`$(sys.SOURCEFILEDIR)`).
    pub source_file_dir: PathBuf,
    /// Full source file path (`$(sys.SOURCEFILEPATH)`).
    pub source_file_path: PathBuf,
    /// Target build architecture (`$(sys.BUILDARCH)`).
    pub build_arch: String,
}

impl Default for SystemVariables {
    fn default() -> Self {
        Self {
            current_dir: PathBuf::from("."),
            source_file_dir: PathBuf::from("."),
            source_file_path: PathBuf::from("main.wxs"),
            build_arch: {
                #[cfg(target_arch = "aarch64")]
                {
                    "arm64".to_string()
                }
                #[cfg(target_arch = "x86")]
                {
                    "x86".to_string()
                }
                #[cfg(not(any(target_arch = "aarch64", target_arch = "x86")))]
                {
                    "x64".to_string()
                }
            },
        }
    }
}

/// Variable context stack supporting scoped definitions and environment variables.
#[derive(Debug, Clone)]
pub struct PreprocessorContext {
    /// Stack of variable scopes (top of stack is deepest scope).
    scopes: Vec<HashMap<String, String>>,
    /// Custom environment variable overrides (for deterministic processing / testing).
    env_overrides: HashMap<String, String>,
    /// Search paths for `<?include?>` resolution.
    include_paths: Vec<PathBuf>,
    /// Set of already included canonical file paths (for include-once semantics).
    included_files: HashSet<PathBuf>,
    /// System variables.
    sys_vars: SystemVariables,
}

impl PreprocessorContext {
    /// Creates a new [`PreprocessorContext`] with default system variables.
    ///
    /// # Returns
    ///
    /// A new initialized context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            env_overrides: HashMap::new(),
            include_paths: Vec::new(),
            included_files: HashSet::new(),
            sys_vars: SystemVariables::default(),
        }
    }

    /// Sets the system variables for this context.
    ///
    /// # Arguments
    ///
    /// * `sys_vars` - System variables.
    pub fn set_system_variables(&mut self, sys_vars: SystemVariables) {
        self.sys_vars = sys_vars;
    }

    /// Adds an include search directory.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory to search when resolving `<?include ?>`.
    pub fn add_include_path(&mut self, path: impl Into<PathBuf>) {
        self.include_paths.push(path.into());
    }

    /// Overrides an environment variable in this context.
    ///
    /// # Arguments
    ///
    /// * `key` - Environment variable name.
    /// * `value` - Environment variable value.
    pub fn set_env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.env_overrides.insert(key.into(), value.into());
    }

    /// Defines a preprocessor variable in the current (innermost) scope.
    ///
    /// # Arguments
    ///
    /// * `name` - Variable name.
    /// * `value` - Variable string value.
    pub fn define_var(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let idx = self.scopes.len().saturating_sub(1);
        self.scopes[idx].insert(name.into(), value.into());
    }

    /// Parses and defines a variable from a definition string (e.g. `VAR=VALUE` or `VAR`).
    ///
    /// # Arguments
    ///
    /// * `def` - Definition expression.
    pub fn parse_define(&mut self, def: &str) {
        if let Some((name, val)) = def.split_once('=') {
            self.define_var(name.trim(), val.trim().trim_matches('"'));
        } else {
            self.define_var(def.trim(), "1");
        }
    }

    /// Pushes a new variable scope onto the stack.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pops the innermost variable scope from the stack.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            let _ = self.scopes.pop();
        }
    }

    /// Resolves a preprocessor variable (`$(var.NAME)`).
    ///
    /// Checks scopes from innermost to outermost.
    ///
    /// # Arguments
    ///
    /// * `name` - Variable name.
    ///
    /// # Returns
    ///
    /// Optional reference to variable value.
    #[must_use]
    pub fn get_var(&self, name: &str) -> Option<&str> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.as_str());
            }
        }
        None
    }

    /// Checks if a preprocessor variable is defined.
    ///
    /// # Arguments
    ///
    /// * `name` - Variable name.
    ///
    /// # Returns
    ///
    /// `true` if defined, `false` otherwise.
    #[must_use]
    pub fn is_defined(&self, name: &str) -> bool {
        self.get_var(name).is_some()
    }

    /// Resolves an environment variable (`$(env.VAR)`).
    ///
    /// # Arguments
    ///
    /// * `name` - Environment variable name.
    ///
    /// # Returns
    ///
    /// Value string if set.
    #[must_use]
    pub fn get_env(&self, name: &str) -> Option<String> {
        if let Some(val) = self.env_overrides.get(name) {
            return Some(val.clone());
        }
        std::env::var(name).ok()
    }
}

impl Default for PreprocessorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// `WiX` Preprocessor engine.
#[derive(Debug, Default)]
pub struct Preprocessor;

impl Preprocessor {
    /// Creates a new [`Preprocessor`].
    ///
    /// # Returns
    ///
    /// A new preprocessor.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Preprocesses the given `WiX` source text with the provided context.
    ///
    /// Executes:
    /// 1. Include file expansion (`<?include ?>`).
    /// 2. Foreach loops (`<?foreach ?> ... <?endforeach?>`).
    /// 3. Conditional directives (`<?if ?>`, `<?ifdef ?>`, `<?ifndef ?>`, `<?elseif ?>`, `<?else?>`, `<?endif?>`).
    /// 4. Variable and macro expansion (`$(var.X)`, `$(env.X)`, `$(sys.X)`).
    ///
    /// # Arguments
    ///
    /// * `source` - Raw `WiX` source string.
    /// * `ctx` - Preprocessor context.
    ///
    /// # Returns
    ///
    /// Fully preprocessed `WiX` XML source string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Preprocessor`] if any directive syntax or macro expansion fails.
    pub fn process(&self, source: &str, ctx: &mut PreprocessorContext) -> Result<String> {
        let lines: Vec<&str> = source.lines().collect();
        let (processed_lines, _) = self.process_lines(&lines, 0, ctx, true)?;
        Ok(processed_lines.join("\n"))
    }

    /// Recursively processes a slice of source lines starting at `start_idx`.
    ///
    /// Returns `(output_lines, next_line_index)`.
    #[allow(clippy::too_many_lines)]
    fn process_lines(
        &self,
        lines: &[&str],
        mut idx: usize,
        ctx: &mut PreprocessorContext,
        is_active: bool,
    ) -> Result<(Vec<String>, usize)> {
        let mut out = Vec::new();

        while idx < lines.len() {
            let line = lines[idx];
            let trimmed = line.trim();

            if trimmed.starts_with("<?if ") {
                let expr = trimmed
                    .trim_start_matches("<?if ")
                    .trim_end_matches("?>")
                    .trim();
                let cond_val = is_active && self.eval_expression(expr, ctx)?;
                let mut condition_met = cond_val;

                idx += 1;
                let (branch_lines, next_idx) = self.process_lines(lines, idx, ctx, cond_val)?;
                if cond_val {
                    out.extend(branch_lines);
                }
                idx = next_idx;

                // Check for <?elseif?> or <?else?>
                while idx < lines.len() {
                    let next_trimmed = lines[idx].trim();
                    if next_trimmed.starts_with("<?elseif ") {
                        let next_expr = next_trimmed
                            .trim_start_matches("<?elseif ")
                            .trim_end_matches("?>")
                            .trim();
                        let branch_active =
                            is_active && !condition_met && self.eval_expression(next_expr, ctx)?;
                        if branch_active {
                            condition_met = true;
                        }
                        idx += 1;
                        let (elif_lines, after_elif) =
                            self.process_lines(lines, idx, ctx, branch_active)?;
                        if branch_active {
                            out.extend(elif_lines);
                        }
                        idx = after_elif;
                    } else if next_trimmed == "<?else?>" || next_trimmed == "<?else ?>" {
                        let else_active = is_active && !condition_met;
                        idx += 1;
                        let (else_lines, after_else) =
                            self.process_lines(lines, idx, ctx, else_active)?;
                        if else_active {
                            out.extend(else_lines);
                        }
                        idx = after_else;
                    } else {
                        break;
                    }
                }

                if idx < lines.len() {
                    idx += 1;
                }
            } else if trimmed.starts_with("<?ifdef ") {
                let var_name = trimmed
                    .trim_start_matches("<?ifdef ")
                    .trim_end_matches("?>")
                    .trim();
                let cond_val = is_active && ctx.is_defined(var_name);
                idx += 1;
                let (branch_lines, next_idx) = self.process_lines(lines, idx, ctx, cond_val)?;
                if cond_val {
                    out.extend(branch_lines);
                }
                idx = next_idx;
                if idx < lines.len() {
                    idx += 1;
                }
            } else if trimmed.starts_with("<?ifndef ") {
                let var_name = trimmed
                    .trim_start_matches("<?ifndef ")
                    .trim_end_matches("?>")
                    .trim();
                let cond_val = is_active && !ctx.is_defined(var_name);
                idx += 1;
                let (branch_lines, next_idx) = self.process_lines(lines, idx, ctx, cond_val)?;
                if cond_val {
                    out.extend(branch_lines);
                }
                idx = next_idx;
                if idx < lines.len() {
                    idx += 1;
                }
            } else if trimmed.starts_with("<?elseif ")
                || trimmed == "<?else?>"
                || trimmed == "<?else ?>"
                || trimmed == "<?endif?>"
                || trimmed == "<?endif ?>"
            {
                // Reached boundary of current conditional block
                return Ok((out, idx));
            } else if trimmed.starts_with("<?foreach ") {
                // <?foreach <var> in <val1;val2;val3> ?>
                let body = trimmed
                    .trim_start_matches("<?foreach ")
                    .trim_end_matches("?>")
                    .trim();
                let parts: Vec<&str> = body.split(" in ").collect();
                if parts.len() != 2 {
                    return Err(Error::Preprocessor {
                        line: idx + 1,
                        column: 1,
                        message: format!("invalid foreach syntax: '{trimmed}'"),
                    });
                }
                let loop_var = parts[0].trim();
                let raw_items = parts[1].trim();
                let items: Vec<&str> = raw_items
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();

                // Collect lines up to <?endforeach?>
                idx += 1;
                let mut loop_body_lines = Vec::new();
                let mut depth = 1;
                while idx < lines.len() {
                    let cur = lines[idx].trim();
                    if cur.starts_with("<?foreach ") {
                        depth += 1;
                    } else if cur == "<?endforeach?>" || cur == "<?endforeach ?>" {
                        depth -= 1;
                        if depth == 0 {
                            idx += 1;
                            break;
                        }
                    }
                    loop_body_lines.push(lines[idx]);
                    idx += 1;
                }

                if is_active {
                    for item in items {
                        ctx.push_scope();
                        ctx.define_var(loop_var, item);
                        let (expanded, _) = self.process_lines(&loop_body_lines, 0, ctx, true)?;
                        out.extend(expanded);
                        ctx.pop_scope();
                    }
                }
            } else if trimmed.starts_with("<?include ") {
                if is_active {
                    let raw_path = trimmed
                        .trim_start_matches("<?include ")
                        .trim_end_matches("?>")
                        .trim();
                    let inc_content = self.resolve_include(raw_path, ctx, idx + 1)?;
                    let sub_lines: Vec<&str> = inc_content.lines().collect();
                    let (sub_processed, _) = self.process_lines(&sub_lines, 0, ctx, true)?;
                    out.extend(sub_processed);
                }
                idx += 1;
            } else if trimmed.starts_with("<?define ") {
                if is_active {
                    let def_body = trimmed
                        .trim_start_matches("<?define ")
                        .trim_end_matches("?>")
                        .trim();
                    let parts: Vec<&str> = def_body.splitn(2, '=').collect();
                    let var_name = parts[0].trim();
                    let var_val = if parts.len() > 1 {
                        parts[1].trim().trim_matches('"')
                    } else {
                        "1"
                    };
                    ctx.define_var(var_name, var_val);
                }
                idx += 1;
            } else if trimmed.starts_with("<?undef ") {
                if is_active {
                    let var_name = trimmed
                        .trim_start_matches("<?undef ")
                        .trim_end_matches("?>")
                        .trim();
                    let idx_scope = ctx.scopes.len().saturating_sub(1);
                    ctx.scopes[idx_scope].remove(var_name);
                }
                idx += 1;
            } else if trimmed.starts_with("<?error ") {
                if is_active {
                    let err_msg = trimmed
                        .trim_start_matches("<?error ")
                        .trim_end_matches("?>")
                        .trim();
                    let expanded_msg = self.expand_macros(err_msg, ctx)?;
                    return Err(Error::Preprocessor {
                        line: idx + 1,
                        column: 1,
                        message: expanded_msg,
                    });
                }
                idx += 1;
            } else if trimmed.starts_with("<?warning ") || trimmed.starts_with("<?pragma ") {
                // Warning and pragma directives are logged/recorded without interrupting execution
                idx += 1;
            } else {
                if is_active {
                    let expanded = self.expand_macros(line, ctx)?;
                    out.push(expanded);
                }
                idx += 1;
            }
        }

        Ok((out, idx))
    }

    /// Resolves and reads an included file path.
    #[allow(clippy::unused_self)]
    fn resolve_include(
        &self,
        raw_path: &str,
        ctx: &mut PreprocessorContext,
        line: usize,
    ) -> Result<String> {
        let clean_path = raw_path.trim_matches('"').trim();
        let target_path = Path::new(clean_path);

        // 1. Direct path relative to source directory or current directory
        let mut candidates = Vec::new();
        candidates.push(ctx.sys_vars.source_file_dir.join(target_path));
        candidates.push(ctx.sys_vars.current_dir.join(target_path));
        for inc in &ctx.include_paths {
            candidates.push(inc.join(target_path));
        }

        let mut resolved = None;
        for c in candidates {
            if c.exists() && c.is_file() {
                resolved = Some(c);
                break;
            }
        }

        let Some(file_path) = resolved else {
            return Err(Error::Preprocessor {
                line,
                column: 1,
                message: format!("cannot resolve include file '{clean_path}'"),
            });
        };

        // Canonicalize for include-once semantics
        let canonical = file_path.canonicalize().unwrap_or(file_path);

        if ctx.included_files.contains(&canonical) {
            // Already included, skip
            return Ok(String::new());
        }

        ctx.included_files.insert(canonical.clone());

        std::fs::read_to_string(&canonical).map_err(|e| Error::Preprocessor {
            line,
            column: 1,
            message: format!("failed to read include file '{}': {e}", canonical.display()),
        })
    }

    /// Compares two operand strings with the specified comparison operator.
    fn eval_comparison(left: &str, op: &str, right: &str) -> bool {
        let l_clean = left.trim().trim_matches('"');
        let r_clean = right.trim().trim_matches('"');

        match op {
            "==" | "=" => l_clean == r_clean,
            "!=" => l_clean != r_clean,
            "~=" => l_clean.eq_ignore_ascii_case(r_clean),
            "<=" | ">=" | "<" | ">" => {
                if let (Ok(ver_l), Ok(ver_r)) = (
                    crate::package::ProductVersion::parse(l_clean),
                    crate::package::ProductVersion::parse(r_clean),
                ) {
                    match op {
                        "<=" => ver_l <= ver_r,
                        ">=" => ver_l >= ver_r,
                        "<" => ver_l < ver_r,
                        _ => ver_l > ver_r,
                    }
                } else if let (Ok(num_l), Ok(num_r)) =
                    (l_clean.parse::<i64>(), r_clean.parse::<i64>())
                {
                    match op {
                        "<=" => num_l <= num_r,
                        ">=" => num_l >= num_r,
                        "<" => num_l < num_r,
                        _ => num_l > num_r,
                    }
                } else {
                    match op {
                        "<=" => l_clean <= r_clean,
                        ">=" => l_clean >= r_clean,
                        "<" => l_clean < r_clean,
                        _ => l_clean > r_clean,
                    }
                }
            }
            _ => false,
        }
    }

    /// Evaluates a preprocessor conditional expression.
    fn eval_expression(&self, expr: &str, ctx: &PreprocessorContext) -> Result<bool> {
        let expanded = self.expand_macros(expr, ctx)?;
        let trimmed = expanded.trim();

        // Handle logical OR: " or " or " || "
        if let Some((left, right)) = trimmed
            .split_once(" or ")
            .or_else(|| trimmed.split_once(" || "))
        {
            return Ok(self.eval_expression(left, ctx)? || self.eval_expression(right, ctx)?);
        }

        // Handle logical AND: " and " or " && "
        if let Some((left, right)) = trimmed
            .split_once(" and ")
            .or_else(|| trimmed.split_once(" && "))
        {
            return Ok(self.eval_expression(left, ctx)? && self.eval_expression(right, ctx)?);
        }

        // Handle logical NOT: "not " or "!"
        if let Some(rest) = trimmed.strip_prefix("not ") {
            return Ok(!self.eval_expression(rest, ctx)?);
        }
        if let Some(rest) = trimmed.strip_prefix('!') {
            return Ok(!self.eval_expression(rest, ctx)?);
        }

        // Handle comparison operators in order of precedence:
        // "!=", "==", "~=", "<=", ">=", "=", "<", ">"
        if let Some((left, right)) = trimmed.split_once("!=") {
            return Ok(Self::eval_comparison(left, "!=", right));
        }
        if let Some((left, right)) = trimmed.split_once("==") {
            return Ok(Self::eval_comparison(left, "==", right));
        }
        if let Some((left, right)) = trimmed.split_once("~=") {
            return Ok(Self::eval_comparison(left, "~=", right));
        }
        if let Some((left, right)) = trimmed.split_once("<=") {
            return Ok(Self::eval_comparison(left, "<=", right));
        }
        if let Some((left, right)) = trimmed.split_once(">=") {
            return Ok(Self::eval_comparison(left, ">=", right));
        }
        if let Some((left, right)) = trimmed.split_once('=') {
            return Ok(Self::eval_comparison(left, "=", right));
        }
        if let Some((left, right)) = trimmed.split_once('<') {
            return Ok(Self::eval_comparison(left, "<", right));
        }
        if let Some((left, right)) = trimmed.split_once('>') {
            return Ok(Self::eval_comparison(left, ">", right));
        }

        // Truthiness: non-empty, non-zero, not "false"
        match trimmed {
            "true" | "1" => Ok(true),
            "false" | "0" | "" => Ok(false),
            _ => Ok(!trimmed.is_empty()),
        }
    }

    /// Expands preprocessor macros (`$(...)`) in the input string.
    ///
    /// Supports:
    /// - `$(var.NAME)`
    /// - `$(env.VAR)`
    /// - `$(sys.CURRENTDIR)`
    /// - `$(sys.SOURCEFILEDIR)`
    /// - `$(sys.SOURCEFILEPATH)`
    ///
    /// # Arguments
    ///
    /// * `input` - Text containing preprocessor macros.
    /// * `ctx` - Preprocessor context.
    ///
    /// # Returns
    ///
    /// Expanded text string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Preprocessor`] if any macro variable cannot be resolved.
    pub fn expand_macros(&self, input: &str, ctx: &PreprocessorContext) -> Result<String> {
        let mut out = String::with_capacity(input.len());
        let mut cursor = 0;
        let bytes = input.as_bytes();

        while cursor < bytes.len() {
            if bytes[cursor] == b'$' && cursor + 1 < bytes.len() && bytes[cursor + 1] == b'(' {
                // Find matching closing parenthesis considering nested parentheses
                let mut depth = 1;
                let mut search_idx = cursor + 2;
                let mut found_close = None;
                while search_idx < bytes.len() {
                    if bytes[search_idx] == b'(' {
                        depth += 1;
                    } else if bytes[search_idx] == b')' {
                        depth -= 1;
                        if depth == 0 {
                            found_close = Some(search_idx);
                            break;
                        }
                    }
                    search_idx += 1;
                }

                if let Some(close_idx) = found_close {
                    let macro_content = &input[cursor + 2..close_idx];
                    let expanded_content = self.expand_macros(macro_content, ctx)?;
                    let replacement = self.resolve_macro(&expanded_content, ctx)?;
                    out.push_str(&replacement);
                    cursor = close_idx + 1;
                    continue;
                }
                return Err(Error::Preprocessor {
                    line: 1,
                    column: cursor + 1,
                    message: format!("unclosed macro expression in '{input}'"),
                });
            }
            out.push(bytes[cursor] as char);
            cursor += 1;
        }

        Ok(out)
    }

    /// Resolves advanced preprocessor functions (`$(fun.NAME(...))`).
    fn resolve_fun_macro(expr: &str) -> Result<String> {
        let Some(paren_open) = expr.find('(') else {
            return Err(Error::Preprocessor {
                line: 1,
                column: 1,
                message: format!("missing function argument list in '$(fun.{expr})'"),
            });
        };
        let Some(paren_close) = expr.rfind(')') else {
            return Err(Error::Preprocessor {
                line: 1,
                column: 1,
                message: format!("unclosed function call in '$(fun.{expr})'"),
            });
        };
        let fn_name = expr[..paren_open].trim();
        let args_str = &expr[paren_open + 1..paren_close];
        let raw_args: Vec<String> = if args_str.is_empty() {
            Vec::new()
        } else {
            args_str
                .split(',')
                .map(|a| a.trim().trim_matches('"').to_string())
                .collect()
        };

        match fn_name {
            "ToUpper" => {
                let s = raw_args.first().map_or("", String::as_str);
                Ok(s.to_uppercase())
            }
            "ToLower" => {
                let s = raw_args.first().map_or("", String::as_str);
                Ok(s.to_lowercase())
            }
            "SubString" => {
                let s = raw_args.first().map_or("", String::as_str);
                let start: usize = raw_args.get(1).and_then(|x| x.parse().ok()).unwrap_or(0);
                let len: usize = raw_args
                    .get(2)
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(s.len());
                let sub: String = s.chars().skip(start).take(len).collect();
                Ok(sub)
            }
            "FileExists" => {
                let path_str = raw_args.first().map_or("", String::as_str);
                let exists = Path::new(path_str).exists();
                Ok(if exists {
                    "1".to_string()
                } else {
                    "0".to_string()
                })
            }
            "FormatVersion" => {
                let ver_str = raw_args.first().map_or("1.0.0", String::as_str);
                Ok(ver_str.to_string())
            }
            "AutoGuid" => {
                let seed = raw_args.join("_");
                let seed_str = if seed.is_empty() { "Seed" } else { &seed };
                let guid =
                    crate::database::tables::types::ComponentGuid::generate("AutoGuid", seed_str);
                Ok(format!("{guid}"))
            }
            other => Err(Error::Preprocessor {
                line: 1,
                column: 1,
                message: format!("unknown preprocessor function '$(fun.{other})'"),
            }),
        }
    }

    /// Resolves an individual macro expression (e.g. `var.NAME`, `env.VAR`, `sys.CURRENTDIR`).
    #[allow(clippy::unused_self)]
    fn resolve_macro(&self, macro_expr: &str, ctx: &PreprocessorContext) -> Result<String> {
        if let Some(rest) = macro_expr.strip_prefix("fun.") {
            return Self::resolve_fun_macro(rest);
        }
        match macro_expr.split_once('.') {
            Some(("var", var_name)) => ctx.get_var(var_name).map_or_else(
                || {
                    Err(Error::Preprocessor {
                        line: 1,
                        column: 1,
                        message: format!("undefined variable '$(var.{var_name})'"),
                    })
                },
                |val| Ok(val.to_string()),
            ),
            Some(("env", env_name)) => ctx.get_env(env_name).map_or_else(
                || {
                    Err(Error::Preprocessor {
                        line: 1,
                        column: 1,
                        message: format!("undefined environment variable '$(env.{env_name})'"),
                    })
                },
                Ok,
            ),
            Some(("sys", "CURRENTDIR")) => {
                Ok(ctx.sys_vars.current_dir.to_string_lossy().to_string())
            }
            Some(("sys", "SOURCEFILEDIR")) => {
                Ok(ctx.sys_vars.source_file_dir.to_string_lossy().to_string())
            }
            Some(("sys", "SOURCEFILEPATH")) => {
                Ok(ctx.sys_vars.source_file_path.to_string_lossy().to_string())
            }
            Some(("sys", "BUILDARCH")) => Ok(ctx.sys_vars.build_arch.clone()),
            Some(("sys", other)) => Err(Error::Preprocessor {
                line: 1,
                column: 1,
                message: format!("unknown system variable '$(sys.{other})'"),
            }),
            _ => Err(Error::Preprocessor {
                line: 1,
                column: 1,
                message: format!("unrecognized macro syntax '$({macro_expr})'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_expansion() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("AppName", "SuperInstaller");
        ctx.set_env("MY_ENV_VAR", "Production");

        let prep = Preprocessor::new();
        let input = "Name: $(var.AppName), Env: $(env.MY_ENV_VAR), Dir: $(sys.CURRENTDIR)";
        let result = prep.expand_macros(input, &ctx)?;

        assert!(result.contains("Name: SuperInstaller"));
        assert!(result.contains("Env: Production"));
        assert!(result.contains("Dir: ."));

        // System source variables
        assert_eq!(prep.expand_macros("$(sys.SOURCEFILEDIR)", &ctx)?, ".");
        assert_eq!(
            prep.expand_macros("$(sys.SOURCEFILEPATH)", &ctx)?,
            "main.wxs"
        );

        // Dollar sign edge cases (not a macro)
        assert_eq!(prep.expand_macros("trailing$", &ctx)?, "trailing$");
        assert_eq!(prep.expand_macros("dollar$word", &ctx)?, "dollar$word");

        // Undefined variable error
        assert!(prep.expand_macros("$(var.MissingVar)", &ctx).is_err());
        assert!(prep.expand_macros("$(env.MissingEnv)", &ctx).is_err());
        assert!(prep.expand_macros("$(sys.UnknownSys)", &ctx).is_err());
        assert!(prep.expand_macros("$(badmacro)", &ctx).is_err());

        Ok(())
    }

    #[test]
    fn test_conditional_directives() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("Platform", "x64");
        ctx.define_var("Release", "1");

        let prep = Preprocessor::new();
        let source = r#"
<?if $(var.Platform) = "x86"?>
<Component Id="x86Comp" />
<?elseif $(var.Platform) = "x64"?>
<Component Id="x64Comp" />
<?else?>
<Component Id="OtherComp" />
<?endif?>
<?ifdef Release?>
<Optimization Level="Full" />
<?endif?>
<?ifndef Debug?>
<Debug Info="None" />
<?endif?>
"#;

        let result = prep.process(source, &mut ctx)?;
        assert!(!result.contains("x86Comp"));
        assert!(result.contains("x64Comp"));
        assert!(!result.contains("OtherComp"));
        assert!(result.contains("Optimization Level=\"Full\""));
        assert!(result.contains("Debug Info=\"None\""));

        Ok(())
    }

    #[test]
    fn test_foreach_loop() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        let prep = Preprocessor::new();

        let source = r#"
<?foreach LANG in en-US;fr-FR;de-DE?>
<Resource Language="$(var.LANG)" />
<?endforeach?>
"#;

        let result = prep.process(source, &mut ctx)?;
        assert!(result.contains("<Resource Language=\"en-US\" />"));
        assert!(result.contains("<Resource Language=\"fr-FR\" />"));
        assert!(result.contains("<Resource Language=\"de-DE\" />"));

        let source_spaced = r#"
<?foreach ITEM in a;b?>
<Item Value="$(var.ITEM)" />
<?endforeach ?>
"#;
        let res_spaced = prep.process(source_spaced, &mut ctx)?;
        assert!(res_spaced.contains("<Item Value=\"a\" />"));

        Ok(())
    }

    #[test]
    fn test_define_and_undef() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        let prep = Preprocessor::new();

        let source = r#"
<?define CustomVal="Alpha"?>
<?define BareFlag?>
<Value>$(var.CustomVal)_$(var.BareFlag)</Value>
<?undef CustomVal?>
<?ifndef CustomVal?>
<Unset>True</Unset>
<?endif?>
"#;

        let result = prep.process(source, &mut ctx)?;
        assert!(result.contains("<Value>Alpha_1</Value>"));
        assert!(result.contains("<Unset>True</Unset>"));

        Ok(())
    }

    #[test]
    fn test_expression_operators() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("Number", "500");
        let prep = Preprocessor::new();

        assert!(prep.eval_expression("$(var.Number) = 500", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) == 500", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) != 400", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) > 400", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) >= 500", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) < 600", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) <= 500", &ctx)?);
        assert!(prep.eval_expression("true", &ctx)?);
        assert!(!prep.eval_expression("false", &ctx)?);

        // Logical operators: and, or, not, &&, ||, !
        assert!(prep.eval_expression("$(var.Number) = 500 and 1 = 1", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) = 500 && 1 = 1", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) = 400 or 1 = 1", &ctx)?);
        assert!(prep.eval_expression("$(var.Number) = 400 || 1 = 1", &ctx)?);
        assert!(!prep.eval_expression("0 or 0", &ctx)?);
        assert!(prep.eval_expression("1 or 0", &ctx)?);
        assert!(!prep.eval_expression("0 and 1", &ctx)?);
        assert!(!prep.eval_expression("1 and 0", &ctx)?);
        assert!(prep.eval_expression("1 and 1", &ctx)?);
        assert!(prep.eval_expression("not $(var.Number) = 400", &ctx)?);
        assert!(prep.eval_expression("!false", &ctx)?);

        // Case-insensitive string comparison: ~=
        assert!(prep.eval_expression("\"Release\" ~= \"release\"", &ctx)?);
        assert!(!prep.eval_expression("\"Release\" ~= \"debug\"", &ctx)?);

        // Version comparisons
        assert!(prep.eval_expression("\"2.1.0\" >= \"2.0.0\"", &ctx)?);
        assert!(prep.eval_expression("\"1.0.0\" < \"2.0.0\"", &ctx)?);
        assert!(prep.eval_expression("\"3.2.1\" > \"3.2.0\"", &ctx)?);
        assert!(prep.eval_expression("\"1.5.0\" <= \"1.5.0\"", &ctx)?);

        Ok(())
    }

    #[test]
    fn test_advanced_preprocessor_functions() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("RawName", "superInstaller");

        let prep = Preprocessor::new();
        assert_eq!(
            prep.expand_macros("$(fun.ToUpper($(var.RawName)))", &ctx)?,
            "SUPERINSTALLER"
        );
        assert_eq!(
            prep.expand_macros("$(fun.ToLower(\"UPPER\"))", &ctx)?,
            "upper"
        );
        assert_eq!(
            prep.expand_macros("$(fun.SubString(\"abcdef\", 1, 3))", &ctx)?,
            "bcd"
        );
        assert_eq!(
            prep.expand_macros("$(fun.FormatVersion(\"2.0.1\", \"x.y.z\"))", &ctx)?,
            "2.0.1"
        );

        let guid = prep.expand_macros("$(fun.AutoGuid(\"Comp\", \"Dir\"))", &ctx)?;
        assert!(guid.starts_with('{'));
        assert!(guid.ends_with('}'));

        let temp_dir = std::env::temp_dir();
        let exists_test_file = temp_dir.join("fun_file_exists.txt");
        let _ = std::fs::write(&exists_test_file, "content");
        let exists_str = prep.expand_macros(
            &format!("$(fun.FileExists(\"{}\"))", exists_test_file.display()),
            &ctx,
        )?;
        assert_eq!(exists_str, "1");
        let not_exists_str =
            prep.expand_macros("$(fun.FileExists(\"/nonexistent/file/path\"))", &ctx)?;
        assert_eq!(not_exists_str, "0");
        let _ = std::fs::remove_file(exists_test_file);

        // Error cases for invalid function syntax
        assert!(prep.expand_macros("$(fun.NoParen)", &ctx).is_err());
        assert!(prep.expand_macros("$(fun.NoClose(123)", &ctx).is_err());
        assert!(Preprocessor::resolve_fun_macro("NoClose(abc").is_err());
        assert!(prep.expand_macros("$(fun.UnknownFunc(1))", &ctx).is_err());

        // sys.BUILDARCH
        let arch = prep.expand_macros("$(sys.BUILDARCH)", &ctx)?;
        assert_ne!(arch, "");

        Ok(())
    }

    #[test]
    fn test_pragmas_warnings_errors_and_parse_define() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.parse_define("DEF_KEY=DEF_VAL");
        ctx.parse_define("FLAG_KEY");
        assert_eq!(ctx.get_var("DEF_KEY"), Some("DEF_VAL"));
        assert_eq!(ctx.get_var("FLAG_KEY"), Some("1"));

        let prep = Preprocessor::new();
        let source = r#"
<?pragma warning disable 1024 ?>
<?warning Compilation is proceeding ?>
<Component Id="TestComp" />
"#;
        let processed = prep.process(source, &mut ctx)?;
        assert!(processed.contains("<Component Id=\"TestComp\" />"));

        let error_source = "
<?error Critical failure in build ?>
";
        assert!(prep.process(error_source, &mut ctx).is_err());

        Ok(())
    }

    #[test]
    fn test_context_scopes_and_includes() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("A", "1");
        assert_eq!(ctx.get_var("A"), Some("1"));

        ctx.push_scope();
        ctx.define_var("A", "2");
        assert_eq!(ctx.get_var("A"), Some("2"));

        ctx.pop_scope();
        assert_eq!(ctx.get_var("A"), Some("1"));
        // Pop when only root scope exists (scopes.len() == 1) exercises false branch
        ctx.pop_scope();
        assert_eq!(ctx.get_var("A"), Some("1"));

        ctx.add_include_path("/tmp/nonexistent");
        let prep = Preprocessor::new();
        // Including nonexistent file should error
        assert!(prep
            .process("<?include \"missing.wxi\"?>", &mut ctx)
            .is_err());

        // Test successful include file resolution and include-once semantics
        let temp_dir = std::env::temp_dir().join(format!("msi_inc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let inc_file = temp_dir.join("msi_test_include.wxi");
        std::fs::write(&inc_file, "<IncludedContent Value=\"1\" />\n")?;
        ctx.add_include_path(&temp_dir);

        let inc_source =
            "<?include \"msi_test_include.wxi\"?>\n<?include \"msi_test_include.wxi\"?>";
        let content = prep.process(inc_source, &mut ctx)?;
        let matches = content.matches("<IncludedContent").count();
        assert_eq!(matches, 1);

        // Include candidate that exists but is a directory (exercises is_file() false branch)
        let sub_dir = temp_dir.join("sub_dir_inc");
        let _ = std::fs::create_dir_all(&sub_dir);
        assert!(prep
            .process("<?include \"sub_dir_inc\"?>", &mut ctx)
            .is_err());

        // Unreadable include file test on unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unreadable = temp_dir.join("unreadable.wxi");
            std::fs::write(&unreadable, "content")?;
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))?;
            assert!(prep
                .process(
                    &format!("<?include \"{}\"?>", unreadable.display()),
                    &mut ctx
                )
                .is_err());
            std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o644))?;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    /// Tests `Preprocessor::default`, `PreprocessorContext::default`, `set_system_variables`, and `process`.
    #[test]
    fn test_preprocessor_default_and_set_system_variables() -> Result<()> {
        let mut def_ctx = PreprocessorContext::default();
        def_ctx.define_var("K", "V");
        assert_eq!(def_ctx.get_var("K"), Some("V"));

        let mut ctx = PreprocessorContext::new();
        let sys_vars = SystemVariables {
            current_dir: PathBuf::from("/tmp"),
            source_file_dir: PathBuf::from("/tmp"),
            source_file_path: PathBuf::from("/tmp/main.wxs"),
            build_arch: "x64".to_string(),
        };
        ctx.set_system_variables(sys_vars);

        let prep = Preprocessor;
        let res = prep.process("<Wix><Product/></Wix>", &mut ctx)?;
        assert!(res.contains("<Product/>"));
        Ok(())
    }

    /// Tests conditional directives: active else, inactive else, elseif false, ifdef missing, ifndef present, and unclosed blocks.
    #[test]
    fn test_preprocessor_conditionals_branches() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("DefinedVar", "yes");
        let prep = Preprocessor::new();

        // Active else branch with elseif false
        let source1 = "\n<?if 0?>\n<FalseIf/>\n<?elseif 0?>\n<FalseElif/>\n<?elseif 1?>\n<TrueElif/>\n<?else ?>\n<FalseElse/>\n<?endif ?>\n";
        let res1 = prep.process(source1, &mut ctx)?;
        assert!(!res1.contains("<FalseIf/>"));
        assert!(!res1.contains("<FalseElif/>"));
        assert!(res1.contains("<TrueElif/>"));
        assert!(!res1.contains("<FalseElse/>"));

        // True else branch
        let source2 = "\n<?if 0?>\n<FalseIf/>\n<?else?>\n<TrueElse/>\n<?endif?>\n";
        let res2 = prep.process(source2, &mut ctx)?;
        assert!(res2.contains("<TrueElse/>"));

        // ifdef with undefined var, ifndef with defined var
        let source3 = "\n<?ifdef UndefinedVar?>\n<FalseIfDef/>\n<?endif?>\n<?ifndef DefinedVar?>\n<FalseIfNDef/>\n<?endif?>\n";
        let res3 = prep.process(source3, &mut ctx)?;
        assert!(!res3.contains("<FalseIfDef/>"));
        assert!(!res3.contains("<FalseIfNDef/>"));

        // Incomplete / unclosed conditional blocks
        assert!(prep.process("<?if 1?> <Incomplete/>", &mut ctx).is_ok());
        assert!(prep
            .process("<?ifdef DefinedVar?> <Incomplete/>", &mut ctx)
            .is_ok());
        assert!(prep
            .process("<?ifndef UndefinedVar?> <Incomplete/>", &mut ctx)
            .is_ok());

        assert!(prep
            .process("<?if 1?> <Line1/>\n<Line2/>", &mut ctx)
            .is_ok());
        assert!(prep
            .process("<?ifdef DefinedVar?> <Line1/>\n<Line2/>", &mut ctx)
            .is_ok());
        assert!(prep
            .process("<?ifndef UndefinedVar?> <Line1/>\n<Line2/>", &mut ctx)
            .is_ok());

        Ok(())
    }

    /// Tests nested conditional blocks, inactive directives, and foreach loops.
    #[test]
    fn test_preprocessor_conditionals_nested_and_loops() -> Result<()> {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("DefinedVar", "yes");
        let prep = Preprocessor::new();

        // Inactive conditional block containing nested directives
        let source4 = "\n<?if 0?>\n<?define InactiveDef=\"val\"?>\n<?undef InactiveDef?>\n<?include \"ignored.wxi\"?>\n<?error Ignored error ?>\n<?foreach X in 1;2;3?>\n<IgnoredLoop/>\n<?endforeach?>\n<IgnoredContent/>\n<?endif?>\n";
        let res4 = prep.process(source4, &mut ctx)?;
        assert!(!res4.contains("<IgnoredContent/>"));

        // Invalid foreach syntax
        assert!(prep
            .process("<?foreach BAD_SYNTAX?>\n<?endforeach?>", &mut ctx)
            .is_err());

        // Nested foreach loops
        let nested_source = "\n<?foreach A in 1;2?>\n<?foreach B in x;y?>\n<Item Value=\"$(var.A)_$(var.B)\"/>\n<?endforeach?>\n<?endforeach?>\n";
        let res_nested = prep.process(nested_source, &mut ctx)?;
        assert!(res_nested.contains("<Item Value=\"1_x\"/>"));
        assert!(res_nested.contains("<Item Value=\"2_y\"/>"));

        // Inactive nested conditionals
        let inactive_nested = "\n<?if 0?>\n<?if 1?>\n<NestedA/>\n<?elseif 1?>\n<NestedB/>\n<?else?>\n<NestedC/>\n<?endif?>\n<?ifdef DefinedVar?>\n<NestedDef/>\n<?endif?>\n<?ifndef UndefinedVar?>\n<NestedNoDef/>\n<?endif?>\n<?endif?>\n";
        assert!(prep.process(inactive_nested, &mut ctx).is_ok());

        // condition_met true before elseif and else
        let condition_met_first = "\n<?if 1?>\n<TrueIfBranch/>\n<?elseif 1?>\n<IgnoredElif/>\n<?else?>\n<IgnoredElse/>\n<?endif?>\n";
        let cond_res = prep.process(condition_met_first, &mut ctx)?;
        assert!(cond_res.contains("<TrueIfBranch/>"));

        // Unclosed foreach loop
        let unclosed_foreach = "<?foreach ITEM in 1;2?>\n<Item Value=\"$(var.ITEM)\"/>";
        assert!(prep.process(unclosed_foreach, &mut ctx).is_ok());

        Ok(())
    }

    /// Tests extended expressions: numeric comparisons, string comparisons, truthiness, unknown op, and unclosed macros.
    #[test]
    fn test_preprocessor_expressions_extended() -> Result<()> {
        let ctx = PreprocessorContext::new();
        let prep = Preprocessor::new();

        // Integer comparisons
        assert!(prep.eval_expression("10 < 20", &ctx)?);
        assert!(prep.eval_expression("10 <= 10", &ctx)?);
        assert!(prep.eval_expression("20 > 10", &ctx)?);
        assert!(prep.eval_expression("20 >= 20", &ctx)?);

        // Fallback string comparisons (non-numeric, non-version)
        assert!(prep.eval_expression("\"apple\" < \"banana\"", &ctx)?);
        assert!(prep.eval_expression("\"apple\" <= \"banana\"", &ctx)?);
        assert!(prep.eval_expression("\"banana\" > \"apple\"", &ctx)?);
        assert!(prep.eval_expression("\"banana\" >= \"apple\"", &ctx)?);

        // Truthiness
        assert!(!prep.eval_expression("0", &ctx)?);
        assert!(!prep.eval_expression("", &ctx)?);
        assert!(prep.eval_expression("\"some_arbitrary_string\"", &ctx)?);

        // Unknown comparison operator fallback
        assert!(!Preprocessor::eval_comparison("a", "??", "b"));

        // Unclosed macro error
        assert!(prep.expand_macros("$(unclosed_macro", &ctx).is_err());

        Ok(())
    }

    /// Tests advanced functions with default arguments: `SubString` defaults, `FormatVersion` default, `AutoGuid` default.
    #[test]
    fn test_advanced_functions_defaults() -> Result<()> {
        let ctx = PreprocessorContext::new();
        let prep = Preprocessor::new();

        assert_eq!(prep.expand_macros("$(fun.SubString())", &ctx)?, "");
        assert_eq!(
            prep.expand_macros("$(fun.SubString(\"hello\"))", &ctx)?,
            "hello"
        );
        assert_eq!(prep.expand_macros("$(fun.FormatVersion())", &ctx)?, "1.0.0");
        let auto_guid_default = prep.expand_macros("$(fun.AutoGuid())", &ctx)?;
        assert!(auto_guid_default.starts_with('{'));

        Ok(())
    }
}
