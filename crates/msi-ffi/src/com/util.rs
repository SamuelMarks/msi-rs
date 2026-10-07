//! Implementations of COM standard objects.

use crate::com::IUnknown;
use crate::com::ULONG;

/// `VTable` structure to dispatch standard COM calls into Rust implementations.
#[derive(Debug)]
pub struct ComVTableBuilder;

impl ComVTableBuilder {
    /// Generic implementation of `AddRef`.
    /// # Safety
    /// Unsafe C FFI.
    /// # Safety
    /// Unsafe C FFI.
    pub unsafe extern "system" fn add_ref<T: ComObject>(this: *mut IUnknown) -> ULONG {
        let obj = &*(this.cast::<T>());
        obj.add_ref()
    }

    /// Generic implementation of `Release`.
    /// # Safety
    /// Unsafe C FFI.
    /// # Safety
    /// Unsafe C FFI.
    pub unsafe extern "system" fn release<T: ComObject>(this: *mut IUnknown) -> ULONG {
        let obj = &*(this.cast::<T>());
        let count = obj.release();
        if count == 0 {
            let _ = Box::from_raw(this.cast::<T>());
        }
        count
    }
}

/// Generic trait for object state tracking.
pub trait ComObject {
    /// Increment reference count.
    fn add_ref(&self) -> ULONG;
    /// Decrement reference count.
    fn release(&self) -> ULONG;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_com_vtable_builder_ptr() {
        let obj = Box::new(DummyObj {
            count: std::sync::atomic::AtomicU32::new(1),
        });
        let ptr = Box::into_raw(obj).cast::<IUnknown>();
        let add_ref_fn: unsafe extern "system" fn(*mut IUnknown) -> ULONG =
            ComVTableBuilder::add_ref::<DummyObj>;
        let release_fn: unsafe extern "system" fn(*mut IUnknown) -> ULONG =
            ComVTableBuilder::release::<DummyObj>;
        unsafe {
            assert_eq!(add_ref_fn(ptr), 2);
            assert_eq!(release_fn(ptr), 1);
            assert_eq!(release_fn(ptr), 0);
        }
    }

    use super::*;

    #[repr(C, align(8))]
    struct DummyObj {
        count: std::sync::atomic::AtomicU32,
    }
    impl ComObject for DummyObj {
        fn add_ref(&self) -> ULONG {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
        }
        fn release(&self) -> ULONG {
            self.count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1
        }
    }

    #[test]
    fn test_com_vtable_builder() {
        let obj = Box::new(DummyObj {
            count: std::sync::atomic::AtomicU32::new(1),
        });
        let ptr = Box::into_raw(obj).cast::<IUnknown>();
        unsafe {
            assert_eq!(ComVTableBuilder::add_ref::<DummyObj>(ptr), 2);
            assert_eq!(ComVTableBuilder::release::<DummyObj>(ptr), 1);
            assert_eq!(ComVTableBuilder::release::<DummyObj>(ptr), 0); // drops obj
        }
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_com_object_trait() {
        struct MockObj {
            count: std::sync::atomic::AtomicU32,
        }
        impl ComObject for MockObj {
            fn add_ref(&self) -> ULONG {
                self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
            }
            fn release(&self) -> ULONG {
                self.count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1
            }
        }

        let obj = MockObj {
            count: std::sync::atomic::AtomicU32::new(1),
        };
        assert_eq!(obj.add_ref(), 2);
        assert_eq!(obj.release(), 1);
    }
}
