//! Implementations of COM standard objects.

use crate::com::IDispatchVtbl;
use crate::com::{GUID, ULONG};

/// Internal COM object stub representing `Installer`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InstallerCOM {
    /// `VTable` for `IDispatch`.
    pub lp_vtbl: *const IDispatchVtbl,
    /// Reference count.
    pub ref_count: ULONG,
}

impl InstallerCOM {
    /// CLSID for WindowsInstaller.Installer
    pub const CLSID: GUID = GUID::new(
        0x000c_1090,
        0x0000,
        0x0000,
        [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
    /// IID for WindowsInstaller.Installer
    pub const IID: GUID = GUID::new(
        0x000c_1090,
        0x0000,
        0x0000,
        [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
}
