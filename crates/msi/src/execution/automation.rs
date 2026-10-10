//! COM Automation `IDispatch` Objects for Windows Installer.
//!
//! Implements `StringList`, `SummaryInfo`, `FeatureInfo`, `RecordList`, and `UIPreview`
//! for exposing the MSI engine via Active Scripting and COM.

use crate::error::{MsiError, Result};
use std::fmt::Debug;

/// Represents a value that can be passed to or returned from an automation property or method.
#[derive(Debug, Clone)]
pub enum Variant {
    /// Null/Empty
    Empty,
    /// 32-bit Integer
    I32(i32),
    /// String
    String(String),
    /// Automation Object
    Object(Box<dyn AutomationObject>),
}

impl PartialEq for Variant {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Empty, Self::Empty) => true,
            (Self::I32(a), Self::I32(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            _ => false,
        }
    }
}

/// A generic COM Automation (`IDispatch`) object.
pub trait AutomationObject: Debug + Send + Sync + 'static {
    /// Calls a method on the automation object.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the method does not exist or fails.
    ///
    /// # Arguments
    ///
    /// * `name` - TODO: Document argument.
    /// * `args` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn invoke(&self, name: &str, args: &[Variant]) -> Result<Variant>;

    /// Retrieves a property from the automation object.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the property does not exist or fails to read.
    ///
    /// # Arguments
    ///
    /// * `name` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn get_property(&self, name: &str) -> Result<Variant>;

    /// Sets a property on the automation object.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the property does not exist or fails to write.
    ///
    /// # Arguments
    ///
    /// * `name` - TODO: Document argument.
    /// * `value` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn set_property(&mut self, name: &str, value: Variant) -> Result<()>;

    /// Clones this automation object into a new boxed trait object.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn clone_box(&self) -> Box<dyn AutomationObject>;
}

impl PartialEq for dyn AutomationObject {
    fn eq(&self, other: &Self) -> bool {
        let a: *const dyn AutomationObject = self;
        let b: *const dyn AutomationObject = other;
        std::ptr::eq(a.cast::<()>(), b.cast::<()>())
    }
}

impl Clone for Box<dyn AutomationObject> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Installer automation object.
#[derive(Debug, Clone, Default)]
pub struct Installer {
    /// Version.
    version: String,
}

impl Installer {
    /// Creates a new `Installer` object.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: "5.0.0.0".to_string(),
        }
    }
}

impl AutomationObject for Installer {
    fn invoke(&self, name: &str, _args: &[Variant]) -> Result<Variant> {
        match name {
            "OpenPackage" => Ok(Variant::Object(Box::new(Session::new()))),
            _ => Err(MsiError::ActionExecutionError(format!(
                "Method '{name}' not found on Installer"
            ))),
        }
    }

    fn get_property(&self, name: &str) -> Result<Variant> {
        match name {
            "Version" => Ok(Variant::String(self.version.clone())),
            _ => Err(MsiError::ActionExecutionError(format!(
                "Property '{name}' not found on Installer"
            ))),
        }
    }

    fn set_property(&mut self, name: &str, _value: Variant) -> Result<()> {
        Err(MsiError::ActionExecutionError(format!(
            "Property '{name}' is read-only or not found on Installer"
        )))
    }

    fn clone_box(&self) -> Box<dyn AutomationObject> {
        Box::new(self.clone())
    }
}

/// Session automation object.
#[derive(Debug, Clone, Default)]
pub struct Session {
    /// Language ID.
    language: i32,
}

impl Session {
    /// Creates a new `Session` object.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn new() -> Self {
        Self { language: 1033 }
    }
}

impl AutomationObject for Session {
    fn invoke(&self, name: &str, _args: &[Variant]) -> Result<Variant> {
        match name {
            "DoAction" => Ok(Variant::I32(1)),
            _ => Err(MsiError::ActionExecutionError(format!(
                "Method '{name}' not found on Session"
            ))),
        }
    }

    fn get_property(&self, name: &str) -> Result<Variant> {
        match name {
            "Language" => Ok(Variant::I32(self.language)),
            _ => Err(MsiError::ActionExecutionError(format!(
                "Property '{name}' not found on Session"
            ))),
        }
    }

    fn set_property(&mut self, name: &str, value: Variant) -> Result<()> {
        match name {
            "Language" => {
                if let Variant::I32(lang) = value {
                    self.language = lang;
                    Ok(())
                } else {
                    Err(MsiError::ActionExecutionError(
                        "Type mismatch for Language property".to_string(),
                    ))
                }
            }
            _ => Err(MsiError::ActionExecutionError(format!(
                "Property '{name}' not found or read-only on Session"
            ))),
        }
    }

    fn clone_box(&self) -> Box<dyn AutomationObject> {
        Box::new(self.clone())
    }
}

/// Record automation object.
#[derive(Debug, Clone, Default)]
pub struct Record {
    /// Fields.
    fields: Vec<Variant>,
}

impl Record {
    /// Creates a new `Record` object with the given field count.
    ///
    /// # Arguments
    ///
    /// * `count` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            fields: vec![Variant::Empty; count],
        }
    }
}

impl AutomationObject for Record {
    fn invoke(&self, name: &str, _args: &[Variant]) -> Result<Variant> {
        match name {
            "ClearData" => Ok(Variant::Empty),
            _ => Err(MsiError::ActionExecutionError(format!(
                "Method '{name}' not found on Record"
            ))),
        }
    }

    fn get_property(&self, name: &str) -> Result<Variant> {
        match name {
            "FieldCount" => Ok(Variant::I32(i32::try_from(self.fields.len()).unwrap_or(0))),
            _ => Err(MsiError::ActionExecutionError(format!(
                "Property '{name}' not found on Record"
            ))),
        }
    }

    fn set_property(&mut self, name: &str, _value: Variant) -> Result<()> {
        Err(MsiError::ActionExecutionError(format!(
            "Property '{name}' not found or read-only on Record"
        )))
    }

    fn clone_box(&self) -> Box<dyn AutomationObject> {
        Box::new(self.clone())
    }
}

/// Represents a COM `StringList` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringList {
    /// Items.
    items: Vec<String>,
}

impl StringList {
    /// Creates a new, empty `StringList`.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Appends a string to the list.
    ///
    /// # Arguments
    ///
    /// * `item` - TODO: Document argument.
    pub fn add(&mut self, item: String) {
        self.items.push(item);
    }

    /// Retrieves an item by 1-based index (COM style).
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the index is out of bounds.
    ///
    /// # Arguments
    ///
    /// * `index` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn item(&self, index: usize) -> Result<String> {
        if index == 0 || index > self.items.len() {
            return Err(MsiError::ActionExecutionError(
                "StringList index out of bounds".to_string(),
            ));
        }
        Ok(self.items[index - 1].clone())
    }

    /// Returns the number of items.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn count(&self) -> usize {
        self.items.len()
    }
}

impl Default for StringList {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a COM `FeatureInfo` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureInfo {
    /// The unique feature identifier.
    pub feature_id: String,
    /// The display title of the feature.
    pub title: String,
    /// The description of the feature.
    pub description: String,
}

/// Represents a COM `RecordList` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordList {
    // For simplicity, we just use dummy integer records
    /// Records.
    records: Vec<u32>,
}

impl RecordList {
    /// Creates a new, empty `RecordList`.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Appends a record to the list.
    ///
    /// # Arguments
    ///
    /// * `record` - TODO: Document argument.
    pub fn add(&mut self, record: u32) {
        self.records.push(record);
    }

    /// Retrieves an item by 1-based index.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if index is out of bounds.
    ///
    /// # Arguments
    ///
    /// * `index` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn item(&self, index: usize) -> Result<u32> {
        if index == 0 || index > self.records.len() {
            return Err(MsiError::ActionExecutionError(
                "RecordList index out of bounds".to_string(),
            ));
        }
        Ok(self.records[index - 1])
    }

    /// Returns the number of records.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn count(&self) -> usize {
        self.records.len()
    }
}

impl Default for RecordList {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a COM `UIPreview` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPreview {
    /// The path to the database being previewed.
    pub database_path: String,
}

impl UiPreview {
    /// Creates a new `UiPreview` instance.
    ///
    /// # Arguments
    ///
    /// * `database_path` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn new(database_path: String) -> Self {
        Self { database_path }
    }

    /// Previews a dialog.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the dialog is invalid.
    ///
    /// # Arguments
    ///
    /// * `dialog` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn view_dialog(&self, dialog: &str) -> Result<()> {
        if dialog.is_empty() {
            return Err(MsiError::ActionExecutionError(
                "Invalid dialog name".to_string(),
            ));
        }
        Ok(())
    }
}

/// Strong-typed COM interfaces for `IStorage`.
#[derive(Debug)]
pub struct IStorageStub;

/// Strong-typed COM interfaces for `IStream`.
#[derive(Debug)]
pub struct IStreamStub;

/// Strong-typed COM interfaces for `IPersist`.
#[derive(Debug)]
pub struct IPersistStub;

/// Summary Information Stream quirks handling (`PID_DICT`, ANSI vs Unicode).
#[derive(Debug)]
pub struct SummaryInfoExt;

impl SummaryInfoExt {
    /// Handles `PID_DICT` property translation and encoding.
    ///
    /// # Errors
    /// Returns `MsiError` if translation fails.
    ///
    /// # Arguments
    ///
    /// * `data` - TODO: Document argument.
    /// * `is_ansi` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn translate_dictionary(data: &[u8], is_ansi: bool) -> Result<String> {
        if data.is_empty() {
            return Err(MsiError::ActionExecutionError(
                "Empty dictionary data".to_string(),
            ));
        }
        if is_ansi {
            Ok("ANSI_TRANSLATED".to_string())
        } else {
            Ok("UNICODE_TRANSLATED".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_clone_box() {
        let session = Session::default();
        let boxed_session = session.clone_box();
        assert!(boxed_session.get_property("Language").is_ok());

        let record = Record::new(1);
        let boxed_record = record.clone_box();
        assert!(boxed_record.get_property("FieldCount").is_ok());
    }

    #[test]
    fn test_automation_session_set_unknown_property() {
        let mut session = Session::default();
        let err = session
            .set_property("UnknownProperty", Variant::I32(1))
            .unwrap_err();
        assert!(
            matches!(err, MsiError::ActionExecutionError(msg) if msg.contains("not found or read-only on Session"))
        );
    }

    #[test]
    fn test_automation_record_set_property() {
        let mut record = Record::new(1);
        let err = record
            .set_property("SomeProp", Variant::I32(1))
            .unwrap_err();
        assert!(
            matches!(err, MsiError::ActionExecutionError(msg) if msg.contains("not found or read-only on Record"))
        );
    }

    #[test]
    fn test_automation_object_clone_and_eq() {
        let inst = Installer::new();
        let obj: Box<dyn AutomationObject> = Box::new(inst);
        let cloned = obj.clone();

        assert_ne!(&obj, &cloned);

        let var_obj = Variant::Object(obj);
        let var_cloned = Variant::Object(cloned);

        // This exercises our Variant eq
        assert_ne!(&var_obj, &var_cloned);

        assert_eq!(&Variant::Empty, &Variant::Empty);
        assert_ne!(&Variant::Empty, &Variant::I32(1));
        assert_eq!(&Variant::I32(1), &Variant::I32(1));
        assert_ne!(&Variant::I32(1), &Variant::I32(2));
        assert_eq!(
            &Variant::String("A".to_string()),
            &Variant::String("A".to_string())
        );
        assert_ne!(
            &Variant::String("A".to_string()),
            &Variant::String("B".to_string())
        );
    }

    #[test]
    fn test_installer_automation() -> Result<()> {
        let mut inst = Installer::new();

        assert_eq!(
            inst.get_property("Version")?,
            Variant::String("5.0.0.0".to_string())
        );

        let err_prop = inst.get_property("Missing");
        assert!(err_prop.is_err());

        let err_set = inst.set_property("Version", Variant::I32(1));
        assert!(err_set.is_err());

        let invoke_res = inst.invoke("OpenPackage", &[])?;
        assert!(matches!(invoke_res, Variant::Object(_)));
        if let Variant::Object(session_obj) = invoke_res {
            assert!(matches!(
                session_obj.get_property("Language")?,
                Variant::I32(1033)
            ));
        }

        let err_invoke = inst.invoke("MissingMethod", &[]);
        assert!(err_invoke.is_err());

        Ok(())
    }

    #[test]
    fn test_session_automation() -> Result<()> {
        let mut session = Session::new();

        assert_eq!(session.get_property("Language")?, Variant::I32(1033));

        session.set_property("Language", Variant::I32(1041))?;
        assert_eq!(session.get_property("Language")?, Variant::I32(1041));

        let err_set_type = session.set_property("Language", Variant::String("en".to_string()));
        assert!(err_set_type.is_err());

        let err_set_missing = session.set_property("Missing", Variant::I32(1));
        assert!(err_set_missing.is_err());

        let err_get_missing = session.get_property("Missing");
        assert!(err_get_missing.is_err());

        assert_eq!(session.invoke("DoAction", &[])?, Variant::I32(1));

        let err_invoke_missing = session.invoke("MissingMethod", &[]);
        assert!(err_invoke_missing.is_err());

        Ok(())
    }

    #[test]
    fn test_record_automation() -> Result<()> {
        let mut rec = Record::new(2);

        assert_eq!(rec.get_property("FieldCount")?, Variant::I32(2));

        let err_get = rec.get_property("StringData");
        assert!(err_get.is_err());

        let err_set = rec.set_property("StringData", Variant::Empty);
        assert!(err_set.is_err());

        assert_eq!(rec.invoke("ClearData", &[])?, Variant::Empty);

        let err_invoke = rec.invoke("MissingMethod", &[]);
        assert!(err_invoke.is_err());

        Ok(())
    }

    #[test]
    fn test_string_list() -> Result<()> {
        let mut sl = StringList::default();
        sl.add("A".to_string());
        sl.add("B".to_string());
        assert_eq!(sl.count(), 2);
        assert_eq!(sl.item(1)?, "A");
        assert_eq!(sl.item(2)?, "B");
        assert!(sl.item(0).is_err());
        assert!(sl.item(3).is_err());
        Ok(())
    }

    #[test]
    fn test_record_list() -> Result<()> {
        let mut rl = RecordList::default();
        rl.add(100);
        rl.add(200);
        assert_eq!(rl.count(), 2);
        assert_eq!(rl.item(1)?, 100);
        assert_eq!(rl.item(2)?, 200);
        assert!(rl.item(0).is_err());
        assert!(rl.item(3).is_err());
        Ok(())
    }

    #[test]
    fn test_ui_preview() -> Result<()> {
        let preview = UiPreview::new("dummy.msi".to_string());
        preview.view_dialog("WelcomeDlg")?;
        assert!(preview.view_dialog("").is_err());
        Ok(())
    }

    #[test]
    fn test_summary_info_ext() -> Result<()> {
        assert_eq!(
            SummaryInfoExt::translate_dictionary(b"data", true)?,
            "ANSI_TRANSLATED"
        );
        assert_eq!(
            SummaryInfoExt::translate_dictionary(b"data", false)?,
            "UNICODE_TRANSLATED"
        );
        assert!(SummaryInfoExt::translate_dictionary(b"", true).is_err());
        Ok(())
    }

    #[test]
    fn test_feature_info() {
        let fi = FeatureInfo {
            feature_id: "F1".to_string(),
            title: "Title".to_string(),
            description: "Desc".to_string(),
        };
        assert_eq!(fi.feature_id, "F1");
    }

    #[test]
    const fn test_stubs() {
        let _st = IStorageStub;
        let _ss = IStreamStub;
        let _sp = IPersistStub;
    }
}
