//! Implementations of COM standard objects.

use crate::com::IDispatchVtbl;
use crate::com::{GUID, ULONG};

/// Internal COM object stub representing `Session`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SessionCOM {
    /// `VTable` for `IDispatch`.
    pub lp_vtbl: *const IDispatchVtbl,
    /// Reference count.
    pub ref_count: ULONG,
}

impl SessionCOM {
    /// IID for WindowsInstaller.Session
    pub const IID: GUID = GUID::new(
        0x000c_109e,
        0x0000,
        0x0000,
        [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
}
