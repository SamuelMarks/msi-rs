//! Implementations of COM standard objects.

use crate::com::IDispatchVtbl;
use crate::com::{GUID, ULONG};

/// Internal COM object stub representing `View`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ViewCOM {
    /// `VTable` for `IDispatch`.
    pub lp_vtbl: *const IDispatchVtbl,
    /// Reference count.
    pub ref_count: ULONG,
}

impl ViewCOM {
    /// IID for WindowsInstaller.View
    pub const IID: GUID = GUID::new(
        0x000c_109c,
        0x0000,
        0x0000,
        [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
}
