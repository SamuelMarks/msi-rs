//! C-ABI implementation of COM VARIANT API.

// use crate::com::bstr::{BSTR, sys_alloc_string, sys_free_string};

/// VARTYPE enum matching Win32 `VARENUM`.
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VARENUM {
    /// Empty variant.
    VtEmpty = 0,
    /// Null variant.
    VtNull = 1,
    /// 16-bit integer.
    VtI2 = 2,
    /// 32-bit integer.
    VtI4 = 3,
    /// BSTR string.
    VtBstr = 8,
    /// `IDispatch` pointer.
    VtDispatch = 9,
    /// Error code.
    VtError = 10,
}

/// COM VARIANT structure.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VARIANT {
    /// Variant type.
    pub vt: u16,
    /// Reserved.
    pub w_reserved_1: u16,
    /// Reserved.
    pub w_reserved_2: u16,
    /// Reserved.
    pub w_reserved_3: u16,
    /// Union payload data.
    pub data: [u8; 8],
}

impl VARIANT {
    /// Creates a new empty `VARIANT`.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn new_empty() -> Self {
        Self {
            vt: VARENUM::VtEmpty as u16,
            w_reserved_1: 0,
            w_reserved_2: 0,
            w_reserved_3: 0,
            data: [0; 8],
        }
    }

    /// Creates a new `VARIANT` containing a 32-bit integer.
    ///
    /// # Arguments
    ///
    /// * `val` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn new_i4(val: i32) -> Self {
        let mut v = Self::new_empty();
        v.vt = VARENUM::VtI4 as u16;
        let bytes = val.to_ne_bytes();
        v.data[0..4].copy_from_slice(&bytes);
        v
    }

    /// Safe, strongly-typed wrapper mapping to Rust enums.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn as_i4(&self) -> Option<i32> {
        (self.vt == VARENUM::VtI4 as u16).then(|| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&self.data[0..4]);
            i32::from_ne_bytes(bytes)
        })
    }

    /// Safe, strongly-typed wrapper mapping to Rust enums.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn as_bstr(&self) -> Option<*mut u16> {
        (self.vt == VARENUM::VtBstr as u16).then(|| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&self.data[0..8]);
            usize::from_ne_bytes(bytes) as *mut u16
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_com_variant() {
        let empty = VARIANT::new_empty();
        assert_eq!(empty.vt, VARENUM::VtEmpty as u16);

        let i4 = VARIANT::new_i4(42);
        assert_eq!(i4.vt, VARENUM::VtI4 as u16);
        assert_eq!(i4.as_i4(), Some(42));
        assert_eq!(empty.as_i4(), None);

        let mut bstr = VARIANT::new_empty();
        bstr.vt = VARENUM::VtBstr as u16;
        assert!(bstr.as_bstr().is_some());
        assert!(empty.as_bstr().is_none());
    }
}
