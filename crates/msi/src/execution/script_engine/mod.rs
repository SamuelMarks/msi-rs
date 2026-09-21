//! Embedded Script Execution Engines for Windows Installer Custom Actions.
//!
//! Provides embedded, pure-Rust, sandboxed interpreters for:
//! - **ECMAScript / `JScript`**: Full AST evaluation, functions, loops, expressions,
//!   and MSI Automation Object Model exposure.
//! - **`VBScript`**: Case-insensitive lexing/parsing, COM `Variant` types, subroutines,
//!   functions, control flow, and `Err` object exception handling (`On Error Resume Next`).
//!
//! Both engines operate strictly in-memory without host filesystem or network access,
//! enforce fuel/step limits to prevent hangs, and provide precise line/column error reporting.

pub mod jscript;
pub mod session;
pub mod vbscript;

pub use jscript::{JScriptEngine, JsValue};
pub use session::{
    ScriptDatabase, ScriptSession, SessionMessage, DEFAULT_SCRIPT_FUEL, MSICONDITION_ERROR_CODE,
    MSICONDITION_FALSE_CODE, MSICONDITION_NONE_CODE, MSICONDITION_TRUE_CODE, MSIRUNMODE_ADMIN,
    MSIRUNMODE_ADVERTISE, MSIRUNMODE_CABPATH, MSIRUNMODE_COMMIT, MSIRUNMODE_LOGENABLED,
    MSIRUNMODE_MAINTENANCE, MSIRUNMODE_OPERATIONS, MSIRUNMODE_REBOOTATEND, MSIRUNMODE_REBOOTNOW,
    MSIRUNMODE_ROLLBACK, MSIRUNMODE_ROLLBACKENABLED, MSIRUNMODE_SCHEDULED,
    MSIRUNMODE_SOURCESHORTNAMES, MSIRUNMODE_TARGETSHORTNAMES, MSIRUNMODE_WINDOWS9X,
    MSIRUNMODE_ZAWENABLED,
};
pub use vbscript::{VBScriptEngine, Variant, VbErr};

use crate::error::Result;

/// Supported embedded script languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLanguage {
    /// Microsoft `JScript` / ECMAScript.
    JScript,
    /// Microsoft `VBScript`.
    VBScript,
}

/// Unified result value from script execution.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    /// Undefined or empty value.
    Undefined,
    /// Null value.
    Null,
    /// Boolean value.
    Boolean(bool),
    /// Numeric value.
    Number(f64),
    /// Text string value.
    String(String),
}

impl From<JsValue> for ScriptValue {
    fn from(val: JsValue) -> Self {
        match val {
            JsValue::Undefined
            | JsValue::Object(_)
            | JsValue::SessionObject
            | JsValue::DatabaseObject
            | JsValue::MathObject => Self::Undefined,
            JsValue::Null => Self::Null,
            JsValue::Boolean(b) => Self::Boolean(b),
            JsValue::Number(n) => Self::Number(n),
            JsValue::String(s) => Self::String(s),
        }
    }
}

impl From<Variant> for ScriptValue {
    fn from(val: Variant) -> Self {
        match val {
            Variant::Empty | Variant::Object(_) => Self::Undefined,
            Variant::Null => Self::Null,
            Variant::Boolean(b) => Self::Boolean(b),
            #[allow(clippy::cast_precision_loss)]
            Variant::Integer(i) => Self::Number(i as f64),
            Variant::String(s) => Self::String(s),
        }
    }
}

/// Unified script execution engine supporting both `JScript` and `VBScript`.
#[derive(Debug, Default)]
pub struct ScriptEngine {
    /// Embedded `JScript` execution engine.
    jscript: JScriptEngine,
    /// Embedded `VBScript` execution engine.
    vbscript: VBScriptEngine,
}

impl ScriptEngine {
    /// Creates a new [`ScriptEngine`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            jscript: JScriptEngine::new(),
            vbscript: VBScriptEngine::new(),
        }
    }

    /// Executes script code in the specified language within a [`ScriptSession`].
    ///
    /// # Arguments
    ///
    /// * `lang` - Target language (`JScript` or `VBScript`).
    /// * `code` - Script source code.
    /// * `session` - Active installer automation session.
    ///
    /// # Returns
    ///
    /// The final [`ScriptValue`] produced by the script.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::ScriptRuntimeError`] on syntax or execution errors.
    pub fn execute(
        &mut self,
        lang: ScriptLanguage,
        code: &str,
        session: &mut ScriptSession,
    ) -> Result<ScriptValue> {
        match lang {
            ScriptLanguage::JScript => {
                let res = self.jscript.execute(code, session)?;
                Ok(ScriptValue::from(res))
            }
            ScriptLanguage::VBScript => {
                let res = self.vbscript.execute(code, session)?;
                Ok(ScriptValue::from(res))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests unified script execution for `JScript`.
    #[test]
    fn test_unified_script_engine_jscript() {
        let mut session = ScriptSession::default();
        let mut engine = ScriptEngine::new();

        let code = r#"
            Session.Property("APP_STATUS") = "READY";
            42;
        "#;
        let res = engine.execute(ScriptLanguage::JScript, code, &mut session);
        assert_eq!(res, Ok(ScriptValue::Number(42.0)));
        assert_eq!(session.property("APP_STATUS"), "READY");
    }

    /// Tests unified script execution for `VBScript`.
    #[test]
    fn test_unified_script_engine_vbscript() {
        let mut session = ScriptSession::default();
        let mut engine = ScriptEngine::new();

        let code = r#"
            Session.Property("APP_STATUS") = "RUNNING"
            Dim x
            x = 100
        "#;
        let res = engine.execute(ScriptLanguage::VBScript, code, &mut session);
        assert!(res.is_ok());
        assert_eq!(session.property("APP_STATUS"), "RUNNING");
    }

    /// Tests `ScriptValue` conversions from both `JsValue` and `Variant`.
    #[test]
    fn test_script_value_conversions() {
        use std::collections::HashMap;

        assert_eq!(
            ScriptValue::from(JsValue::Undefined),
            ScriptValue::Undefined
        );
        assert_eq!(
            ScriptValue::from(JsValue::Object(HashMap::new())),
            ScriptValue::Undefined
        );
        assert_eq!(
            ScriptValue::from(JsValue::SessionObject),
            ScriptValue::Undefined
        );
        assert_eq!(
            ScriptValue::from(JsValue::DatabaseObject),
            ScriptValue::Undefined
        );
        assert_eq!(
            ScriptValue::from(JsValue::MathObject),
            ScriptValue::Undefined
        );
        assert_eq!(
            ScriptValue::from(JsValue::Boolean(true)),
            ScriptValue::Boolean(true)
        );
        assert_eq!(ScriptValue::from(JsValue::Null), ScriptValue::Null);
        assert_eq!(
            ScriptValue::from(JsValue::Number(3.5)),
            ScriptValue::Number(3.5)
        );
        assert_eq!(
            ScriptValue::from(JsValue::String("hello".to_string())),
            ScriptValue::String("hello".to_string())
        );

        assert_eq!(ScriptValue::from(Variant::Empty), ScriptValue::Undefined);
        assert_eq!(
            ScriptValue::from(Variant::Object("Obj".to_string())),
            ScriptValue::Undefined
        );
        assert_eq!(ScriptValue::from(Variant::Null), ScriptValue::Null);
        assert_eq!(
            ScriptValue::from(Variant::Boolean(false)),
            ScriptValue::Boolean(false)
        );
        assert_eq!(
            ScriptValue::from(Variant::Integer(10)),
            ScriptValue::Number(10.0)
        );
        assert_eq!(
            ScriptValue::from(Variant::String("vb".to_string())),
            ScriptValue::String("vb".to_string())
        );
    }
}
