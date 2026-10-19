//! Summary Information and Database Commit Win32 `stdcall` API endpoints.
//!
//! Provides `MsiGetSummaryInformationA/W`, `MsiSummaryInfoGetPropertyA/W`,
//! `MsiSummaryInfoSetPropertyA/W`, `MsiSummaryInfoGetPropertyCount`,
//! `MsiSummaryInfoPersist`, and `MsiDatabaseCommit`.

use std::panic;

use crate::handles::{alloc_handle, with_handle, with_handle_mut, MsiHandle, MsiObject};
use crate::types::MsiSummaryInfoHandle;
use crate::win32::database::MSIDBSTATE_WRITE;
use crate::win32::strings::{
    lpcstr_to_string, lpcwstr_to_string, string_to_lpstr, string_to_lpwstr,
};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_FUNCTION_FAILED, ERROR_INSTALL_FAILURE,
    ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
};
use msi::database::summary_info::{
    SummaryInfo, PID_APPNAME, PID_AUTHOR, PID_CHARCOUNT, PID_CODEPAGE, PID_COMMENTS,
    PID_CREATE_DTM, PID_KEYWORDS, PID_LASTAUTHOR, PID_LASTPRINTED, PID_LASTSAVE_DTM, PID_PAGECOUNT,
    PID_REVNUMBER, PID_SECURITY, PID_SUBJECT, PID_TEMPLATE, PID_TITLE, PID_WORDCOUNT, VT_FILETIME,
    VT_I2, VT_I4, VT_LPSTR,
};

/// Error code returned when a summary property ID is unrecognized (`ERROR_UNKNOWN_PROPERTY`).
const ERROR_UNKNOWN_PROPERTY: Uint = 1608;

/// Error code returned when the summary property data type mismatches (`ERROR_DATATYPE_MISMATCH`).
const ERROR_DATATYPE_MISMATCH: Uint = 1629;

/// OLE Variant type representing an empty/null property (`VT_EMPTY`).
const VT_EMPTY: Uint = 0;

/// Shared implementation for `MsiGetSummaryInformationA` and `MsiGetSummaryInformationW`.
fn get_summary_information(
    h_database: MsiHandle,
    path: Option<String>,
    ui_update_count: Uint,
    ph_summary_info: *mut MsiHandle,
) -> Uint {
    let (summary_info, resolved_path) = if h_database != 0 {
        let Some(res) = with_handle(h_database, |obj| {
            if let MsiObject::Database(db_handle) = obj {
                Ok((
                    db_handle.inner.summary_info.clone(),
                    db_handle.inner.backing_path.clone(),
                ))
            } else {
                Err(ERROR_INVALID_HANDLE)
            }
        }) else {
            return ERROR_INVALID_HANDLE;
        };
        match res {
            Ok(pair) => pair,
            Err(code) => return code,
        }
    } else if let Some(p) = path {
        let si = msi::package::Package::open(&p)
            .map(|pkg| pkg.summary_info().clone())
            .unwrap_or_default();
        (si, Some(p))
    } else {
        (SummaryInfo::default(), None)
    };

    let handle_obj = MsiObject::SummaryInfo(MsiSummaryInfoHandle {
        inner: summary_info,
        db_handle: h_database,
        path: resolved_path,
        update_count: ui_update_count,
    });

    // SAFETY: Caller verified `ph_summary_info` is not null.
    unsafe {
        *ph_summary_info = alloc_handle(handle_obj);
    }
    ERROR_SUCCESS
}

/// Obtains a handle for the `\005SummaryInformation` stream for a Windows Installer database (Unicode).
///
/// # Arguments
///
/// * `hDatabase` - Handle to the open database, or `0` if opening via `szDatabasePath`.
/// * `szDatabasePath` - Null-terminated UTF-16 path to the database on disk when `hDatabase == 0`.
/// * `uiUpdateCount` - Maximum number of updated values allowed before persisting.
/// * `phSummaryInfo` - Output pointer receiving the allocated summary information handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiGetSummaryInformationW(
    hDatabase: MsiHandle,
    szDatabasePath: Lpcwstr,
    uiUpdateCount: Uint,
    phSummaryInfo: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phSummaryInfo.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let path = lpcwstr_to_string(szDatabasePath);
        get_summary_information(hDatabase, path, uiUpdateCount, phSummaryInfo)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Obtains a handle for the `\005SummaryInformation` stream for a Windows Installer database (ANSI).
///
/// # Arguments
///
/// * `hDatabase` - Handle to the open database, or `0` if opening via `szDatabasePath`.
/// * `szDatabasePath` - Null-terminated ANSI path to the database on disk when `hDatabase == 0`.
/// * `uiUpdateCount` - Maximum number of updated values allowed before persisting.
/// * `phSummaryInfo` - Output pointer receiving the allocated summary information handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiGetSummaryInformationA(
    hDatabase: MsiHandle,
    szDatabasePath: Lpcstr,
    uiUpdateCount: Uint,
    phSummaryInfo: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phSummaryInfo.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let path = lpcstr_to_string(szDatabasePath);
        get_summary_information(hDatabase, path, uiUpdateCount, phSummaryInfo)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Represents a resolved property value from a [`SummaryInfo`] struct.
enum SummaryPropValue {
    /// Empty / unset property (`VT_EMPTY`).
    Empty,
    /// 16-bit integer property (`VT_I2`).
    I2(i32),
    /// 32-bit integer property (`VT_I4`).
    I4(i32),
    /// 64-bit Windows `FILETIME` property (`VT_FILETIME`).
    FileTime(u64),
    /// String property (`VT_LPSTR`).
    LpStr(String),
}

/// Reads a single property from a [`SummaryInfo`] by its OLE property ID (`PID_*`).
fn extract_summary_property(info: &SummaryInfo, prop_id: Uint) -> Result<SummaryPropValue, Uint> {
    let val = match prop_id {
        PID_CODEPAGE => info.codepage.map_or(SummaryPropValue::Empty, |v| {
            SummaryPropValue::I2(i32::from(v))
        }),
        PID_TITLE => info
            .title
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_SUBJECT => info
            .subject
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_AUTHOR => info
            .author
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_KEYWORDS => info
            .keywords
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_COMMENTS => info
            .comments
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_TEMPLATE => info
            .template
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_LASTAUTHOR => info
            .last_author
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_REVNUMBER => info
            .rev_number
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_LASTPRINTED => info
            .last_printed
            .map_or(SummaryPropValue::Empty, SummaryPropValue::FileTime),
        PID_CREATE_DTM => info
            .create_time
            .map_or(SummaryPropValue::Empty, SummaryPropValue::FileTime),
        PID_LASTSAVE_DTM => info
            .last_save_time
            .map_or(SummaryPropValue::Empty, SummaryPropValue::FileTime),
        PID_PAGECOUNT => info
            .page_count
            .map_or(SummaryPropValue::Empty, SummaryPropValue::I4),
        PID_WORDCOUNT => info
            .word_count
            .map_or(SummaryPropValue::Empty, SummaryPropValue::I4),
        PID_CHARCOUNT => info
            .char_count
            .map_or(SummaryPropValue::Empty, SummaryPropValue::I4),
        PID_APPNAME => info
            .app_name
            .clone()
            .map_or(SummaryPropValue::Empty, SummaryPropValue::LpStr),
        PID_SECURITY => info
            .security
            .map_or(SummaryPropValue::Empty, SummaryPropValue::I4),
        _ => return Err(ERROR_UNKNOWN_PROPERTY),
    };
    Ok(val)
}

/// Writes the non-string outputs (`puiDataType`, `piValue`, `pftValue`) for `MsiSummaryInfoGetPropertyA/W`.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
fn write_summary_prop_scalars(
    prop: &SummaryPropValue,
    pui_data_type: *mut Uint,
    pi_value: *mut i32,
    pft_value: *mut std::ffi::c_void,
) {
    let vt = match prop {
        SummaryPropValue::Empty => VT_EMPTY,
        SummaryPropValue::I2(_) => VT_I2,
        SummaryPropValue::I4(_) => VT_I4,
        SummaryPropValue::FileTime(_) => VT_FILETIME,
        SummaryPropValue::LpStr(_) => VT_LPSTR,
    };
    if let Some(dt_out) = unsafe { pui_data_type.as_mut() } {
        *dt_out = vt;
    }
    if let SummaryPropValue::I2(v) | SummaryPropValue::I4(v) = prop {
        if let Some(val_out) = unsafe { pi_value.as_mut() } {
            *val_out = *v;
        }
        return;
    }
    if let SummaryPropValue::FileTime(ft) = prop {
        if !pft_value.is_null() {
            let bytes = ft.to_le_bytes();
            // SAFETY: `pftValue` points to an 8-byte `FILETIME` struct.
            unsafe {
                std::slice::from_raw_parts_mut(pft_value.cast::<u8>(), 8).copy_from_slice(&bytes);
            }
        }
    }
}

/// Gets a single property from the summary information stream (Unicode).
///
/// # Arguments
///
/// * `hSummaryInfo` - Handle to summary information.
/// * `uiProperty` - Property ID (`PID_*`).
/// * `puiDataType` - Optional pointer receiving the `VT_*` property type.
/// * `piValue` - Optional pointer receiving integer property value.
/// * `pftValue` - Optional pointer receiving 64-bit `FILETIME` value.
/// * `szValueBuf` - Optional UTF-16 output buffer for string properties.
/// * `pcchValueBuf` - Optional pointer to buffer character length.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_UNKNOWN_PROPERTY`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiSummaryInfoGetPropertyW(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    puiDataType: *mut Uint,
    piValue: *mut i32,
    pftValue: *mut std::ffi::c_void,
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(res) = with_handle(hSummaryInfo, |obj| {
            if let MsiObject::SummaryInfo(si_handle) = obj {
                Ok(extract_summary_property(&si_handle.inner, uiProperty))
            } else {
                Err(ERROR_INVALID_HANDLE)
            }
        }) else {
            return ERROR_INVALID_HANDLE;
        };

        let prop = match res {
            Ok(Ok(p)) => p,
            Ok(Err(code)) | Err(code) => return code,
        };

        write_summary_prop_scalars(&prop, puiDataType, piValue, pftValue);
        if let SummaryPropValue::LpStr(ref s) = prop {
            if !pcchValueBuf.is_null() || !szValueBuf.is_null() {
                return string_to_lpwstr(s, szValueBuf, pcchValueBuf);
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets a single property from the summary information stream (ANSI).
///
/// # Arguments
///
/// * `hSummaryInfo` - Handle to summary information.
/// * `uiProperty` - Property ID (`PID_*`).
/// * `puiDataType` - Optional pointer receiving the `VT_*` property type.
/// * `piValue` - Optional pointer receiving integer property value.
/// * `pftValue` - Optional pointer receiving 64-bit `FILETIME` value.
/// * `szValueBuf` - Optional ANSI output buffer for string properties.
/// * `pcchValueBuf` - Optional pointer to buffer character length.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_UNKNOWN_PROPERTY`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiSummaryInfoGetPropertyA(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    puiDataType: *mut Uint,
    piValue: *mut i32,
    pftValue: *mut std::ffi::c_void,
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(res) = with_handle(hSummaryInfo, |obj| {
            if let MsiObject::SummaryInfo(si_handle) = obj {
                Ok(extract_summary_property(&si_handle.inner, uiProperty))
            } else {
                Err(ERROR_INVALID_HANDLE)
            }
        }) else {
            return ERROR_INVALID_HANDLE;
        };

        let prop = match res {
            Ok(Ok(p)) => p,
            Ok(Err(code)) | Err(code) => return code,
        };

        write_summary_prop_scalars(&prop, puiDataType, piValue, pftValue);
        if let SummaryPropValue::LpStr(ref s) = prop {
            if !pcchValueBuf.is_null() || !szValueBuf.is_null() {
                return string_to_lpstr(s, szValueBuf, pcchValueBuf);
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Applies a property mutation directly to a [`SummaryInfo`] instance.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
fn apply_summary_property_mutation(
    info: &mut SummaryInfo,
    ui_property: Uint,
    ui_data_type: Uint,
    i_value: i32,
    pft_value: *mut std::ffi::c_void,
    str_val: Option<String>,
) -> Uint {
    let target_str = match (ui_property, ui_data_type) {
        (PID_CODEPAGE, VT_EMPTY) => {
            info.codepage = None;
            return ERROR_SUCCESS;
        }
        (PID_CODEPAGE, VT_I2 | VT_I4) => {
            info.codepage = Some(i_value as u16);
            return ERROR_SUCCESS;
        }
        (PID_TITLE, VT_EMPTY | VT_LPSTR) => &mut info.title,
        (PID_SUBJECT, VT_EMPTY | VT_LPSTR) => &mut info.subject,
        (PID_AUTHOR, VT_EMPTY | VT_LPSTR) => &mut info.author,
        (PID_KEYWORDS, VT_EMPTY | VT_LPSTR) => &mut info.keywords,
        (PID_COMMENTS, VT_EMPTY | VT_LPSTR) => &mut info.comments,
        (PID_TEMPLATE, VT_EMPTY | VT_LPSTR) => &mut info.template,
        (PID_LASTAUTHOR, VT_EMPTY | VT_LPSTR) => &mut info.last_author,
        (PID_REVNUMBER, VT_EMPTY | VT_LPSTR) => &mut info.rev_number,
        (PID_APPNAME, VT_EMPTY | VT_LPSTR) => &mut info.app_name,
        (PID_PAGECOUNT, VT_EMPTY | VT_I4) => {
            info.page_count = (ui_data_type == VT_I4).then_some(i_value);
            return ERROR_SUCCESS;
        }
        (PID_WORDCOUNT, VT_EMPTY | VT_I4) => {
            info.word_count = (ui_data_type == VT_I4).then_some(i_value);
            return ERROR_SUCCESS;
        }
        (PID_CHARCOUNT, VT_EMPTY | VT_I4) => {
            info.char_count = (ui_data_type == VT_I4).then_some(i_value);
            return ERROR_SUCCESS;
        }
        (PID_SECURITY, VT_EMPTY | VT_I4) => {
            info.security = (ui_data_type == VT_I4).then_some(i_value);
            return ERROR_SUCCESS;
        }
        (PID_LASTPRINTED | PID_CREATE_DTM | PID_LASTSAVE_DTM, VT_EMPTY) => {
            match ui_property {
                PID_LASTPRINTED => info.last_printed = None,
                PID_CREATE_DTM => info.create_time = None,
                _ => info.last_save_time = None,
            }
            return ERROR_SUCCESS;
        }
        (PID_LASTPRINTED | PID_CREATE_DTM | PID_LASTSAVE_DTM, VT_FILETIME) => {
            if pft_value.is_null() {
                return ERROR_INVALID_PARAMETER;
            }
            let mut ft_bytes = [0u8; 8];
            // SAFETY: `pft_value` is non-null and points to an 8-byte `FILETIME`.
            unsafe {
                std::ptr::copy_nonoverlapping(pft_value.cast::<u8>(), ft_bytes.as_mut_ptr(), 8);
            }
            let ft = u64::from_le_bytes(ft_bytes);
            match ui_property {
                PID_LASTPRINTED => info.last_printed = Some(ft),
                PID_CREATE_DTM => info.create_time = Some(ft),
                _ => info.last_save_time = Some(ft),
            }
            return ERROR_SUCCESS;
        }
        (
            PID_CODEPAGE | PID_TITLE | PID_SUBJECT | PID_AUTHOR | PID_KEYWORDS | PID_COMMENTS
            | PID_TEMPLATE | PID_LASTAUTHOR | PID_REVNUMBER | PID_LASTPRINTED | PID_CREATE_DTM
            | PID_LASTSAVE_DTM | PID_PAGECOUNT | PID_WORDCOUNT | PID_CHARCOUNT | PID_APPNAME
            | PID_SECURITY,
            _,
        ) => return ERROR_DATATYPE_MISMATCH,
        _ => return ERROR_UNKNOWN_PROPERTY,
    };

    if ui_data_type == VT_EMPTY {
        *target_str = None;
        ERROR_SUCCESS
    } else if let Some(s) = str_val {
        *target_str = Some(s);
        ERROR_SUCCESS
    } else {
        ERROR_INVALID_PARAMETER
    }
}

/// Shared implementation for `MsiSummaryInfoSetPropertyA/W`.
fn set_summary_property(
    h_summary_info: MsiHandle,
    ui_property: Uint,
    ui_data_type: Uint,
    i_value: i32,
    pft_value: *mut std::ffi::c_void,
    str_val: Option<String>,
) -> Uint {
    let mut parent_db_to_sync: Option<(MsiHandle, SummaryInfo)> = None;

    let status = with_handle_mut(h_summary_info, |obj| {
        let MsiObject::SummaryInfo(si_handle) = obj else {
            return ERROR_INVALID_HANDLE;
        };

        if si_handle.update_count == 0 {
            return ERROR_FUNCTION_FAILED;
        }

        let res = apply_summary_property_mutation(
            &mut si_handle.inner,
            ui_property,
            ui_data_type,
            i_value,
            pft_value,
            str_val,
        );
        if res != ERROR_SUCCESS {
            return res;
        }

        si_handle.update_count -= 1;
        if si_handle.db_handle != 0 {
            parent_db_to_sync = Some((si_handle.db_handle, si_handle.inner.clone()));
        }
        ERROR_SUCCESS
    })
    .unwrap_or(ERROR_INVALID_HANDLE);

    if let Some((db_h, updated_si)) = parent_db_to_sync {
        let _ = with_handle_mut(db_h, |obj| {
            if let MsiObject::Database(db) = obj {
                db.inner.summary_info = updated_si;
            }
        });
    }

    status
}

/// Sets a single property in the summary information stream (Unicode).
///
/// # Arguments
///
/// * `hSummaryInfo` - Handle to summary information.
/// * `uiProperty` - Property ID (`PID_*`).
/// * `uiDataType` - OLE property variant type (`VT_*`).
/// * `iValue` - Integer value when `uiDataType` is `VT_I2` or `VT_I4`.
/// * `pftValue` - Pointer to `FILETIME` when `uiDataType` is `VT_FILETIME`.
/// * `szValue` - Pointer to UTF-16 string when `uiDataType` is `VT_LPSTR`.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_DATATYPE_MISMATCH`, `ERROR_UNKNOWN_PROPERTY`, `ERROR_FUNCTION_FAILED`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiSummaryInfoSetPropertyW(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    uiDataType: Uint,
    iValue: i32,
    pftValue: *mut std::ffi::c_void,
    szValue: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let str_val = lpcwstr_to_string(szValue);
        set_summary_property(
            hSummaryInfo,
            uiProperty,
            uiDataType,
            iValue,
            pftValue,
            str_val,
        )
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a single property in the summary information stream (ANSI).
///
/// # Arguments
///
/// * `hSummaryInfo` - Handle to summary information.
/// * `uiProperty` - Property ID (`PID_*`).
/// * `uiDataType` - OLE property variant type (`VT_*`).
/// * `iValue` - Integer value when `uiDataType` is `VT_I2` or `VT_I4`.
/// * `pftValue` - Pointer to `FILETIME` when `uiDataType` is `VT_FILETIME`.
/// * `szValue` - Pointer to ANSI string when `uiDataType` is `VT_LPSTR`.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_DATATYPE_MISMATCH`, `ERROR_UNKNOWN_PROPERTY`, `ERROR_FUNCTION_FAILED`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiSummaryInfoSetPropertyA(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    uiDataType: Uint,
    iValue: i32,
    pftValue: *mut std::ffi::c_void,
    szValue: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let str_val = lpcstr_to_string(szValue);
        set_summary_property(
            hSummaryInfo,
            uiProperty,
            uiDataType,
            iValue,
            pftValue,
            str_val,
        )
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the number of existing properties in the summary information stream.
///
/// # Arguments
///
/// * `hSummaryInfo` - Handle to summary information.
/// * `puiPropertyCount` - Pointer to a variable that receives the property count.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiSummaryInfoGetPropertyCount(
    hSummaryInfo: MsiHandle,
    puiPropertyCount: *mut Uint,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if puiPropertyCount.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        with_handle(hSummaryInfo, |obj| {
            if let MsiObject::SummaryInfo(si_handle) = obj {
                // SAFETY: `puiPropertyCount` was verified non-null above.
                unsafe {
                    *puiPropertyCount = si_handle.inner.property_count();
                }
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Writes the modified summary information stream back to the underlying database and/or `.msi` storage container on disk.
///
/// # Arguments
///
/// * `hSummaryInfo` - Handle to summary information.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_FUNCTION_FAILED`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiSummaryInfoPersist(hSummaryInfo: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(info_snapshot) = with_handle(hSummaryInfo, |obj| {
            if let MsiObject::SummaryInfo(si_handle) = obj {
                Some((
                    si_handle.inner.clone(),
                    si_handle.db_handle,
                    si_handle.path.clone(),
                ))
            } else {
                None
            }
        })
        .flatten() else {
            return ERROR_INVALID_HANDLE;
        };

        let (summary_info, db_handle, standalone_path) = info_snapshot;

        if db_handle != 0 {
            let Some(res) = with_handle_mut(db_handle, |obj| {
                if let MsiObject::Database(db) = obj {
                    db.inner.summary_info = summary_info;
                    if let Some(ref path) = db
                        .inner
                        .backing_path
                        .as_ref()
                        .filter(|_| db.state == MSIDBSTATE_WRITE)
                    {
                        if db.inner.commit_to_path(path).is_err() {
                            return ERROR_FUNCTION_FAILED;
                        }
                    }
                    ERROR_SUCCESS
                } else {
                    ERROR_INVALID_HANDLE
                }
            }) else {
                return ERROR_INVALID_HANDLE;
            };
            return res;
        }

        if let Some(ref path) = standalone_path {
            let mut db = msi::package::Package::open(path)
                .map(|pkg| pkg.database().clone())
                .unwrap_or_default();
            db.summary_info = summary_info;
            if db.commit_to_path(path).is_err() {
                return ERROR_FUNCTION_FAILED;
            }
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Commits changes made to a database to the backing `.msi` file on disk.
///
/// Flushes modified summary properties and table streams when opened in write mode
/// (`MSIDBOPEN_TRANSACT`, `MSIDBOPEN_DIRECT`, `MSIDBOPEN_CREATE`, or `MSIDBOPEN_CREATEDIRECT`).
///
/// # Arguments
///
/// * `hDatabase` - Handle to the database to commit.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_FUNCTION_FAILED`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiDatabaseCommit(hDatabase: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle_mut(hDatabase, |obj| {
            if let MsiObject::Database(db_handle) = obj {
                if let Some(ref path) = db_handle
                    .inner
                    .backing_path
                    .as_ref()
                    .filter(|_| db_handle.state == MSIDBSTATE_WRITE)
                {
                    if db_handle.inner.commit_to_path(path).is_err() {
                        return ERROR_FUNCTION_FAILED;
                    }
                }
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::{alloc_handle, close_handle, MsiObject};
    use crate::win32::database::{
        MsiOpenDatabaseA, MsiOpenDatabaseW, MSIDBOPEN_CREATE, MSIDBOPEN_READONLY,
        MSIDBOPEN_TRANSACT,
    };
    use crate::win32::ERROR_MORE_DATA;
    use std::ffi::CString;

    #[test]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cognitive_complexity)]
    fn test_summary_info_lifecycle_and_persist() {
        // Invalid handle / parameter checks
        assert_eq!(
            MsiGetSummaryInformationW(0, std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetSummaryInformationA(0, std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiDatabaseCommit(0), ERROR_INVALID_HANDLE);
        assert_eq!(MsiSummaryInfoPersist(0), ERROR_INVALID_HANDLE);
        assert_eq!(
            MsiSummaryInfoGetPropertyCount(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        let mut count = 0;
        assert_eq!(
            MsiSummaryInfoGetPropertyCount(0, &raw mut count),
            ERROR_INVALID_HANDLE
        );

        let rec = msi::database::tables::record::Record::new();
        let h_rec = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));
        assert_eq!(MsiDatabaseCommit(h_rec), ERROR_INVALID_HANDLE);
        assert_eq!(MsiSummaryInfoPersist(h_rec), ERROR_INVALID_HANDLE);
        assert_eq!(
            MsiSummaryInfoGetPropertyCount(h_rec, &raw mut count),
            ERROR_INVALID_HANDLE
        );

        let mut h_si = 0;
        assert_eq!(
            MsiGetSummaryInformationW(999_999, std::ptr::null(), 5, &raw mut h_si),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiGetSummaryInformationW(h_rec, std::ptr::null(), 5, &raw mut h_si),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                0,
                PID_TITLE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_rec,
                PID_TITLE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                0,
                PID_TITLE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                h_rec,
                PID_TITLE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyW(
                0,
                PID_TITLE,
                VT_LPSTR,
                0,
                std::ptr::null_mut(),
                std::ptr::null()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyW(
                h_rec,
                PID_TITLE,
                VT_LPSTR,
                0,
                std::ptr::null_mut(),
                std::ptr::null()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyA(
                0,
                PID_TITLE,
                VT_LPSTR,
                0,
                std::ptr::null_mut(),
                std::ptr::null()
            ),
            ERROR_INVALID_HANDLE
        );

        // Open SummaryInfo without database or path (in-memory) with 30 updates allowed
        assert_eq!(
            MsiGetSummaryInformationW(0, std::ptr::null(), 30, &raw mut h_si),
            ERROR_SUCCESS
        );
        assert_ne!(h_si, 0);
        assert_eq!(
            MsiSummaryInfoGetPropertyCount(h_si, &raw mut count),
            ERROR_SUCCESS
        );
        assert_eq!(count, 0);

        // Test setting and getting all PID_* properties
        assert_eq!(
            MsiSummaryInfoSetPropertyW(
                h_si,
                PID_CODEPAGE,
                VT_I2,
                1252,
                std::ptr::null_mut(),
                std::ptr::null()
            ),
            ERROR_SUCCESS
        );
        for (pid, text) in [
            (PID_TITLE, "TestTitle"),
            (PID_SUBJECT, "TestSubject"),
            (PID_AUTHOR, "TestAuthor"),
            (PID_KEYWORDS, "TestKeywords"),
            (PID_COMMENTS, "TestComments"),
            (PID_TEMPLATE, "x64;1033"),
            (PID_LASTAUTHOR, "TestLastAuthor"),
            (PID_REVNUMBER, "{12345678-1234-1234-1234-1234567890AB}"),
            (PID_APPNAME, "TestApp"),
        ] {
            let cstr = CString::new(text).unwrap_or_default();
            assert_eq!(
                MsiSummaryInfoSetPropertyA(
                    h_si,
                    pid,
                    VT_LPSTR,
                    0,
                    std::ptr::null_mut(),
                    cstr.as_ptr()
                ),
                ERROR_SUCCESS
            );
            // Null string pointer error check
            assert_eq!(
                MsiSummaryInfoSetPropertyA(
                    h_si,
                    pid,
                    VT_LPSTR,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null()
                ),
                ERROR_INVALID_PARAMETER
            );
        }

        let wide_title: Vec<u16> = "WideTitle\0".encode_utf16().collect();
        assert_eq!(
            MsiSummaryInfoSetPropertyW(
                h_si,
                PID_TITLE,
                VT_LPSTR,
                0,
                std::ptr::null_mut(),
                wide_title.as_ptr()
            ),
            ERROR_SUCCESS
        );

        for (pid, val) in [
            (PID_PAGECOUNT, 200),
            (PID_WORDCOUNT, 2),
            (PID_CHARCOUNT, 10),
            (PID_SECURITY, 0),
        ] {
            assert_eq!(
                MsiSummaryInfoSetPropertyW(
                    h_si,
                    pid,
                    VT_I4,
                    val,
                    std::ptr::null_mut(),
                    std::ptr::null()
                ),
                ERROR_SUCCESS
            );
        }

        let mut ft_in: u64 = 0x01D9_0000_1234_5678;
        for pid in [PID_LASTPRINTED, PID_CREATE_DTM, PID_LASTSAVE_DTM] {
            assert_eq!(
                MsiSummaryInfoSetPropertyW(
                    h_si,
                    pid,
                    VT_FILETIME,
                    0,
                    (&raw mut ft_in).cast::<std::ffi::c_void>(),
                    std::ptr::null()
                ),
                ERROR_SUCCESS
            );
            assert_eq!(
                MsiSummaryInfoSetPropertyW(
                    h_si,
                    pid,
                    VT_FILETIME,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null()
                ),
                ERROR_INVALID_PARAMETER
            );
        }

        // Datatype mismatch & unknown property
        assert_eq!(
            MsiSummaryInfoSetPropertyW(
                h_si,
                PID_TITLE,
                VT_I4,
                1,
                std::ptr::null_mut(),
                std::ptr::null()
            ),
            ERROR_DATATYPE_MISMATCH
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyW(h_si, 999, VT_I4, 1, std::ptr::null_mut(), std::ptr::null()),
            ERROR_UNKNOWN_PROPERTY
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_si,
                999,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_UNKNOWN_PROPERTY
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                h_si,
                999,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_UNKNOWN_PROPERTY
        );

        assert_eq!(
            MsiSummaryInfoGetPropertyCount(h_si, &raw mut count),
            ERROR_SUCCESS
        );
        assert_eq!(count, 17);

        // Read back all properties via GetPropertyW / GetPropertyA
        let mut dt: Uint = 0;
        let mut ival: i32 = 0;
        let mut ft_out: u64 = 0;
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_si,
                PID_CODEPAGE,
                &raw mut dt,
                &raw mut ival,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(dt, VT_I2);
        assert_eq!(ival, 1252);
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_si,
                PID_CODEPAGE,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_si,
                PID_LASTPRINTED,
                &raw mut dt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                h_si,
                PID_PAGECOUNT,
                &raw mut dt,
                &raw mut ival,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(dt, VT_I4);
        assert_eq!(ival, 200);

        for pid in [
            PID_WORDCOUNT,
            PID_CHARCOUNT,
            PID_SECURITY,
            PID_LASTPRINTED,
            PID_CREATE_DTM,
            PID_LASTSAVE_DTM,
        ] {
            assert_eq!(
                MsiSummaryInfoGetPropertyW(
                    h_si,
                    pid,
                    &raw mut dt,
                    &raw mut ival,
                    (&raw mut ft_out).cast::<std::ffi::c_void>(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_SUCCESS
            );
        }
        assert_eq!(ft_out, ft_in);

        // Read string properties with buffer and without buffer
        let mut wbuf = [0u16; 64];
        let mut cch: Dword = 64;
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_si,
                PID_TITLE,
                &raw mut dt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                wbuf.as_mut_ptr(),
                &raw mut cch
            ),
            ERROR_SUCCESS
        );
        assert_eq!(dt, VT_LPSTR);
        assert_eq!(cch, 9);

        let mut abuf = [0i8; 64];
        for pid in [
            PID_SUBJECT,
            PID_AUTHOR,
            PID_KEYWORDS,
            PID_COMMENTS,
            PID_TEMPLATE,
            PID_LASTAUTHOR,
            PID_REVNUMBER,
            PID_APPNAME,
        ] {
            let mut acch: Dword = 64;
            assert_eq!(
                MsiSummaryInfoGetPropertyA(
                    h_si,
                    pid,
                    &raw mut dt,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    abuf.as_mut_ptr(),
                    &raw mut acch
                ),
                ERROR_SUCCESS
            );
            assert_eq!(dt, VT_LPSTR);
        }

        // Also test string property query when both szValueBuf and pcchValueBuf are NULL
        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                h_si,
                PID_TITLE,
                &raw mut dt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                h_si,
                PID_TITLE,
                &raw mut dt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );

        // Clear all properties with VT_EMPTY
        let mut h_si_clear = 0;
        assert_eq!(
            MsiGetSummaryInformationA(0, std::ptr::null(), 25, &raw mut h_si_clear),
            ERROR_SUCCESS
        );
        for pid in [
            PID_CODEPAGE,
            PID_TITLE,
            PID_SUBJECT,
            PID_AUTHOR,
            PID_KEYWORDS,
            PID_COMMENTS,
            PID_TEMPLATE,
            PID_LASTAUTHOR,
            PID_REVNUMBER,
            PID_LASTPRINTED,
            PID_CREATE_DTM,
            PID_LASTSAVE_DTM,
            PID_PAGECOUNT,
            PID_WORDCOUNT,
            PID_CHARCOUNT,
            PID_APPNAME,
            PID_SECURITY,
        ] {
            assert_eq!(
                MsiSummaryInfoSetPropertyW(
                    h_si_clear,
                    pid,
                    VT_EMPTY,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null()
                ),
                ERROR_SUCCESS
            );
            assert_eq!(
                MsiSummaryInfoGetPropertyW(
                    h_si_clear,
                    pid,
                    &raw mut dt,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_SUCCESS
            );
            assert_eq!(dt, VT_EMPTY);
        }
        assert_eq!(MsiSummaryInfoPersist(h_si_clear), ERROR_SUCCESS);

        // Test update_count == 0 failure
        let mut h_si_ro = 0;
        assert_eq!(
            MsiGetSummaryInformationW(0, std::ptr::null(), 0, &raw mut h_si_ro),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyW(
                h_si_ro,
                PID_PAGECOUNT,
                VT_I4,
                100,
                std::ptr::null_mut(),
                std::ptr::null()
            ),
            ERROR_FUNCTION_FAILED
        );

        // Test MsiOpenDatabaseW (MSIDBOPEN_CREATE), MsiSummaryInfoPersist, MsiDatabaseCommit, and re-opening from disk
        let tmp_msi = std::env::temp_dir().join("test_summary_persist_commit.msi");
        let _ = std::fs::remove_file(&tmp_msi);
        let tmp_msi_str = tmp_msi.to_string_lossy().to_string();
        let tmp_msi_w: Vec<u16> = tmp_msi_str
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let tmp_msi_a = CString::new(tmp_msi_str).unwrap_or_default();

        let mut h_db_write = 0;
        assert_eq!(
            MsiOpenDatabaseW(tmp_msi_w.as_ptr(), MSIDBOPEN_CREATE, &raw mut h_db_write),
            ERROR_SUCCESS
        );
        let mut h_si_db = 0;
        assert_eq!(
            MsiGetSummaryInformationW(h_db_write, std::ptr::null(), 5, &raw mut h_si_db),
            ERROR_SUCCESS
        );
        let persisted_title = CString::new("Persisted MSI Title").unwrap_or_default();
        assert_eq!(
            MsiSummaryInfoSetPropertyA(
                h_si_db,
                PID_TITLE,
                VT_LPSTR,
                0,
                std::ptr::null_mut(),
                persisted_title.as_ptr()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(MsiSummaryInfoPersist(h_si_db), ERROR_SUCCESS);
        assert_eq!(MsiDatabaseCommit(h_db_write), ERROR_SUCCESS);
        assert!(tmp_msi.exists());

        // Re-open SummaryInfo directly via szDatabasePath (hDatabase = 0) and verify persisted title
        let mut h_si_disk = 0;
        assert_eq!(
            MsiGetSummaryInformationA(0, tmp_msi_a.as_ptr(), 5, &raw mut h_si_disk),
            ERROR_SUCCESS
        );
        let mut probe_cch: Dword = 0;
        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                h_si_disk,
                PID_TITLE,
                &raw mut dt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &raw mut probe_cch
            ),
            ERROR_MORE_DATA
        );
        assert_eq!(probe_cch, 19);

        // Update via standalone path handle and persist directly to disk
        let updated_subject = CString::new("Standalone Subject").unwrap_or_default();
        assert_eq!(
            MsiSummaryInfoSetPropertyA(
                h_si_disk,
                PID_SUBJECT,
                VT_LPSTR,
                0,
                std::ptr::null_mut(),
                updated_subject.as_ptr()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(MsiSummaryInfoPersist(h_si_disk), ERROR_SUCCESS);

        // Open database in TRANSACT mode via MsiOpenDatabaseA and verify both Title and Subject loaded
        let mut h_db_reopen = 0;
        assert_eq!(
            MsiOpenDatabaseA(
                tmp_msi_a.as_ptr(),
                MSIDBOPEN_TRANSACT.cast::<i8>(),
                &raw mut h_db_reopen
            ),
            ERROR_SUCCESS
        );
        let mut h_si_reopen = 0;
        assert_eq!(
            MsiGetSummaryInformationA(h_db_reopen, std::ptr::null(), 1, &raw mut h_si_reopen),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyCount(h_si_reopen, &raw mut count),
            ERROR_SUCCESS
        );
        assert!(count >= 2);

        // Test read-only database handle persist (updates in-memory only)
        let mut h_db_ro = 0;
        assert_eq!(
            MsiOpenDatabaseW(tmp_msi_w.as_ptr(), MSIDBOPEN_READONLY, &raw mut h_db_ro),
            ERROR_SUCCESS
        );
        let mut h_si_ro_db = 0;
        assert_eq!(
            MsiGetSummaryInformationW(h_db_ro, std::ptr::null(), 1, &raw mut h_si_ro_db),
            ERROR_SUCCESS
        );
        assert_eq!(MsiSummaryInfoPersist(h_si_ro_db), ERROR_SUCCESS);
        assert_eq!(MsiDatabaseCommit(h_db_ro), ERROR_SUCCESS);

        // Test error paths when parent db_handle is closed or invalid path
        let _ = close_handle(h_db_ro);
        assert_eq!(MsiSummaryInfoPersist(h_si_ro_db), ERROR_INVALID_HANDLE);

        // Invalid directory path for commit_to_path failure
        let bad_dir_path = "/nonexistent_dir_xyz_987654/bad.msi".to_string();
        let bad_db = msi::wix::linker::LinkedDatabase {
            backing_path: Some(bad_dir_path.clone()),
            ..msi::wix::linker::LinkedDatabase::default()
        };
        let h_bad_db = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: bad_db,
            state: MSIDBSTATE_WRITE,
        }));
        assert_eq!(MsiDatabaseCommit(h_bad_db), ERROR_FUNCTION_FAILED);

        let mut h_si_bad_db = 0;
        assert_eq!(
            MsiGetSummaryInformationW(h_bad_db, std::ptr::null(), 1, &raw mut h_si_bad_db),
            ERROR_SUCCESS
        );
        assert_eq!(MsiSummaryInfoPersist(h_si_bad_db), ERROR_FUNCTION_FAILED);

        // Standalone SummaryInfo with invalid path
        let bad_path_c = CString::new(bad_dir_path).unwrap_or_default();
        let mut h_si_bad_standalone = 0;
        assert_eq!(
            MsiGetSummaryInformationA(0, bad_path_c.as_ptr(), 1, &raw mut h_si_bad_standalone),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSummaryInfoPersist(h_si_bad_standalone),
            ERROR_FUNCTION_FAILED
        );

        // Non-database handle as parent db_handle in MsiSummaryInfoHandle
        let h_si_wrong_parent = alloc_handle(MsiObject::SummaryInfo(MsiSummaryInfoHandle {
            inner: SummaryInfo::default(),
            db_handle: h_rec,
            path: None,
            update_count: 1,
        }));
        assert_eq!(
            MsiSummaryInfoPersist(h_si_wrong_parent),
            ERROR_INVALID_HANDLE
        );

        let _ = std::fs::remove_file(&tmp_msi);
        let _ = close_handle(h_rec);
        let _ = close_handle(h_si);
        let _ = close_handle(h_si_clear);
        let _ = close_handle(h_si_ro);
        let _ = close_handle(h_db_write);
        let _ = close_handle(h_si_db);
        let _ = close_handle(h_si_disk);
        let _ = close_handle(h_db_reopen);
        let _ = close_handle(h_si_reopen);
        let _ = close_handle(h_si_ro_db);
        let _ = close_handle(h_bad_db);
        let _ = close_handle(h_si_bad_db);
        let _ = close_handle(h_si_bad_standalone);
        let _ = close_handle(h_si_wrong_parent);
    }
}
