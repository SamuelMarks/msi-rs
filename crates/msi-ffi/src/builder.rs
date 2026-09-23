//! C-ABI interface for creating and configuring Windows Installer packages via `PackageBuilder`.
//!
//! Exposes complete package authoring capabilities to external languages.

use crate::error::{
    c_str_to_opt_str, c_str_to_str, ffi_boundary, map_msi_error, MSI_ERROR_INVALID_ARGUMENT,
    MSI_ERROR_NULL_POINTER, MSI_SUCCESS,
};
use crate::types::{MsiPackageBuilderHandle, MsiPackageHandle};
use msi::cab::folder::CompressionType;
use msi::cab::writer::CabinetWriter;
use msi::database::tables::core::{
    file_attributes, ComponentRow, DirectoryRow, FeatureComponentsRow, FeatureRow, FileHashRow,
    FileRow, MediaRow,
};
use msi::database::tables::types::{
    ComponentGuid, ComponentName, DirectoryId, FeatureName, FileKey,
};
use msi::package::{PackageBuilder, ProductVersion};
use std::ffi::c_char;
use std::fs;
use std::path::Path;

/// Constructs a new [`MsiPackageBuilderHandle`].
///
/// # Arguments
///
/// * `product_name` - Descriptive product name.
/// * `manufacturer` - Manufacturer name.
/// * `ver_major` - Major product version component.
/// * `ver_minor` - Minor product version component.
/// * `ver_build` - Build product version component.
/// * `product_code` - Product code GUID in `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` format.
/// * `out_builder` - Pointer receiving the allocated builder handle.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Errors
///
/// Returns [`MSI_ERROR_NULL_POINTER`] or [`MSI_ERROR_INVALID_ARGUMENT`] if arguments are invalid.
///
/// # Safety
///
/// All string pointers must be valid null-terminated C strings.
/// `out_builder` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_create(
    product_name: *const c_char,
    manufacturer: *const c_char,
    ver_major: u8,
    ver_minor: u8,
    ver_build: u16,
    product_code: *const c_char,
    out_builder: *mut *mut MsiPackageBuilderHandle,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if out_builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "out_builder pointer must not be null".to_string(),
                ));
            }

            let name = c_str_to_str(product_name, "product_name")?;
            let mfr = c_str_to_str(manufacturer, "manufacturer")?;
            let code = c_str_to_str(product_code, "product_code")?;

            let version = ProductVersion::new(ver_major, ver_minor, ver_build);

            let builder = PackageBuilder::default()
                .product_name(name)
                .manufacturer(mfr)
                .version(version)
                .product_code(code);

            *out_builder = Box::into_raw(Box::new(MsiPackageBuilderHandle { inner: builder }));
            Ok(MSI_SUCCESS)
        })
    }
}

/// Sets the product name on the builder.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `name` - Product name.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
/// `name` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_set_product_name(
    builder: *mut MsiPackageBuilderHandle,
    name: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let val = c_str_to_str(name, "name")?;
            (*builder).inner = std::mem::take(&mut (*builder).inner).product_name(val);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Sets the manufacturer name on the builder.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `manufacturer` - Manufacturer name.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
/// `manufacturer` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_set_manufacturer(
    builder: *mut MsiPackageBuilderHandle,
    manufacturer: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let val = c_str_to_str(manufacturer, "manufacturer")?;
            (*builder).inner = std::mem::take(&mut (*builder).inner).manufacturer(val);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Sets the product version on the builder.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `ver_major` - Major version.
/// * `ver_minor` - Minor version.
/// * `ver_build` - Build version.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_set_version(
    builder: *mut MsiPackageBuilderHandle,
    ver_major: u8,
    ver_minor: u8,
    ver_build: u16,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let version = ProductVersion::new(ver_major, ver_minor, ver_build);
            (*builder).inner = std::mem::take(&mut (*builder).inner).version(version);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Sets the product code GUID on the builder.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `product_code` - GUID string in `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` format.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
/// `product_code` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_set_product_code(
    builder: *mut MsiPackageBuilderHandle,
    product_code: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let code = c_str_to_str(product_code, "product_code")?;
            (*builder).inner = std::mem::take(&mut (*builder).inner).product_code(code);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Sets the upgrade code GUID on the builder.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `upgrade_code` - GUID string in `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` format.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
/// `upgrade_code` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_set_upgrade_code(
    builder: *mut MsiPackageBuilderHandle,
    upgrade_code: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let code = c_str_to_str(upgrade_code, "upgrade_code")?;
            (*builder).inner = std::mem::take(&mut (*builder).inner).upgrade_code(code);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Adds a property to the package's `Property` table.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `name` - Property identifier.
/// * `value` - Property value string.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
/// `name` and `value` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_add_property(
    builder: *mut MsiPackageBuilderHandle,
    name: *const c_char,
    value: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let prop_name = c_str_to_str(name, "name")?;
            let prop_val = c_str_to_str(value, "value")?;
            (*builder).inner =
                std::mem::take(&mut (*builder).inner).add_property(prop_name, prop_val);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Adds a directory definition to the package.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `dir_id` - Directory identifier (e.g. "TARGETDIR" or "INSTALLDIR").
/// * `parent_id` - Optional parent directory identifier, or NULL for root.
/// * `default_dir` - Directory path specifier (e.g. "`SourceDir`" or "PFiles|Program Files").
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All non-null string pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_add_directory(
    builder: *mut MsiPackageBuilderHandle,
    dir_id: *const c_char,
    parent_id: *const c_char,
    default_dir: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let id_str = c_str_to_str(dir_id, "dir_id")?;
            let directory =
                DirectoryId::new(id_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let parent_opt = c_str_to_opt_str(parent_id, "parent_id")?;
            let directory_parent = match parent_opt {
                Some(s) => {
                    Some(DirectoryId::new(s).map_err(|e| (map_msi_error(&e), e.to_string()))?)
                }
                None => None,
            };

            let def_dir = c_str_to_str(default_dir, "default_dir")?;

            let row = DirectoryRow {
                directory,
                directory_parent,
                default_dir: def_dir.to_string(),
            };

            (*builder).inner = std::mem::take(&mut (*builder).inner).add_directory(row);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Adds a component definition to the package.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `comp_id` - Component identifier.
/// * `comp_guid` - Optional Component GUID `{...}`, or NULL for unmanaged.
/// * `dir_id` - Foreign key to Directory table.
/// * `attributes` - Component attributes bitmask.
/// * `condition` - Optional installation condition expression, or NULL.
/// * `keypath` - Optional keypath file or registry key, or NULL.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All non-null string pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_add_component(
    builder: *mut MsiPackageBuilderHandle,
    comp_id: *const c_char,
    comp_guid: *const c_char,
    dir_id: *const c_char,
    attributes: i16,
    condition: *const c_char,
    keypath: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let id_str = c_str_to_str(comp_id, "comp_id")?;
            let component =
                ComponentName::new(id_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let guid_opt = c_str_to_opt_str(comp_guid, "comp_guid")?;
            let component_id = match guid_opt {
                Some(g) => {
                    Some(ComponentGuid::parse(g).map_err(|e| (map_msi_error(&e), e.to_string()))?)
                }
                None => None,
            };

            let dir_str = c_str_to_str(dir_id, "dir_id")?;
            let directory =
                DirectoryId::new(dir_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let cond_opt = c_str_to_opt_str(condition, "condition")?.map(ToString::to_string);
            let keypath_opt = c_str_to_opt_str(keypath, "keypath")?.map(ToString::to_string);

            let row = ComponentRow {
                component,
                component_id,
                directory,
                attributes,
                condition: cond_opt,
                key_path: keypath_opt,
            };

            (*builder).inner = std::mem::take(&mut (*builder).inner).add_component(row);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Adds a feature definition to the package.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `feat_id` - Feature identifier.
/// * `parent_id` - Optional parent feature identifier, or NULL.
/// * `title` - Optional UI title, or NULL.
/// * `description` - Optional UI description, or NULL.
/// * `display` - Optional UI display flag (>= 0 for value, < 0 for NULL).
/// * `level` - Initial installation level.
/// * `dir_id` - Optional directory reference, or NULL.
/// * `attributes` - Feature attribute bitmask.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All non-null string pointers must be valid null-terminated C strings.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn msi_package_builder_add_feature(
    builder: *mut MsiPackageBuilderHandle,
    feat_id: *const c_char,
    parent_id: *const c_char,
    title: *const c_char,
    description: *const c_char,
    display: i16,
    level: i16,
    dir_id: *const c_char,
    attributes: i16,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let id_str = c_str_to_str(feat_id, "feat_id")?;
            let feature =
                FeatureName::new(id_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let parent_opt = c_str_to_opt_str(parent_id, "parent_id")?;
            let feature_parent = match parent_opt {
                Some(p) => {
                    Some(FeatureName::new(p).map_err(|e| (map_msi_error(&e), e.to_string()))?)
                }
                None => None,
            };

            let title_opt = c_str_to_opt_str(title, "title")?.map(ToString::to_string);
            let desc_opt = c_str_to_opt_str(description, "description")?.map(ToString::to_string);
            let display_opt = (display >= 0).then_some(display);

            let dir_opt = c_str_to_opt_str(dir_id, "dir_id")?;
            let directory = match dir_opt {
                Some(d) => {
                    Some(DirectoryId::new(d).map_err(|e| (map_msi_error(&e), e.to_string()))?)
                }
                None => None,
            };

            let row = FeatureRow {
                feature,
                feature_parent,
                title: title_opt,
                description: desc_opt,
                display: display_opt,
                level,
                directory,
                attributes,
            };

            (*builder).inner = std::mem::take(&mut (*builder).inner).add_feature(row);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Links a component to a feature in the `FeatureComponents` table.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `feat_id` - Feature identifier.
/// * `comp_id` - Component identifier.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All string pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_add_feature_component(
    builder: *mut MsiPackageBuilderHandle,
    feat_id: *const c_char,
    comp_id: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let feat_str = c_str_to_str(feat_id, "feat_id")?;
            let comp_str = c_str_to_str(comp_id, "comp_id")?;

            let feature =
                FeatureName::new(feat_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;
            let component =
                ComponentName::new(comp_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let row = FeatureComponentsRow { feature, component };
            (*builder).inner = std::mem::take(&mut (*builder).inner)
                .add_record("FeatureComponents", row.to_record());
            Ok(MSI_SUCCESS)
        })
    }
}

/// Adds a file definition to the `File` table.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `file_id` - File identifier.
/// * `comp_id` - Foreign key to Component table.
/// * `file_name` - File name (e.g. "app.exe" or "app.exe|Application.exe").
/// * `file_size` - File size in bytes.
/// * `version` - Optional version string, or NULL.
/// * `language` - Optional language string, or NULL.
/// * `attributes` - File attribute flags.
/// * `sequence` - Sequence number in media.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All non-null string pointers must be valid null-terminated C strings.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn msi_package_builder_add_file(
    builder: *mut MsiPackageBuilderHandle,
    file_id: *const c_char,
    comp_id: *const c_char,
    file_name: *const c_char,
    file_size: u32,
    version: *const c_char,
    language: *const c_char,
    attributes: i16,
    sequence: i16,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let f_id = c_str_to_str(file_id, "file_id")?;
            let c_id = c_str_to_str(comp_id, "comp_id")?;
            let f_name = c_str_to_str(file_name, "file_name")?;

            let file = FileKey::new(f_id).map_err(|e| (map_msi_error(&e), e.to_string()))?;
            let component =
                ComponentName::new(c_id).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let ver_opt = c_str_to_opt_str(version, "version")?.map(ToString::to_string);
            let lang_opt = c_str_to_opt_str(language, "language")?.map(ToString::to_string);

            let row = FileRow {
                file,
                component,
                file_name: f_name.to_string(),
                file_size: i32::try_from(file_size).unwrap_or(i32::MAX),
                version: ver_opt,
                language: lang_opt,
                attributes: Some(attributes),
                sequence,
            };

            (*builder).inner = std::mem::take(&mut (*builder).inner).add_file(row);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Adds a media disk definition to the `Media` table.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `disk_id` - 1-based disk number.
/// * `last_sequence` - Maximum sequence number of file on this disk.
/// * `disk_prompt` - Optional prompt string, or NULL.
/// * `cabinet` - Optional cabinet name (e.g. "#cab1.cab"), or NULL.
/// * `volume_label` - Optional volume label, or NULL.
/// * `source` - Optional source folder property, or NULL.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All non-null string pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_add_media(
    builder: *mut MsiPackageBuilderHandle,
    disk_id: i16,
    last_sequence: u32,
    disk_prompt: *const c_char,
    cabinet: *const c_char,
    volume_label: *const c_char,
    source: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            let prompt_opt = c_str_to_opt_str(disk_prompt, "disk_prompt")?.map(ToString::to_string);
            let cab_opt = c_str_to_opt_str(cabinet, "cabinet")?.map(ToString::to_string);
            let vol_opt = c_str_to_opt_str(volume_label, "volume_label")?.map(ToString::to_string);
            let src_opt = c_str_to_opt_str(source, "source")?.map(ToString::to_string);

            let row = MediaRow {
                disk_id,
                last_sequence: i32::try_from(last_sequence).unwrap_or(i32::MAX),
                disk_prompt: prompt_opt,
                cabinet: cab_opt,
                volume_label: vol_opt,
                source: src_opt,
            };

            (*builder).inner = std::mem::take(&mut (*builder).inner).add_media(row);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Injects an embedded cabinet stream archive into the package.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `cab_name` - Cabinet stream name (e.g. "#cab1.cab").
/// * `cab_data` - Pointer to cabinet file bytes.
/// * `cab_len` - Length of cabinet file bytes.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `cab_name` must be a valid null-terminated C string.
/// `cab_data` must point to valid memory of at least `cab_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_add_embedded_cabinet(
    builder: *mut MsiPackageBuilderHandle,
    cab_name: *const c_char,
    cab_data: *const u8,
    cab_len: usize,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            if cab_data.is_null() && cab_len > 0 {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "cab_data must not be null when cab_len > 0".to_string(),
                ));
            }

            let name = c_str_to_str(cab_name, "cab_name")?;
            let data = if cab_len > 0 {
                std::slice::from_raw_parts(cab_data, cab_len).to_vec()
            } else {
                Vec::new()
            };

            (*builder).inner =
                std::mem::take(&mut (*builder).inner).add_embedded_cabinet(name, data);
            Ok(MSI_SUCCESS)
        })
    }
}

/// Automatically reads files from disk, generates hashes, compresses an embedded cabinet,
/// and populates the `File`, `FileHash`, `Media`, and cabinet streams.
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `source_paths` - Array of local filesystem source paths.
/// * `target_file_ids` - Array of unique File table identifiers.
/// * `component_ids` - Array of Component table foreign keys.
/// * `file_count` - Number of files in the arrays.
/// * `compression_type` - Compression algorithm (`0` None, `1` MSZIP, `2` Quantum, `3` LZX).
/// * `cabinet_name` - Cabinet stream name, or NULL to default to `"#cab1.cab"`.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// All array pointers must point to arrays of at least `file_count` valid null-terminated C strings.
#[no_mangle]
#[allow(clippy::too_many_lines)]
pub unsafe extern "C" fn msi_package_builder_pack_files_from_disk(
    builder: *mut MsiPackageBuilderHandle,
    source_paths: *const *const c_char,
    target_file_ids: *const *const c_char,
    component_ids: *const *const c_char,
    file_count: usize,
    compression_type: i32,
    cabinet_name: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }

            if file_count == 0 {
                return Ok(MSI_SUCCESS);
            }

            let Ok(last_seq_i16) = i16::try_from(file_count) else {
                return Err((
                    MSI_ERROR_INVALID_ARGUMENT,
                    format!("Total file count {file_count} overflows i16 sequence limit"),
                ));
            };
            let last_seq = i32::from(last_seq_i16);

            if source_paths.is_null() || target_file_ids.is_null() || component_ids.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "file arrays must not be null when file_count > 0".to_string(),
                ));
            }

            let comp_type = match compression_type {
                0 => CompressionType::None,
                2 => CompressionType::Quantum,
                3 => CompressionType::Lzx { window_bits: 17 },
                _ => CompressionType::Mszip,
            };

            let cab_name = c_str_to_opt_str(cabinet_name, "cabinet_name")?.unwrap_or("#cab1.cab");

            let mut cab_writer = CabinetWriter::new(comp_type);

            for i in 0..file_count {
                let src_ptr = *source_paths.add(i);
                let fid_ptr = *target_file_ids.add(i);
                let cid_ptr = *component_ids.add(i);

                let src_path = c_str_to_str(src_ptr, "source_paths[i]")?;
                let file_id = c_str_to_str(fid_ptr, "target_file_ids[i]")?;
                let comp_id = c_str_to_str(cid_ptr, "component_ids[i]")?;

                let data = fs::read(src_path).map_err(|e| {
                    (
                        map_msi_error(&msi::Error::Io(e.to_string())),
                        format!("Failed reading source file '{src_path}'"),
                    )
                })?;

                let digest = crate::md5::compute_md5(&data);
                let hash_part1 = i32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
                let hash_part2 = i32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]);
                let hash_part3 = i32::from_le_bytes([digest[8], digest[9], digest[10], digest[11]]);
                let hash_part4 =
                    i32::from_le_bytes([digest[12], digest[13], digest[14], digest[15]]);

                let file_key =
                    FileKey::new(file_id).map_err(|e| (map_msi_error(&e), e.to_string()))?;
                let component_name =
                    ComponentName::new(comp_id).map_err(|e| (map_msi_error(&e), e.to_string()))?;

                let hash_row = FileHashRow {
                    file: file_key.clone(),
                    options: 0,
                    hash_part1,
                    hash_part2,
                    hash_part3,
                    hash_part4,
                };

                let file_name_str = Path::new(src_path)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(file_id);

                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let seq = (i + 1) as i16;
                let file_size = i32::try_from(data.len()).unwrap_or(i32::MAX);

                let file_row = FileRow {
                    file: file_key,
                    component: component_name,
                    file_name: file_name_str.to_string(),
                    file_size,
                    version: None,
                    language: None,
                    attributes: Some(file_attributes::COMPRESSED),
                    sequence: seq,
                };

                (*builder).inner = std::mem::take(&mut (*builder).inner)
                    .add_record("FileHash", hash_row.to_record())
                    .add_file(file_row);

                cab_writer
                    .add_file(file_id, &data)
                    .map_err(|e| (map_msi_error(&e), e.to_string()))?;
            }

            let cab_bytes = cab_writer.build();

            let media_row = MediaRow {
                disk_id: 1,
                last_sequence: last_seq,
                disk_prompt: None,
                cabinet: Some(cab_name.to_string()),
                volume_label: None,
                source: None,
            };

            (*builder).inner = std::mem::take(&mut (*builder).inner)
                .add_media(media_row)
                .add_embedded_cabinet(cab_name, cab_bytes);

            Ok(MSI_SUCCESS)
        })
    }
}

/// Builds and validates the package, returning an [`MsiPackageHandle`].
///
/// # Arguments
///
/// * `builder` - Target builder handle.
/// * `out_package` - Pointer receiving the constructed package handle.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `builder` must be a valid pointer obtained from `msi_package_builder_create`.
/// `out_package` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_build(
    builder: *mut MsiPackageBuilderHandle,
    out_package: *mut *mut MsiPackageHandle,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if builder.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "builder handle must not be null".to_string(),
                ));
            }
            if out_package.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "out_package pointer must not be null".to_string(),
                ));
            }

            let pkg = (*builder)
                .inner
                .clone()
                .build()
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            *out_package = Box::into_raw(Box::new(MsiPackageHandle { inner: pkg }));
            Ok(MSI_SUCCESS)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::*;
    use crate::types::{msi_package_builder_destroy, msi_package_destroy};
    use std::ffi::CString;
    use std::ptr;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_builder_null_checks() {
        // SAFETY: Testing null pointer validation across all package builder FFI entry points.
        unsafe {
            assert_eq!(
                msi_package_builder_create(
                    ptr::null(),
                    ptr::null(),
                    1,
                    0,
                    0,
                    ptr::null(),
                    ptr::null_mut()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_product_name(ptr::null_mut(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_manufacturer(ptr::null_mut(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_version(ptr::null_mut(), 1, 0, 0),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_product_code(ptr::null_mut(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_upgrade_code(ptr::null_mut(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_property(ptr::null_mut(), ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_directory(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_component(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_feature_component(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_file(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_media(
                    ptr::null_mut(),
                    1,
                    1,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    0
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    1,
                    0,
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_build(ptr::null_mut(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );

            // Valid builder handle with null arguments
            let name = CString::new("App").unwrap_or_default();
            let mfr = CString::new("Acme").unwrap_or_default();
            let code = CString::new("{12345678-1234-1234-1234-1234567890AB}").unwrap_or_default();
            let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_create(
                    name.as_ptr(),
                    mfr.as_ptr(),
                    1,
                    0,
                    0,
                    code.as_ptr(),
                    &raw mut builder
                ),
                MSI_SUCCESS
            );

            assert_eq!(
                msi_package_builder_set_product_name(builder, ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_manufacturer(builder, ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_product_code(builder, ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_set_upgrade_code(builder, ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_property(builder, ptr::null(), name.as_ptr()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_property(builder, name.as_ptr(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_directory(builder, ptr::null(), ptr::null(), name.as_ptr()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_directory(builder, name.as_ptr(), ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_component(
                    builder,
                    ptr::null(),
                    ptr::null(),
                    name.as_ptr(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_component(
                    builder,
                    name.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    builder,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_feature_component(builder, ptr::null(), name.as_ptr()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_feature_component(builder, name.as_ptr(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_file(
                    builder,
                    ptr::null(),
                    name.as_ptr(),
                    name.as_ptr(),
                    100,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_file(
                    builder,
                    name.as_ptr(),
                    ptr::null(),
                    name.as_ptr(),
                    100,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_file(
                    builder,
                    name.as_ptr(),
                    name.as_ptr(),
                    ptr::null(),
                    100,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(builder, ptr::null(), ptr::null(), 0),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(builder, name.as_ptr(), ptr::null(), 10),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(builder, name.as_ptr(), ptr::null(), 0),
                MSI_SUCCESS
            );
            let dummy_arr: [*const c_char; 1] = [name.as_ptr()];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    1,
                    0,
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    dummy_arr.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    1,
                    0,
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    dummy_arr.as_ptr(),
                    dummy_arr.as_ptr(),
                    ptr::null(),
                    1,
                    0,
                    ptr::null()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null()
                ),
                MSI_SUCCESS
            );
            assert_eq!(
                msi_package_builder_build(builder, ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );

            msi_package_builder_destroy(builder);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_builder_lifecycle_and_build() {
        let name = CString::new("App").unwrap_or_default();
        let mfr = CString::new("Acme").unwrap_or_default();
        let code = CString::new("{12345678-1234-1234-1234-1234567890AB}").unwrap_or_default();
        let upg = CString::new("{87654321-4321-4321-4321-BA0987654321}").unwrap_or_default();

        let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
        unsafe {
            let res = msi_package_builder_create(
                name.as_ptr(),
                mfr.as_ptr(),
                1,
                2,
                3,
                code.as_ptr(),
                &raw mut builder,
            );
            assert_eq!(res, MSI_SUCCESS);
            assert!(!builder.is_null());

            // Setters
            let new_name = CString::new("App 2").unwrap_or_default();
            assert_eq!(
                msi_package_builder_set_product_name(builder, new_name.as_ptr()),
                MSI_SUCCESS
            );
            assert_eq!(
                msi_package_builder_set_manufacturer(builder, mfr.as_ptr()),
                MSI_SUCCESS
            );
            assert_eq!(
                msi_package_builder_set_version(builder, 2, 0, 0),
                MSI_SUCCESS
            );
            assert_eq!(
                msi_package_builder_set_product_code(builder, code.as_ptr()),
                MSI_SUCCESS
            );
            assert_eq!(
                msi_package_builder_set_upgrade_code(builder, upg.as_ptr()),
                MSI_SUCCESS
            );

            // Add Property
            let p_key = CString::new("CustomProp").unwrap_or_default();
            let p_val = CString::new("Value").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_property(builder, p_key.as_ptr(), p_val.as_ptr()),
                MSI_SUCCESS
            );

            // Add Directory
            let dir_id = CString::new("TARGETDIR").unwrap_or_default();
            let def_dir = CString::new("SourceDir").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_directory(
                    builder,
                    dir_id.as_ptr(),
                    ptr::null(),
                    def_dir.as_ptr()
                ),
                MSI_SUCCESS
            );

            // Add Component
            let comp_id = CString::new("MainComp").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_component(
                    builder,
                    comp_id.as_ptr(),
                    ptr::null(),
                    dir_id.as_ptr(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_SUCCESS
            );

            // Add Parent Feature
            let parent_feat = CString::new("ParentFeat").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_feature(
                    builder,
                    parent_feat.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0
                ),
                MSI_SUCCESS
            );

            // Add Feature with parent_id (testing line 479)
            let feat_id = CString::new("MainFeat").unwrap_or_default();
            let feat_title = CString::new("Main Feature").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_feature(
                    builder,
                    feat_id.as_ptr(),
                    parent_feat.as_ptr(),
                    feat_title.as_ptr(),
                    ptr::null(),
                    1,
                    1,
                    dir_id.as_ptr(),
                    0
                ),
                MSI_SUCCESS
            );

            // Link Feature and Component
            assert_eq!(
                msi_package_builder_add_feature_component(
                    builder,
                    feat_id.as_ptr(),
                    comp_id.as_ptr()
                ),
                MSI_SUCCESS
            );

            // Add File
            let file_id = CString::new("AppFile").unwrap_or_default();
            let file_name = CString::new("app.exe").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_file(
                    builder,
                    file_id.as_ptr(),
                    comp_id.as_ptr(),
                    file_name.as_ptr(),
                    1024,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_SUCCESS
            );

            // Add Media
            let cab_name = CString::new("#cab1.cab").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_media(
                    builder,
                    1,
                    1,
                    ptr::null(),
                    cab_name.as_ptr(),
                    ptr::null(),
                    ptr::null()
                ),
                MSI_SUCCESS
            );

            // Add embedded cabinet
            let cab_bytes = [0u8; 16];
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(
                    builder,
                    cab_name.as_ptr(),
                    cab_bytes.as_ptr(),
                    cab_bytes.len()
                ),
                MSI_SUCCESS
            );

            // Build package
            let mut pkg_handle: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_build(builder, &raw mut pkg_handle),
                MSI_SUCCESS
            );
            assert!(!pkg_handle.is_null());

            msi_package_destroy(pkg_handle);
            msi_package_builder_destroy(builder);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_builder_validation_errors() {
        let name = CString::new("ValidApp").unwrap_or_default();
        let mfr = CString::new("ValidMfr").unwrap_or_default();
        let code = CString::new("{12345678-1234-1234-1234-1234567890AB}").unwrap_or_default();
        let empty = CString::new("").unwrap_or_default();
        let valid_id = CString::new("ValidId").unwrap_or_default();

        let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
        // SAFETY: Builder operations with invalid identifier inputs to test validation error mapping.
        unsafe {
            assert_eq!(
                msi_package_builder_create(
                    name.as_ptr(),
                    mfr.as_ptr(),
                    1,
                    0,
                    0,
                    code.as_ptr(),
                    &raw mut builder
                ),
                MSI_SUCCESS
            );

            // Directory validations
            assert_eq!(
                msi_package_builder_add_directory(
                    builder,
                    empty.as_ptr(),
                    ptr::null(),
                    valid_id.as_ptr()
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_directory(
                    builder,
                    valid_id.as_ptr(),
                    empty.as_ptr(),
                    valid_id.as_ptr()
                ),
                MSI_ERROR_VALIDATION
            );

            // Component validations
            assert_eq!(
                msi_package_builder_add_component(
                    builder,
                    empty.as_ptr(),
                    ptr::null(),
                    valid_id.as_ptr(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_component(
                    builder,
                    valid_id.as_ptr(),
                    empty.as_ptr(),
                    valid_id.as_ptr(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_component(
                    builder,
                    valid_id.as_ptr(),
                    ptr::null(),
                    empty.as_ptr(),
                    0,
                    ptr::null(),
                    ptr::null()
                ),
                MSI_ERROR_VALIDATION
            );

            // Feature validations
            assert_eq!(
                msi_package_builder_add_feature(
                    builder,
                    empty.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    builder,
                    valid_id.as_ptr(),
                    empty.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    builder,
                    valid_id.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    empty.as_ptr(),
                    0
                ),
                MSI_ERROR_VALIDATION
            );

            // Feature-Component validations
            assert_eq!(
                msi_package_builder_add_feature_component(
                    builder,
                    empty.as_ptr(),
                    valid_id.as_ptr()
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_feature_component(
                    builder,
                    valid_id.as_ptr(),
                    empty.as_ptr()
                ),
                MSI_ERROR_VALIDATION
            );

            // File validations
            assert_eq!(
                msi_package_builder_add_file(
                    builder,
                    empty.as_ptr(),
                    valid_id.as_ptr(),
                    valid_id.as_ptr(),
                    10,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_ERROR_VALIDATION
            );
            assert_eq!(
                msi_package_builder_add_file(
                    builder,
                    valid_id.as_ptr(),
                    empty.as_ptr(),
                    valid_id.as_ptr(),
                    10,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1
                ),
                MSI_ERROR_VALIDATION
            );

            msi_package_builder_destroy(builder);

            // Build validation error when builder has empty product name
            let mut empty_builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_create(
                    empty.as_ptr(),
                    mfr.as_ptr(),
                    1,
                    0,
                    0,
                    code.as_ptr(),
                    &raw mut empty_builder
                ),
                MSI_SUCCESS
            );
            let mut fail_pkg: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_build(empty_builder, &raw mut fail_pkg),
                MSI_ERROR_VALIDATION
            );
            msi_package_builder_destroy(empty_builder);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_pack_files_from_disk() {
        let temp_dir = std::env::temp_dir().join("msi_ffi_pack_test");
        let _ = fs::create_dir_all(&temp_dir);
        let f1_path = temp_dir.join("test1.txt");
        let f2_path = temp_dir.join("test2.txt");
        let _ = fs::write(&f1_path, b"hello from file 1");
        let _ = fs::write(&f2_path, b"hello from file 2");

        let name = CString::new("App").unwrap_or_default();
        let mfr = CString::new("Acme").unwrap_or_default();
        let code = CString::new("{12345678-1234-1234-1234-1234567890AB}").unwrap_or_default();
        let empty = CString::new("").unwrap_or_default();

        let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
        unsafe {
            assert_eq!(
                msi_package_builder_create(
                    name.as_ptr(),
                    mfr.as_ptr(),
                    1,
                    0,
                    0,
                    code.as_ptr(),
                    &raw mut builder
                ),
                MSI_SUCCESS
            );

            let s1 = CString::new(f1_path.to_str().unwrap_or("")).unwrap_or_default();
            let s2 = CString::new(f2_path.to_str().unwrap_or("")).unwrap_or_default();
            let fid1 = CString::new("File1").unwrap_or_default();
            let fid2 = CString::new("File2").unwrap_or_default();
            let comp1 = CString::new("Comp1").unwrap_or_default();
            let comp2 = CString::new("Comp2").unwrap_or_default();

            let sources = [s1.as_ptr(), s2.as_ptr()];
            let file_id_ptrs = [fid1.as_ptr(), fid2.as_ptr()];
            let comp_id_ptrs = [comp1.as_ptr(), comp2.as_ptr()];

            // 1. Successful packing with compression types (None, Quantum, LZX, MSZIP)
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    2,
                    0, // None
                    ptr::null(),
                ),
                MSI_SUCCESS
            );

            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    2,
                    2, // Quantum
                    ptr::null(),
                ),
                MSI_SUCCESS
            );

            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    2,
                    3, // LZX
                    ptr::null(),
                ),
                MSI_SUCCESS
            );

            // 2. Sequence overflow limit validation
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    40_000,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // 3. Nonexistent file read failure
            let bad_src = CString::new("/nonexistent_dir_test/nofile.bin").unwrap_or_default();
            let bad_sources = [bad_src.as_ptr()];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    bad_sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_IO
            );

            // 4. FileKey validation failure
            let empty_fids = [empty.as_ptr()];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    empty_fids.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_VALIDATION
            );

            // 5. ComponentName validation failure
            let empty_comps = [empty.as_ptr()];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    empty_comps.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_VALIDATION
            );

            // 6. Duplicate file id in cabinet
            let dup_sources = [s1.as_ptr(), s1.as_ptr()];
            let dup_fids = [fid1.as_ptr(), fid1.as_ptr()];
            let dup_comps = [comp1.as_ptr(), comp1.as_ptr()];
            assert_ne!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    dup_sources.as_ptr(),
                    dup_fids.as_ptr(),
                    dup_comps.as_ptr(),
                    2,
                    1,
                    ptr::null(),
                ),
                MSI_SUCCESS
            );

            // 7. Invalid UTF-8 in pack_files_from_disk
            let invalid_utf8 = [0xFF_u8, 0xFE, 0xFD, 0x00];
            let inv_ptr = invalid_utf8.as_ptr().cast::<c_char>();
            let inv_sources = [inv_ptr];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    inv_sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            let inv_fids = [inv_ptr];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    inv_fids.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            let inv_comps = [inv_ptr];
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    inv_comps.as_ptr(),
                    1,
                    1,
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    file_id_ptrs.as_ptr(),
                    comp_id_ptrs.as_ptr(),
                    1,
                    1,
                    inv_ptr,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            msi_package_builder_destroy(builder);
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    /// Tests that invalid UTF-8 strings return [`MSI_ERROR_INVALID_ARGUMENT`] across builder methods.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_builder_invalid_utf8_arguments() {
        let invalid_utf8 = [0xFF_u8, 0xFE, 0xFD, 0x00];
        let inv = invalid_utf8.as_ptr().cast::<c_char>();
        let valid_name = CString::new("ValidName").unwrap_or_default();
        let valid_code = CString::new("{12345678-1234-1234-1234-1234567890AB}").unwrap_or_default();

        unsafe {
            let mut out_builder: *mut MsiPackageBuilderHandle = ptr::null_mut();

            // Create with invalid UTF-8
            assert_eq!(
                msi_package_builder_create(
                    inv,
                    valid_name.as_ptr(),
                    1,
                    0,
                    0,
                    valid_code.as_ptr(),
                    &raw mut out_builder,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_create(
                    valid_name.as_ptr(),
                    inv,
                    1,
                    0,
                    0,
                    valid_code.as_ptr(),
                    &raw mut out_builder,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_create(
                    valid_name.as_ptr(),
                    valid_name.as_ptr(),
                    1,
                    0,
                    0,
                    inv,
                    &raw mut out_builder,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // Valid builder creation
            assert_eq!(
                msi_package_builder_create(
                    valid_name.as_ptr(),
                    valid_name.as_ptr(),
                    1,
                    0,
                    0,
                    valid_code.as_ptr(),
                    &raw mut out_builder,
                ),
                MSI_SUCCESS
            );

            // Directory with invalid UTF-8 parent
            assert_eq!(
                msi_package_builder_add_directory(
                    out_builder,
                    valid_name.as_ptr(),
                    inv,
                    valid_name.as_ptr(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // Component with invalid UTF-8 optional parameters
            assert_eq!(
                msi_package_builder_add_component(
                    out_builder,
                    valid_name.as_ptr(),
                    inv,
                    valid_name.as_ptr(),
                    0,
                    ptr::null(),
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_component(
                    out_builder,
                    valid_name.as_ptr(),
                    ptr::null(),
                    valid_name.as_ptr(),
                    0,
                    inv,
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_component(
                    out_builder,
                    valid_name.as_ptr(),
                    ptr::null(),
                    valid_name.as_ptr(),
                    0,
                    ptr::null(),
                    inv,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // Feature with invalid UTF-8 optional parameters
            assert_eq!(
                msi_package_builder_add_feature(
                    out_builder,
                    valid_name.as_ptr(),
                    inv,
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    out_builder,
                    valid_name.as_ptr(),
                    ptr::null(),
                    inv,
                    ptr::null(),
                    0,
                    1,
                    ptr::null(),
                    0,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    out_builder,
                    valid_name.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    inv,
                    0,
                    1,
                    ptr::null(),
                    0,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_feature(
                    out_builder,
                    valid_name.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    1,
                    inv,
                    0,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // File with invalid UTF-8 version / language
            assert_eq!(
                msi_package_builder_add_file(
                    out_builder,
                    valid_name.as_ptr(),
                    valid_name.as_ptr(),
                    valid_name.as_ptr(),
                    100,
                    inv,
                    ptr::null(),
                    0,
                    1,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_file(
                    out_builder,
                    valid_name.as_ptr(),
                    valid_name.as_ptr(),
                    valid_name.as_ptr(),
                    100,
                    ptr::null(),
                    inv,
                    0,
                    1,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // Media with invalid UTF-8 optional fields
            assert_eq!(
                msi_package_builder_add_media(
                    out_builder,
                    1,
                    10,
                    inv,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_media(
                    out_builder,
                    1,
                    10,
                    ptr::null(),
                    inv,
                    ptr::null(),
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_media(
                    out_builder,
                    1,
                    10,
                    ptr::null(),
                    ptr::null(),
                    inv,
                    ptr::null(),
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );
            assert_eq!(
                msi_package_builder_add_media(
                    out_builder,
                    1,
                    10,
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                    inv,
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            msi_package_builder_destroy(out_builder);
        }
    }
}
