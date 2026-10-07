use super::stream::{IStream, STATSTG};
use super::{IUnknownVtbl, HRESULT, ULONG};
use std::ffi::c_void;

/// `IStorage` interface identifier.
pub const IID_ISTORAGE: super::GUID = super::GUID::new(
    0x0000_000B,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

/// SNB type (pointer to pointer to OLECHAR).
pub type SNB = *mut *mut u16;

/// `IEnumSTATSTG` interface identifier.
pub const IID_IENUMSTATSTG: super::GUID = super::GUID::new(
    0x0000_000D,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

/// Virtual table for `IEnumSTATSTG`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IEnumSTATSTGVtbl {
    /// Base `IUnknown` methods.
    pub parent: IUnknownVtbl,
    /// Retrieves next items.
    pub Next: unsafe extern "system" fn(
        this: *mut IEnumSTATSTG,
        celt: ULONG,
        rgelt: *mut STATSTG,
        pceltFetched: *mut ULONG,
    ) -> HRESULT,
    /// Skips items.
    pub Skip: unsafe extern "system" fn(this: *mut IEnumSTATSTG, celt: ULONG) -> HRESULT,
    /// Resets enumeration.
    pub Reset: unsafe extern "system" fn(this: *mut IEnumSTATSTG) -> HRESULT,
    /// Clones the enumerator.
    pub Clone: unsafe extern "system" fn(
        this: *mut IEnumSTATSTG,
        ppenum: *mut *mut IEnumSTATSTG,
    ) -> HRESULT,
}

/// COM `IEnumSTATSTG` interface.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IEnumSTATSTG {
    /// Pointer to the virtual table.
    pub lpVtbl: *const IEnumSTATSTGVtbl,
}

/// Virtual table for `IStorage`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IStorageVtbl {
    /// Base `IUnknown` methods.
    pub parent: IUnknownVtbl,
    /// Creates a new stream.
    pub CreateStream: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsName: *const u16,
        grfMode: u32,
        reserved1: u32,
        reserved2: u32,
        ppstm: *mut *mut IStream,
    ) -> HRESULT,
    /// Opens an existing stream.
    pub OpenStream: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsName: *const u16,
        reserved1: *mut c_void,
        grfMode: u32,
        reserved2: u32,
        ppstm: *mut *mut IStream,
    ) -> HRESULT,
    /// Creates a new storage object.
    pub CreateStorage: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsName: *const u16,
        grfMode: u32,
        reserved1: u32,
        reserved2: u32,
        ppstg: *mut *mut IStorage,
    ) -> HRESULT,
    /// Opens an existing storage object.
    pub OpenStorage: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsName: *const u16,
        pstgPriority: *mut IStorage,
        grfMode: u32,
        snbExclude: SNB,
        reserved: u32,
        ppstg: *mut *mut IStorage,
    ) -> HRESULT,
    /// Copies elements to another storage object.
    pub CopyTo: unsafe extern "system" fn(
        this: *mut IStorage,
        ciidExclude: u32,
        rgiidExclude: *const super::GUID,
        snbExclude: SNB,
        pstgDest: *mut IStorage,
    ) -> HRESULT,
    /// Moves elements to another storage object.
    pub MoveElementTo: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsName: *const u16,
        pstgDest: *mut IStorage,
        pwcsNewName: *const u16,
        grfFlags: u32,
    ) -> HRESULT,
    /// Commits changes.
    pub Commit: unsafe extern "system" fn(this: *mut IStorage, grfCommitFlags: u32) -> HRESULT,
    /// Reverts changes.
    pub Revert: unsafe extern "system" fn(this: *mut IStorage) -> HRESULT,
    /// Enumerates elements in this storage.
    pub EnumElements: unsafe extern "system" fn(
        this: *mut IStorage,
        reserved1: u32,
        reserved2: *mut c_void,
        reserved3: u32,
        ppenum: *mut *mut IEnumSTATSTG,
    ) -> HRESULT,
    /// Destroys an element.
    pub DestroyElement:
        unsafe extern "system" fn(this: *mut IStorage, pwcsName: *const u16) -> HRESULT,
    /// Renames an element.
    pub RenameElement: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsOldName: *const u16,
        pwcsNewName: *const u16,
    ) -> HRESULT,
    /// Sets modification, creation, and access times.
    pub SetElementTimes: unsafe extern "system" fn(
        this: *mut IStorage,
        pwcsName: *const u16,
        pctime: *const u64,
        patime: *const u64,
        pmtime: *const u64,
    ) -> HRESULT,
    /// Sets the CLSID of the storage object.
    pub SetClass:
        unsafe extern "system" fn(this: *mut IStorage, clsid: *const super::GUID) -> HRESULT,
    /// Sets state bits.
    pub SetStateBits:
        unsafe extern "system" fn(this: *mut IStorage, grfStateBits: u32, grfMask: u32) -> HRESULT,
    /// Retrieves statistics.
    pub Stat: unsafe extern "system" fn(
        this: *mut IStorage,
        pstatstg: *mut STATSTG,
        grfStatFlag: u32,
    ) -> HRESULT,
}

/// COM `IStorage` interface.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IStorage {
    /// Pointer to the virtual table.
    pub lpVtbl: *const IStorageVtbl,
}
