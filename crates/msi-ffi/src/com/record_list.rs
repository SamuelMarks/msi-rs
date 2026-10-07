//! Implementations of COM standard objects.

use crate::com::IDispatchVtbl;
use crate::com::{GUID, ULONG};

/// Internal COM object stub representing `RecordList`.
#[repr(C)]
#[derive(Debug)]
pub struct RecordListCOM {
    /// `VTable` for `IDispatch`.
    pub lp_vtbl: *const IDispatchVtbl,
    /// Reference count.
    pub ref_count: std::sync::atomic::AtomicU32,
}

impl RecordListCOM {
    /// IID for WindowsInstaller.RecordList
    pub const IID: GUID = GUID::new(
        0x000c_1096,
        0x0000,
        0x0000,
        [0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
}

impl crate::com::util::ComObject for RecordListCOM {
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
    fn test_record_list_com() {
        let list = RecordListCOM {
            lp_vtbl: ptr::null(),
            ref_count: std::sync::atomic::AtomicU32::new(0),
        };
        assert_eq!(list.add_ref(), 1);
        assert_eq!(list.add_ref(), 2);
        assert_eq!(list.release(), 1);
        assert_eq!(list.release(), 0);
        assert_eq!(list.release(), 0); // No underflow
    }
}
