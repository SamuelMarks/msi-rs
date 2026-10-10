//! Match Wine stub behavior for unimplemented/deprecated functions
use crate::win32::{Dword, Lpcstr, Uint};
use std::ffi::c_void;


/// Migrates 10 cached packages (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn Migrate10CachedPackagesA(
    _szProductCode: Lpcstr,
    _szUserSid: Lpcstr,
    _szPreflightCheck: Lpcstr,
    _dwReserved: Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wine_stubs() {
        assert_eq!(Migrate10CachedPackagesA(std::ptr::null(), std::ptr::null(), std::ptr::null(), 0), 120);
    }
}
