//! Source Resiliency & Media Prompts.
//!
//! Handles source list registry parsing, network share fallback resolution,
//! and UI callbacks for media disk prompts.

use crate::error::{MsiError, Result};
use std::path::{Path, PathBuf};

/// Represents a local file system path source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSource {
    /// The base directory path.
    pub base_path: PathBuf,
    /// The media disk ID (optional).
    pub disk_id: Option<u32>,
    /// The volume label (optional).
    pub volume_label: Option<String>,
    /// The disk prompt string (optional).
    pub disk_prompt: Option<String>,
}

/// Represents a network share path source (UNC format).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSource {
    /// The UNC path string.
    pub unc_path: String,
}

/// Represents an HTTP or HTTPS URL source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlSource {
    /// The URL string.
    pub url: String,
}

/// Strong type for representing network paths and shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePath {
    /// Local file system path.
    Media(MediaSource),
    /// Network share path (UNC format like `\\server\share`).
    Network(NetworkSource),
    /// HTTP or HTTPS URL.
    Url(UrlSource),
}

/// Represents the list of available sources for an installation.
#[derive(Debug, Clone, Default)]
pub struct SourceList {
    /// Array of fallback sources in order of preference.
    pub sources: Vec<SourcePath>,
    /// Last used source path (if any).
    pub last_used: Option<SourcePath>,
}

impl SourceList {
    /// Parses a `SourceList` from a semicolon-separated string of paths.
    ///
    /// # Arguments
    ///
    /// * `raw_list` - Semicolon-separated path string.
    ///
    /// # Returns
    ///
    /// Parsed [`SourceList`].
    #[must_use]
    pub fn parse(raw_list: &str) -> Self {
        let mut sources = Vec::new();
        for s in raw_list.split(';') {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                sources.push(SourcePath::Url(UrlSource {
                    url: trimmed.to_string(),
                }));
            } else if trimmed.starts_with(r"\\") {
                sources.push(SourcePath::Network(NetworkSource {
                    unc_path: trimmed.to_string(),
                }));
            } else {
                sources.push(SourcePath::Media(MediaSource {
                    base_path: PathBuf::from(trimmed),
                    disk_id: None,
                    volume_label: None,
                    disk_prompt: None,
                }));
            }
        }
        Self {
            sources,
            last_used: None,
        }
    }

    /// Resolves a relative file path against the available sources.
    ///
    /// # Arguments
    ///
    /// * `relative_path` - The relative file path to resolve.
    /// * `prompt_callback` - Optional callback invoked when media needs to be inserted.
    ///
    /// # Returns
    ///
    /// The resolved absolute [`PathBuf`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::FileNotFound`] if the file cannot be found in any source.
    pub fn resolve_file<F>(
        &self,
        relative_path: &Path,
        mut prompt_callback: Option<F>,
    ) -> Result<PathBuf>
    where
        F: FnMut(&str) -> bool, // returns true if user inserted disk and wants to retry
    {
        // Try last used first
        if let Some(SourcePath::Media(media)) = &self.last_used {
            let candidate = media.base_path.join(relative_path);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Try all fallback sources
        for source in &self.sources {
            match source {
                SourcePath::Media(media) => {
                    let candidate = media.base_path.join(relative_path);

                    // Loop for retry on prompt
                    loop {
                        if candidate.exists() {
                            return Ok(candidate);
                        }

                        if let Some(ref mut cb) = prompt_callback {
                            let prompt_msg = format!(
                                "Please insert disk containing {}",
                                relative_path.display()
                            );
                            if !cb(&prompt_msg) {
                                break; // User canceled or failed to provide media
                            }
                        } else {
                            break; // No prompt callback, just fail for this source
                        }
                    }
                }
                SourcePath::Network(network) => {
                    // For POSIX, we attempt to map UNC to a local mount point if possible.
                    let mapped = Self::map_unc_path(&network.unc_path);
                    let candidate = mapped.join(relative_path);
                    if candidate.exists() {
                        return Ok(candidate);
                    }
                }
                SourcePath::Url(_url) => {
                    // URL resolution would involve downloading, currently unsupported in mock
                    continue;
                }
            }
        }

        Err(MsiError::FileNotFound {
            path: relative_path.to_string_lossy().to_string(),
        })
    }

    /// Maps a UNC path (`\\server\share`) to a local mount path (e.g., `/mnt/server/share` or SMB alias).
    ///
    /// # Arguments
    ///
    /// * `unc` - The unc argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn map_unc_path(unc: &str) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(unc)
        }
        #[cfg(not(windows))]
        {
            // Emulate SMB mapping on POSIX
            let stripped = unc.strip_prefix(r"\\").unwrap_or(unc);
            let normalized = stripped.replace('\\', "/");

            #[cfg(not(test))]
            let base = "/mnt/";
            #[cfg(test)]
            let base = "/tmp/msi_test_mnt/";
            PathBuf::from(format!("{base}{normalized}"))
        }
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn test_network_source_resolution() {
        use std::fs;
        let base_mnt = Path::new("/tmp/msi_test_mnt/");
        fs::create_dir_all(base_mnt.join("myshare")).expect("Expected success");
        let test_file = base_mnt.join("myshare").join("test_file.txt");
        fs::write(&test_file, "data").expect("Expected success");

        let mut list = SourceList {
            sources: Vec::new(),
            last_used: None,
        };

        list.sources.push(SourcePath::Network(NetworkSource {
            unc_path: r"\\myshare".to_string(),
        }));

        let res = list
            .resolve_file(Path::new("test_file.txt"), None::<fn(&str) -> bool>)
            .expect("test");
        assert_eq!(res.file_name().expect("Expected a value"), "test_file.txt");
    }

    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_source_list_parse() {
        let list = SourceList::parse(r"C:\Local;\\Server\Share;https://example.com/msi;");
        assert_eq!(list.sources.len(), 3);
        assert_eq!(
            list.sources[0],
            SourcePath::Media(MediaSource {
                base_path: PathBuf::from(r"C:\Local"),
                disk_id: None,
                volume_label: None,
                disk_prompt: None,
            })
        );
        assert_eq!(
            list.sources[1],
            SourcePath::Network(NetworkSource {
                unc_path: r"\\Server\Share".to_string()
            })
        );
        assert_eq!(
            list.sources[2],
            SourcePath::Url(UrlSource {
                url: "https://example.com/msi".to_string()
            })
        );
    }

    #[test]
    fn test_unc_mapping() {
        let mapped = SourceList::map_unc_path(r"\\Server\Share\Folder");
        #[cfg(windows)]
        assert_eq!(mapped, PathBuf::from(r"\\Server\Share\Folder"));
        #[cfg(not(windows))]
        assert_eq!(
            mapped,
            PathBuf::from("/tmp/msi_test_mnt/Server/Share/Folder")
        );
    }

    #[test]
    fn test_resolve_file_local() {
        let dir = tempdir().expect("test");
        let file_path = dir.path().join("test.cab");
        fs::write(&file_path, "dummy data").expect("test");

        let list = SourceList {
            sources: vec![SourcePath::Media(MediaSource {
                base_path: dir.path().to_path_buf(),
                disk_id: None,
                volume_label: None,
                disk_prompt: None,
            })],
            last_used: None,
        };

        let resolved = list
            .resolve_file(Path::new("test.cab"), None::<fn(&str) -> bool>)
            .expect("test");
        assert_eq!(resolved, file_path);
    }

    #[test]
    fn test_resolve_file_last_used() {
        let dir = tempdir().expect("test");
        let file_path = dir.path().join("test.cab");
        fs::write(&file_path, "dummy data").expect("test");

        let list = SourceList {
            sources: vec![],
            last_used: Some(SourcePath::Media(MediaSource {
                base_path: dir.path().to_path_buf(),
                disk_id: None,
                volume_label: None,
                disk_prompt: None,
            })),
        };

        let resolved = list
            .resolve_file(Path::new("test.cab"), None::<fn(&str) -> bool>)
            .expect("test");
        assert_eq!(resolved, file_path);
    }

    #[test]
    fn test_resolve_file_prompt_retry() {
        let dir = tempdir().expect("test");
        let file_path = dir.path().join("delayed.cab");

        let list = SourceList {
            sources: vec![SourcePath::Media(MediaSource {
                base_path: dir.path().to_path_buf(),
                disk_id: None,
                volume_label: None,
                disk_prompt: None,
            })],
            last_used: None,
        };

        let mut prompt_count = 0;
        let callback = |msg: &str| -> bool {
            assert!(msg.contains("delayed.cab"));
            prompt_count += 1;
            if prompt_count == 2 {
                // Simulate user inserting disk on 2nd prompt
                fs::write(&file_path, "delayed data").expect("test");
            }
            true // keep retrying, or exit loop if found
        };

        let resolved = list.resolve_file(Path::new("delayed.cab"), Some(callback));
        assert!(resolved.is_ok());

        // Do another resolution for abort
        let temp2 = tempdir().expect("test");
        let _file_path2 = temp2.path().join("abort.cab");
        let mut list2 = SourceList::parse(temp2.path().to_str().expect("test"));
        list2.sources.push(SourcePath::Media(MediaSource {
            base_path: temp2.path().to_path_buf(),
            disk_id: Some(1),
            volume_label: None,
            disk_prompt: Some(String::new()),
        }));

        let mut prompt_count2 = 0;
        let callback2 = |msg: &str| -> bool {
            assert!(msg.contains("abort.cab"));
            prompt_count2 += 1;
            prompt_count2 < 2
        };
        let resolved2 = list2.resolve_file(Path::new("abort.cab"), Some(callback2));
        assert!(resolved2.is_err());
    }

    #[test]
    fn test_resolve_file_prompt_cancel() {
        let dir = tempdir().expect("test");

        let list = SourceList {
            sources: vec![SourcePath::Media(MediaSource {
                base_path: dir.path().to_path_buf(),
                disk_id: None,
                volume_label: None,
                disk_prompt: None,
            })],
            last_used: None,
        };

        let callback = |_: &str| -> bool {
            false // user cancels immediately
        };

        let res = list.resolve_file(Path::new("missing.cab"), Some(callback));
        assert!(res
            .expect_err("Expected an error")
            .to_string()
            .contains("File not found"));
    }
}

#[cfg(test)]
mod source_resiliency_additional_tests {
    use super::*;

    #[test]
    fn test_resolve_file_network_and_url() {
        let list = SourceList::parse(r"\\Server\Share;https://example.com");
        let res = list.resolve_file(Path::new("dummy.txt"), None::<fn(&str) -> bool>);
        assert!(res.is_err());
    }

    #[test]
    fn test_resolve_file_local_no_prompt_break() {
        let list = SourceList::parse("C:\\MissingFolder");
        let res = list.resolve_file(Path::new("dummy.txt"), None::<fn(&str) -> bool>);
        assert!(res.is_err());
    }

    #[test]
    fn test_resolve_file_last_used() {
        let temp = tempfile::tempdir().expect("test");
        let file_path = temp.path().join("last_used.txt");
        std::fs::write(&file_path, "data").expect("test");

        let mut list = SourceList::parse(temp.path().to_str().expect("test"));
        list.last_used = Some(SourcePath::Media(MediaSource {
            base_path: temp.path().to_path_buf(),
            disk_id: Some(1),
            volume_label: None,
            disk_prompt: Some(String::new()),
        }));

        let resolved = list
            .resolve_file(Path::new("last_used.txt"), None::<fn(&str) -> bool>)
            .expect("test");
        assert_eq!(resolved, file_path);

        let res_missing =
            list.resolve_file(Path::new("does_not_exist.txt"), None::<fn(&str) -> bool>);
        assert!(res_missing.is_err());

        let callback2_called = std::cell::Cell::new(false);
        let callback2 = |_: &str| -> bool {
            callback2_called.set(true);
            false
        };
        let resolved2 = list
            .resolve_file(Path::new("last_used.txt"), Some(&callback2))
            .expect("test");
        assert_eq!(resolved2, file_path);
        // callback should not be called because last_used works
        assert!(!callback2_called.get());

        // Add a media source to trigger the prompt loop
        list.sources.push(SourcePath::Media(MediaSource {
            base_path: temp.path().to_path_buf(),
            disk_id: Some(1),
            volume_label: None,
            disk_prompt: Some(String::new()),
        }));

        let _ = list.resolve_file(Path::new("trigger_false.txt"), Some(&callback2));
        assert!(callback2_called.get());

        // Add a media source to trigger the prompt loop
        list.sources.push(SourcePath::Media(MediaSource {
            base_path: temp.path().to_path_buf(),
            disk_id: Some(1),
            volume_label: None,
            disk_prompt: Some(String::new()),
        }));

        let mut callback3_called = false;
        let callback3 = |_: &str| -> bool {
            callback3_called = true;
            false
        };
        let _ = list.resolve_file(Path::new("trigger_false.txt"), Some(callback3));
        assert!(callback3_called);
    }

    #[test]
    fn test_resolve_file_prompt_abort() {
        let temp = tempfile::tempdir().expect("test");
        let list = SourceList::parse(temp.path().to_str().expect("test"));

        let mut prompt_count = 0;
        let callback = |_: &str| -> bool {
            prompt_count += 1;
            if prompt_count < 2 {
                true // retry
            } else {
                false // abort on second prompt, hitting line 334
            }
        };

        let res = list.resolve_file(Path::new("missing.txt"), Some(callback));
        assert!(res.is_err());
        assert_eq!(prompt_count, 2);
    }

    #[test]
    fn test_source_list_cache() {
        let cache = source_list_cache();
        let is_empty = cache.read().expect("test").is_empty();
        assert!(is_empty);
    }

    #[test]
    fn test_source_models_derives() {
        let ms = MediaSource {
            base_path: PathBuf::from("C:\\"),
            disk_id: Some(1),
            volume_label: None,
            disk_prompt: Some("disk".to_string()),
        };
        let ms2 = ms.clone();
        assert_eq!(ms, ms2);
        let _ = format!("{ms:?}");

        let ns = NetworkSource {
            unc_path: "\\\\server\\share".to_string(),
        };
        let _ = format!("{ns:?}");

        let us = UrlSource {
            url: "http://example.com".to_string(),
        };
        let _ = format!("{us:?}");

        let sp = SourcePath::Media(ms);
        let _ = format!("{sp:?}");

        let sl = SourceList::default();
        let _ = format!("{sl:?}");
    }
}

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Gets the global source list cache.
///
/// # Returns
///
/// The return value.
pub fn source_list_cache() -> &'static RwLock<HashMap<String, SourceList>> {
    static CACHE: OnceLock<RwLock<HashMap<String, SourceList>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}
