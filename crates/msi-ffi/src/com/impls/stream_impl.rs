//! Native CFB-backed `IStream` implementation.

use crate::com::stream::{IStream, IStreamVtbl, LARGE_INTEGER, STATSTG, ULARGE_INTEGER};
use crate::com::util::ComVTableBuilder;
use crate::com::{IUnknown, IUnknownVtbl, GUID, HRESULT, S_OK, ULONG};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

/// A native implementation of `IStream` over a `Vec<u8>`.
#[repr(C)]
#[derive(Debug)]
pub struct NativeStream {
    /// `VTable` pointer must be first.
    pub lp_vtbl: *const IStreamVtbl,
    /// Reference count.
    pub ref_count: AtomicU32,
    /// Underlying stream data.
    pub data: Vec<u8>,
    /// Current seek offset.
    pub offset: u64,
}

impl NativeStream {
    /// Creates a new native `IStream`.
    ///
    /// # Arguments
    ///
    /// * `data` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Box<Self> {
        Box::new(Self {
            lp_vtbl: &NATIVE_STREAM_VTBL,
            ref_count: AtomicU32::new(1),
            data,
            offset: 0,
        })
    }
}

impl crate::com::util::ComObject for NativeStream {
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

unsafe extern "system" fn read(
    this: *mut IStream,
    pv: *mut c_void,
    cb: ULONG,
    pcb_read: *mut ULONG,
) -> HRESULT {
    let obj = &mut *(this.cast::<NativeStream>());
    if pv.is_null() {
        return crate::com::E_POINTER;
    }

    let remaining = obj.data.len() as u64 - obj.offset;
    let to_read = std::cmp::min(u64::from(cb), remaining) as usize;

    ptr::copy_nonoverlapping(
        obj.data.as_ptr().add(obj.offset as usize),
        pv.cast::<u8>(),
        to_read,
    );
    obj.offset += to_read as u64;

    if !pcb_read.is_null() {
        *pcb_read = to_read as u32;
    }
    S_OK
}

const unsafe extern "system" fn write(
    _this: *mut IStream,
    _pv: *const c_void,
    _cb: ULONG,
    _pcb_written: *mut ULONG,
) -> HRESULT {
    crate::error::E_FAIL // Read-only for now
}

unsafe extern "system" fn seek(
    this: *mut IStream,
    dlib_move: LARGE_INTEGER,
    dw_origin: u32,
    plib_new_position: *mut ULARGE_INTEGER,
) -> HRESULT {
    let obj = &mut *(this.cast::<NativeStream>());
    let new_pos = match dw_origin {
        0 => dlib_move.QuadPart,                         // STREAM_SEEK_SET
        1 => obj.offset as i64 + dlib_move.QuadPart,     // STREAM_SEEK_CUR
        2 => obj.data.len() as i64 + dlib_move.QuadPart, // STREAM_SEEK_END
        _ => return crate::error::E_INVALIDARG,
    };

    if new_pos < 0 {
        return crate::error::E_INVALIDARG;
    }
    obj.offset = new_pos as u64;

    if !plib_new_position.is_null() {
        (*plib_new_position).QuadPart = obj.offset;
    }
    S_OK
}

const unsafe extern "system" fn set_size(
    _this: *mut IStream,
    _lib_new_size: ULARGE_INTEGER,
) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn copy_to(
    _this: *mut IStream,
    _pstm: *mut IStream,
    _cb: ULARGE_INTEGER,
    _pcb_read: *mut ULARGE_INTEGER,
    _pcb_written: *mut ULARGE_INTEGER,
) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn commit(_this: *mut IStream, _grf_commit_flags: u32) -> HRESULT {
    S_OK
}

const unsafe extern "system" fn revert(_this: *mut IStream) -> HRESULT {
    S_OK
}

const unsafe extern "system" fn lock_region(
    _this: *mut IStream,
    _lib_offset: ULARGE_INTEGER,
    _cb: ULARGE_INTEGER,
    _dw_lock_type: u32,
) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn unlock_region(
    _this: *mut IStream,
    _lib_offset: ULARGE_INTEGER,
    _cb: ULARGE_INTEGER,
    _dw_lock_type: u32,
) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn stat(
    _this: *mut IStream,
    _pstatstg: *mut STATSTG,
    _grf_stat_flag: u32,
) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn clone(_this: *mut IStream, _ppstm: *mut *mut IStream) -> HRESULT {
    crate::error::E_FAIL
}

const unsafe extern "system" fn query_interface(
    _this: *mut IUnknown,
    _riid: *const GUID,
    _ppv_object: *mut *mut c_void,
) -> HRESULT {
    crate::com::E_NOINTERFACE
}

static NATIVE_STREAM_VTBL: IStreamVtbl = IStreamVtbl {
    parent: IUnknownVtbl {
        QueryInterface: query_interface,
        AddRef: ComVTableBuilder::add_ref::<NativeStream>,
        Release: ComVTableBuilder::release::<NativeStream>,
    },
    Read: read,
    Write: write,
    Seek: seek,
    SetSize: set_size,
    CopyTo: copy_to,
    Commit: commit,
    Revert: revert,
    LockRegion: lock_region,
    UnlockRegion: unlock_region,
    Stat: stat,
    Clone: clone,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::util::ComObject;

    #[test]
    fn test_native_stream_com() {
        let mut stream = NativeStream::new(vec![1, 2, 3]);
        let ptr = ptr::from_mut(&mut *stream).cast::<IStream>();
        let unk = ptr.cast::<IUnknown>();

        assert_eq!(stream.add_ref(), 2);
        assert_eq!(stream.release(), 1);
        assert_eq!(stream.release(), 0);
        assert_eq!(stream.release(), 0);

        unsafe {
            assert_eq!(
                query_interface(unk, ptr::null(), ptr::null_mut()),
                crate::com::E_NOINTERFACE
            );

            let mut read_cb = 0;
            let mut buf = [0u8; 2];
            assert_eq!(
                read(ptr, buf.as_mut_ptr().cast::<c_void>(), 2, &mut read_cb),
                S_OK
            );
            assert_eq!(read_cb, 2);
            assert_eq!(buf, [1, 2]);

            let mut buf2 = [0u8; 1];
            assert_eq!(
                read(ptr, buf2.as_mut_ptr().cast::<c_void>(), 1, ptr::null_mut()),
                S_OK
            );
            assert_eq!(buf2, [3]);

            assert_eq!(
                read(ptr, ptr::null_mut(), 2, &mut read_cb),
                crate::com::E_POINTER
            );

            assert_eq!(
                write(ptr, ptr::null(), 0, ptr::null_mut()),
                crate::error::E_FAIL
            );

            let mut new_pos = ULARGE_INTEGER { QuadPart: 0 };
            assert_eq!(
                seek(ptr, LARGE_INTEGER { QuadPart: 0 }, 0, &mut new_pos),
                S_OK
            ); // SET to 0
            assert_eq!(new_pos.QuadPart, 0);

            assert_eq!(
                seek(ptr, LARGE_INTEGER { QuadPart: 0 }, 0, ptr::null_mut()),
                S_OK
            );

            assert_eq!(
                seek(ptr, LARGE_INTEGER { QuadPart: 1 }, 1, &mut new_pos),
                S_OK
            ); // CUR + 1
            assert_eq!(new_pos.QuadPart, 1);

            assert_eq!(
                seek(ptr, LARGE_INTEGER { QuadPart: -1 }, 2, &mut new_pos),
                S_OK
            ); // END - 1
            assert_eq!(new_pos.QuadPart, 2);

            assert_eq!(
                seek(ptr, LARGE_INTEGER { QuadPart: -10 }, 2, &mut new_pos),
                crate::error::E_INVALIDARG
            ); // < 0
            assert_eq!(
                seek(ptr, LARGE_INTEGER { QuadPart: 0 }, 99, &mut new_pos),
                crate::error::E_INVALIDARG
            ); // invalid origin

            assert_eq!(
                set_size(ptr, ULARGE_INTEGER { QuadPart: 0 }),
                crate::error::E_FAIL
            );
            assert_eq!(
                copy_to(
                    ptr,
                    ptr::null_mut(),
                    ULARGE_INTEGER { QuadPart: 0 },
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                crate::error::E_FAIL
            );
            assert_eq!(commit(ptr, 0), S_OK);
            assert_eq!(revert(ptr), S_OK);
            assert_eq!(
                lock_region(
                    ptr,
                    ULARGE_INTEGER { QuadPart: 0 },
                    ULARGE_INTEGER { QuadPart: 0 },
                    0
                ),
                crate::error::E_FAIL
            );
            assert_eq!(
                unlock_region(
                    ptr,
                    ULARGE_INTEGER { QuadPart: 0 },
                    ULARGE_INTEGER { QuadPart: 0 },
                    0
                ),
                crate::error::E_FAIL
            );
            assert_eq!(stat(ptr, ptr::null_mut(), 0), crate::error::E_FAIL);
            assert_eq!(clone(ptr, ptr::null_mut()), crate::error::E_FAIL);
        }
    }
}
