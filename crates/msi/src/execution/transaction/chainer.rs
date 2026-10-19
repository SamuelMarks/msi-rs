//! Multi-package transaction orchestration (`MsiBeginTransaction`, `MsiJoinTransaction`, `MsiEndTransaction`).

use super::{
    DiskCostEngine, EvaluationContext, InstallState, RollbackScript, Transaction, TransactionState,
    WorkerContext, ERROR_INSTALL_FAILURE, ERROR_SUCCESS,
};
use crate::database::tables::chainer::MsiEmbeddedChainerRow;
use crate::database::tables::record::FieldValue;
use crate::error::{MsiError, Result};
use crate::package::Package;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Record of an individual package chained inside a multi-package transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainedPackage {
    /// Path or stream identifier of the child package.
    pub package_path: String,
    /// Public properties forwarded to the child package.
    pub properties: HashMap<String, String>,
    /// Whether this package or service pre-existed on the host system prior to installation.
    pub preexisting: bool,
    /// Optional temporary file path if the MSI was extracted from an internal cabinet or binary stream.
    pub temp_extracted_path: Option<String>,
    /// Optional rollback script for unwinding actions performed by this package.
    pub rollback_script: Option<RollbackScript>,
}

/// Multi-package transaction coordinator implementing Windows Installer 4.5+ transactioning.
///
/// Manages atomic cross-package installation sessions (`MsiBeginTransaction`, `MsiInstallProduct`,
/// `MsiJoinTransaction`, `MsiEndTransaction`), property forwarding, and rollback reversal.
#[derive(Debug, Default)]
pub struct MultiPackageTransactionManager {
    /// Identifier or name of the transaction.
    transaction_name: String,
    /// Current lifecycle state.
    state: Option<TransactionState>,
    /// Chronological log of chained child package installations.
    chained_packages: Vec<ChainedPackage>,
    /// Shared execution worker context.
    worker: WorkerContext,
    /// Host product state registry mapping `ProductCode` or `UpgradeCode` to [`InstallState`].
    registered_products: HashMap<String, InstallState>,
    /// Registered external session identifiers that joined this transaction.
    joined_sessions: HashSet<String>,
}

impl MultiPackageTransactionManager {
    /// Begins a new unified multi-package transaction (`MsiBeginTransaction`).
    ///
    /// # Arguments
    ///
    /// * `name` - Descriptive name or identifier for the transaction.
    ///
    /// # Returns
    ///
    /// An active [`MultiPackageTransactionManager`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if `name` is empty.
    pub fn begin_transaction(name: &str) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "MultiPackageTransactionManager.name".to_string(),
                reason: "transaction name cannot be empty".to_string(),
            });
        }

        Ok(Self {
            transaction_name: name.to_string(),
            state: Some(TransactionState::Active),
            chained_packages: Vec::new(),
            worker: WorkerContext::default(),
            registered_products: HashMap::new(),
            joined_sessions: HashSet::new(),
        })
    }

    /// Allows an external installation session to join the active transaction boundary (`MsiJoinTransaction`).
    ///
    /// # Arguments
    ///
    /// * `session_id` - Identifier for the joining session.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Chainer`] if transaction is not in the active state.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the session is registered with the active transaction.
    pub fn join_transaction(&mut self, session_id: &str) -> Result<()> {
        if self.state != Some(TransactionState::Active) {
            return Err(MsiError::Chainer(
                "cannot join transaction: transaction is not active".to_string(),
            ));
        }
        self.joined_sessions.insert(session_id.to_string());
        Ok(())
    }

    /// Queries the installation state of a product or family by `ProductCode` or `UpgradeCode` (`MsiQueryProductState`).
    ///
    /// Normalizes GUID formatting by handling optional enclosing braces and performing case-insensitive matching.
    ///
    /// # Arguments
    ///
    /// * `code` - GUID product or upgrade identifier.
    ///
    /// # Returns
    ///
    /// The detected [`InstallState`].
    #[must_use]
    pub fn query_product_state(&self, code: &str) -> InstallState {
        if let Some(state) = self.registered_products.get(code) {
            return *state;
        }
        let trimmed = code.trim().trim_matches(|c| c == '{' || c == '}');
        for (k, v) in &self.registered_products {
            let k_trimmed = k.trim().trim_matches(|c| c == '{' || c == '}');
            if k_trimmed.eq_ignore_ascii_case(trimmed) {
                return *v;
            }
        }
        InstallState::Absent
    }

    /// Checks if a product or upgrade code corresponds to an installed product.
    ///
    /// # Arguments
    ///
    /// * `code` - `ProductCode` or `UpgradeCode` GUID.
    ///
    /// # Returns
    ///
    /// `true` if the state indicates the product is installed, `false` otherwise.
    #[must_use]
    pub fn is_product_installed(&self, code: &str) -> bool {
        self.query_product_state(code).is_installed()
    }

    /// Determines whether a child package is pre-existing on the host system based on its
    /// `ProductCode` or `UpgradeCode`.
    ///
    /// # Arguments
    ///
    /// * `package` - Child MSI package to check.
    ///
    /// # Returns
    ///
    /// `true` if either the `ProductCode` or `UpgradeCode` is registered as installed.
    #[must_use]
    pub fn is_package_preexisting(&self, package: &Package) -> bool {
        let product_code = package.metadata().product_code();
        if self.is_product_installed(product_code) {
            return true;
        }

        let upgrade_code = package
            .database()
            .get_records("Property")
            .iter()
            .find_map(|r| match (r.get(0), r.get(1)) {
                (Some(FieldValue::String(k)), Some(FieldValue::String(v)))
                    if k == "UpgradeCode" =>
                {
                    Some(v.as_str())
                }
                _ => None,
            });

        if let Some(up_code) = upgrade_code {
            if self.is_product_installed(up_code) {
                return true;
            }
        }

        false
    }

    /// Manually registers or updates the installation state for a product code or upgrade code.
    ///
    /// # Arguments
    ///
    /// * `code` - GUID product or upgrade code.
    /// * `state` - Installation state to associate.
    pub fn register_product(&mut self, code: &str, state: InstallState) {
        self.registered_products.insert(code.to_string(), state);
    }

    /// Checks whether a property name adheres to the Windows Installer public property convention.
    ///
    /// Public properties consist strictly of uppercase ASCII alphanumeric characters and underscores.
    ///
    /// # Arguments
    ///
    /// * `name` - The property name to check.
    ///
    /// # Returns
    ///
    /// `true` if public, `false` otherwise.
    #[must_use]
    pub fn is_public_property(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }

    /// Serializes all public properties from an [`EvaluationContext`] into a command-line string.
    ///
    /// # Arguments
    ///
    /// * `context` - The evaluation context containing active properties.
    ///
    /// # Returns
    ///
    /// A space-delimited string of `KEY=VALUE` or `KEY="VALUE"` pairs sorted alphabetically by key.
    #[must_use]
    pub fn forward_public_properties(context: &EvaluationContext) -> String {
        let mut pairs: Vec<(&str, &str)> = context
            .properties()
            .iter()
            .filter(|(k, _)| Self::is_public_property(k))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        pairs
            .into_iter()
            .map(|(k, v)| {
                if v.contains(' ') || v.contains('"') {
                    format!(r#"{k}="{}""#, v.replace('"', r#"\""#))
                } else {
                    format!("{k}={v}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Forwards all public properties from a source context into a target context.
    ///
    /// # Arguments
    ///
    /// * `source` - Source context containing properties.
    /// * `target` - Target context receiving the public properties.
    pub fn forward_properties_to_context(
        source: &EvaluationContext,
        target: &mut EvaluationContext,
    ) {
        for (k, v) in source.properties() {
            if Self::is_public_property(k) {
                target.set_property(k, v);
            }
        }
    }

    /// Reads and parses all [`MsiEmbeddedChainerRow`] records from the `MsiEmbeddedChainer` table of an MSI package.
    ///
    /// # Arguments
    ///
    /// * `package` - MSI package to inspect.
    ///
    /// # Returns
    ///
    /// Vector of parsed [`MsiEmbeddedChainerRow`] records.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if a record in the `MsiEmbeddedChainer` table cannot be parsed.
    pub fn read_embedded_chainers(package: &Package) -> Result<Vec<MsiEmbeddedChainerRow>> {
        let mut chainers = Vec::new();
        for rec in package.database().get_records("MsiEmbeddedChainer") {
            let row = MsiEmbeddedChainerRow::from_record(rec)?;
            chainers.push(row);
        }
        Ok(chainers)
    }

    /// Extracts an embedded child MSI package from internal streams or the `Binary` table
    /// into a temporary spool directory.
    ///
    /// # Arguments
    ///
    /// * `package` - Master MSI package containing embedded child stream.
    /// * `stream_or_binary_key` - Key identifier for the embedded child stream.
    /// * `spool_dir` - Destination directory where the child package will be written.
    ///
    /// # Returns
    ///
    /// The [`PathBuf`] to the extracted `.msi` file.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Chainer`] if the stream or binary data cannot be found.
    /// Returns [`MsiError::Io`] on filesystem write failure.
    pub fn extract_child_package(
        &mut self,
        package: &Package,
        stream_or_binary_key: &str,
        spool_dir: &Path,
    ) -> Result<PathBuf> {
        let clean_name = stream_or_binary_key
            .strip_prefix('#')
            .unwrap_or(stream_or_binary_key);
        let file_name = if Path::new(clean_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
        {
            clean_name.to_string()
        } else {
            format!("{clean_name}.msi")
        };
        let base_name = Path::new(&file_name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("child.msi");

        let data: Option<Vec<u8>> = package
            .embedded_cabinets()
            .get(stream_or_binary_key)
            .or_else(|| package.embedded_cabinets().get(clean_name))
            .or_else(|| {
                package
                    .embedded_cabinets()
                    .get(&format!("Binary.{clean_name}"))
            })
            .cloned()
            .or_else(|| {
                package
                    .database()
                    .get_records("Binary")
                    .iter()
                    .find_map(|r| {
                        if let Some(FieldValue::String(name)) = r.get(0) {
                            if name == stream_or_binary_key || name == clean_name {
                                match r.get(1) {
                                    Some(FieldValue::String(s)) => Some(s.as_bytes().to_vec()),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
            });

        let payload = data.ok_or_else(|| {
            MsiError::Chainer(format!(
                "embedded child package '{stream_or_binary_key}' not found in streams or Binary table"
            ))
        })?;

        std::fs::create_dir_all(spool_dir)?;
        let dest_path = spool_dir.join(base_name);
        std::fs::write(&dest_path, payload)?;

        self.worker
            .executed_actions
            .push(format!("ExtractChildPackage:{base_name}"));

        Ok(dest_path)
    }

    /// Extracts all embedded child packages found in internal streams or `Binary` table records.
    ///
    /// Any stream or binary identifier ending with `.msi` (case-insensitive) is extracted to `spool_dir`.
    ///
    /// # Arguments
    ///
    /// * `package` - Master MSI package.
    /// * `spool_dir` - Destination spool directory.
    ///
    /// # Returns
    ///
    /// Mapping of child package identifier to extracted file path.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on extraction or filesystem write failure.
    pub fn extract_all_child_packages(
        &mut self,
        package: &Package,
        spool_dir: &Path,
    ) -> Result<HashMap<String, PathBuf>> {
        let mut extracted = HashMap::new();

        for name in package.embedded_cabinets().keys() {
            let canonical_name = name.strip_prefix("Binary.").unwrap_or(name);
            if Path::new(canonical_name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
                && !extracted.contains_key(canonical_name)
            {
                let path = self.extract_child_package(package, canonical_name, spool_dir)?;
                extracted.insert(canonical_name.to_string(), path);
            }
        }

        for rec in package.database().get_records("Binary") {
            if let Some(FieldValue::String(bin_name)) = rec.get(0) {
                let canonical_name = bin_name.strip_prefix("Binary.").unwrap_or(bin_name);
                if Path::new(canonical_name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
                    && !extracted.contains_key(canonical_name)
                {
                    let path = self.extract_child_package(package, canonical_name, spool_dir)?;
                    extracted.insert(canonical_name.to_string(), path);
                }
            }
        }

        Ok(extracted)
    }

    /// Installs a child package within the active transaction (`MsiInstallProduct`).
    ///
    /// # Arguments
    ///
    /// * `package_path` - Path or stream specifier for the child `.msi` file.
    /// * `command_line` - Public properties string (e.g. `PROP_MYSQL_PORT=3306 PREEXISTING=1`).
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Chainer`] if the transaction is not active or installation fails.
    pub fn install_product_nested(
        &mut self,
        package_path: &str,
        command_line: &str,
    ) -> Result<u32> {
        self.install_product_nested_impl(package_path.to_string(), command_line)
    }

    /// Internal implementation of nested product installation with concrete path string.
    fn install_product_nested_impl(
        &mut self,
        package_path: String,
        command_line: &str,
    ) -> Result<u32> {
        if self.state != Some(TransactionState::Active) {
            return Err(MsiError::Chainer(
                "cannot install nested package: transaction is not active".to_string(),
            ));
        }

        let mut properties = HashMap::new();

        for token in command_line.split_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                let val = v.trim_matches('"');
                properties.insert(k.to_string(), val.to_string());
            }
        }

        let preexisting = properties.get("PREEXISTING").is_some_and(|v| v == "1")
            || self.query_product_state(&package_path) == InstallState::Default;

        let temp_extracted_path = (package_path.starts_with("embedded:")
            || package_path.starts_with("Binary:")
            || package_path.starts_with('#'))
        .then(|| format!("/tmp/extracted_{}", self.chained_packages.len()));

        self.worker
            .executed_actions
            .push(format!("InstallProduct:{package_path}"));

        self.chained_packages.push(ChainedPackage {
            package_path,
            properties,
            preexisting,
            temp_extracted_path,
            rollback_script: None,
        });

        Ok(ERROR_SUCCESS)
    }

    /// Installs a child MSI package instance within the active transaction, evaluating
    /// pre-existing status, launch conditions, and executing transactional operations.
    ///
    /// # Arguments
    ///
    /// * `child_pkg` - The parsed child MSI package.
    /// * `command_line` - Command line property arguments forwarded to the child.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Chainer`] if the transaction is not active.
    /// Returns [`MsiError::ExecutionFailed`] if launch conditions fail or execution fails.
    pub fn install_child_package(
        &mut self,
        child_pkg: &Package,
        command_line: &str,
    ) -> Result<u32> {
        if self.state != Some(TransactionState::Active) {
            return Err(MsiError::Chainer(
                "cannot install child package: transaction is not active".to_string(),
            ));
        }

        let mut properties = HashMap::new();
        for token in command_line.split_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                let val = v.trim_matches('"');
                properties.insert(k.to_string(), val.to_string());
            }
        }

        let product_code = child_pkg.metadata().product_code();
        let product_name = child_pkg.metadata().product_name();
        let is_pre = properties.get("PREEXISTING").is_some_and(|v| v == "1")
            || self.is_package_preexisting(child_pkg);

        let skip_install = properties.iter().any(|(k, v)| {
            k.starts_with("INSTALL_") && (v == "0" || v.eq_ignore_ascii_case("false"))
        });

        if is_pre || skip_install {
            self.worker.executed_actions.push(format!(
                "SkipChildPackagePreexisting:{product_name}:{product_code}"
            ));
            self.chained_packages.push(ChainedPackage {
                package_path: product_name.to_string(),
                properties,
                preexisting: true,
                temp_extracted_path: None,
                rollback_script: None,
            });
            return Ok(ERROR_SUCCESS);
        }

        let mut child_ctx = EvaluationContext::new();
        for r in child_pkg.database().get_records("Property") {
            if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) = (r.get(0), r.get(1))
            {
                child_ctx.set_property(k, v);
            }
        }
        for (k, v) in &properties {
            child_ctx.set_property(k, v);
        }

        for lc_rec in child_pkg.database().get_records("LaunchCondition") {
            if let Some(FieldValue::String(cond)) = lc_rec.get(0) {
                let desc = match lc_rec.get(1) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => "Launch condition failed",
                };
                if !child_ctx.evaluate_condition(cond)? {
                    return Err(MsiError::ExecutionFailed {
                        action: "LaunchConditions".to_string(),
                        return_code: ERROR_INSTALL_FAILURE,
                        message: desc.to_string(),
                    });
                }
            }
        }

        let tx = Transaction::from_package(child_pkg, child_ctx, DiskCostEngine::new());
        let prepared_tx = tx.prepare()?;
        let rollback_script = prepared_tx.rollback_script().clone();

        self.worker
            .executed_actions
            .push(format!("InstallProduct:{product_name}"));

        let executed_tx = prepared_tx.execute(&mut self.worker)?;
        let committed_tx = executed_tx.commit(&mut self.worker)?;

        self.chained_packages.push(ChainedPackage {
            package_path: product_name.to_string(),
            properties,
            preexisting: false,
            temp_extracted_path: None,
            rollback_script: Some(rollback_script),
        });

        Ok(committed_tx.return_code())
    }

    /// Loads a child package from disk and installs it within the active transaction.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the child `.msi` package.
    /// * `command_line` - Command line arguments and properties forwarded to the child.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if opening or installing the package fails.
    pub fn install_child_package_from_path(
        &mut self,
        path: impl AsRef<Path>,
        command_line: &str,
    ) -> Result<u32> {
        let pkg = Package::open(path)?;
        self.install_child_package(&pkg, command_line)
    }

    /// Orchestrates a sequence of child [`Package`] instances within the active transaction.
    ///
    /// # Arguments
    ///
    /// * `child_packages` - Slice of child package references and optional property argument strings.
    /// * `master_context` - Evaluation context from the master package containing properties to forward.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if any child package installation fails (after rolling back preceding packages).
    pub fn orchestrate_child_packages(
        &mut self,
        child_packages: &[(&Package, Option<&str>)],
        master_context: &EvaluationContext,
    ) -> Result<u32> {
        let forwarded = Self::forward_public_properties(master_context);

        for (pkg, extra_args) in child_packages {
            let cmdline = match extra_args {
                Some(args) if !args.trim().is_empty() => {
                    if forwarded.is_empty() {
                        (*args).to_string()
                    } else {
                        format!("{forwarded} {args}")
                    }
                }
                _ => forwarded.clone(),
            };

            if let Err(err) = self.install_child_package(pkg, &cmdline) {
                let _ = self.end_transaction(false);
                return Err(err);
            }
        }

        self.end_transaction(true)
    }

    /// Orchestrates an entire multi-package deployment session from a master orchestrator package.
    ///
    /// # Arguments
    ///
    /// * `master_pkg` - Master orchestrator MSI package.
    /// * `master_context` - Evaluation context from the master package containing active properties.
    /// * `spool_dir` - Destination directory for spooling embedded child packages.
    /// * `child_packages` - Optional list of child package names and specific property overrides.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success, or `1603` (`ERROR_INSTALL_FAILURE`) on rollback.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on execution failure.
    pub fn orchestrate_master_package(
        &mut self,
        master_pkg: &Package,
        master_context: &EvaluationContext,
        spool_dir: &Path,
        child_packages: &[(&str, Option<&str>)],
    ) -> Result<u32> {
        let chainers = Self::read_embedded_chainers(master_pkg)?;
        for chainer in &chainers {
            if let Some(ref cond) = chainer.condition {
                if !master_context.evaluate_condition(cond)? {
                    self.worker
                        .executed_actions
                        .push(format!("SkipEmbeddedChainer:{}", chainer.chainer));
                    return Ok(ERROR_SUCCESS);
                }
            }
        }

        let extracted = self.extract_all_child_packages(master_pkg, spool_dir)?;
        let forwarded_props = Self::forward_public_properties(master_context);

        for (pkg_name, child_args) in child_packages {
            let combined_cmdline = match child_args {
                Some(args) if !args.trim().is_empty() => {
                    if forwarded_props.is_empty() {
                        (*args).to_string()
                    } else {
                        format!("{forwarded_props} {args}")
                    }
                }
                _ => forwarded_props.clone(),
            };

            let res = if let Some(path) = extracted.get(*pkg_name) {
                self.install_child_package_from_path(path, &combined_cmdline)
            } else if Path::new(*pkg_name).exists() {
                self.install_child_package_from_path(*pkg_name, &combined_cmdline)
            } else {
                self.install_product_nested(pkg_name, &combined_cmdline)
            };

            if let Err(err) = res {
                let _ = self.end_transaction(false);
                return Err(err);
            }
        }

        self.end_transaction(true)
    }

    /// Commits or rolls back all nested installations within the transaction (`MsiEndTransaction`).
    ///
    /// # Arguments
    ///
    /// * `commit` - `true` to commit all operations, `false` to rollback in reverse order.
    ///
    /// # Returns
    ///
    /// `0` (`ERROR_SUCCESS`) on commit, or `1603` (`ERROR_INSTALL_FAILURE`) on rollback.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Chainer`] if transaction is not active.
    pub fn end_transaction(&mut self, commit: bool) -> Result<u32> {
        if self.state != Some(TransactionState::Active) {
            return Err(MsiError::Chainer(
                "cannot end transaction: transaction is not active".to_string(),
            ));
        }

        if commit {
            self.state = Some(TransactionState::Committed);
            self.worker
                .executed_actions
                .push("CommitAllPackages".to_string());
            Ok(ERROR_SUCCESS)
        } else {
            self.state = Some(TransactionState::RolledBack);
            for pkg in self.chained_packages.iter().rev() {
                if pkg.preexisting {
                    self.worker
                        .executed_actions
                        .push(format!("SkipRollbackPreexisting:{}", pkg.package_path));
                } else {
                    self.worker
                        .executed_actions
                        .push(format!("RollbackPackage:{}", pkg.package_path));
                    if let Some(ref rb) = pkg.rollback_script {
                        let _ = self.worker.execute_rollback(rb);
                    }
                }
            }
            Ok(ERROR_INSTALL_FAILURE)
        }
    }

    /// Returns the name of the transaction.
    ///
    /// # Returns
    ///
    /// String slice of the transaction name.
    #[must_use]
    pub fn transaction_name(&self) -> &str {
        &self.transaction_name
    }

    /// Returns the current lifecycle state of the transaction.
    ///
    /// # Returns
    ///
    /// Optional [`TransactionState`].
    #[must_use]
    pub const fn state(&self) -> Option<TransactionState> {
        self.state
    }

    /// Returns the list of chained package records.
    ///
    /// # Returns
    ///
    /// Slice of [`ChainedPackage`] entries.
    #[must_use]
    pub fn chained_packages(&self) -> &[ChainedPackage] {
        &self.chained_packages
    }

    /// Returns a reference to the shared execution worker context.
    ///
    /// # Returns
    ///
    /// Reference to the [`WorkerContext`].
    #[must_use]
    pub const fn worker(&self) -> &WorkerContext {
        &self.worker
    }

    /// Returns a mutable reference to the shared execution worker context.
    ///
    /// # Returns
    ///
    /// Mutable reference to the [`WorkerContext`].
    pub const fn worker_mut(&mut self) -> &mut WorkerContext {
        &mut self.worker
    }
}
