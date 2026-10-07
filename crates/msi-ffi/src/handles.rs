//! Global handle table and `MSIHANDLE` lifecycle management.
//!
//! The Windows Installer API uses 32-bit `MSIHANDLE` integers
//! rather than opaque pointers. This module implements a thread-safe,
//! global handle registry mapping `u32` IDs to internal Rust objects.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

#[cfg(not(test))]
use std::sync::OnceLock;

/// A strongly-typed Windows Installer handle.
pub type MsiHandle = u32;

/// A null or invalid handle value.
pub const MSI_NULL_HANDLE: MsiHandle = 0;

/// Enumeration of all possible internal objects that can be referenced by an `MseHandle`.
#[derive(Debug)]
pub enum MsiObject {
    /// A relational database.
    Database(crate::types::MsiDatabaseHandle),
    /// A multi-package transaction manager.
    Transaction(Box<crate::types::MsiTransactionHandle>),
    /// A relational database record.
    Record(crate::types::MsiRecordHandle),
    /// A relational database view.
    View(crate::types::MsiViewHandle),
    /// A UI preview session.
    UiPreview(Box<crate::types::MsiUiPreviewHandle>),
}

/// Handle table state.
pub(crate) struct HandleTable {
    /// Atomic counter for the next ID.
    pub(crate) next_id: AtomicU32,
    /// Thread-safe map of handles.
    pub(crate) map: RwLock<HashMap<MsiHandle, MsiObject>>,
}

impl HandleTable {
    /// Creates a new `HandleTable`.
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            map: RwLock::new(HashMap::new()),
        }
    }
}

/// Global singleton instance.
#[cfg(not(test))]
pub(crate) static HANDLE_TABLE: OnceLock<HandleTable> = OnceLock::new();

/// Executes a closure with the global handle table instance.
#[cfg(not(test))]
pub(crate) fn with_handle_table<R, F: FnOnce(&HandleTable) -> R>(f: F) -> R {
    f(HANDLE_TABLE.get_or_init(HandleTable::new))
}

#[cfg(test)]
thread_local! {
    static THREAD_HANDLE_TABLE: HandleTable = HandleTable::new();
}

/// Executes a closure with the thread-local handle table instance.
#[cfg(test)]
pub(crate) fn with_handle_table<R, F: FnOnce(&HandleTable) -> R>(f: F) -> R {
    THREAD_HANDLE_TABLE.with(f)
}

/// Allocates a new handle for the given object.
#[must_use]
pub fn alloc_handle(obj: MsiObject) -> MsiHandle {
    with_handle_table(|table| {
        let mut handle = table.next_id.fetch_add(1, Ordering::Relaxed);
        if handle == MSI_NULL_HANDLE {
            handle = table.next_id.fetch_add(1, Ordering::Relaxed);
        }

        let mut map = table
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.insert(handle, obj);
        handle
    })
}

/// Closes an open handle, freeing the associated object.
#[must_use]
pub fn close_handle(handle: MsiHandle) -> bool {
    with_handle_table(|table| {
        let mut map = table
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.remove(&handle).is_some()
    })
}

/// Closes all open handles.
pub fn close_all_handles() {
    with_handle_table(|table| {
        let mut map = table
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.clear();
    });
}

/// Executes a closure with a reference to the object associated with the handle.
pub fn with_handle<F, R>(handle: MsiHandle, f: F) -> Option<R>
where
    F: FnOnce(&MsiObject) -> R,
{
    with_handle_table(|table| {
        let map = table
            .map
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(&handle).map(f)
    })
}

/// Executes a closure with a mutable reference to the object associated with the handle.
pub fn with_handle_mut<F, R>(handle: MsiHandle, f: F) -> Option<R>
where
    F: FnOnce(&mut MsiObject) -> R,
{
    with_handle_table(|table| {
        let mut map = table
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get_mut(&handle).map(f)
    })
}

/// Executes a closure with mutable access to one handle and read access to another.
/// Safely extracts and restores objects from the handle table to satisfy borrow rules.
pub fn with_handle_mut_and_read<F, R>(
    handle_mut: MsiHandle,
    handle_read: MsiHandle,
    f: F,
) -> Option<R>
where
    F: FnOnce(&mut MsiObject, &MsiObject) -> R,
{
    if handle_mut == handle_read {
        return None;
    }

    with_handle_table(|table| {
        let mut map = table
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut obj_mut = map.remove(&handle_mut)?;
        let obj_read = map.remove(&handle_read);

        let res = if let Some(read_val) = &obj_read {
            f(&mut obj_mut, read_val)
        } else {
            // Restore immediately if read handle was invalid
            map.insert(handle_mut, obj_mut);
            return None;
        };

        map.insert(handle_mut, obj_mut);
        if let Some(read_val) = obj_read {
            map.insert(handle_read, read_val);
        }

        Some(res)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_allocation_and_close() {
        let rec = msi::database::tables::record::Record::new();
        let h = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));
        assert_ne!(h, MSI_NULL_HANDLE);

        let mut found = false;
        with_handle(h, |obj| {
            assert!(matches!(obj, MsiObject::Record(_)));
            found = true;
        });
        assert!(found);

        let mut found_mut = false;
        with_handle_mut(h, |obj| {
            assert!(matches!(obj, MsiObject::Record(_)));
            found_mut = true;
        });
        assert!(found_mut);

        // Test non-Record object
        let db = msi::wix::linker::LinkedDatabase::default();
        let h2 = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: db,
        }));
        let mut is_db = false;
        with_handle(h2, |obj| {
            assert!(matches!(obj, MsiObject::Database(_)));
            is_db = true;
        });
        assert!(is_db);

        let mut is_db_mut = false;
        with_handle_mut(h2, |obj| {
            assert!(matches!(obj, MsiObject::Database(_)));
            is_db_mut = true;
        });
        assert!(is_db_mut);

        // Mismatch check
        let mut checked_h2 = false;
        with_handle(h2, |obj| {
            assert!(!matches!(obj, MsiObject::Record(_)));
            checked_h2 = true;
        });
        assert!(checked_h2);

        let mut checked_h = false;
        with_handle_mut(h, |obj| {
            assert!(!matches!(obj, MsiObject::Database(_)));
            checked_h = true;
        });
        assert!(checked_h);

        assert!(close_handle(h));
        assert!(!close_handle(h));
        assert!(close_handle(h2));
    }

    #[test]
    fn test_handle_wraparound() {
        with_handle_table(|table| {
            table.next_id.store(0xFFFF_FFFF, Ordering::Relaxed);
        });
        let rec1 = msi::database::tables::record::Record::new();
        let h1 = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec1,
        }));

        let rec2 = msi::database::tables::record::Record::new();
        let h2 = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec2,
        }));

        assert_ne!(h1, MSI_NULL_HANDLE);
        assert_ne!(h2, MSI_NULL_HANDLE);

        // Test with_handle_mut_and_read valid handles
        let res = with_handle_mut_and_read(h1, h2, |mut_obj, read_obj| true);
        assert!(res.is_some());

        // Test with_handle_mut_and_read invalid read handle
        let res = with_handle_mut_and_read(h1, MSI_NULL_HANDLE, |_, _| true);
        assert!(res.is_none());

        // Also test with_handle_mut_and_read same handle
        let res = with_handle_mut_and_read(h1, h1, |_, _| true);
        assert!(res.is_none());

        assert!(close_handle(h1));
        assert!(close_handle(h2));
    }

    #[test]
    fn test_close_all_handles() {
        let rec1 = msi::database::tables::record::Record::new();
        let h1 = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec1,
        }));
        let rec2 = msi::database::tables::record::Record::new();
        let h2 = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec2,
        }));

        assert_ne!(h1, MSI_NULL_HANDLE);
        assert_ne!(h2, MSI_NULL_HANDLE);

        close_all_handles();

        let mut checked1 = false;
        with_handle(h1, |_| {
            checked1 = true;
        });
        assert!(!checked1);

        let mut checked2 = false;
        with_handle(h2, |_| {
            checked2 = true;
        });
        assert!(!checked2);
    }
}
