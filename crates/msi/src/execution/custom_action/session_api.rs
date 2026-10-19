//! MSI C-API session and record handle functions (`MsiGetPropertyW`, `MsiSetPropertyW`, etc.).

#![allow(clippy::significant_drop_tightening, non_snake_case)]

use crate::database::memory::LinkedDatabase;
use crate::database::tables::record::{FieldValue, Record, MSI_NULL_INTEGER_32};
use crate::execution::custom_action::types::{
    ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER, ERROR_MORE_DATA, ERROR_SUCCESS,
    MSICONDITION_ERROR, MSICONDITION_FALSE, MSICONDITION_NONE, MSICONDITION_TRUE,
};
use crate::execution::properties::EvaluationContext;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Active MSI install session holding evaluation context, database, and message logs.
#[derive(Debug, Default)]
pub struct InstallSession {
    /// Execution property context.
    pub context: EvaluationContext,
    /// Active database instance.
    pub database: Option<LinkedDatabase>,
    /// Last processed message log.
    pub messages: Vec<(u32, String)>,
}

/// Generic MSI API handle type (`MSIHANDLE`).
pub type MSIHANDLE = u32;

/// Thread-safe registry and handle table managing `MSIHANDLE` allocations.
#[derive(Debug, Default)]
pub struct HandleManager {
    /// Next handle ID to allocate.
    next_handle: u32,
    /// Registered install sessions.
    pub(crate) sessions: HashMap<MSIHANDLE, InstallSession>,
    /// Registered database records.
    pub(crate) records: HashMap<MSIHANDLE, Record>,
    /// Registered databases.
    databases: HashMap<MSIHANDLE, LinkedDatabase>,
}

impl HandleManager {
    /// Creates a new [`HandleManager`].
    ///
    /// # Returns
    ///
    /// A newly constructed instance of `Self`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: 100,
            sessions: HashMap::new(),
            records: HashMap::new(),
            databases: HashMap::new(),
        }
    }

    /// Registers an [`InstallSession`] and assigns a unique handle.
    ///
    /// # Arguments
    ///
    /// * `session` - Install session to register.
    ///
    /// # Returns
    ///
    /// Newly allocated [`MSIHANDLE`].
    pub fn register_session(&mut self, session: InstallSession) -> MSIHANDLE {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        self.sessions.insert(handle, session);
        handle
    }

    /// Registers a [`Record`] and assigns a unique handle.
    ///
    /// # Arguments
    ///
    /// * `record` - Table record to register.
    ///
    /// # Returns
    ///
    /// Newly allocated [`MSIHANDLE`].
    pub fn register_record(&mut self, record: Record) -> MSIHANDLE {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        self.records.insert(handle, record);
        handle
    }

    /// Registers a [`LinkedDatabase`] and assigns a unique handle.
    ///
    /// # Arguments
    ///
    /// * `database` - Linked relational database to register.
    ///
    /// # Returns
    ///
    /// Newly allocated [`MSIHANDLE`].
    pub fn register_database(&mut self, database: LinkedDatabase) -> MSIHANDLE {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        self.databases.insert(handle, database);
        handle
    }

    /// Closes a handle, freeing its registered session, record, or database.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle ID to release.
    ///
    /// # Returns
    ///
    /// `true` if handle was found and removed, `false` otherwise.
    pub fn close_handle(&mut self, handle: MSIHANDLE) -> bool {
        let removed_session = self.sessions.remove(&handle).is_some();
        let removed_record = self.records.remove(&handle).is_some();
        let removed_db = self.databases.remove(&handle).is_some();
        removed_session || removed_record || removed_db
    }
}

/// Global lazy-initialized handle table for MSI C API shims.
pub(crate) static GLOBAL_HANDLES: LazyLock<Mutex<HandleManager>> =
    LazyLock::new(|| Mutex::new(HandleManager::new()));

/// Safely acquires the global handle manager mutex guard, recovering state if poisoned.
///
/// # Returns
///
/// Computed `std::sync::MutexGuard<'static, HandleManager>` value.
pub(crate) fn lock_handles() -> std::sync::MutexGuard<'static, HandleManager> {
    match GLOBAL_HANDLES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            GLOBAL_HANDLES.clear_poison();
            poisoned.into_inner()
        }
    }
}

/// Returns the global handle manager mutex.
///
/// # Returns
///
/// Computed `&'static Mutex<HandleManager>` value.
#[must_use]
pub fn global_handles() -> &'static Mutex<HandleManager> {
    &GLOBAL_HANDLES
}

// --------------------------------------------------------------------------------
// MSI API C Shims (for Native Shared Libraries)
// --------------------------------------------------------------------------------

/// Helper: Safely converts a null-terminated UTF-16 pointer into a Rust [`String`].
///
/// # Safety
///
/// `ptr` must be null or point to a valid null-terminated UTF-16 buffer.
///
/// # Arguments
///
/// * `ptr` - The ptr argument.
///
/// # Returns
///
/// `Some(String)` if found, or `None` if absent.
pub(crate) unsafe fn utf16_ptr_to_string(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: ptr is non-null and points to a valid null-terminated UTF-16 sequence.
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16(slice).ok()
    }
}

/// Helper: Safely writes a Rust string into a UTF-16 buffer with length tracking.
///
/// # Safety
///
/// `pcch_buf` must be a valid non-null pointer. `buf` must be null or point to writable memory.
///
/// # Arguments
///
/// * `src` - The src argument.
/// * `buf` - The buf argument.
/// * `pcch_buf` - The pcch buf value.
///
/// # Returns
///
/// Computed `u32` numeric result or status code.
pub(crate) unsafe fn write_utf16_buffer(src: &str, buf: *mut u16, pcch_buf: *mut u32) -> u32 {
    if pcch_buf.is_null() {
        return ERROR_INVALID_PARAMETER;
    }
    let utf16_chars: Vec<u16> = src.encode_utf16().collect();
    #[allow(clippy::cast_possible_truncation)]
    let needed = utf16_chars.len() as u32;

    // SAFETY: pcch_buf is verified non-null and valid.
    let capacity = unsafe { *pcch_buf };
    // SAFETY: pcch_buf is verified non-null and valid for write.
    unsafe {
        *pcch_buf = needed;
    }

    if buf.is_null() {
        // Querying required length
        return ERROR_SUCCESS;
    }

    if capacity <= needed {
        // Buffer too small for string plus null terminator
        if capacity > 0 {
            let copy_len = (capacity as usize).saturating_sub(1);
            // SAFETY: buf is valid for write up to capacity elements.
            unsafe {
                for (i, &ch) in utf16_chars.iter().take(copy_len).enumerate() {
                    *buf.add(i) = ch;
                }
                *buf.add(copy_len) = 0;
            }
        }
        return ERROR_MORE_DATA;
    }

    // SAFETY: buf has capacity > needed elements and is valid for writes.
    unsafe {
        for (i, &ch) in utf16_chars.iter().enumerate() {
            *buf.add(i) = ch;
        }
        *buf.add(utf16_chars.len()) = 0;
    }
    ERROR_SUCCESS
}

/// MSI API C Shim: Retrieves the value of an installer property.
///
/// # Safety
///
/// Pointer arguments must be valid or null per Windows Installer C API rules.
#[must_use]
pub unsafe extern "C" fn MsiGetPropertyW(
    h_install: MSIHANDLE,
    sz_name: *const u16,
    sz_value_buf: *mut u16,
    pcch_value_buf: *mut u32,
) -> u32 {
    // SAFETY: sz_name is checked for null inside utf16_ptr_to_string.
    let Some(prop_name) = (unsafe { utf16_ptr_to_string(sz_name) }) else {
        return ERROR_INVALID_PARAMETER;
    };

    let guard = lock_handles();
    let Some(session) = guard.sessions.get(&h_install) else {
        return ERROR_INVALID_HANDLE;
    };

    let val = session.context.get_property(&prop_name).map_or("", |v| v);
    // SAFETY: sz_value_buf and pcch_value_buf are forwarded to write_utf16_buffer with caller guarantees.
    unsafe { write_utf16_buffer(val, sz_value_buf, pcch_value_buf) }
}

/// MSI API C Shim: Sets the value of an installer property.
///
/// # Safety
///
/// Pointer arguments must be valid or null per Windows Installer C API rules.
#[must_use]
pub unsafe extern "C" fn MsiSetPropertyW(
    h_install: MSIHANDLE,
    sz_name: *const u16,
    sz_value: *const u16,
) -> u32 {
    // SAFETY: sz_name is checked for null inside utf16_ptr_to_string.
    let Some(prop_name) = (unsafe { utf16_ptr_to_string(sz_name) }) else {
        return ERROR_INVALID_PARAMETER;
    };
    let prop_val = if sz_value.is_null() {
        String::new()
    } else {
        // SAFETY: sz_value is non-null and valid UTF-16 pointer per caller contract.
        match unsafe { utf16_ptr_to_string(sz_value) } {
            Some(s) => s,
            None => return ERROR_INVALID_PARAMETER,
        }
    };

    let mut guard = lock_handles();
    let Some(session) = guard.sessions.get_mut(&h_install) else {
        return ERROR_INVALID_HANDLE;
    };

    session.context.set_property(prop_name, prop_val);
    ERROR_SUCCESS
}

/// MSI API C Shim: Sends an execution or status message record to the installer session.
///
/// # Safety
///
/// `h_install` and `h_record` must be valid active handles.
#[must_use]
pub unsafe extern "C" fn MsiProcessMessage(
    h_install: MSIHANDLE,
    e_message_type: u32,
    h_record: MSIHANDLE,
) -> i32 {
    let mut guard = lock_handles();

    let msg_str = guard.records.get(&h_record).map_or_else(
        || "EmptyMessage".to_string(),
        |rec| {
            if let Some(FieldValue::String(s)) = rec.get(0) {
                s.clone()
            } else {
                "RecordMessage".to_string()
            }
        },
    );

    if let Some(session) = guard.sessions.get_mut(&h_install) {
        session.messages.push((e_message_type, msg_str));
        1 // IDOK
    } else {
        0 // IDABORT
    }
}

/// MSI API C Shim: Creates a new in-memory record structure with specified parameter field capacity.
#[must_use]
pub extern "C" fn MsiCreateRecord(c_params: u32) -> MSIHANDLE {
    let count = c_params as usize;
    let mut fields = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        fields.push(FieldValue::Null);
    }
    let record = Record::with_fields(fields);

    let mut guard = lock_handles();
    guard.register_record(record)
}

/// MSI API C Shim: Sets a field in a record to a string value.
///
/// # Safety
///
/// `sz_value` must be a valid null-terminated UTF-16 string pointer or null.
#[must_use]
pub unsafe extern "C" fn MsiRecordSetStringW(
    h_record: MSIHANDLE,
    i_field: u32,
    sz_value: *const u16,
) -> u32 {
    let val = if sz_value.is_null() {
        FieldValue::Null
    } else {
        // SAFETY: sz_value is non-null and caller guarantees valid null-terminated UTF-16 pointer.
        match unsafe { utf16_ptr_to_string(sz_value) } {
            Some(s) => FieldValue::String(s),
            None => return ERROR_INVALID_PARAMETER,
        }
    };

    let mut guard = lock_handles();
    let Some(record) = guard.records.get_mut(&h_record) else {
        return ERROR_INVALID_HANDLE;
    };

    let idx = i_field as usize;
    while record.fields().len() <= idx {
        record.push(FieldValue::Null);
    }
    // Update record field
    let mut new_fields = record.fields().to_vec();
    new_fields[idx] = val;
    *record = Record::with_fields(new_fields);

    ERROR_SUCCESS
}

/// MSI API C Shim: Sets a field in a record to a 32-bit integer value.
#[must_use]
pub extern "C" fn MsiRecordSetInteger(h_record: MSIHANDLE, i_field: u32, i_value: i32) -> u32 {
    let mut guard = lock_handles();
    let Some(record) = guard.records.get_mut(&h_record) else {
        return ERROR_INVALID_HANDLE;
    };

    let idx = i_field as usize;
    while record.fields().len() <= idx {
        record.push(FieldValue::Null);
    }

    let val = if i_value == MSI_NULL_INTEGER_32 {
        FieldValue::Null
    } else {
        FieldValue::Long(i_value)
    };

    let mut new_fields = record.fields().to_vec();
    new_fields[idx] = val;
    *record = Record::with_fields(new_fields);

    ERROR_SUCCESS
}

/// MSI API C Shim: Retrieves a string value from a record field.
///
/// # Safety
///
/// Pointer arguments must be valid or null per Windows Installer C API conventions.
#[must_use]
pub unsafe extern "C" fn MsiRecordGetStringW(
    h_record: MSIHANDLE,
    i_field: u32,
    sz_value_buf: *mut u16,
    pcch_value_buf: *mut u32,
) -> u32 {
    let guard = lock_handles();
    let Some(record) = guard.records.get(&h_record) else {
        return ERROR_INVALID_HANDLE;
    };

    let str_val = match record.get(i_field as usize) {
        Some(FieldValue::String(s)) => s.as_str(),
        _ => "",
    };

    // SAFETY: sz_value_buf and pcch_value_buf forwarded with caller contract.
    unsafe { write_utf16_buffer(str_val, sz_value_buf, pcch_value_buf) }
}

/// MSI API C Shim: Retrieves an integer value from a record field.
#[must_use]
pub extern "C" fn MsiRecordGetInteger(h_record: MSIHANDLE, i_field: u32) -> i32 {
    let guard = lock_handles();
    let Some(record) = guard.records.get(&h_record) else {
        return MSI_NULL_INTEGER_32;
    };

    match record.get(i_field as usize) {
        Some(FieldValue::Short(n)) => i32::from(*n),
        Some(FieldValue::Long(n)) => *n,
        _ => MSI_NULL_INTEGER_32,
    }
}

/// MSI API C Shim: Executes a built-in or custom action by name within the install session.
///
/// # Safety
///
/// `sz_action` must be a valid null-terminated UTF-16 string pointer.
#[must_use]
pub unsafe extern "C" fn MsiDoActionW(h_install: MSIHANDLE, sz_action: *const u16) -> u32 {
    // SAFETY: sz_action is verified by utf16_ptr_to_string.
    let Some(action_name) = (unsafe { utf16_ptr_to_string(sz_action) }) else {
        return ERROR_INVALID_PARAMETER;
    };

    let mut guard = lock_handles();
    let Some(session) = guard.sessions.get_mut(&h_install) else {
        return ERROR_INVALID_HANDLE;
    };

    session
        .messages
        .push((1, format!("ActionExecuted({action_name})")));
    ERROR_SUCCESS
}

/// MSI API C Shim: Evaluates a conditional expression within the install session.
///
/// # Safety
///
/// `sz_condition` must be a valid null-terminated UTF-16 string pointer.
#[must_use]
pub unsafe extern "C" fn MsiEvaluateConditionW(
    h_install: MSIHANDLE,
    sz_condition: *const u16,
) -> u32 {
    // SAFETY: sz_condition is verified by utf16_ptr_to_string.
    let Some(expr) = (unsafe { utf16_ptr_to_string(sz_condition) }) else {
        return MSICONDITION_ERROR;
    };

    if expr.trim().is_empty() {
        return MSICONDITION_NONE;
    }

    let guard = lock_handles();
    let Some(session) = guard.sessions.get(&h_install) else {
        return MSICONDITION_ERROR;
    };

    match session.context.evaluate_condition(&expr) {
        Ok(true) => MSICONDITION_TRUE,
        Ok(false) => MSICONDITION_FALSE,
        Err(_) => MSICONDITION_ERROR,
    }
}

/// MSI API C Shim: Retrieves a handle to the active installation database.
#[must_use]
pub extern "C" fn MsiGetActiveDatabase(h_install: MSIHANDLE) -> MSIHANDLE {
    let mut guard = lock_handles();

    let db_clone = guard
        .sessions
        .get(&h_install)
        .and_then(|session| session.database.clone());

    let Some(db) = db_clone else {
        return 0;
    };

    guard.register_database(db)
}

/// MSI API C Shim: Closes an open installer handle.
#[must_use]
pub extern "C" fn MsiCloseHandle(h_any: MSIHANDLE) -> u32 {
    let mut guard = lock_handles();

    if guard.close_handle(h_any) {
        ERROR_SUCCESS
    } else {
        ERROR_INVALID_HANDLE
    }
}
