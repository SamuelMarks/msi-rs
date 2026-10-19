//! Shared thread-safe registry backend for Win32 MSI registry queries.

use msi::platform::SqliteRegistryDriver;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::thread::ThreadId;

/// Returns a reference to the process-wide [`SqliteRegistryDriver`] mutex.
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub fn global_registry() -> &'static Mutex<SqliteRegistryDriver> {
    /// Process-wide singleton registry store.
    static GLOBAL_REGISTRY: OnceLock<Mutex<SqliteRegistryDriver>> = OnceLock::new();
    GLOBAL_REGISTRY.get_or_init(|| Mutex::new(SqliteRegistryDriver::default()))
}

/// Returns a reference to the [`SqliteRegistryDriver`] map used by [`with_registry`].
fn registries() -> &'static Mutex<HashMap<Option<ThreadId>, SqliteRegistryDriver>> {
    /// Registry store map keyed by optional thread ID for parallel test isolation.
    static REGISTRIES: OnceLock<Mutex<HashMap<Option<ThreadId>, SqliteRegistryDriver>>> =
        OnceLock::new();
    REGISTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Executes a closure with exclusive access to the active [`SqliteRegistryDriver`].
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut SqliteRegistryDriver) -> R,
{
    let key = cfg!(test).then(|| std::thread::current().id());
    let mut map = registries()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(map.entry(key).or_default())
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
