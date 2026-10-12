//! C-API Exports for creating `IStorage`.

use crate::com::impls::storage_impl::NativeStorage;
use crate::com::storage::IStorage;
use msi::cfb::reader::CfbReader;

/// Creates a new `IStorage` instance wrapping a CFB reader from memory.
///
/// # Arguments
///
/// * `data` - Pointer to the CFB memory buffer.
/// * `len` - Length of the buffer.
/// * `ppstg` - Output pointer to the `IStorage` interface.
///
/// # Returns
///
/// `S_OK` on success, or an error HRESULT.
///
/// # Safety
///
/// `data` must point to `len` bytes of readable memory.
/// `ppstg` must be a valid pointer to receive the `IStorage` pointer.
#[no_mangle]
pub unsafe extern "C" fn msi_create_storage_from_memory(
    data: *const u8,
    len: usize,
    ppstg: *mut *mut IStorage,
) -> crate::com::HRESULT {
    if data.is_null() || ppstg.is_null() {
        return crate::com::E_POINTER;
    }

    let slice = std::slice::from_raw_parts(data, len);

    match CfbReader::new(slice) {
        Ok(reader) => {
            let storage = NativeStorage::new(Some(reader));
            *ppstg = Box::into_raw(storage).cast::<IStorage>();
            crate::com::S_OK
        }
        Err(_) => crate::error::E_FAIL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_create_storage_from_memory_nulls() {
        let mut stg: *mut IStorage = ptr::null_mut();
        // Bad data (fails CFB parsing)
        let data = [0u8; 10];
        let res = unsafe { msi_create_storage_from_memory(data.as_ptr(), data.len(), &mut stg) };
        assert_eq!(res, crate::error::E_FAIL);

        // Good CFB data
        let cfb_data = {
            let mut writer = msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
            writer.add_stream("TestStream", b"content").unwrap();
            writer.build()
        };
        let res =
            unsafe { msi_create_storage_from_memory(cfb_data.as_ptr(), cfb_data.len(), &mut stg) };
        assert_eq!(res, crate::com::S_OK);

        unsafe {
            let _ = Box::from_raw(stg.cast::<NativeStorage>());
        }

        let mut stg: *mut IStorage = ptr::null_mut();
        // Null data
        let res = unsafe { msi_create_storage_from_memory(ptr::null(), 0, &mut stg) };
        assert_eq!(res, crate::com::E_POINTER);

        // Null out pointer
        let data = [0u8; 10];
        let res =
            unsafe { msi_create_storage_from_memory(data.as_ptr(), data.len(), ptr::null_mut()) };
        assert_eq!(res, crate::com::E_POINTER);
    }
}
