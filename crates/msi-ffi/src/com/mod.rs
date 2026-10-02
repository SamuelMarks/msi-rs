//! Core COM (Component Object Model) interface definitions.
//!
//! Provides the standard ABI definitions for `IUnknown` and `IDispatch`
//! required for Windows Installer OLE automation.

use std::ffi::c_void;

/// Globally Unique Identifier matching the Win32 `GUID` struct.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
pub struct GUID {
    /// Data 1.
    pub Data1: u32,
    /// Data 2.
    pub Data2: u16,
    /// Data 3.
    pub Data3: u16,
    /// Data 4.
    pub Data4: [u8; 8],
}

impl GUID {
    /// Creates a new `GUID`.
    #[must_use]
    pub const fn new(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> Self {
        Self {
            Data1: d1,
            Data2: d2,
            Data3: d3,
            Data4: d4,
        }
    }
}

/// COM return code.
pub type HRESULT = i32;
/// Unsigned long integer.
pub type ULONG = u32;

/// Operation successful.
pub const S_OK: HRESULT = 0;
/// Interface not supported.
pub const E_NOINTERFACE: HRESULT = -2_147_467_262; // 0x80004002
/// Invalid pointer.
pub const E_POINTER: HRESULT = -2_147_467_261; // 0x80004003

/// Virtual table for `IUnknown`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IUnknownVtbl {
    /// Queries an object for a specific interface.
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut IUnknown,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    /// Increments the reference count.
    pub AddRef: unsafe extern "system" fn(this: *mut IUnknown) -> ULONG,
    /// Decrements the reference count.
    pub Release: unsafe extern "system" fn(this: *mut IUnknown) -> ULONG,
}

/// Base COM interface.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IUnknown {
    /// Pointer to the virtual table.
    pub lpVtbl: *const IUnknownVtbl,
}

/// Locale ID.
pub type LCID = u32;
/// Dispatch ID.
pub type DISPID = i32;

/// Virtual table for `IDispatch`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IDispatchVtbl {
    /// Base `IUnknown` methods.
    pub parent: IUnknownVtbl,
    /// Retrieves the number of type information interfaces.
    pub GetTypeInfoCount:
        unsafe extern "system" fn(this: *mut IDispatch, pctinfo: *mut u32) -> HRESULT,
    /// Retrieves the type information.
    pub GetTypeInfo: unsafe extern "system" fn(
        this: *mut IDispatch,
        iTInfo: u32,
        lcid: LCID,
        ppTInfo: *mut *mut c_void,
    ) -> HRESULT,
    /// Maps a single member and an optional set of argument names to a corresponding set of integer DISPIDs.
    pub GetIDsOfNames: unsafe extern "system" fn(
        this: *mut IDispatch,
        riid: *const GUID,
        rgszNames: *mut *mut u16,
        cNames: u32,
        lcid: LCID,
        rgDispId: *mut DISPID,
    ) -> HRESULT,
    /// Provides access to properties and methods exposed by an object.
    pub Invoke: unsafe extern "system" fn(
        this: *mut IDispatch,
        dispIdMember: DISPID,
        riid: *const GUID,
        lcid: LCID,
        wFlags: u16,
        pDispParams: *mut c_void,
        pVarResult: *mut c_void,
        pExcepInfo: *mut c_void,
        puArgErr: *mut u32,
    ) -> HRESULT,
}

/// OLE Automation interface.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IDispatch {
    /// Pointer to the virtual table.
    pub lpVtbl: *const IDispatchVtbl,
}

pub mod variant;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guid_creation() {
        let guid = GUID::new(1, 2, 3, [4, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(guid.Data1, 1);
        assert_eq!(guid.Data2, 2);
        assert_eq!(guid.Data3, 3);
        assert_eq!(guid.Data4, [4, 5, 6, 7, 8, 9, 10, 11]);
    }

    #[test]
    fn test_com_constants() {
        assert_eq!(S_OK, 0);
        assert_eq!(E_NOINTERFACE, -2_147_467_262);
        assert_eq!(E_POINTER, -2_147_467_261);
    }
}

pub mod installer;

pub mod util;

pub mod session;

pub mod database;

pub mod view;

pub mod record;

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_hresult_and_ulong() {
        let _h: HRESULT = S_OK;
        let _u: ULONG = 0;
    }
}

#[cfg(test)]
mod tests_final {
    use super::database::DatabaseCOM;
    use super::installer::InstallerCOM;
    use super::record::RecordCOM;
    use super::session::SessionCOM;
    use super::view::ViewCOM;

    #[test]
    fn test_iids() {
        assert_eq!(InstallerCOM::IID.Data1, 0x000c1090);
        assert_eq!(SessionCOM::IID.Data1, 0x000c109e);
        assert_eq!(DatabaseCOM::IID.Data1, 0x000c109d);
        assert_eq!(ViewCOM::IID.Data1, 0x000c109c);
        assert_eq!(RecordCOM::IID.Data1, 0x000c1093);
    }
}
