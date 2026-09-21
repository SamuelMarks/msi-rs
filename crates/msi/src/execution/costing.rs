//! Disk Costing Engine for Windows Installer execution (`CostInitialize`, `FileCost`, `CostFinalize`).
//!
//! Grounded directly in official MSI SDK disk costing specifications:
//! - Tracks volume space usage taking cluster sizes into account (rounding to cluster boundaries).
//! - Calculates net cost differences for new files and file overwrites.
//! - Validates available disk space before deferred installation begins.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Default filesystem cluster size in bytes (4096 bytes).
pub const DEFAULT_CLUSTER_SIZE: u64 = 4096;

/// Queries the live operating system for available disk space and allocation cluster size.
///
/// On Unix systems, this calls `libc::statvfs` on the target path.
/// If the path does not exist, it checks parent directories until a mount point or root is found.
/// If querying fails or on non-Unix platforms, it falls back to `(u64::MAX, DEFAULT_CLUSTER_SIZE)`.
///
/// # Arguments
///
/// * `path` - Mount point or directory path to query.
///
/// # Returns
///
/// A tuple containing `(available_bytes, cluster_size)`.
#[must_use]
pub fn query_host_volume_metrics(path: &str) -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::path::Path;

        let mut current = Path::new(path);
        loop {
            if let Ok(c_path) = CString::new(current.as_os_str().as_encoded_bytes()) {
                // SAFETY: stat structure is initialized to zero and safely passed to statvfs.
                let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
                // SAFETY: c_path is a valid null-terminated C string and &raw mut stat points to valid memory.
                let ret = unsafe { libc::statvfs(c_path.as_ptr(), &raw mut stat) };
                if ret == 0 {
                    #[allow(clippy::unnecessary_cast, trivial_numeric_casts)]
                    let frsize = stat.f_frsize as u64;
                    #[allow(clippy::cast_lossless, clippy::unnecessary_cast, trivial_numeric_casts)]
                    let bavail = stat.f_bavail as u64;
                    let available = bavail.saturating_mul(frsize);
                    return (available, frsize);
                }
            }

            if let Some(parent) = current.parent() {
                if parent.as_os_str().is_empty() {
                    break;
                }
                current = parent;
            } else {
                break;
            }
        }
    }

    #[cfg(not(unix))]
    let _ = path;

    (u64::MAX, DEFAULT_CLUSTER_SIZE)
}

/// Disk costing information for a single storage volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeCost {
    /// Volume identifier or mount point (e.g. `C:` or `/`).
    volume: String,
    /// Filesystem allocation unit / cluster size in bytes.
    cluster_size: u64,
    /// Initially available free space on the volume in bytes.
    available_bytes: u64,
    /// Net required disk space change in bytes (can be positive or negative for removals).
    cost_bytes: i64,
    /// Total number of files scheduled for this volume.
    file_count: u32,
}

impl VolumeCost {
    /// Creates a new [`VolumeCost`] record.
    ///
    /// # Arguments
    ///
    /// * `volume` - Volume path identifier or drive letter.
    /// * `cluster_size` - Filesystem cluster size in bytes (default 4096).
    /// * `available_bytes` - Free space available on the volume in bytes.
    ///
    /// # Returns
    ///
    /// A new [`VolumeCost`] initialized with zero cost.
    #[must_use]
    pub fn new(volume: impl Into<String>, cluster_size: u64, available_bytes: u64) -> Self {
        Self::new_inner(volume.into(), cluster_size, available_bytes)
    }

    /// Internal non-generic constructor for [`VolumeCost`].
    const fn new_inner(volume: String, cluster_size: u64, available_bytes: u64) -> Self {
        let cluster = if cluster_size == 0 {
            DEFAULT_CLUSTER_SIZE
        } else {
            cluster_size
        };
        Self {
            volume,
            cluster_size: cluster,
            available_bytes,
            cost_bytes: 0,
            file_count: 0,
        }
    }

    /// Returns the volume identifier.
    #[must_use]
    pub fn volume(&self) -> &str {
        &self.volume
    }

    /// Returns the cluster allocation size.
    #[must_use]
    pub const fn cluster_size(&self) -> u64 {
        self.cluster_size
    }

    /// Returns the initial available bytes.
    #[must_use]
    pub const fn available_bytes(&self) -> u64 {
        self.available_bytes
    }

    /// Returns the net cost change in bytes.
    #[must_use]
    pub const fn cost_bytes(&self) -> i64 {
        self.cost_bytes
    }

    /// Returns the number of files allocated on this volume.
    #[must_use]
    pub const fn file_count(&self) -> u32 {
        self.file_count
    }

    /// Rounds a byte length up to the nearest cluster boundary.
    ///
    /// Zero-byte files consume zero clusters.
    ///
    /// # Arguments
    ///
    /// * `size` - Raw file size in bytes.
    ///
    /// # Returns
    ///
    /// Size rounded up to cluster boundary.
    #[must_use]
    pub const fn round_to_cluster(&self, size: u64) -> u64 {
        if size == 0 {
            0
        } else {
            size.div_ceil(self.cluster_size) * self.cluster_size
        }
    }

    /// Adds the disk cost of a file to this volume.
    ///
    /// If `existing_size` is specified, the cost difference is computed (replacing existing file).
    ///
    /// # Arguments
    ///
    /// * `new_size` - Size in bytes of the new file to install.
    /// * `existing_size` - Optional size in bytes of existing file being replaced.
    pub fn add_file(&mut self, new_size: u64, existing_size: Option<u64>) {
        let new_clustered = self.round_to_cluster(new_size);
        let existing_clustered = existing_size.map_or(0, |s| self.round_to_cluster(s));

        let diff = if new_clustered >= existing_clustered {
            i64::try_from(new_clustered - existing_clustered).unwrap_or(i64::MAX)
        } else {
            -i64::try_from(existing_clustered - new_clustered).unwrap_or(i64::MAX)
        };

        self.cost_bytes = self.cost_bytes.saturating_add(diff);
        self.file_count = self.file_count.saturating_add(1);
    }

    /// Returns remaining available space after applying net costs.
    ///
    /// # Returns
    ///
    /// Remaining space in bytes, can be negative if required space exceeds available.
    #[must_use]
    pub fn remaining_bytes(&self) -> i64 {
        let avail = i64::try_from(self.available_bytes).unwrap_or(i64::MAX);
        avail.saturating_sub(self.cost_bytes)
    }

    /// Checks whether required cost exceeds available space on this volume.
    ///
    /// # Returns
    ///
    /// `true` if insufficient space, `false` otherwise.
    #[must_use]
    pub fn is_exceeded(&self) -> bool {
        u64::try_from(self.cost_bytes).is_ok_and(|cost| cost > self.available_bytes)
    }
}

/// Disk costing calculation engine coordinating volume costs across installation actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskCostEngine {
    /// Map of volume identifier to [`VolumeCost`].
    volumes: HashMap<String, VolumeCost>,
    /// Default cluster size to use when registering new volumes.
    default_cluster_size: u64,
}

impl Default for DiskCostEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskCostEngine {
    /// Creates a new empty [`DiskCostEngine`].
    ///
    /// # Returns
    ///
    /// A new [`DiskCostEngine`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            volumes: HashMap::new(),
            default_cluster_size: DEFAULT_CLUSTER_SIZE,
        }
    }

    /// Registers a target volume with available free space and optional cluster size.
    ///
    /// # Arguments
    ///
    /// * `volume` - Target volume identifier (e.g. `C:` or `/`).
    /// * `available_bytes` - Available free space in bytes.
    /// * `cluster_size` - Optional filesystem cluster size (defaults to 4096).
    pub fn register_volume(
        &mut self,
        volume: impl Into<String>,
        available_bytes: u64,
        cluster_size: Option<u64>,
    ) {
        let vol_str = volume.into();
        let cluster = cluster_size.unwrap_or(self.default_cluster_size);
        self.volumes.insert(
            vol_str.clone(),
            VolumeCost::new(vol_str, cluster, available_bytes),
        );
    }

    /// Automatically queries the host OS and registers a volume using live metrics.
    ///
    /// # Arguments
    ///
    /// * `volume` - Target volume identifier or path.
    pub fn register_host_volume(&mut self, volume: impl Into<String>) {
        let vol_str = volume.into();
        let (avail, cluster) = query_host_volume_metrics(&vol_str);
        self.register_volume(vol_str, avail, Some(cluster));
    }

    /// Standard Action: `CostInitialize`
    ///
    /// Resets all accumulated file costs while preserving volume definitions.
    pub fn cost_initialize(&mut self) {
        for vol in self.volumes.values_mut() {
            vol.cost_bytes = 0;
            vol.file_count = 0;
        }
    }

    /// Standard Action: `FileCost`
    ///
    /// Costs a single file against the target volume.
    ///
    /// If the volume was not previously registered, it is automatically initialized with
    /// live available space and cluster size from the host filesystem.
    ///
    /// # Arguments
    ///
    /// * `volume` - Volume where file will reside.
    /// * `file_size` - File size in bytes.
    /// * `existing_size` - Optional size of existing file being replaced.
    pub fn file_cost(&mut self, volume: &str, file_size: u64, existing_size: Option<u64>) {
        let (host_avail, host_cluster) = query_host_volume_metrics(volume);
        let entry = self
            .volumes
            .entry(volume.to_string())
            .or_insert_with(|| VolumeCost::new(volume, host_cluster, host_avail));
        entry.add_file(file_size, existing_size);
    }

    /// Standard Action: `CostFinalize`
    ///
    /// Verifies that all volumes have sufficient disk space to satisfy the installation.
    ///
    /// # Returns
    ///
    /// A reference to the volume cost map on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DiskCostExceeded`] if any volume requires more bytes than available.
    pub fn cost_finalize(&self) -> Result<&HashMap<String, VolumeCost>> {
        for (vol_name, cost) in &self.volumes {
            if cost.is_exceeded() {
                return Err(Error::DiskCostExceeded {
                    volume: vol_name.clone(),
                    required_bytes: u64::try_from(cost.cost_bytes).unwrap_or(0),
                    available_bytes: cost.available_bytes,
                });
            }
        }
        Ok(&self.volumes)
    }

    /// Returns a reference to the cost record for a volume if registered.
    ///
    /// # Arguments
    ///
    /// * `volume` - Volume name.
    ///
    /// # Returns
    ///
    /// Optional reference to [`VolumeCost`].
    #[must_use]
    pub fn get_volume_cost(&self, volume: &str) -> Option<&VolumeCost> {
        self.volumes.get(volume)
    }

    /// Returns an iterator over all volume costs.
    ///
    /// # Returns
    ///
    /// Iterator over volume cost references.
    pub fn volumes(&self) -> impl Iterator<Item = &VolumeCost> {
        self.volumes.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests [`VolumeCost`] cluster rounding logic.
    #[test]
    fn test_volume_cost_cluster_rounding() {
        let vc = VolumeCost::new(r"C:\", 4096, 1_000_000);
        assert_eq!(vc.round_to_cluster(0), 0);
        assert_eq!(vc.round_to_cluster(1), 4096);
        assert_eq!(vc.round_to_cluster(4095), 4096);
        assert_eq!(vc.round_to_cluster(4096), 4096);
        assert_eq!(vc.round_to_cluster(4097), 8192);

        // Custom cluster size fallback to default on 0
        let vc0 = VolumeCost::new("D:", 0, 500_000);
        assert_eq!(vc0.cluster_size(), DEFAULT_CLUSTER_SIZE);
        assert_eq!(vc0.volume(), "D:");
        assert_eq!(vc0.available_bytes(), 500_000);
    }

    /// Tests file cost additions, overwrites, and remaining space calculations.
    #[test]
    fn test_volume_cost_add_file_and_remaining() {
        let mut vc = VolumeCost::new(r"C:\", 4096, 20_000);
        assert_eq!(vc.cost_bytes(), 0);
        assert_eq!(vc.file_count(), 0);
        assert_eq!(vc.remaining_bytes(), 20_000);
        assert!(!vc.is_exceeded());

        // New file: 5000 bytes -> 8192 clustered
        vc.add_file(5000, None);
        assert_eq!(vc.cost_bytes(), 8192);
        assert_eq!(vc.file_count(), 1);
        assert_eq!(vc.remaining_bytes(), 20000 - 8192);
        assert!(!vc.is_exceeded());

        // Overwrite file: replacing 8192 bytes with 4096 bytes (smaller file)
        vc.add_file(2000, Some(5000));
        assert_eq!(vc.cost_bytes(), 8192 - 4096); // 4096 net
        assert_eq!(vc.file_count(), 2);

        // Exceed volume space
        vc.add_file(100_000, None);
        assert!(vc.is_exceeded());
        assert!(vc.remaining_bytes() < 0);
    }

    /// Tests [`DiskCostEngine`] sequence: `CostInitialize`, `FileCost`, `CostFinalize`.
    #[test]
    fn test_disk_cost_engine_lifecycle_success() {
        let mut engine = DiskCostEngine::default();
        engine.register_volume(r"C:\", 1_000_000, Some(4096));
        engine.register_volume("D:", 2_000_000, None);

        // Add some dummy initial files
        engine.file_cost(r"C:\", 10_000, None);
        assert_eq!(
            engine.get_volume_cost(r"C:\").map(VolumeCost::file_count),
            Some(1)
        );

        // CostInitialize resets costs
        engine.cost_initialize();
        assert_eq!(
            engine.get_volume_cost(r"C:\").map(VolumeCost::cost_bytes),
            Some(0)
        );
        assert_eq!(
            engine.get_volume_cost(r"C:\").map(VolumeCost::file_count),
            Some(0)
        );

        // FileCost calls
        engine.file_cost(r"C:\", 1000, None); // 4096
        engine.file_cost(r"C:\", 5000, None); // 8192
        engine.file_cost("D:", 10_000, None); // 12288

        // Auto-registered volume on new drive
        engine.file_cost("E:", 2048, None);
        assert!(engine.get_volume_cost("E:").is_some());

        // CostFinalize succeeds
        let res = engine.cost_finalize();
        assert!(res.is_ok());
        assert_eq!(engine.volumes().count(), 3);
    }

    /// Tests [`DiskCostEngine::cost_finalize`] failure when space is exceeded.
    #[test]
    fn test_disk_cost_engine_insufficient_space() {
        let mut engine = DiskCostEngine::new();
        engine.register_volume(r"C:\", 4096, Some(4096)); // Only 4KB free

        // Attempting to install 8KB file
        engine.file_cost(r"C:\", 5000, None);

        let res = engine.cost_finalize();
        assert_eq!(
            res,
            Err(Error::DiskCostExceeded {
                volume: r"C:\".to_string(),
                required_bytes: 8192,
                available_bytes: 4096,
            })
        );
    }

    /// Tests live host volume queries and auto-registration via [`query_host_volume_metrics`].
    #[test]
    fn test_host_volume_queries_and_registration() {
        // Query live current directory
        let (avail, cluster) = query_host_volume_metrics(".");
        assert!(avail > 0);
        assert!(cluster > 0);

        // Query non-existent or synthetic path
        let (fallback_avail, fallback_cluster) =
            query_host_volume_metrics("/non/existent/path/999");
        #[cfg(unix)]
        {
            // On Unix it walks up to root / and finds real space
            assert!(fallback_avail > 0);
            assert!(fallback_cluster > 0);
        }
        #[cfg(not(unix))]
        {
            assert_eq!(fallback_avail, u64::MAX);
            assert_eq!(fallback_cluster, DEFAULT_CLUSTER_SIZE);
        }

        // Test non-existent relative path where parent is empty string (breaks loop)
        let (empty_avail, empty_cluster) =
            query_host_volume_metrics("non_existent_relative_dir_xyz_123");
        assert_eq!(empty_avail, u64::MAX);
        assert_eq!(empty_cluster, DEFAULT_CLUSTER_SIZE);

        // Test empty path string (parent is None)
        let (root_avail, root_cluster) = query_host_volume_metrics("");
        assert_eq!(root_avail, u64::MAX);
        assert_eq!(root_cluster, DEFAULT_CLUSTER_SIZE);

        // Test path containing interior null byte (CString::new fails)
        let (null_avail, null_cluster) = query_host_volume_metrics("bad\0path");
        assert_eq!(null_avail, u64::MAX);
        assert_eq!(null_cluster, DEFAULT_CLUSTER_SIZE);

        // Test register_host_volume
        let mut engine = DiskCostEngine::new();
        engine.register_host_volume(".");
        assert_eq!(engine.volumes().count(), 1);
        for cost in engine.volumes() {
            assert_eq!(cost.volume(), ".");
            assert!(cost.available_bytes() > 0);
            assert!(cost.cluster_size() > 0);
        }
    }
}
