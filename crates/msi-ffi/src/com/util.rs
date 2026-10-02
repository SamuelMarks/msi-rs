//! Implementations of COM standard objects.

use crate::com::IUnknown;
use crate::com::ULONG;

/// `VTable` structure to dispatch standard COM calls into Rust implementations.
#[derive(Debug)]
pub struct ComVTableBuilder;

impl ComVTableBuilder {
    /// Generic implementation of `AddRef`.
    pub unsafe extern "system" fn add_ref<T: ComObject>(this: *mut IUnknown) -> ULONG {
        let obj = &mut *(this.cast::<T>());
        obj.add_ref()
    }

    /// Generic implementation of `Release`.
    pub unsafe extern "system" fn release<T: ComObject>(this: *mut IUnknown) -> ULONG {
        let obj = &mut *(this.cast::<T>());
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
    fn add_ref(&mut self) -> ULONG;
    /// Decrement reference count.
    fn release(&mut self) -> ULONG;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyObj {
        count: ULONG,
    }
    impl ComObject for DummyObj {
        fn add_ref(&mut self) -> ULONG {
            self.count += 1;
            self.count
        }
        fn release(&mut self) -> ULONG {
            self.count -= 1;
            self.count
        }
    }

    #[test]
    fn test_com_vtable_builder() {
        let mut obj = Box::new(DummyObj { count: 1 });
        let ptr = (&raw mut *obj).cast::<IUnknown>();
        std::mem::forget(obj);
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
            count: ULONG,
        }
        impl ComObject for MockObj {
            fn add_ref(&mut self) -> ULONG {
                self.count += 1;
                self.count
            }
            fn release(&mut self) -> ULONG {
                self.count -= 1;
                self.count
            }
        }

        let mut obj = MockObj { count: 1 };
        assert_eq!(obj.add_ref(), 2);
        assert_eq!(obj.release(), 1);
    }
}
