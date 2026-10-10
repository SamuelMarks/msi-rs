use msi::platform::SqliteRegistryDriver;
use std::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref GLOBAL_REGISTRY: Mutex<SqliteRegistryDriver> = Mutex::new({
        let path = std::env::var("MSI_REGISTRY_PATH")
            .unwrap_or_else(|_| SqliteRegistryDriver::system_db_path().to_string());
        SqliteRegistryDriver::default()
    });
}

#[allow(dead_code, unreachable_pub)]
pub fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut SqliteRegistryDriver) -> R,
{
    let mut reg = GLOBAL_REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut reg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::catch_unwind;

    #[test]
    fn test_with_registry() {
        let result = with_registry(|_reg| 42);
        assert_eq!(result, 42);

        // Test poisoning recovery
        let _ = catch_unwind(|| {
            let _lock = GLOBAL_REGISTRY.lock().unwrap();
            panic!("poisoning lock");
        });

        let recovered_result = with_registry(|_reg| 84);
        assert_eq!(recovered_result, 84);
    }
}
