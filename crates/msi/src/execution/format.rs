//! Formatting Engine (`MsiFormatRecord`).
//!
//! Handles template string parsing for `[Property]`, `[#FileKey]`,
//! environment variables `[%ENV_VAR]`, and component directories `[$ComponentKey]`.

use crate::database::tables::Record;
use crate::error::Result;
use std::collections::HashMap;

/// Formatting context containing properties and variables for substitution.
#[derive(Debug, Clone, Default)]
pub struct FormatContext {
    /// Active properties map.
    pub properties: HashMap<String, String>,
    /// Environment variables mock (for testing and isolated execution).
    pub env_vars: HashMap<String, String>,
    /// File paths map mapping `FileKey` to full path.
    pub files: HashMap<String, String>,
    /// Component paths map mapping `ComponentKey` to directory path.
    pub components: HashMap<String, String>,
}

/// Formats a record template string against a formatting context.
///
/// # Arguments
///
/// * `template` - The template string to format.
/// * `record` - The record containing fields that might be referenced (e.g., `[1]`, `[2]`).
/// * `context` - The format context.
///
/// # Returns
///
/// The fully formatted string.
///
/// # Errors
///
/// Returns an error if the template cannot be formatted.
pub fn format_record(template: &str, record: &Record, context: &FormatContext) -> Result<String> {
    format_internal(template, Some(record), context)
}

/// Formats a string without a record context.
///
/// # Arguments
///
/// * `template` - The `template` parameter.
/// * `context` - The `context` parameter.
///
/// # Returns
///
/// The resulting value or `Ok(())` on success.
///
/// # Errors
///
/// Returns an error if the template cannot be formatted.
pub fn format_string(template: &str, context: &FormatContext) -> Result<String> {
    format_internal(template, None, context)
}

/// Internal formatter entrypoint.
fn format_internal(
    template: &str,
    record: Option<&Record>,
    context: &FormatContext,
) -> Result<String> {
    let mut chars = template.chars();
    parse_until(&mut chars, None, record, context)
}

/// Parses the template string until a stop character.
fn parse_until(
    chars: &mut std::str::Chars<'_>,
    stop_char: Option<char>,
    record: Option<&Record>,
    context: &FormatContext,
) -> Result<String> {
    let mut result = String::new();

    while let Some(c) = chars.next() {
        if Some(c) == stop_char {
            return Ok(result);
        }

        if c == '[' {
            // Lookahead for [\x]
            let mut ahead = chars.clone();
            if ahead.next() == Some('\\') {
                if let Some(x) = ahead.next() {
                    if ahead.next() == Some(']') {
                        // It's [\x]
                        result.push(x);
                        chars.next(); // consume '\'
                        chars.next(); // consume x
                        chars.next(); // consume ']'
                        continue;
                    }
                }
            }

            // Check if it closes
            let mut ahead2 = chars.clone();
            let mut depth = 1;
            let mut closes = false;
            while let Some(ac) = ahead2.next() {
                if ac == '[' {
                    // Check for nested [\x] so we don't count its brackets
                    let mut ah = ahead2.clone();
                    if ah.next() == Some('\\') && ah.next().is_some() && ah.next() == Some(']') {
                        ahead2.next();
                        ahead2.next();
                        ahead2.next();
                        continue;
                    }
                    depth += 1;
                } else if ac == ']' {
                    depth -= 1;
                    if depth == 0 {
                        closes = true;
                        break;
                    }
                }
            }

            if !closes {
                result.push('[');
                for rest in chars.by_ref() {
                    result.push(rest);
                }
                break;
            }

            // Parse inner
            let inner = parse_until(chars, Some(']'), record, context)?;
            resolve_bracket_token(&inner, record, context, &mut result);
        } else if c == ']' {
            result.push(']');
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

/// Resolves a parsed `[...]` token against `record` and `context` and appends to `result`.
fn resolve_bracket_token(
    inner: &str,
    record: Option<&Record>,
    context: &FormatContext,
    result: &mut String,
) {
    if inner == "~" {
        result.push('\0');
    } else if let Some(var) = inner.strip_prefix('%') {
        if let Some(val) = context.env_vars.get(var) {
            result.push_str(val);
        } else if let Ok(val) = std::env::var(var) {
            result.push_str(&val);
        }
    } else if let Some(var) = inner.strip_prefix('#') {
        if let Some(val) = context.files.get(var) {
            result.push_str(val);
        }
    } else if let Some(var) = inner.strip_prefix('$') {
        if let Some(val) = context.components.get(var) {
            result.push_str(val);
        }
    } else if let Ok(idx) = inner.parse::<usize>() {
        if let Some(rec) = record {
            if let Some(field) = rec.get(idx) {
                result.push_str(&field.to_string());
            }
        }
    } else if inner.starts_with('!') || inner.starts_with('?') {
        // Not fully supported yet, leave empty for now as fallback
    } else if let Some(val) = context.properties.get(inner) {
        result.push_str(val);
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::database::tables::FieldValue;

    #[test]
    fn test_format_record_basic() {
        let rec = Record::with_fields(vec![
            FieldValue::Null, // 0th field is usually template but we pass it as arg
            FieldValue::String("Value1".to_string()),
            FieldValue::Short(42),
        ]);
        let mut ctx = FormatContext::default();
        ctx.properties
            .insert("MyProp".to_string(), "PropVal".to_string());
        ctx.env_vars
            .insert("MY_ENV".to_string(), "EnvVal".to_string());
        ctx.files
            .insert("file1".to_string(), "C:\\Path\\file1.txt".to_string());
        ctx.components
            .insert("comp1".to_string(), "C:\\Path\\".to_string());

        assert_eq!(
            format_record("Test [MyProp]", &rec, &ctx),
            Ok("Test PropVal".to_string())
        );
        assert_eq!(
            format_record("Test [%MY_ENV]", &rec, &ctx),
            Ok("Test EnvVal".to_string())
        );
        assert_eq!(
            format_record("Test [#file1]", &rec, &ctx),
            Ok("Test C:\\Path\\file1.txt".to_string())
        );
        assert_eq!(
            format_record("Test [$comp1]", &rec, &ctx),
            Ok("Test C:\\Path\\".to_string())
        );
        assert_eq!(
            format_record("Field 1: [1], Field 2: [2]", &rec, &ctx),
            Ok("Field 1: Value1, Field 2: 42".to_string())
        );

        // Unclosed bracket
        assert_eq!(
            format_record("Test [unclosed", &rec, &ctx),
            Ok("Test [unclosed".to_string())
        );
    }

    #[test]
    fn test_format_record_env_fallback() {
        let rec = Record::with_fields(vec![FieldValue::Null]);
        let ctx = FormatContext::default();
        std::env::set_var("MSI_TEST_VAR", "SystemVal");
        assert_eq!(
            format_record("Test [%MSI_TEST_VAR]", &rec, &ctx),
            Ok("Test SystemVal".to_string())
        );
        assert_eq!(
            format_record("Test [%UNKNOWN_VAR]", &rec, &ctx),
            Ok("Test ".to_string())
        );
    }

    #[test]
    fn test_format_record_edge_cases() {
        let rec = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("Value1".to_string()),
        ]);
        let ctx = FormatContext::default();

        // [\ - ends abruptly
        assert_eq!(
            format_record("Test [\\", &rec, &ctx),
            Ok("Test [\\".to_string())
        );

        // [\x - ends abruptly
        assert_eq!(
            format_record("Test [\\x", &rec, &ctx),
            Ok("Test [\\x".to_string())
        );

        // [\xy - ends abruptly
        assert_eq!(
            format_record("Test [\\xy", &rec, &ctx),
            Ok("Test [\\xy".to_string())
        );

        // [\] - missing x, evaluates as unknown property \
        assert_eq!(
            format_record("Test [\\]", &rec, &ctx),
            Ok("Test ".to_string())
        );

        // [\[ - missing ]
        assert_eq!(
            format_record("Test [\\[", &rec, &ctx),
            Ok("Test [\\[".to_string())
        );

        // Test inner loops ah.next() edge cases
        // [[\ - ends abruptly inside
        assert_eq!(
            format_record("Test [[\\", &rec, &ctx),
            Ok("Test [[\\".to_string())
        );
        // [[\x - ends abruptly inside
        assert_eq!(
            format_record("Test [[\\x", &rec, &ctx),
            Ok("Test [[\\x".to_string())
        );
        // [[\xy - continues inside
        assert_eq!(
            format_record("Test [[\\xy", &rec, &ctx),
            Ok("Test [[\\xy".to_string())
        );

        // [99] - idx out of bounds
        assert_eq!(
            format_record("Test [99]", &rec, &ctx),
            Ok("Test ".to_string())
        );

        // [1] - record is None
        assert_eq!(format_string("Test [1]", &ctx), Ok("Test ".to_string()));
    }

    #[test]
    fn test_format_string_escapes_and_nested() {
        let mut ctx = FormatContext::default();
        ctx.properties
            .insert("Outer".to_string(), "OutVal".to_string());
        ctx.properties
            .insert("Inner".to_string(), "Outer".to_string());
        ctx.properties.insert("Missing".to_string(), String::new());

        // Escapes
        assert_eq!(format_string("[\\[]", &ctx), Ok("[".to_string()));
        assert_eq!(format_string("[\\]]", &ctx), Ok("]".to_string()));
        assert_eq!(format_string("[\\x]", &ctx), Ok("x".to_string()));

        // Null character
        assert_eq!(format_string("[~]", &ctx), Ok("\0".to_string()));

        // Nested
        assert_eq!(format_string("[[Inner]]", &ctx), Ok("OutVal".to_string()));
        assert_eq!(format_string("[Missing]", &ctx), Ok(String::new()));

        // Unmatched right bracket
        assert_eq!(format_string("foo]", &ctx), Ok("foo]".to_string()));
    }

    #[test]
    fn test_format_escaped_brackets_inside_property() {
        use super::format_string;
        use super::FormatContext;
        use std::collections::HashMap;

        let ctx = FormatContext {
            properties: HashMap::new(),
            env_vars: HashMap::new(),
            files: HashMap::new(),
            components: HashMap::new(),
        };

        // Just checking that we traverse that execution branch inside format_string without panicking.
        let formatted = format_string(r"Prop is [MyProp[\]]", &ctx);
        // Since MyProp[\] is not resolved to anything (because of depth nesting mismatch or whatever it produces),
        // we just assert it doesn't fail and returns the string.
        assert_eq!(formatted, Ok(r"Prop is [MyProp[\]]".to_string()));
    }

    #[test]
    fn test_format_additional_branches() {
        use super::format_record;
        use super::FormatContext;
        use crate::database::tables::record::{FieldValue, Record};
        use std::collections::HashMap;

        let mut components = HashMap::new();
        components.insert("Comp1".to_string(), "C:\\CompDir".to_string());

        let mut properties = HashMap::new();
        properties.insert("MyProp".to_string(), "PropValue".to_string());

        let ctx = FormatContext {
            properties,
            env_vars: HashMap::new(),
            files: HashMap::new(),
            components,
        };

        let rec = Record::with_fields(vec![
            FieldValue::String("A".to_string()),
            FieldValue::String("B".to_string()),
        ]);

        // $Comp1
        assert_eq!(
            format_record("Component is [$Comp1]", &rec, &ctx),
            Ok("Component is C:\\CompDir".to_string())
        );

        // !file
        assert_eq!(
            format_record("File is [!file]", &rec, &ctx),
            Ok("File is ".to_string())
        ); // unsupported

        // ?file
        assert_eq!(
            format_record("File is [?file]", &rec, &ctx),
            Ok("File is ".to_string())
        ); // unsupported

        // Invalid record index
        assert_eq!(
            format_record("Value is [5]", &rec, &ctx),
            Ok("Value is ".to_string())
        );

        // Missing property
        assert_eq!(
            format_record("Missing is [MissingProp]", &rec, &ctx),
            Ok("Missing is ".to_string())
        );

        // Standalone bracket
        assert_eq!(
            format_record("Close bracket ]", &rec, &ctx),
            Ok("Close bracket ]".to_string())
        );
    }
}
