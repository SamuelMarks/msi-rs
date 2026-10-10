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
    if let Some(home) = dirs::home_dir() {
        return Some(home.join(".wine"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_wine_prefix() {
        // Just test that it doesn't panic. The actual value depends on the environment.
        let _prefix = get_wine_prefix();
    }
}
