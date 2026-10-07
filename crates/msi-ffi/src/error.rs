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

/// Maps an internal [`msi::MsiError`] to an FFI error code.
///
/// # Arguments
///
/// * `err` - Reference to internal error.
///
/// # Returns
///
/// Corresponding FFI error code constant.
#[must_use]
pub const fn map_msi_error(err: &msi::MsiError) -> i32 {
    match err {
        msi::MsiError::Validation { .. } => MSI_ERROR_VALIDATION,
        msi::MsiError::Io(_) => MSI_ERROR_IO,
        msi::MsiError::InvalidCabSignature { .. }
        | msi::MsiError::InvalidCabVersion { .. }
        | msi::MsiError::InvalidCabChecksum { .. }
        | msi::MsiError::InvalidCabData { .. }
        | msi::MsiError::DecompressionFailed { .. }
        | msi::MsiError::CompressionFailed { .. }
        | msi::MsiError::CabinetFileNotFound { .. } => MSI_ERROR_CABINET,
        msi::MsiError::MissingTable { .. }
        | msi::MsiError::RecordLengthMismatch { .. }
        | msi::MsiError::InvalidStringPool { .. }
        | msi::MsiError::StringPoolIndexOutOfBounds { .. }
        | msi::MsiError::InvalidSummaryInfo { .. }
        | msi::MsiError::InvalidColumnType { .. } => MSI_ERROR_DATABASE,
        msi::MsiError::Preprocessor { .. }
        | msi::MsiError::XmlParse { .. }
        | msi::MsiError::WixCompiler { .. }
        | msi::MsiError::InvalidWixObject { .. }
        | msi::MsiError::WixLinker { .. }
        | msi::MsiError::IceValidation { .. }
        | msi::MsiError::PatchXmlParse { .. }
        | msi::MsiError::PatchCorruptCab { .. } => MSI_ERROR_WIX,
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

/// Generic failure.
pub const E_FAIL: i32 = -2147467259; // 0x80004005

/// Invalid argument.
pub const E_INVALIDARG: i32 = -2147024809; // 0x80070057

/// File not found.
pub const STG_E_FILENOTFOUND: i32 = -2147287038; // 0x80030002

/// Read fault.
pub const STG_E_READFAULT: i32 = -2147287010; // 0x8003001E

/// Not implemented.
pub const E_NOTIMPL: i32 = -2147467263; // 0x80004001

/// COM class string error.
pub const CO_E_CLASSSTRING: i32 = -2147221005; // 0x800401F3

/// Maps an internal [`msi::MsiError`] to an FFI HRESULT error code.
///
/// # Arguments
///
/// * `err` - Reference to internal error.
///
/// # Returns
///
/// Corresponding COM HRESULT.
#[must_use]
pub const fn map_msi_error_to_hresult(err: &msi::MsiError) -> i32 {
    match err {
        msi::MsiError::Io { .. } => STG_E_READFAULT,
        msi::MsiError::InvalidArgument { .. } => E_INVALIDARG,
        msi::MsiError::MissingTable { .. } => STG_E_FILENOTFOUND,
        msi::MsiError::FileNotFound { .. } => STG_E_FILENOTFOUND,
        msi::MsiError::Validation { .. } => E_FAIL,
        msi::MsiError::Unsupported { .. } => E_NOTIMPL,
        msi::MsiError::Sql { .. } => E_FAIL,
        msi::MsiError::InvalidCfbSignature { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbClsid { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbMinorVersion { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbMajorVersion { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbByteOrder { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbSectorShift { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbMiniSectorShift { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbReserved { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbDirectorySectors { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCfbMiniStreamCutoff { .. } => E_INVALIDARG,
        msi::MsiError::InvalidSector { .. } => E_INVALIDARG,
        msi::MsiError::SectorChainCycle { .. } => E_FAIL,
        msi::MsiError::InvalidDirectoryEntry { .. } => E_INVALIDARG,
        msi::MsiError::StreamNotFound { .. } => STG_E_FILENOTFOUND,
        msi::MsiError::DuplicateDirectoryEntry { .. } => E_FAIL,
        msi::MsiError::CfbCorrupted { .. } => E_FAIL,
        msi::MsiError::InvalidStreamName { .. } => E_INVALIDARG,
        msi::MsiError::StreamSizeMismatch { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCabSignature { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCabVersion { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCabChecksum { .. } => E_INVALIDARG,
        msi::MsiError::InvalidCabData { .. } => E_INVALIDARG,
        msi::MsiError::DecompressionFailed { .. } => E_FAIL,
        msi::MsiError::CompressionFailed { .. } => E_FAIL,
        msi::MsiError::CabinetFileNotFound { .. } => STG_E_FILENOTFOUND,
        msi::MsiError::InvalidColumnType { .. } => E_INVALIDARG,
        msi::MsiError::InvalidStringPool { .. } => E_INVALIDARG,
        msi::MsiError::StringPoolIndexOutOfBounds { .. } => E_INVALIDARG,
        msi::MsiError::InvalidSummaryInfo { .. } => E_INVALIDARG,
        msi::MsiError::RecordLengthMismatch { .. } => E_INVALIDARG,
        msi::MsiError::Preprocessor { .. } => E_FAIL,
        msi::MsiError::XmlParse { .. } => E_FAIL,
        msi::MsiError::WixCompiler { .. } => E_FAIL,
        msi::MsiError::InvalidWixObject { .. } => E_INVALIDARG,
        msi::MsiError::WixLinker { .. } => E_FAIL,
        msi::MsiError::WixExtension { .. } => E_FAIL,
        msi::MsiError::ExtensionXmlParse { .. } => E_FAIL,
        msi::MsiError::LinkerPayloadError { .. } => E_FAIL,
        msi::MsiError::CustomActionBridgeError { .. } => E_FAIL,
        msi::MsiError::IceValidation { .. } => E_FAIL,
        msi::MsiError::ExecutionFailed { .. } => E_FAIL,
        msi::MsiError::RollbackFailed { .. } => E_FAIL,
        msi::MsiError::TransactionStateMismatch { .. } => E_INVALIDARG,
        msi::MsiError::DiskCostExceeded { .. } => E_FAIL,
        msi::MsiError::CustomActionFailed { .. } => E_FAIL,
        msi::MsiError::ScriptError { .. } => E_FAIL,
        msi::MsiError::UiError { .. } => E_FAIL,
        msi::MsiError::ScriptRuntimeError { .. } => E_FAIL,
        msi::MsiError::WorkerIpcError { .. } => E_FAIL,
        msi::MsiError::BootHarnessError { .. } => E_FAIL,
        msi::MsiError::ConsoleInitError { .. } => E_FAIL,
        msi::MsiError::UkiPackageError { .. } => E_FAIL,
        msi::MsiError::BlockDeviceError { .. } => E_FAIL,
        msi::MsiError::PartitionError { .. } => E_FAIL,
        msi::MsiError::FileSystemFormatError { .. } => E_FAIL,
        msi::MsiError::SysrootMountError { .. } => E_FAIL,
        msi::MsiError::RegistryHiveError { .. } => E_FAIL,
        msi::MsiError::DriverServicingError { .. } => E_FAIL,
        msi::MsiError::BootloaderError { .. } => E_FAIL,
        msi::MsiError::UnattendError { .. } => E_FAIL,
        msi::MsiError::UnsupportedPlatform { .. } => E_NOTIMPL,
        msi::MsiError::UnsupportedPlatformFeature { .. } => E_NOTIMPL,
        msi::MsiError::GuiError { .. } => E_FAIL,
        msi::MsiError::NetworkConfigError { .. } => E_FAIL,
        msi::MsiError::UserProvisioningError { .. } => E_FAIL,
        msi::MsiError::LiveMediaError { .. } => E_FAIL,
        msi::MsiError::BurnBundleError { .. } => E_FAIL,
        msi::MsiError::Chainer { .. } => E_FAIL,
        msi::MsiError::SqlProvisioning { .. } => E_FAIL,
        msi::MsiError::ServiceConfiguration { .. } => E_FAIL,
        msi::MsiError::InvalidArchitecture { .. } => E_INVALIDARG,
        msi::MsiError::InvalidSummaryTemplate { .. } => E_INVALIDARG,
        msi::MsiError::InvalidStorageClsid { .. } => E_INVALIDARG,
        msi::MsiError::WimInvalidMagic { .. } => E_INVALIDARG,
        msi::MsiError::WimChecksumMismatch { .. } => E_INVALIDARG,
        msi::MsiError::WimDecompressionError { .. } => E_FAIL,
        msi::MsiError::WimXmlParseError { .. } => E_FAIL,
        msi::MsiError::PhysicalLayoutError { .. } => E_FAIL,
        msi::MsiError::DataIntegrityError { .. } => E_FAIL,
        msi::MsiError::SystemConfigurationError { .. } => E_FAIL,
        msi::MsiError::OdbcError { .. } => E_FAIL,
        msi::MsiError::FontRegistrationError { .. } => E_FAIL,
        msi::MsiError::SystemdError { .. } => E_FAIL,
        msi::MsiError::LaunchdError { .. } => E_FAIL,
        msi::MsiError::SmfError { .. } => E_FAIL,
        msi::MsiError::RcError { .. } => E_FAIL,
        msi::MsiError::IpcError { .. } => E_FAIL,
        msi::MsiError::RegistryError { .. } => E_FAIL,
        msi::MsiError::FileOperationError { .. } => STG_E_READFAULT,
        msi::MsiError::OdbcConfigError { .. } => E_FAIL,
        msi::MsiError::ComRpcError { .. } => CO_E_CLASSSTRING,
        msi::MsiError::MsiServerError { .. } => E_FAIL,
        msi::MsiError::AssemblyError { .. } => E_FAIL,
        msi::MsiError::SxSError { .. } => E_FAIL,
        msi::MsiError::User32RenderError { .. } => E_FAIL,
        msi::MsiError::DispatchBridgeError { .. } => CO_E_CLASSSTRING,
        msi::MsiError::ActiveXError { .. } => CO_E_CLASSSTRING,
        msi::MsiError::PatchApplyError { .. } => E_FAIL,
        msi::MsiError::DeltaDecodeError { .. } => E_FAIL,
        msi::MsiError::ActionExecutionError { .. } => E_FAIL,
        msi::MsiError::LocatorError { .. } => E_FAIL,
        msi::MsiError::AssemblyResolutionError { .. } => E_FAIL,
        msi::MsiError::DatabaseMergeError { .. } => E_FAIL,
        msi::MsiError::DatabaseExportError { .. } => E_FAIL,
        msi::MsiError::SourceListError { .. } => E_FAIL,
        msi::MsiError::AdvertisementError { .. } => E_FAIL,
        msi::MsiError::PatchApplicationError { .. } => E_FAIL,
        msi::MsiError::UiPreviewError { .. } => E_FAIL,
        msi::MsiError::LoggingCallbackError { .. } => E_FAIL,
        msi::MsiError::ComRegistrationError { .. } => E_FAIL,
        msi::MsiError::IisConfigurationError { .. } => E_FAIL,
        msi::MsiError::FirewallConfigError { .. } => E_FAIL,
        msi::MsiError::UserManagementError { .. } => E_FAIL,
        msi::MsiError::TransformConflict { .. } => 1624,
        msi::MsiError::InvalidTransform { .. } => 1624,
        msi::MsiError::PatchSuperseded(_) => 1648,
        msi::MsiError::WrongPatchBaseline(_) => 1642,
        msi::MsiError::InvalidPatchSequence(_) => 1643,
        msi::MsiError::PatchXmlParse { .. } => E_FAIL,
        msi::MsiError::PatchCorruptCab { .. } => STG_E_READFAULT,
    }
}

/// Maps an internal [`msi::MsiError`] to an FFI LSTATUS (Win32 Error) code.
///
/// # Arguments
///
/// * `err` - Reference to internal error.
///
/// # Returns
///
/// Corresponding Win32 LSTATUS code.
#[must_use]
pub const fn map_msi_error_to_lstatus(err: &msi::MsiError) -> u32 {
    match err {
        msi::MsiError::Io { .. } => crate::win32::ERROR_OPEN_FAILED,
        msi::MsiError::InvalidArgument { .. } => crate::win32::ERROR_INVALID_PARAMETER,
        msi::MsiError::MissingTable { .. } => crate::win32::ERROR_FILE_NOT_FOUND,
        msi::MsiError::FileNotFound { .. } => crate::win32::ERROR_FILE_NOT_FOUND,
        msi::MsiError::Validation { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::Unsupported { .. } => crate::win32::ERROR_NOT_SUPPORTED,
        msi::MsiError::Sql { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::InvalidCfbSignature { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbClsid { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbMinorVersion { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbMajorVersion { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbByteOrder { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbSectorShift { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbMiniSectorShift { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbReserved { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbDirectorySectors { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCfbMiniStreamCutoff { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidSector { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::SectorChainCycle { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::InvalidDirectoryEntry { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::StreamNotFound { .. } => crate::win32::ERROR_FILE_NOT_FOUND,
        msi::MsiError::DuplicateDirectoryEntry { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::CfbCorrupted { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::InvalidStreamName { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::StreamSizeMismatch { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCabSignature { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCabVersion { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCabChecksum { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidCabData { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::DecompressionFailed { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::CompressionFailed { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::CabinetFileNotFound { .. } => crate::win32::ERROR_FILE_NOT_FOUND,
        msi::MsiError::InvalidColumnType { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidStringPool { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::StringPoolIndexOutOfBounds { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidSummaryInfo { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::RecordLengthMismatch { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::Preprocessor { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::XmlParse { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::WixCompiler { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::InvalidWixObject { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::WixLinker { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::WixExtension { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ExtensionXmlParse { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::LinkerPayloadError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::CustomActionBridgeError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::IceValidation { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ExecutionFailed { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::RollbackFailed { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::TransactionStateMismatch { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::DiskCostExceeded { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::CustomActionFailed { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ScriptError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UiError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ScriptRuntimeError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::WorkerIpcError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::BootHarnessError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ConsoleInitError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UkiPackageError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::BlockDeviceError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::PartitionError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::FileSystemFormatError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SysrootMountError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::RegistryHiveError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::DriverServicingError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::BootloaderError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UnattendError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UnsupportedPlatform { .. } => crate::win32::ERROR_NOT_SUPPORTED,
        msi::MsiError::UnsupportedPlatformFeature { .. } => crate::win32::ERROR_NOT_SUPPORTED,
        msi::MsiError::GuiError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::NetworkConfigError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UserProvisioningError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::LiveMediaError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::BurnBundleError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::Chainer { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SqlProvisioning { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ServiceConfiguration { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::InvalidArchitecture { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidSummaryTemplate { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::InvalidStorageClsid { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::WimInvalidMagic { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::WimChecksumMismatch { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::WimDecompressionError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::WimXmlParseError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::PhysicalLayoutError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::DataIntegrityError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SystemConfigurationError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::OdbcError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::FontRegistrationError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SystemdError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::LaunchdError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SmfError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::RcError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::IpcError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::RegistryError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::FileOperationError { .. } => crate::win32::ERROR_OPEN_FAILED,
        msi::MsiError::OdbcConfigError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ComRpcError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::MsiServerError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::AssemblyError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SxSError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::User32RenderError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::DispatchBridgeError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ActiveXError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::PatchApplyError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::DeltaDecodeError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ActionExecutionError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::LocatorError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::AssemblyResolutionError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::DatabaseMergeError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::DatabaseExportError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::SourceListError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::AdvertisementError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::PatchApplicationError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UiPreviewError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::LoggingCallbackError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::ComRegistrationError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::IisConfigurationError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::FirewallConfigError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::UserManagementError { .. } => crate::win32::ERROR_INSTALL_FAILURE,
        msi::MsiError::TransformConflict { .. } => 1624,
        msi::MsiError::InvalidTransform { .. } => 1624,
        msi::MsiError::PatchSuperseded(_) => 1648,
        msi::MsiError::WrongPatchBaseline(_) => 1642,
        msi::MsiError::InvalidPatchSequence(_) => 1643,
        msi::MsiError::PatchXmlParse { .. } => crate::win32::ERROR_INVALID_DATA,
        msi::MsiError::PatchCorruptCab { .. } => crate::win32::ERROR_INVALID_DATA,
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
            map_msi_error(&msi::MsiError::InvalidArgument {
                argument: "test".to_string(),
                reason: "bad".to_string()
            }),
            MSI_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            map_msi_error(&msi::MsiError::Validation {
                element: "test".to_string(),
                reason: "bad".to_string()
            }),
            MSI_ERROR_VALIDATION
        );
        assert_eq!(
            map_msi_error(&msi::MsiError::Io("io".to_string())),
            MSI_ERROR_IO
        );
        assert_eq!(
            map_msi_error(&msi::MsiError::InvalidCabSignature { found: [0; 4] }),
            MSI_ERROR_CABINET
        );
        assert_eq!(
            map_msi_error(&msi::MsiError::MissingTable {
                name: "test".to_string()
            }),
            MSI_ERROR_DATABASE
        );
        assert_eq!(
            map_msi_error(&msi::MsiError::WixLinker {
                message: "err".to_string()
            }),
            MSI_ERROR_WIX
        );
    }

    #[test]
    fn test_map_msi_error_to_hresult_all() {
        let errs = vec![
            msi::MsiError::Io("io".to_string()),
            msi::MsiError::InvalidArgument {
                argument: String::new(),
                reason: String::new(),
            },
            msi::MsiError::MissingTable {
                name: String::new(),
            },
            msi::MsiError::FileNotFound {
                path: String::new(),
            },
            msi::MsiError::Validation {
                element: String::new(),
                reason: String::new(),
            },
            msi::MsiError::Unsupported {
                name: String::new(),
            },
            msi::MsiError::Sql {
                message: String::new(),
            },
            msi::MsiError::InvalidCfbSignature { found: [0; 8] },
            msi::MsiError::InvalidCfbClsid { found: [0; 16] },
            msi::MsiError::InvalidCfbMinorVersion { found: 0 },
            msi::MsiError::InvalidCfbMajorVersion { found: 0 },
            msi::MsiError::InvalidCfbByteOrder { found: 0 },
            msi::MsiError::InvalidCfbSectorShift {
                major_version: 0,
                shift: 0,
            },
            msi::MsiError::InvalidCfbMiniSectorShift { shift: 0 },
            msi::MsiError::InvalidCfbReserved { found: [0; 6] },
            msi::MsiError::InvalidCfbDirectorySectors {
                major_version: 0,
                count: 0,
            },
            msi::MsiError::InvalidCfbMiniStreamCutoff { cutoff: 0 },
            msi::MsiError::InvalidSector {
                sector: 0,
                reason: String::new(),
            },
            msi::MsiError::SectorChainCycle { sector: 0 },
            msi::MsiError::InvalidDirectoryEntry {
                index: 0,
                reason: String::new(),
            },
            msi::MsiError::StreamNotFound {
                name: String::new(),
            },
            msi::MsiError::DuplicateDirectoryEntry {
                name: String::new(),
            },
            msi::MsiError::CfbCorrupted {
                offset: 0,
                reason: String::new(),
            },
            msi::MsiError::InvalidStreamName {
                name: String::new(),
                reason: String::new(),
            },
            msi::MsiError::StreamSizeMismatch {
                expected: 0,
                actual: 0,
            },
            msi::MsiError::InvalidCabSignature { found: [0; 4] },
            msi::MsiError::InvalidCabVersion { major: 0, minor: 0 },
            msi::MsiError::InvalidCabChecksum {
                expected: 0,
                actual: 0,
            },
            msi::MsiError::InvalidCabData {
                reason: String::new(),
            },
            msi::MsiError::DecompressionFailed {
                method: String::new(),
                reason: String::new(),
            },
            msi::MsiError::CompressionFailed {
                method: String::new(),
                reason: String::new(),
            },
            msi::MsiError::CabinetFileNotFound {
                name: String::new(),
            },
            msi::MsiError::InvalidColumnType { raw: 0 },
            msi::MsiError::InvalidStringPool {
                reason: String::new(),
            },
            msi::MsiError::StringPoolIndexOutOfBounds { index: 0, max: 0 },
            msi::MsiError::InvalidSummaryInfo {
                reason: String::new(),
            },
            msi::MsiError::RecordLengthMismatch {
                expected: 0,
                actual: 0,
            },
            msi::MsiError::Preprocessor {
                line: 0,
                column: 0,
                message: String::new(),
            },
            msi::MsiError::XmlParse {
                line: 0,
                column: 0,
                message: String::new(),
            },
            msi::MsiError::WixCompiler {
                element: String::new(),
                message: String::new(),
            },
            msi::MsiError::InvalidWixObject {
                reason: String::new(),
            },
            msi::MsiError::WixLinker {
                message: String::new(),
            },
            msi::MsiError::WixExtension {
                extension: String::new(),
                message: String::new(),
            },
            msi::MsiError::ExtensionXmlParse {
                extension: String::new(),
                reason: String::new(),
            },
            msi::MsiError::LinkerPayloadError {
                payload_id: String::new(),
                reason: String::new(),
            },
            msi::MsiError::CustomActionBridgeError {
                action: String::new(),
                reason: String::new(),
            },
            msi::MsiError::IceValidation {
                ice: String::new(),
                message: String::new(),
            },
            msi::MsiError::ExecutionFailed {
                action: String::new(),
                return_code: 0,
                message: String::new(),
            },
            msi::MsiError::RollbackFailed {
                action: String::new(),
                reason: String::new(),
            },
            msi::MsiError::TransactionStateMismatch {
                expected: String::new(),
                actual: String::new(),
            },
            msi::MsiError::DiskCostExceeded {
                volume: String::new(),
                required_bytes: 0,
                available_bytes: 0,
            },
            msi::MsiError::CustomActionFailed {
                action: String::new(),
                reason: String::new(),
            },
            msi::MsiError::ScriptError {
                opcode: String::new(),
                reason: String::new(),
            },
            msi::MsiError::UiError {
                dialog: String::new(),
                control: String::new(),
                reason: String::new(),
            },
            msi::MsiError::ScriptRuntimeError {
                line: 0,
                col: 0,
                message: String::new(),
            },
            msi::MsiError::WorkerIpcError {
                reason: String::new(),
            },
            msi::MsiError::BootHarnessError {
                recipe: String::new(),
                reason: String::new(),
            },
            msi::MsiError::ConsoleInitError {
                device: String::new(),
                reason: String::new(),
            },
            msi::MsiError::UkiPackageError {
                reason: String::new(),
            },
            msi::MsiError::BlockDeviceError {
                path: String::new(),
                reason: String::new(),
            },
            msi::MsiError::PartitionError {
                reason: String::new(),
            },
            msi::MsiError::FileSystemFormatError {
                fs_type: String::new(),
                reason: String::new(),
            },
            msi::MsiError::SysrootMountError {
                path: String::new(),
                reason: String::new(),
            },
            msi::MsiError::RegistryHiveError {
                hive: String::new(),
                reason: String::new(),
            },
            msi::MsiError::DriverServicingError {
                inf: String::new(),
                reason: String::new(),
            },
            msi::MsiError::BootloaderError {
                target: String::new(),
                reason: String::new(),
            },
            msi::MsiError::UnattendError {
                reason: String::new(),
            },
            msi::MsiError::UnsupportedPlatform {
                platform: String::new(),
                reason: String::new(),
            },
            msi::MsiError::UnsupportedPlatformFeature {
                feature: String::new(),
                target_os: msi::platform::TargetOs::Windows,
                reason: String::new(),
            },
            msi::MsiError::GuiError {
                reason: String::new(),
            },
            msi::MsiError::NetworkConfigError {
                reason: String::new(),
            },
            msi::MsiError::UserProvisioningError {
                reason: String::new(),
            },
            msi::MsiError::LiveMediaError {
                reason: String::new(),
            },
            msi::MsiError::BurnBundleError {
                reason: String::new(),
            },
            msi::MsiError::Chainer(String::new()),
            msi::MsiError::SqlProvisioning(String::new()),
            msi::MsiError::ServiceConfiguration(String::new()),
            msi::MsiError::InvalidArchitecture {
                name: String::new(),
            },
            msi::MsiError::InvalidSummaryTemplate {
                template: String::new(),
                reason: String::new(),
            },
            msi::MsiError::InvalidStorageClsid {
                clsid: String::new(),
            },
            msi::MsiError::WimInvalidMagic { magic: [0; 8] },
            msi::MsiError::WimChecksumMismatch {
                expected: String::new(),
                actual: String::new(),
            },
            msi::MsiError::WimDecompressionError {
                algorithm: String::new(),
                reason: String::new(),
            },
            msi::MsiError::WimXmlParseError {
                reason: String::new(),
            },
            msi::MsiError::PhysicalLayoutError {
                reason: String::new(),
            },
            msi::MsiError::DataIntegrityError {
                reason: String::new(),
            },
            msi::MsiError::SystemConfigurationError(String::new()),
            msi::MsiError::OdbcError(String::new()),
            msi::MsiError::FontRegistrationError(String::new()),
            msi::MsiError::SystemdError(String::new()),
            msi::MsiError::LaunchdError(String::new()),
            msi::MsiError::SmfError(String::new()),
            msi::MsiError::RcError(String::new()),
            msi::MsiError::IpcError(String::new()),
            msi::MsiError::RegistryError(String::new()),
            msi::MsiError::FileOperationError(String::new()),
            msi::MsiError::OdbcConfigError(String::new()),
            msi::MsiError::ComRpcError(String::new()),
            msi::MsiError::MsiServerError(String::new()),
            msi::MsiError::AssemblyError(String::new()),
            msi::MsiError::SxSError(String::new()),
            msi::MsiError::User32RenderError(String::new()),
            msi::MsiError::DispatchBridgeError(String::new()),
            msi::MsiError::ActiveXError(String::new()),
            msi::MsiError::PatchApplyError(String::new()),
            msi::MsiError::DeltaDecodeError(String::new()),
            msi::MsiError::ActionExecutionError(String::new()),
            msi::MsiError::LocatorError(String::new()),
            msi::MsiError::AssemblyResolutionError(String::new()),
            msi::MsiError::DatabaseMergeError(String::new()),
            msi::MsiError::DatabaseExportError(String::new()),
            msi::MsiError::SourceListError(String::new()),
            msi::MsiError::AdvertisementError(String::new()),
            msi::MsiError::PatchApplicationError(String::new()),
            msi::MsiError::UiPreviewError(String::new()),
            msi::MsiError::LoggingCallbackError(String::new()),
            msi::MsiError::ComRegistrationError(String::new()),
            msi::MsiError::IisConfigurationError(String::new()),
            msi::MsiError::FirewallConfigError(String::new()),
            msi::MsiError::UserManagementError(String::new()),
            msi::MsiError::TransformConflict {
                reason: String::new(),
            },
            msi::MsiError::InvalidTransform {
                reason: String::new(),
            },
            msi::MsiError::PatchSuperseded(String::new()),
            msi::MsiError::WrongPatchBaseline(String::new()),
            msi::MsiError::InvalidPatchSequence(String::new()),
            msi::MsiError::PatchXmlParse {
                reason: String::new(),
            },
            msi::MsiError::PatchCorruptCab {
                reason: String::new(),
            },
        ];
        for err in &errs {
            let _ = map_msi_error_to_hresult(err);
            let _ = map_msi_error_to_lstatus(err);
        }
    }

    #[test]
    fn test_map_msi_error_to_hresult() {
        assert_eq!(
            map_msi_error_to_hresult(&msi::MsiError::InvalidArgument {
                argument: "test".to_string(),
                reason: "bad".to_string()
            }),
            E_INVALIDARG
        );
        assert_eq!(
            map_msi_error_to_hresult(&msi::MsiError::CabinetFileNotFound {
                name: "test".to_string(),
            }),
            STG_E_FILENOTFOUND
        );
        assert_eq!(
            map_msi_error_to_hresult(&msi::MsiError::Io("io".to_string())),
            STG_E_READFAULT
        );
        assert_eq!(
            map_msi_error_to_hresult(&msi::MsiError::Unsupported {
                name: "test".to_string(),
            }),
            E_NOTIMPL
        );
    }

    #[test]
    fn test_map_msi_error_to_lstatus() {
        assert_eq!(
            map_msi_error_to_lstatus(&msi::MsiError::InvalidArgument {
                argument: "test".to_string(),
                reason: "bad".to_string()
            }),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            map_msi_error_to_lstatus(&msi::MsiError::CabinetFileNotFound {
                name: "test".to_string(),
            }),
            crate::win32::ERROR_FILE_NOT_FOUND
        );
        assert_eq!(
            map_msi_error_to_lstatus(&msi::MsiError::Io("io".to_string())),
            crate::win32::ERROR_OPEN_FAILED
        );
        assert_eq!(
            map_msi_error_to_lstatus(&msi::MsiError::Unsupported {
                name: "test".to_string(),
            }),
            crate::win32::ERROR_NOT_SUPPORTED
        );
    }
}
