//! Detection and path resolution for Wine prefixes.
//!
//! Handles locating the `$WINEPREFIX` environment variable or falling back to
//! `~/.wine` on Unix systems when running in native POSIX mode.

use std::path::PathBuf;

/// Locates the root directory of the current Wine prefix.
///
/// # Returns
///
/// Returns the path to the Wine prefix, or `None` if it cannot be determined
/// (e.g., no `$WINEPREFIX` and no home directory).
#[must_use]
pub fn get_wine_prefix() -> Option<PathBuf> {
    if let Ok(env_prefix) = std::env::var("WINEPREFIX") {
        if !env_prefix.is_empty() {
            return Some(PathBuf::from(env_prefix));
        }
    }

    // Fallback to ~/.wine
    #[cfg(unix)]
    if let Some(home) = if std::env::var("WINEPREFIX_MOCK_NO_HOME").is_ok() {
        None
    } else {
        dirs::home_dir()
    } {
        return Some(home.join(".wine"));
    }

    None
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: String,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn new(key: &str) -> Self {
            Self {
                key: key.to_string(),
                original: env::var_os(key),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref val) = self.original {
                env::set_var(&self.key, val);
            } else {
                env::remove_var(&self.key);
            }
        }
    }

    #[test]
    fn test_env_guard_coverage() {
        env::set_var("WINE_TEST_GUARD", "1");
        {
            let _guard = EnvGuard::new("WINE_TEST_GUARD");
        }
        env::remove_var("WINE_TEST_GUARD");
        {
            let _guard = EnvGuard::new("WINE_TEST_GUARD");
        }
    }

    #[test]
    fn test_get_wine_prefix_with_env() {
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _guard_wine = EnvGuard::new("WINEPREFIX");
        let _guard_mock = EnvGuard::new("WINEPREFIX_MOCK_NO_HOME");

        env::remove_var("WINEPREFIX_MOCK_NO_HOME");
        env::set_var("WINEPREFIX", "/tmp/mock_wine_prefix");
        let prefix = get_wine_prefix();
        assert_eq!(prefix, Some(PathBuf::from("/tmp/mock_wine_prefix")));

        env::set_var("WINEPREFIX", "");
        let _prefix2 = get_wine_prefix();
        #[cfg(unix)]
        assert_eq!(_prefix2, dirs::home_dir().map(|h| h.join(".wine")));
        #[cfg(not(unix))]
        assert_eq!(_prefix2, None);
    }

    #[test]
    fn test_get_wine_prefix_without_env() {
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _guard_wine = EnvGuard::new("WINEPREFIX");
        let _guard_mock = EnvGuard::new("WINEPREFIX_MOCK_NO_HOME");

        env::remove_var("WINEPREFIX_MOCK_NO_HOME");
        env::remove_var("WINEPREFIX");
        let _prefix = get_wine_prefix();
        #[cfg(unix)]
        assert_eq!(_prefix, dirs::home_dir().map(|h| h.join(".wine")));
        #[cfg(not(unix))]
        assert_eq!(_prefix, None);
    }

    #[test]
    fn test_get_wine_prefix_no_home() {
        let _lock = ENV_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _guard_wine = EnvGuard::new("WINEPREFIX");
        let _guard_mock = EnvGuard::new("WINEPREFIX_MOCK_NO_HOME");

        env::remove_var("WINEPREFIX");
        env::set_var("WINEPREFIX_MOCK_NO_HOME", "1");

        let prefix = get_wine_prefix();
        assert_eq!(prefix, None);
    }
}
