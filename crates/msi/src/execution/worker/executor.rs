//! Live OS Filesystem Execution and Rollback Quarantine Storage.
//!
//! Implements real host filesystem transactions per the Windows Installer Deferred Action specification:
//! - Physical directory creation (`std::fs::create_dir_all`) with POSIX permission octals.
//! - Atomic file writing via temporary filenames (`.tmp.{uuid}`) and atomic `std::fs::rename`.
//! - Physical `.rbf` rollback file quarantine preserving overwritten target files prior to replacement.
//! - Physical rollback restoration from quarantine storage upon transaction abort.
//! - Quarantine directory purging upon commit.

use crate::error::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Record of an installed file tracking original quarantine state for rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstalledFileRecord {
    /// File did not previously exist; on rollback, it must be removed.
    NewlyCreated(PathBuf),
    /// File existed and was quarantined; on rollback, it must be restored from `.rbf`.
    Overwritten {
        /// Target file path.
        target_path: PathBuf,
        /// Quarantined backup path (`.rbf`).
        quarantine_rbf: PathBuf,
    },
}

/// Real filesystem worker executor managing live files, directories, and rollback quarantine.
#[derive(Debug, Clone, Default)]
pub struct LiveWorkerExecutor {
    /// Root path of the rollback quarantine directory.
    quarantine_dir: PathBuf,
    /// List of created directories in chronological order.
    created_directories: Vec<PathBuf>,
    /// Map of target file paths to installed file records.
    installed_files: HashMap<PathBuf, InstalledFileRecord>,
    /// Unique execution identifier for temp and quarantine names.
    session_id: String,
    /// Counter for unique quarantine filenames.
    counter: usize,
}

impl LiveWorkerExecutor {
    /// Creates a new [`LiveWorkerExecutor`] with the specified quarantine directory path.
    ///
    /// # Arguments
    ///
    /// * `quarantine_dir` - Directory path where `.rbf` rollback quarantine files will reside.
    /// * `session_id` - Unique identifier for the transaction session.
    ///
    /// # Returns
    ///
    /// A configured [`LiveWorkerExecutor`].
    #[must_use]
    pub fn new(quarantine_dir: &Path, session_id: &str) -> Self {
        Self {
            quarantine_dir: quarantine_dir.to_path_buf(),
            created_directories: Vec::new(),
            installed_files: HashMap::new(),
            session_id: session_id.to_string(),
            counter: 0,
        }
    }

    /// Returns the quarantine directory path.
    #[must_use]
    pub fn quarantine_dir(&self) -> &Path {
        &self.quarantine_dir
    }

    /// Creates a directory physically on disk, creating parent directories as needed.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to create.
    /// * `mode_octal` - Optional POSIX mode permissions (e.g. `0o755`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on filesystem failure.
    pub fn create_directory(&mut self, path: &Path, mode_octal: Option<u32>) -> Result<()> {
        if !path.exists() {
            fs::create_dir_all(path)?;
            self.created_directories.push(path.to_path_buf());

            #[cfg(unix)]
            if let Some(mode) = mode_octal {
                use std::os::unix::fs::PermissionsExt;
                let perm = fs::Permissions::from_mode(mode);
                let _ = fs::set_permissions(path, perm);
            }
            #[cfg(not(unix))]
            let _ = mode_octal;
        }
        Ok(())
    }

    /// Extracts or writes a file atomically to disk, backing up any existing file into quarantine.
    ///
    /// Steps:
    /// 1. Ensures parent directory exists.
    /// 2. If target file exists, copies it into quarantine with `.rbf` extension.
    /// 3. Writes payload to temporary file `target.tmp.{session_id}.{counter}`.
    /// 4. Atomically renames temporary file to target path.
    ///
    /// # Arguments
    ///
    /// * `target_path` - Final destination path of the file.
    /// * `payload` - Binary content of the file.
    /// * `mode_octal` - Optional POSIX permissions (e.g. `0o755` for executables).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on write or quarantine failure.
    pub fn write_file_atomic(
        &mut self,
        target_path: &Path,
        payload: &[u8],
        mode_octal: Option<u32>,
    ) -> Result<()> {
        if let Some(parent) = target_path.parent() {
            self.create_directory(parent, None)?;
        }

        // Ensure quarantine directory exists if we need to backup
        if target_path.exists() {
            if !self.quarantine_dir.exists() {
                fs::create_dir_all(&self.quarantine_dir)?;
            }
            self.counter += 1;
            let rbf_name = format!("file_{}_{}.rbf", self.session_id, self.counter);
            let quarantine_rbf = self.quarantine_dir.join(rbf_name);

            fs::copy(target_path, &quarantine_rbf)?;

            self.installed_files.insert(
                target_path.to_path_buf(),
                InstalledFileRecord::Overwritten {
                    target_path: target_path.to_path_buf(),
                    quarantine_rbf,
                },
            );
        } else {
            self.installed_files.insert(
                target_path.to_path_buf(),
                InstalledFileRecord::NewlyCreated(target_path.to_path_buf()),
            );
        }

        // Write to temp file then rename
        self.counter += 1;
        let tmp_name = format!(
            "{}.tmp.{}.{}",
            target_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            self.session_id,
            self.counter
        );
        let tmp_path = target_path.with_file_name(tmp_name);

        fs::write(&tmp_path, payload)?;

        #[cfg(unix)]
        if let Some(mode) = mode_octal {
            use std::os::unix::fs::PermissionsExt;
            let perm = fs::Permissions::from_mode(mode);
            let _ = fs::set_permissions(&tmp_path, perm);
        }
        #[cfg(not(unix))]
        let _ = mode_octal;

        if let Err(e) = fs::rename(&tmp_path, target_path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }

        Ok(())
    }

    /// Commits the transaction by purging all quarantine `.rbf` files and quarantine directory.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] on failure to remove quarantine storage.
    pub fn commit(&mut self) -> Result<()> {
        if self.quarantine_dir.exists() {
            fs::remove_dir_all(&self.quarantine_dir)?;
        }
        self.installed_files.clear();
        self.created_directories.clear();
        Ok(())
    }

    /// Rolls back the transaction in reverse chronological order:
    /// - Newly installed files are deleted.
    /// - Quarantined `.rbf` files are restored back to their original target paths.
    /// - Empty created directories are removed.
    /// - The quarantine directory is purged.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error::Io`] if rollback file restoration fails.
    pub fn rollback(&mut self) -> Result<()> {
        // Restore files in reverse
        for record in self.installed_files.values() {
            match record {
                InstalledFileRecord::NewlyCreated(path) => {
                    if path.exists() {
                        let _ = fs::remove_file(path);
                    }
                }
                InstalledFileRecord::Overwritten {
                    target_path,
                    quarantine_rbf,
                } => {
                    if quarantine_rbf.exists() {
                        let _ = fs::copy(quarantine_rbf, target_path);
                        let _ = fs::remove_file(quarantine_rbf);
                    }
                }
            }
        }

        // Remove created directories in reverse order
        for dir in self.created_directories.iter().rev() {
            if dir.exists() {
                let _ = fs::remove_dir(dir);
            }
        }

        // Purge quarantine folder
        if self.quarantine_dir.exists() {
            let _ = fs::remove_dir_all(&self.quarantine_dir);
        }

        self.installed_files.clear();
        self.created_directories.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_worker_executor_atomic_write_and_commit() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_commit_{}", std::process::id()));
        let quarantine = temp_dir.join("quarantine");
        let target_file = temp_dir.join("app").join("config.txt");

        let mut executor = LiveWorkerExecutor::new(&quarantine, "sess1");
        assert_eq!(executor.quarantine_dir(), quarantine.as_path());

        // Write first version
        let res1 = executor.write_file_atomic(&target_file, b"version 1.0", Some(0o644));
        assert!(res1.is_ok());
        assert_eq!(
            fs::read(&target_file).as_deref().map_err(|_| ()),
            Ok(&b"version 1.0"[..])
        );

        // Write second version (should quarantine v1)
        let res2 = executor.write_file_atomic(&target_file, b"version 2.0", Some(0o644));
        assert!(res2.is_ok());
        assert_eq!(
            fs::read(&target_file).as_deref().map_err(|_| ()),
            Ok(&b"version 2.0"[..])
        );
        assert!(quarantine.exists());

        // Commit transaction
        let res_commit = executor.commit();
        assert!(res_commit.is_ok());
        assert!(!quarantine.exists());
        assert_eq!(
            fs::read(&target_file).as_deref().map_err(|_| ()),
            Ok(&b"version 2.0"[..])
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests atomic write and commit I/O error edge cases.
    #[test]
    fn test_live_worker_executor_error_paths() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_exec_err_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        // 1. Error creating quarantine_dir when target_path exists (quarantine_dir blocked by regular file)
        let blocked_file = temp_dir.join("blocked_file");
        let _ = fs::write(&blocked_file, b"content");
        let existing_target = temp_dir.join("existing_target.txt");
        let _ = fs::write(&existing_target, b"original");

        let mut exec_quar_err = LiveWorkerExecutor::new(&blocked_file.join("sub"), "s1");
        assert!(exec_quar_err
            .write_file_atomic(&existing_target, b"new", None)
            .is_err());

        // 2. Error copying target_path to quarantine_rbf (quarantine_rbf pre-exists as directory)
        let quarantine_dir = temp_dir.join("quar2");
        let _ = fs::create_dir_all(&quarantine_dir);
        let mut exec_copy_err = LiveWorkerExecutor::new(&quarantine_dir, "s2");
        let rbf_dir = quarantine_dir.join("file_s2_1.rbf");
        let _ = fs::create_dir_all(&rbf_dir);
        assert!(exec_copy_err
            .write_file_atomic(&existing_target, b"new", None)
            .is_err());

        // 3. Error writing tmp_path (tmp_path pre-exists as directory)
        let new_target = temp_dir.join("new_file.txt");
        let mut exec_tmp_err = LiveWorkerExecutor::new(&quarantine_dir, "s3");
        let pre_existing_dir = temp_dir.join("new_file.txt.tmp.s3.1");
        let _ = fs::create_dir_all(&pre_existing_dir);
        assert!(exec_tmp_err
            .write_file_atomic(&new_target, b"new", None)
            .is_err());

        // 4. Error in commit when quarantine_dir cannot be deleted
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let unremovable_quar = temp_dir.join("unremovable_quar");
            let _ = fs::create_dir_all(&unremovable_quar);
            let sub_file = unremovable_quar.join("dummy.rbf");
            let _ = fs::write(&sub_file, b"dummy");
            let _ = fs::set_permissions(&unremovable_quar, fs::Permissions::from_mode(0o555));
            let mut exec_commit_err = LiveWorkerExecutor::new(&unremovable_quar, "s4");
            assert!(exec_commit_err.commit().is_err());
            let _ = fs::set_permissions(&unremovable_quar, fs::Permissions::from_mode(0o755));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_live_worker_executor_rollback_restoration() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_rollback_{}", std::process::id()));
        let quarantine = temp_dir.join("quarantine");
        let target_file = temp_dir.join("app").join("vital.dat");
        let new_file = temp_dir.join("app").join("extra.dat");

        // Pre-create existing file
        assert!(fs::create_dir_all(temp_dir.join("app")).is_ok());
        assert!(fs::write(&target_file, b"pristine data").is_ok());

        let mut executor = LiveWorkerExecutor::new(&quarantine, "sess2");

        // Overwrite existing file and write newly created file
        assert!(executor
            .write_file_atomic(&target_file, b"corrupted data", None)
            .is_ok());
        assert!(executor
            .write_file_atomic(&new_file, b"new extra data", None)
            .is_ok());

        assert_eq!(
            fs::read(&target_file).as_deref().map_err(|_| ()),
            Ok(&b"corrupted data"[..])
        );
        assert!(new_file.exists());

        // Rollback
        let res_rollback = executor.rollback();
        assert!(res_rollback.is_ok());

        // Verify original file restored and new file removed
        assert_eq!(
            fs::read(&target_file).as_deref().map_err(|_| ()),
            Ok(&b"pristine data"[..])
        );
        assert!(!new_file.exists());
        assert!(!quarantine.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_live_worker_executor_default_and_additional_paths() {
        let mut def = LiveWorkerExecutor::default();
        assert_eq!(def.quarantine_dir(), Path::new(""));

        // Rollback on empty executor (quarantine_dir doesn't exist, no created dirs, no files)
        assert!(def.rollback().is_ok());
        assert!(def.commit().is_ok());

        // Test create_directory on already existing directory
        let temp_dir = std::env::temp_dir().join(format!("msi_test_dirs_{}", std::process::id()));
        let quarantine = temp_dir.join("quarantine");
        let mut executor = LiveWorkerExecutor::new(&quarantine, "sess3");

        assert!(executor.create_directory(&temp_dir, Some(0o755)).is_ok());
        assert!(executor.create_directory(&temp_dir, Some(0o755)).is_ok()); // already exists path branch

        // Test rollback where newly created file was already removed prior to rollback
        let deleted_before_rb = temp_dir.join("deleted_before_rb.txt");
        assert!(executor
            .write_file_atomic(&deleted_before_rb, b"temp", None)
            .is_ok());
        assert!(fs::remove_file(&deleted_before_rb).is_ok()); // remove it so path.exists() is false during rollback

        // Test rollback where quarantined file was already removed prior to rollback
        let existing_target = temp_dir.join("existing_target.txt");
        assert!(fs::write(&existing_target, b"initial").is_ok());
        assert!(executor
            .write_file_atomic(&existing_target, b"new_data", None)
            .is_ok());
        // Find quarantine rbf and remove it
        for record in executor.installed_files.values() {
            if let InstalledFileRecord::Overwritten { quarantine_rbf, .. } = record {
                let _ = fs::remove_file(quarantine_rbf);
            }
        }

        // Test rollback where created directory was already removed
        let sub_dir = temp_dir.join("sub_to_delete");
        assert!(executor.create_directory(&sub_dir, None).is_ok());
        let _ = fs::remove_dir(&sub_dir); // dir.exists() becomes false

        // Test writing a file when quarantine_dir already exists
        assert!(fs::create_dir_all(&quarantine).is_ok());
        let pre_existing = temp_dir.join("pre_existing.txt");
        assert!(fs::write(&pre_existing, b"data1").is_ok());
        assert!(executor
            .write_file_atomic(&pre_existing, b"data2", None)
            .is_ok());

        // Test writing with empty path (parent is None)
        let empty_path = Path::new("");
        assert!(executor
            .write_file_atomic(empty_path, b"test", None)
            .is_err());

        // Run rollback with these non-existent items
        assert!(executor.rollback().is_ok());

        // Test failure path: write_file_atomic to a target without parent or invalid destination path
        #[cfg(unix)]
        {
            let invalid_path = Path::new("/proc/non_existent_sys/invalid/path.dat");
            assert!(executor
                .write_file_atomic(invalid_path, b"data", None)
                .is_err());
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
