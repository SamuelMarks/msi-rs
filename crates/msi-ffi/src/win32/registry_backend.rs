use msi::platform::SqliteRegistryDriver;
use std::sync::Mutex;
use std::sync::OnceLock;

#[allow(unreachable_pub)]
pub fn global_registry() -> &'static Mutex<SqliteRegistryDriver> {
    static GLOBAL_REGISTRY: OnceLock<Mutex<SqliteRegistryDriver>> = OnceLock::new();
    GLOBAL_REGISTRY.get_or_init(|| Mutex::new(SqliteRegistryDriver::default()))
}

#[allow(dead_code, unreachable_pub)]
pub fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut SqliteRegistryDriver) -> R,
{
    let mut reg = global_registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_registry() {
        let result = with_registry(|_reg| 42);
        assert_eq!(result, 42);
    }
}
