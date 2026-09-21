//! Error codes, thread-local diagnostics, and panic boundary handlers for the C-ABI.
//!
//! Enforces panic-safety and deterministic error propagation across language boundaries.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};
use std::ptr;

/// Operation completed successfully (`0`).
pub const MSI_SUCCESS: i32 = 0;

/// Required pointer argument is NULL (`-1`).
pub const MSI_ERROR_NULL_POINTER: i32 = -1;

/// Argument failed validation or parsing (`-2`).
pub const MSI_ERROR_INVALID_ARGUMENT: i32 = -2;

/// Package or relational schema validation failed (`-3`).
pub const MSI_ERROR_VALIDATION: i32 = -3;

/// Filesystem or I/O failure (`-4`).
pub const MSI_ERROR_IO: i32 = -4;

/// Cabinet archive compression, decompression, or packing failure (`-5`).
pub const MSI_ERROR_CABINET: i32 = -5;

/// Relational database constraint or catalog violation (`-6`).
pub const MSI_ERROR_DATABASE: i32 = -6;

/// `WiX` preprocessor, compiler, or linker evaluation failure (`-7`).
pub const MSI_ERROR_WIX: i32 = -7;

/// Provided output buffer size is insufficient (`-8`).
pub const MSI_ERROR_BUFFER_TOO_SMALL: i32 = -8;

/// Unhandled panic intercepted at the FFI boundary (`-99`).
pub const MSI_ERROR_PANIC: i32 = -99;

thread_local! {
    /// Thread-local storage holding the most recent error code and diagnostic message.
    static LAST_ERROR: RefCell<(i32, String)> = const { RefCell::new((MSI_SUCCESS, String::new())) };
}

/// Sets the thread-local error state.
///
/// # Arguments
///
/// * `code` - Numeric status code.
/// * `msg` - Diagnostic description.
pub fn set_last_error(code: i32, msg: impl Into<String>) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = (code, msg.into());
    });
}

/// Retrieves the thread-local error state.
///
/// # Returns
///
/// Tuple of status code and diagnostic message.
#[must_use]
pub fn get_last_error() -> (i32, String) {
    LAST_ERROR.with(|cell| cell.borrow().clone())
}

/// Clears the thread-local error state.
pub fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = (MSI_SUCCESS, String::new());
    });
}

/// Copies the most recent error message into a caller-supplied buffer.
///
/// If `buffer` is NULL or `capacity` is too small, the total required buffer length
/// (including null terminator) is written to `*out_written` and
/// [`MSI_ERROR_BUFFER_TOO_SMALL`] is returned.
///
/// # Arguments
///
/// * `buffer` - Destination character buffer (caller allocated).
/// * `capacity` - Capacity of `buffer` in bytes.
/// * `out_written` - Optional pointer receiving the number of bytes written.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or [`MSI_ERROR_BUFFER_TOO_SMALL`].
///
/// # Safety
///
/// `buffer` must point to valid writable memory of at least `capacity` bytes if non-null.
/// `out_written` must point to valid writable memory if non-null.
#[no_mangle]
pub unsafe extern "C" fn msi_get_last_error_message(
    buffer: *mut c_char,
    capacity: usize,
    out_written: *mut usize,
) -> i32 {
    unsafe {
        let (_, msg) = get_last_error();
        let msg_bytes = msg.as_bytes();
        let required_len = msg_bytes.len().saturating_add(1);

        if !out_written.is_null() {
            *out_written = required_len;
        }

        if buffer.is_null() || capacity < required_len {
            return MSI_ERROR_BUFFER_TOO_SMALL;
        }

        ptr::copy_nonoverlapping(msg_bytes.as_ptr(), buffer.cast::<u8>(), msg_bytes.len());
        *buffer.add(msg_bytes.len()) = 0;

        MSI_SUCCESS
    }
}

/// Retrieves the status code of the most recent error on the calling thread.
///
/// # Returns
///
/// Numeric error code, or [`MSI_SUCCESS`] if no error has occurred.
#[no_mangle]
#[must_use]
pub extern "C" fn msi_get_last_error_code() -> i32 {
    let (code, _) = get_last_error();
    code
}

/// Resets the thread-local error state to [`MSI_SUCCESS`].
#[no_mangle]
pub extern "C" fn msi_clear_last_error() {
    clear_last_error();
}

/// Maps an internal [`msi::Error`] to an FFI error code.
///
/// # Arguments
///
/// * `err` - Reference to internal error.
///
/// # Returns
///
/// Corresponding FFI error code constant.
#[must_use]
pub const fn map_msi_error(err: &msi::Error) -> i32 {
    match err {
        msi::Error::Validation { .. } => MSI_ERROR_VALIDATION,
        msi::Error::Io(_) => MSI_ERROR_IO,
        msi::Error::InvalidCabSignature { .. }
        | msi::Error::InvalidCabVersion { .. }
        | msi::Error::InvalidCabChecksum { .. }
        | msi::Error::InvalidCabData { .. }
        | msi::Error::DecompressionFailed { .. }
        | msi::Error::CompressionFailed { .. }
        | msi::Error::CabinetFileNotFound { .. } => MSI_ERROR_CABINET,
        msi::Error::MissingTable { .. }
        | msi::Error::RecordLengthMismatch { .. }
        | msi::Error::InvalidStringPool { .. }
        | msi::Error::StringPoolIndexOutOfBounds { .. }
        | msi::Error::InvalidSummaryInfo { .. }
        | msi::Error::InvalidColumnType { .. } => MSI_ERROR_DATABASE,
        msi::Error::Preprocessor { .. }
        | msi::Error::XmlParse { .. }
        | msi::Error::WixCompiler { .. }
        | msi::Error::InvalidWixObject { .. }
        | msi::Error::WixLinker { .. }
        | msi::Error::IceValidation { .. } => MSI_ERROR_WIX,
        _ => MSI_ERROR_INVALID_ARGUMENT,
    }
}

/// Handles a successful FFI operation by clearing errors and returning the success code.
fn handle_success(code: i32) -> i32 {
    clear_last_error();
    code
}

/// Handles an operational error by recording it in thread-local storage and returning its code.
fn handle_error(code: i32, msg: String) -> i32 {
    set_last_error(code, msg);
    code
}

/// Handles an uncaught panic across the FFI boundary, safely converting the panic payload
/// into a diagnostic error message and returning [`MSI_ERROR_PANIC`].
fn handle_panic(panic_payload: &(dyn std::any::Any + Send + 'static)) -> i32 {
    let msg = panic_payload.downcast_ref::<&str>().map_or_else(
        || {
            panic_payload.downcast_ref::<String>().map_or_else(
                || "Panic occurred inside native msi-ffi library boundary".to_string(),
                Clone::clone,
            )
        },
        ToString::to_string,
    );
    set_last_error(MSI_ERROR_PANIC, msg);
    MSI_ERROR_PANIC
}

/// Trampoline helper to invoke a [`FnOnce`] closure passed as an untyped pointer.
///
/// # Safety
///
/// `data` must point to a valid [`std::mem::ManuallyDrop<F>`] and must be read exactly once.
unsafe fn trampoline<F: FnOnce() -> Result<i32, (i32, String)>>(
    data: *mut (),
) -> Result<i32, (i32, String)> {
    let actual_f = unsafe { ptr::read(data.cast::<F>()) };
    actual_f()
}

/// Internal non-generic executor for [`ffi_boundary`] that captures panics and error states.
fn ffi_boundary_impl(caller: fn(*mut ()) -> Result<i32, (i32, String)>, data: *mut ()) -> i32 {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| caller(data)));
    match res {
        Ok(Ok(code)) => handle_success(code),
        Ok(Err((code, msg))) => handle_error(code, msg),
        Err(panic_payload) => handle_panic(&*panic_payload),
    }
}

/// Encloses an FFI operation within a panic catch boundary and records errors.
///
/// # Arguments
///
/// * `f` - Closure executing the FFI operation.
///
/// # Returns
///
/// Numeric status code.
pub fn ffi_boundary<F>(f: F) -> i32
where
    F: FnOnce() -> Result<i32, (i32, String)> + std::panic::UnwindSafe,
{
    let mut slot = std::mem::ManuallyDrop::new(f);
    ffi_boundary_impl(
        |data| unsafe { trampoline::<F>(data) },
        (&raw mut slot).cast::<()>(),
    )
}

/// Converts a raw C string pointer into a borrowed UTF-8 string slice.
///
/// # Arguments
///
/// * `ptr` - Pointer to null-terminated C string.
/// * `name` - Descriptive argument identifier for diagnostic messages.
///
/// # Returns
///
/// Borrowed string slice on success.
///
/// # Errors
///
/// Returns `(MSI_ERROR_NULL_POINTER, msg)` if null, or `(MSI_ERROR_INVALID_ARGUMENT, msg)`
/// if non-UTF-8.
///
/// # Safety
///
/// `ptr` must point to valid memory up to and including the null terminator if non-null.
pub unsafe fn c_str_to_str<'a>(
    ptr: *const c_char,
    name: &'static str,
) -> Result<&'a str, (i32, String)> {
    unsafe {
        if ptr.is_null() {
            return Err((
                MSI_ERROR_NULL_POINTER,
                format!("Argument '{name}' must not be null"),
            ));
        }
        CStr::from_ptr(ptr).to_str().map_err(|e| {
            (
                MSI_ERROR_INVALID_ARGUMENT,
                format!("Argument '{name}' is not valid UTF-8: {e}"),
            )
        })
    }
}

/// Converts an optional raw C string pointer into an optional borrowed UTF-8 string slice.
///
/// If `ptr` is NULL, returns `Ok(None)`.
///
/// # Arguments
///
/// * `ptr` - Pointer to null-terminated C string or NULL.
/// * `name` - Descriptive argument identifier for diagnostic messages.
///
/// # Returns
///
/// `Some(&str)` if non-null, `None` if null.
///
/// # Errors
///
/// Returns `(MSI_ERROR_INVALID_ARGUMENT, msg)` if non-UTF-8.
///
/// # Safety
///
/// `ptr` must point to valid memory up to and including the null terminator if non-null.
pub unsafe fn c_str_to_opt_str<'a>(
    ptr: *const c_char,
    name: &'static str,
) -> Result<Option<&'a str>, (i32, String)> {
    unsafe {
        if ptr.is_null() {
            return Ok(None);
        }
        CStr::from_ptr(ptr).to_str().map(Some).map_err(|e| {
            (
                MSI_ERROR_INVALID_ARGUMENT,
                format!("Argument '{name}' is not valid UTF-8: {e}"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_error_lifecycle_and_clear() {
        clear_last_error();
        assert_eq!(msi_get_last_error_code(), MSI_SUCCESS);

        set_last_error(MSI_ERROR_VALIDATION, "validation test failure");
        assert_eq!(msi_get_last_error_code(), MSI_ERROR_VALIDATION);

        // Test with null out_written pointer
        // SAFETY: Testing null buffer and null out_written is supported and returns buffer too small without crash.
        let res_null_written =
            unsafe { msi_get_last_error_message(ptr::null_mut(), 0, ptr::null_mut()) };
        assert_eq!(res_null_written, MSI_ERROR_BUFFER_TOO_SMALL);

        let mut out_len = 0;
        // SAFETY: Pointer to out_len is valid.
        let res = unsafe { msi_get_last_error_message(ptr::null_mut(), 0, &raw mut out_len) };
        assert_eq!(res, MSI_ERROR_BUFFER_TOO_SMALL);
        assert_eq!(out_len, "validation test failure".len() + 1);

        // Test capacity too small with non-null buffer
        let mut tiny_buf = [0_i8; 2];
        // SAFETY: Buffer has capacity 2, which is smaller than out_len.
        let res_tiny = unsafe {
            msi_get_last_error_message(tiny_buf.as_mut_ptr(), tiny_buf.len(), &raw mut out_len)
        };
        assert_eq!(res_tiny, MSI_ERROR_BUFFER_TOO_SMALL);

        let mut buf = vec![0_i8; out_len];
        // SAFETY: Buffer has sufficient capacity out_len.
        let res2 =
            unsafe { msi_get_last_error_message(buf.as_mut_ptr(), buf.len(), &raw mut out_len) };
        assert_eq!(res2, MSI_SUCCESS);
        // SAFETY: buf is a valid null-terminated C string written by msi_get_last_error_message.
        let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
        assert_eq!(c_str.to_str().unwrap_or(""), "validation test failure");

        msi_clear_last_error();
        assert_eq!(msi_get_last_error_code(), MSI_SUCCESS);
    }

    #[test]
    fn test_ffi_boundary_success_and_error() {
        let ok_code = ffi_boundary(|| Ok(MSI_SUCCESS));
        assert_eq!(ok_code, MSI_SUCCESS);

        let err_code = ffi_boundary(|| Err((MSI_ERROR_IO, "disk I/O error".to_string())));
        assert_eq!(err_code, MSI_ERROR_IO);
        assert_eq!(msi_get_last_error_code(), MSI_ERROR_IO);
    }

    #[test]
    fn test_ffi_boundary_panic_catch() {
        // Panic with &str
        let code_str = ffi_boundary(|| std::panic::panic_any("str panic payload"));
        assert_eq!(code_str, MSI_ERROR_PANIC);
        let (_, msg_str) = get_last_error();
        assert!(msg_str.contains("str panic payload"));

        // Panic with String
        let code_string =
            ffi_boundary(|| std::panic::panic_any(String::from("owned string panic payload")));
        assert_eq!(code_string, MSI_ERROR_PANIC);
        let (_, msg_string) = get_last_error();
        assert!(msg_string.contains("owned string panic payload"));

        // Panic with non-string payload
        let code_int = ffi_boundary(|| std::panic::panic_any(42_i32));
        assert_eq!(code_int, MSI_ERROR_PANIC);
        let (_, msg_int) = get_last_error();
        assert!(msg_int.contains("Panic occurred inside native msi-ffi library boundary"));
    }

    #[test]
    fn test_c_str_helpers() {
        let valid = CString::new("hello world").unwrap_or_default();
        // SAFETY: Pointer is a valid CString.
        let s = unsafe { c_str_to_str(valid.as_ptr(), "test").unwrap_or("") };
        assert_eq!(s, "hello world");

        // SAFETY: Null pointer to c_str_to_str returns MSI_ERROR_NULL_POINTER.
        let null_res = unsafe { c_str_to_str(ptr::null(), "null_arg") };
        assert!(matches!(null_res, Err((MSI_ERROR_NULL_POINTER, _))));

        // Invalid UTF-8 in c_str_to_str
        let invalid_utf8 = [0xFF_u8, 0xFE, 0xFD, 0x00];
        // SAFETY: Valid null-terminated non-UTF8 bytes.
        let invalid_res =
            unsafe { c_str_to_str(invalid_utf8.as_ptr().cast::<c_char>(), "invalid_utf8_arg") };
        assert!(matches!(invalid_res, Err((MSI_ERROR_INVALID_ARGUMENT, _))));

        // SAFETY: Null pointer to c_str_to_opt_str returns Ok(None).
        let opt_null = unsafe { c_str_to_opt_str(ptr::null(), "opt_arg").unwrap_or(None) };
        assert!(opt_null.is_none());

        // SAFETY: Valid pointer to c_str_to_opt_str returns Ok(Some(...)).
        let opt_some = unsafe { c_str_to_opt_str(valid.as_ptr(), "opt_arg").unwrap_or(None) };
        assert_eq!(opt_some, Some("hello world"));

        // Invalid UTF-8 in c_str_to_opt_str
        // SAFETY: Valid null-terminated non-UTF8 bytes.
        let opt_invalid_res = unsafe {
            c_str_to_opt_str(
                invalid_utf8.as_ptr().cast::<c_char>(),
                "invalid_opt_utf8_arg",
            )
        };
        assert!(matches!(
            opt_invalid_res,
            Err((MSI_ERROR_INVALID_ARGUMENT, _))
        ));
    }

    #[test]
    fn test_map_msi_error() {
        assert_eq!(
            map_msi_error(&msi::Error::InvalidArgument {
                argument: "test".to_string(),
                reason: "bad".to_string()
            }),
            MSI_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            map_msi_error(&msi::Error::Validation {
                element: "test".to_string(),
                reason: "bad".to_string()
            }),
            MSI_ERROR_VALIDATION
        );
        assert_eq!(
            map_msi_error(&msi::Error::Io("io".to_string())),
            MSI_ERROR_IO
        );
        assert_eq!(
            map_msi_error(&msi::Error::InvalidCabSignature { found: [0; 4] }),
            MSI_ERROR_CABINET
        );
        assert_eq!(
            map_msi_error(&msi::Error::MissingTable {
                name: "test".to_string()
            }),
            MSI_ERROR_DATABASE
        );
        assert_eq!(
            map_msi_error(&msi::Error::WixLinker {
                message: "err".to_string()
            }),
            MSI_ERROR_WIX
        );
    }
}
