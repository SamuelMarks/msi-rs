//! Shared thread-safe registry backend for Win32 MSI registry queries.

use msi::platform::SqliteRegistryDriver;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Returns a reference to the process-wide [`SqliteRegistryDriver`] mutex.
#[allow(unreachable_pub)]
pub fn global_registry() -> &'static Mutex<SqliteRegistryDriver> {
    /// Process-wide singleton registry store.
    static GLOBAL_REGISTRY: OnceLock<Mutex<SqliteRegistryDriver>> = OnceLock::new();
    GLOBAL_REGISTRY.get_or_init(|| Mutex::new(SqliteRegistryDriver::default()))
}

#[cfg(test)]
std::thread_local! {
    /// Thread-local isolated registry store for parallel unit tests.
    static TEST_REGISTRY: std::cell::RefCell<SqliteRegistryDriver> =
        std::cell::RefCell::new(SqliteRegistryDriver::default());
}

/// Executes a closure with exclusive access to the active [`SqliteRegistryDriver`].
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut SqliteRegistryDriver) -> R,
{
    #[cfg(test)]
    {
        TEST_REGISTRY.with(|cell| f(&mut cell.borrow_mut()))
    }
    #[cfg(not(test))]
    {
        let mut reg = global_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut reg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies `with_registry` and `global_registry` acquire the registry lock and return the closure result.
    #[test]
    fn test_with_registry() {
        let _guard = global_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = with_registry(|_reg| 42);
        assert_eq!(result, 42);
    }
}
