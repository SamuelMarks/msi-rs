use super::{IUnknownVtbl, HRESULT, ULONG};
use std::ffi::c_void;

/// `IStream` interface identifier.
pub const IID_ISTREAM: super::GUID = super::GUID::new(
    0x0000_000C,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

/// Large integer wrapper.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct LARGE_INTEGER {
    /// 64-bit value.
    pub QuadPart: i64,
}

/// Unsigned large integer wrapper.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct ULARGE_INTEGER {
    /// 64-bit unsigned value.
    pub QuadPart: u64,
}

/// Status of a stream or storage object.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct STATSTG {
    /// Name.
    pub pwcsName: *mut u16,
    /// Type of storage object.
    pub type_: u32,
    /// Size.
    pub cbSize: ULARGE_INTEGER,
    /// Modification time.
    pub mtime: u64, // FILETIME
    /// Creation time.
    pub ctime: u64,
    /// Access time.
    pub atime: u64,
    /// Mode.
    pub grfMode: u32,
    /// Supported locks.
    pub grfLocksSupported: u32,
    /// Class ID.
    pub clsid: super::GUID,
    /// State bits.
    pub grfStateBits: u32,
    /// Reserved.
    pub reserved: u32,
}

/// Virtual table for `IStream`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IStreamVtbl {
    /// Base `IUnknown` methods.
    pub parent: IUnknownVtbl,
    /// Reads from the stream.
    pub Read: unsafe extern "system" fn(
        this: *mut IStream,
        pv: *mut c_void,
        cb: ULONG,
        pcbRead: *mut ULONG,
    ) -> HRESULT,
    /// Writes to the stream.
    pub Write: unsafe extern "system" fn(
        this: *mut IStream,
        pv: *const c_void,
        cb: ULONG,
        pcbWritten: *mut ULONG,
    ) -> HRESULT,
    /// Seeks in the stream.
    pub Seek: unsafe extern "system" fn(
        this: *mut IStream,
        dlibMove: LARGE_INTEGER,
        dwOrigin: u32,
        plibNewPosition: *mut ULARGE_INTEGER,
    ) -> HRESULT,
    /// Sets the stream size.
    pub SetSize:
        unsafe extern "system" fn(this: *mut IStream, libNewSize: ULARGE_INTEGER) -> HRESULT,
    /// Copies the stream to another.
    pub CopyTo: unsafe extern "system" fn(
        this: *mut IStream,
        pstm: *mut IStream,
        cb: ULARGE_INTEGER,
        pcbRead: *mut ULARGE_INTEGER,
        pcbWritten: *mut ULARGE_INTEGER,
    ) -> HRESULT,
    /// Commits changes.
    pub Commit: unsafe extern "system" fn(this: *mut IStream, grfCommitFlags: u32) -> HRESULT,
    /// Reverts changes.
    pub Revert: unsafe extern "system" fn(this: *mut IStream) -> HRESULT,
    /// Locks a region.
    pub LockRegion: unsafe extern "system" fn(
        this: *mut IStream,
        libOffset: ULARGE_INTEGER,
        cb: ULARGE_INTEGER,
        dwLockType: u32,
    ) -> HRESULT,
    /// Unlocks a region.
    pub UnlockRegion: unsafe extern "system" fn(
        this: *mut IStream,
        libOffset: ULARGE_INTEGER,
        cb: ULARGE_INTEGER,
        dwLockType: u32,
    ) -> HRESULT,
    /// Gets statistics.
    pub Stat: unsafe extern "system" fn(
        this: *mut IStream,
        pstatstg: *mut STATSTG,
        grfStatFlag: u32,
    ) -> HRESULT,
    /// Clones the stream.
    pub Clone: unsafe extern "system" fn(this: *mut IStream, ppstm: *mut *mut IStream) -> HRESULT,
}

/// COM `IStream` interface.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[allow(non_snake_case)]
pub struct IStream {
    /// Pointer to the virtual table.
    pub lpVtbl: *const IStreamVtbl,
}
