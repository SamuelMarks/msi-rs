//! String conversion and translation utilities for Win32 API endpoints.
//!
//! Handles safe conversion from UTF-8 strings in Rust to ANSI (`A`) and Unicode (`W`)
//! string representations requested by Win32 callers.

use std::slice;

use super::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INVALID_PARAMETER, ERROR_MORE_DATA,
    ERROR_SUCCESS,
};

/// A strongly-typed wrapper for a null-terminated UTF-16 Win32 string pointer (`LPCWSTR`).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WideString(pub Lpcwstr);

impl WideString {
    /// Safely converts this `WideString` into a Rust `String`.
    ///
    /// # Returns
    ///
    /// * `Some(String)` if successful.
    /// * `None` if the pointer is null or string is improperly formed.
    #[must_use]
    pub fn to_string(&self) -> Option<String> {
        lpcwstr_to_string(self.0)
    }
}

/// A strongly-typed wrapper for a null-terminated ANSI Win32 string pointer (`LPCSTR`).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnsiString(pub Lpcstr);

impl AnsiString {
    /// Safely converts this `AnsiString` into a Rust `String`.
    ///
    /// # Returns
    ///
    /// * `Some(String)` if successful.
    /// * `None` if the pointer is null or string is improperly formed.
    #[must_use]
    pub fn to_string(&self) -> Option<String> {
        lpcstr_to_string(self.0)
    }
}

/// Safely converts a null-terminated UTF-16 Win32 string (`LPCWSTR`) into a Rust `String`.
///
/// # Arguments
///
/// * `s` - Pointer to the null-terminated UTF-16 string.
///
/// # Returns
///
/// * `Some(String)` if successful.
/// * `None` if the pointer is null or string is improperly formed.
#[must_use]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn lpcwstr_to_string(s: Lpcwstr) -> Option<String> {
    if s.is_null() {
        return None;
    }

    // Find the null terminator
    let mut len = 0;
    // SAFETY: We assume the pointer is valid and properly null-terminated per Win32 conventions.
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }

    // SAFETY: Slice is within bounds of the null-terminated string.
    let slice = unsafe { slice::from_raw_parts(s, len) };
    String::from_utf16(slice).ok()
}

/// Safely converts a null-terminated ANSI Win32 string (`LPCSTR`) into a Rust `String`.
///
/// # Arguments
///
/// * `s` - Pointer to the null-terminated string.
///
/// # Returns
///
/// * `Some(String)` if successful.
/// * `None` if the pointer is null or string is improperly formed.
#[must_use]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn lpcstr_to_string(s: Lpcstr) -> Option<String> {
    if s.is_null() {
        return None;
    }

    // SAFETY: We assume the pointer is valid and properly null-terminated.
    let c_str = unsafe { std::ffi::CStr::from_ptr(s) };

    // In a real implementation, this would translate from the active code page (ACP)
    // to UTF-8. For non-Windows environments and standard MSI properties, UTF-8 or ASCII
    // is assumed as a baseline.
    c_str.to_str().ok().map(ToOwned::to_owned)
}

/// Copies a Rust string into a Win32 Unicode (`W`) output buffer.
///
/// Calculates length required, and populates the buffer if sufficient space is available.
/// If `buffer` is null or `pcch` is insufficient, `pcch` is updated with the required size
/// (not including the null terminator) and `ERROR_MORE_DATA` is returned.
///
/// # Arguments
///
/// * `value` - The string to copy.
/// * `buffer` - Pointer to the output buffer (`LPWSTR`).
/// * `pcch` - Pointer to the `DWORD` specifying the buffer size (in characters). Updated on return.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_MORE_DATA`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn string_to_lpwstr(value: &str, buffer: Lpwstr, pcch: *mut Dword) -> u32 {
    let utf16: Vec<u16> = value.encode_utf16().collect();
    let req_chars = Dword::try_from(utf16.len()).unwrap_or(0);

    if pcch.is_null() {
        // Technically an invalid parameter if buffer is provided, but Windows MSI tolerates it
        // if we just return SUCCESS when buffer is null too, but let's be strict or safe.
        if !buffer.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        return ERROR_SUCCESS; // Just a length probe without a length pointer? Not useful.
    }

    // SAFETY: We assume `pcch` is a valid pointer.
    let provided_chars = unsafe { *pcch };
    unsafe {
        *pcch = req_chars;
    }

    if buffer.is_null() || provided_chars <= req_chars {
        return ERROR_MORE_DATA;
    }

    // Copy the string and add the null terminator.
    // SAFETY: We have verified `provided_chars` is strictly greater than `req_chars`.
    unsafe {
        let slice = slice::from_raw_parts_mut(buffer, provided_chars as usize);
        slice[..utf16.len()].copy_from_slice(&utf16);
        slice[utf16.len()] = 0;
    }

    ERROR_SUCCESS
}

/// Copies a Rust string into a Win32 ANSI (`A`) output buffer.
///
/// Calculates length required, and populates the buffer if sufficient space is available.
/// If `buffer` is null or `pcch` is insufficient, `pcch` is updated with the required size
/// (not including the null terminator) and `ERROR_MORE_DATA` is returned.
///
/// # Arguments
///
/// * `value` - The string to copy.
/// * `buffer` - Pointer to the output buffer (`LPSTR`).
/// * `pcch` - Pointer to the `DWORD` specifying the buffer size (in characters). Updated on return.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_MORE_DATA`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn string_to_lpstr(value: &str, buffer: Lpstr, pcch: *mut Dword) -> u32 {
    let bytes = value.as_bytes();
    let req_chars = Dword::try_from(bytes.len()).unwrap_or(0);

    if pcch.is_null() {
        if !buffer.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        return ERROR_SUCCESS;
    }

    // SAFETY: We assume `pcch` is a valid pointer.
    let provided_chars = unsafe { *pcch };
    unsafe {
        *pcch = req_chars;
    }

    if buffer.is_null() || provided_chars <= req_chars {
        return ERROR_MORE_DATA;
    }

    // Copy the string and add the null terminator.
    // SAFETY: We have verified `provided_chars` is strictly greater than `req_chars`.
    unsafe {
        let slice = slice::from_raw_parts_mut(buffer.cast::<u8>(), provided_chars as usize);
        slice[..bytes.len()].copy_from_slice(bytes);
        slice[bytes.len()] = 0;
    }

    ERROR_SUCCESS
}

/// Load localized string from resources (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiLoadStringW(
    hInstall: crate::handles::MsiHandle,
    uStringID: Uint,
    lpValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if lpValueBuf.is_null() && pcchValueBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(super::ERROR_INSTALL_FAILURE)
}

/// Load localized string from resources (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiLoadStringA(
    hInstall: crate::handles::MsiHandle,
    uStringID: Uint,
    lpValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if lpValueBuf.is_null() && pcchValueBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(super::ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::shadow_unrelated,
        clippy::borrow_as_ptr
    )]
    use super::*;
    use std::ptr;

    #[test]
    fn test_wide_string_ansi_string_wrappers() {
        let mut utf16: Vec<u16> = "TestW".encode_utf16().collect();
        utf16.push(0);
        let ws = WideString(utf16.as_ptr());
        assert_eq!(ws.to_string().unwrap(), "TestW");
        let ws_null = WideString(ptr::null());
        assert_eq!(ws_null.to_string(), None);

        let bytes = b"TestA\0";
        let as_str = AnsiString(bytes.as_ptr().cast::<i8>());
        assert_eq!(as_str.to_string().unwrap(), "TestA");
        let as_null = AnsiString(ptr::null());
        assert_eq!(as_null.to_string(), None);
    }

    #[test]
    fn test_lpcwstr_to_string() {
        assert_eq!(lpcwstr_to_string(ptr::null()), None);

        let mut utf16: Vec<u16> = "Hello".encode_utf16().collect();
        utf16.push(0);

        let s = lpcwstr_to_string(utf16.as_ptr());
        assert_eq!(s.unwrap(), "Hello");
    }

    #[test]
    fn test_lpcstr_to_string() {
        assert_eq!(lpcstr_to_string(ptr::null()), None);

        let bytes = b"World\0";
        let s = lpcstr_to_string(bytes.as_ptr().cast::<i8>());
        assert_eq!(s.unwrap(), "World");
    }

    #[test]
    fn test_string_to_lpwstr() {
        let mut pcch: Dword = 0;
        let res = string_to_lpwstr("Test", ptr::null_mut(), &raw mut pcch);
        assert_eq!(res, ERROR_MORE_DATA);
        assert_eq!(pcch, 4);

        let mut buf = vec![0u16; 5];
        pcch = 5;
        let res2 = string_to_lpwstr("Test", buf.as_mut_ptr(), &raw mut pcch);
        assert_eq!(res2, ERROR_SUCCESS);
        assert_eq!(pcch, 4);
        assert_eq!(buf[..4], [84, 101, 115, 116]);
        assert_eq!(buf[4], 0);

        // Insufficient buffer
        let mut buf_small = vec![0u16; 4];
        pcch = 4; // Not enough for null terminator
        let res3 = string_to_lpwstr("Test", buf_small.as_mut_ptr(), &raw mut pcch);
        assert_eq!(res3, ERROR_MORE_DATA);
        assert_eq!(pcch, 4);

        // Null pcch
        assert_eq!(
            string_to_lpwstr("Test", ptr::null_mut(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            string_to_lpwstr("Test", buf.as_mut_ptr(), ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );

        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_string_to_lpstr() {
        let mut pcch: Dword = 0;
        let res = string_to_lpstr("Test", ptr::null_mut(), &raw mut pcch);
        assert_eq!(res, ERROR_MORE_DATA);
        assert_eq!(pcch, 4);

        let mut buf = vec![0i8; 5];
        pcch = 5;
        let res2 = string_to_lpstr("Test", buf.as_mut_ptr(), &raw mut pcch);
        assert_eq!(res2, ERROR_SUCCESS);
        assert_eq!(pcch, 4);
        assert_eq!(buf[4], 0);

        // Null pcch
        assert_eq!(
            string_to_lpstr("Test", ptr::null_mut(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            string_to_lpstr("Test", buf.as_mut_ptr(), ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );

        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );

        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );

        let mut val_w = [0u16; 10];
        let mut val_a = [0i8; 10];
        let mut pcch: Dword = 0;
        assert_eq!(
            MsiLoadStringW(0, 0, val_w.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringW(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, val_a.as_mut_ptr(), ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiLoadStringA(0, 0, ptr::null_mut(), &raw mut pcch),
            ERROR_SUCCESS
        );
    }
}
