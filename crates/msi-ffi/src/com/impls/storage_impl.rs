//! Native CFB-backed `IStorage` implementation.

use crate::com::storage::{IEnumSTATSTG, IStorage, IStorageVtbl, SNB};
use crate::com::stream::{IStream, STATSTG};
use crate::com::util::ComVTableBuilder;
use crate::com::{IUnknown, IUnknownVtbl, GUID, HRESULT, S_OK, ULONG};
use crate::error::STG_E_FILENOTFOUND;
use msi::cfb::reader::CfbReader;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

/// A native implementation of `IStorage` over an `msi::cfb::reader::CfbReader`.
#[repr(C)]
#[derive(Debug)]
pub struct NativeStorage {
    /// `VTable` pointer must be first.
    pub lp_vtbl: *const IStorageVtbl,
    /// Reference count.
    pub ref_count: AtomicU32,
    /// Underlying CFB reader (if opened for reading).
    pub reader: Option<CfbReader>,
}

impl NativeStorage {
    /// Creates a new native `IStorage`.
    #[must_use]
    pub fn new(reader: Option<CfbReader>) -> Box<Self> {
        Box::new(Self {
            lp_vtbl: &NATIVE_STORAGE_VTBL,
            ref_count: AtomicU32::new(1),
            reader,
        })
    }
}

impl Default for NativeStorage {
    fn default() -> Self {
        Self {
            lp_vtbl: &NATIVE_STORAGE_VTBL,
            ref_count: AtomicU32::new(1),
            reader: None,
        }
    }
}

impl crate::com::util::ComObject for NativeStorage {
    fn add_ref(&self) -> ULONG {
        self.ref_count.fetch_add(1, Ordering::SeqCst) + 1
    }
    fn release(&self) -> ULONG {
        let prev = self.ref_count.fetch_sub(1, Ordering::SeqCst);
        if prev == 0 {
            self.ref_count.store(0, Ordering::SeqCst);
            return 0;
        }
        prev - 1
    }
}

// Stubs for IStorage VTable.
const unsafe extern "system" fn create_stream(
    _this: *mut IStorage,
    _pwcs_name: *const u16,
    _grf_mode: u32,
    _reserved1: u32,
    _reserved2: u32,
    _ppstm: *mut *mut IStream,
) -> HRESULT {
    crate::error::E_FAIL
}
unsafe extern "system" fn open_stream(
    this: *mut IStorage,
    pwcs_name: *const u16,
    _reserved1: *mut c_void,
    _grf_mode: u32,
    _reserved2: u32,
    ppstm: *mut *mut IStream,
) -> HRESULT {
    if pwcs_name.is_null() || ppstm.is_null() {
        return crate::com::E_POINTER;
    }

    let obj = &mut *(this.cast::<NativeStorage>());
    let reader = match obj.reader.as_ref() {
        Some(r) => r,
        None => return crate::error::E_FAIL,
    };

    let len = (0..).take_while(|&i| *pwcs_name.add(i) != 0).count();
    let name_slice = std::slice::from_raw_parts(pwcs_name, len);
    let name_str = String::from_utf16_lossy(name_slice);

    match reader.read_stream(&name_str) {
        Ok(data) => {
            let stream = crate::com::impls::stream_impl::NativeStream::new(data);
            *ppstm = Box::into_raw(stream).cast::<IStream>();
            S_OK
        }
        Err(_) => STG_E_FILENOTFOUND,
    }
}
const unsafe extern "system" fn create_storage(
    _this: *mut IStorage,
    _pwcs_name: *const u16,
    _grf_mode: u32,
    _reserved1: u32,
    _reserved2: u32,
    _ppstg: *mut *mut IStorage,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn open_storage(
    _this: *mut IStorage,
    _pwcs_name: *const u16,
    _pstg_priority: *mut IStorage,
    _grf_mode: u32,
    _snb_exclude: SNB,
    _reserved: u32,
    _ppstg: *mut *mut IStorage,
) -> HRESULT {
    STG_E_FILENOTFOUND
}
const unsafe extern "system" fn copy_to(
    _this: *mut IStorage,
    _ciid_exclude: u32,
    _rgiid_exclude: *const GUID,
    _snb_exclude: SNB,
    _pstg_dest: *mut IStorage,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn move_element_to(
    _this: *mut IStorage,
    _pwcs_name: *const u16,
    _pstg_dest: *mut IStorage,
    _pwcs_new_name: *const u16,
    _grf_flags: u32,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn commit(_this: *mut IStorage, _grf_commit_flags: u32) -> HRESULT {
    S_OK
}
const unsafe extern "system" fn revert(_this: *mut IStorage) -> HRESULT {
    S_OK
}
const unsafe extern "system" fn enum_elements(
    _this: *mut IStorage,
    _reserved1: u32,
    _reserved2: *mut c_void,
    _reserved3: u32,
    _ppenum: *mut *mut IEnumSTATSTG,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn destroy_element(
    _this: *mut IStorage,
    _pwcs_name: *const u16,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn rename_element(
    _this: *mut IStorage,
    _pwcs_old_name: *const u16,
    _pwcs_new_name: *const u16,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn set_element_times(
    _this: *mut IStorage,
    _pwcs_name: *const u16,
    _pctime: *const u64,
    _patime: *const u64,
    _pmtime: *const u64,
) -> HRESULT {
    crate::error::E_FAIL
}
const unsafe extern "system" fn set_class(_this: *mut IStorage, _clsid: *const GUID) -> HRESULT {
    S_OK
}
const unsafe extern "system" fn set_state_bits(
    _this: *mut IStorage,
    _grf_state_bits: u32,
    _grf_mask: u32,
) -> HRESULT {
    S_OK
}
const unsafe extern "system" fn stat(
    _this: *mut IStorage,
    _pstatstg: *mut STATSTG,
    _grf_stat_flag: u32,
) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn query_interface(
    _this: *mut IUnknown,
    _riid: *const GUID,
    _ppv_object: *mut *mut c_void,
) -> HRESULT {
    crate::com::E_NOINTERFACE
}

static NATIVE_STORAGE_VTBL: IStorageVtbl = IStorageVtbl {
    parent: IUnknownVtbl {
        QueryInterface: query_interface,
        AddRef: ComVTableBuilder::add_ref::<NativeStorage>,
        Release: ComVTableBuilder::release::<NativeStorage>,
    },
    CreateStream: create_stream,
    OpenStream: open_stream,
    CreateStorage: create_storage,
    OpenStorage: open_storage,
    CopyTo: copy_to,
    MoveElementTo: move_element_to,
    Commit: commit,
    Revert: revert,
    EnumElements: enum_elements,
    DestroyElement: destroy_element,
    RenameElement: rename_element,
    SetElementTimes: set_element_times,
    SetClass: set_class,
    SetStateBits: set_state_bits,
    Stat: stat,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::util::ComObject;

    #[test]
    fn test_native_storage_com() {
        let mut storage = NativeStorage::default();
        let ptr = std::ptr::from_mut(&mut storage).cast::<IStorage>();
        let unk = ptr.cast::<IUnknown>();

        assert_eq!(storage.add_ref(), 2);
        assert_eq!(storage.release(), 1);
        assert_eq!(storage.release(), 0);
        assert_eq!(storage.release(), 0);

        unsafe {
            assert_eq!(
                query_interface(unk, std::ptr::null(), std::ptr::null_mut()),
                crate::com::E_NOINTERFACE
            );

            assert_eq!(
                create_stream(ptr, std::ptr::null(), 0, 0, 0, std::ptr::null_mut()),
                crate::error::E_FAIL
            );

            assert_eq!(
                open_stream(
                    ptr,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    0,
                    0,
                    std::ptr::null_mut()
                ),
                crate::com::E_POINTER
            );
            assert_eq!(
                open_stream(
                    ptr,
                    [0u16].as_ptr(),
                    std::ptr::null_mut(),
                    0,
                    0,
                    &mut std::ptr::null_mut()
                ),
                crate::error::E_FAIL
            );

            assert_eq!(
                create_storage(ptr, std::ptr::null(), 0, 0, 0, std::ptr::null_mut()),
                crate::error::E_FAIL
            );
            assert_eq!(
                open_storage(
                    ptr,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut()
                ),
                STG_E_FILENOTFOUND
            );
            assert_eq!(
                copy_to(
                    ptr,
                    0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                crate::error::E_FAIL
            );
            assert_eq!(
                move_element_to(
                    ptr,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0
                ),
                crate::error::E_FAIL
            );
            assert_eq!(commit(ptr, 0), S_OK);
            assert_eq!(revert(ptr), S_OK);
            assert_eq!(
                enum_elements(ptr, 0, std::ptr::null_mut(), 0, std::ptr::null_mut()),
                crate::error::E_FAIL
            );
            assert_eq!(destroy_element(ptr, std::ptr::null()), crate::error::E_FAIL);
            assert_eq!(
                rename_element(ptr, std::ptr::null(), std::ptr::null()),
                crate::error::E_FAIL
            );
            assert_eq!(
                set_element_times(
                    ptr,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null()
                ),
                crate::error::E_FAIL
            );
            assert_eq!(set_class(ptr, std::ptr::null()), S_OK);
            assert_eq!(set_state_bits(ptr, 0, 0), S_OK);
            assert_eq!(stat(ptr, std::ptr::null_mut(), 0), crate::error::E_FAIL);

            let cfb_data_vec = {
                let mut writer = msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
                writer
                    .add_stream("\u{0005}SummaryInformation", b"content")
                    .unwrap();
                writer.build()
            };
            let cfb_data = &cfb_data_vec;
            let reader = CfbReader::new(cfb_data).unwrap();
            let mut storage = NativeStorage::new(Some(reader));
            let ptr = std::ptr::from_mut(&mut *storage).cast::<IStorage>();

            let name: Vec<u16> = "\u{0005}SummaryInformation\0".encode_utf16().collect();
            let mut pstm = std::ptr::null_mut();
            assert_eq!(
                open_stream(ptr, name.as_ptr(), std::ptr::null_mut(), 0, 0, &mut pstm),
                S_OK
            );
            assert!(!pstm.is_null());
            let _ = Box::from_raw(pstm.cast::<crate::com::impls::stream_impl::NativeStream>());

            // test null pointers
            assert_eq!(
                open_stream(ptr, std::ptr::null(), std::ptr::null_mut(), 0, 0, &mut pstm),
                crate::com::E_POINTER
            );
            assert_eq!(
                open_stream(
                    ptr,
                    name.as_ptr(),
                    std::ptr::null_mut(),
                    0,
                    0,
                    std::ptr::null_mut()
                ),
                crate::com::E_POINTER
            );

            let bad_name: Vec<u16> = "NonExistent\0".encode_utf16().collect();
            assert_eq!(
                open_stream(
                    ptr,
                    bad_name.as_ptr(),
                    std::ptr::null_mut(),
                    0,
                    0,
                    &mut pstm
                ),
                STG_E_FILENOTFOUND
            );
        }
    }
}
