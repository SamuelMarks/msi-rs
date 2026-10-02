//! Implementations of COM standard objects.

use crate::com::IDispatchVtbl;
use crate::com::{GUID, ULONG};

/// Internal COM object stub representing `Record`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RecordCOM {
    /// `VTable` for `IDispatch`.
    pub lp_vtbl: *const IDispatchVtbl,
    /// Reference count.
    pub ref_count: ULONG,
}

impl RecordCOM {
    /// IID for WindowsInstaller.Record
    pub const IID: GUID = GUID::new(
        0x000c1093,
        0x0000,
        0x0000,
        [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
}
