//! Implementations of COM standard objects.

use crate::com::IDispatchVtbl;
use crate::com::{GUID, ULONG};

/// Internal COM object stub representing `View`.
#[repr(C)]
#[derive(Debug)]
pub struct ViewCOM {
    /// `VTable` for `IDispatch`.
    pub lp_vtbl: *const IDispatchVtbl,
    /// Reference count.
    pub ref_count: std::sync::atomic::AtomicU32,
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

impl crate::com::util::ComObject for ViewCOM {
    fn add_ref(&self) -> ULONG {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1
    }
    fn release(&self) -> ULONG {
        let prev = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if prev == 0 {
            self.ref_count.store(0, std::sync::atomic::Ordering::SeqCst);
            return 0;
        }
        prev - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::util::ComObject;
    use std::ptr;

    #[test]
    fn test_view_com() {
        let view = ViewCOM {
            lp_vtbl: ptr::null(),
            ref_count: std::sync::atomic::AtomicU32::new(0),
        };
        assert_eq!(view.add_ref(), 1);
        assert_eq!(view.add_ref(), 2);
        assert_eq!(view.release(), 1);
        assert_eq!(view.release(), 0);
        assert_eq!(view.release(), 0); // No underflow
    }
}
