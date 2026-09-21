/**
 * @file msi.h
 * @brief C-ABI Foreign Function Interface (FFI) for msi-rs Windows Installer toolkit.
 *
 * Grounded in the Microsoft Windows Installer (MSI) SDK, [MS-CFB], and Cabinet specifications.
 * Allows C, C++, Python, Go, C#, Node.js, and other languages to create, configure, pack,
 * compile, inspect, and extract .msi packages.
 */

#ifndef MSI_RS_FFI_H
#define MSI_RS_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ========================================================================= */
/* Status and Error Codes                                                    */
/* ========================================================================= */

/** Operation completed successfully. */
#define MSI_SUCCESS 0

/** Required pointer argument is NULL. */
#define MSI_ERROR_NULL_POINTER -1

/** Argument failed validation or parsing. */
#define MSI_ERROR_INVALID_ARGUMENT -2

/** Package or relational schema validation failed. */
#define MSI_ERROR_VALIDATION -3

/** Filesystem or stream I/O failure. */
#define MSI_ERROR_IO -4

/** Cabinet archive compression, decompression, or packing failure. */
#define MSI_ERROR_CABINET -5

/** Relational database constraint or catalog violation. */
#define MSI_ERROR_DATABASE -6

/** WiX preprocessor, compiler, or linker evaluation failure. */
#define MSI_ERROR_WIX -7

/** Provided output buffer size is insufficient. */
#define MSI_ERROR_BUFFER_TOO_SMALL -8

/** Unhandled panic intercepted at the FFI boundary. */
#define MSI_ERROR_PANIC -99

/* ========================================================================= */
/* Compression Algorithms                                                    */
/* ========================================================================= */

/** Uncompressed folder payload. */
#define MSI_COMPRESSION_NONE 0

/** MSZIP (Deflate with 2-byte magic frame). */
#define MSI_COMPRESSION_MSZIP 1

/** Quantum arithmetic entropy compression. */
#define MSI_COMPRESSION_QUANTUM 2

/** LZX compression with sliding window. */
#define MSI_COMPRESSION_LZX 3

/* ========================================================================= */
/* Opaque Handle Types                                                       */
/* ========================================================================= */

/** Opaque handle representing an active package builder. */
typedef struct MsiPackageBuilderHandle MsiPackageBuilderHandle;

/** Opaque handle representing a complete Windows Installer package. */
typedef struct MsiPackageHandle MsiPackageHandle;

/** Opaque handle representing a relational database. */
typedef struct MsiDatabaseHandle MsiDatabaseHandle;

/** Opaque handle representing a database record. */
typedef struct MsiRecordHandle MsiRecordHandle;

/** Opaque handle representing Summary Information stream properties. */
typedef struct MsiSummaryInfoHandle MsiSummaryInfoHandle;

/** Buffer descriptor representing allocated native memory. */
typedef struct MsiBufferHandle {
    /** Pointer to the raw byte buffer. */
    uint8_t* data;
    /** Length of the buffer in bytes. */
    size_t len;
} MsiBufferHandle;

/* ========================================================================= */
/* Diagnostics and Error Inspection                                          */
/* ========================================================================= */

/**
 * @brief Retrieves the status code of the most recent error on the calling thread.
 * @return Numeric status code, or MSI_SUCCESS if no error has occurred.
 */
int32_t msi_get_last_error_code(void);

/**
 * @brief Copies the most recent error description into a caller-supplied buffer.
 * @param buffer Destination buffer.
 * @param capacity Capacity of the buffer in bytes.
 * @param out_written Pointer receiving the required or written length.
 * @return MSI_SUCCESS on success, or MSI_ERROR_BUFFER_TOO_SMALL.
 */
int32_t msi_get_last_error_message(char* buffer, size_t capacity, size_t* out_written);

/**
 * @brief Resets the thread-local error state to MSI_SUCCESS.
 */
void msi_clear_last_error(void);

/* ========================================================================= */
/* Memory Lifecycle Destructors                                              */
/* ========================================================================= */

/**
 * @brief Frees a package builder handle. Safe no-op if handle is NULL.
 * @param handle Builder handle to destroy.
 */
void msi_package_builder_destroy(MsiPackageBuilderHandle* handle);

/**
 * @brief Frees a package handle. Safe no-op if handle is NULL.
 * @param handle Package handle to destroy.
 */
void msi_package_destroy(MsiPackageHandle* handle);

/**
 * @brief Frees a database handle. Safe no-op if handle is NULL.
 * @param handle Database handle to destroy.
 */
void msi_database_destroy(MsiDatabaseHandle* handle);

/**
 * @brief Frees a record handle. Safe no-op if handle is NULL.
 * @param handle Record handle to destroy.
 */
void msi_record_destroy(MsiRecordHandle* handle);

/**
 * @brief Frees a summary info handle. Safe no-op if handle is NULL.
 * @param handle Summary info handle to destroy.
 */
void msi_summary_info_destroy(MsiSummaryInfoHandle* handle);

/**
 * @brief Frees a null-terminated C string allocated by the library. Safe no-op if ptr is NULL.
 * @param ptr String pointer to free.
 */
void msi_string_free(char* ptr);

/**
 * @brief Frees a byte buffer allocated by the library. Safe no-op if ptr is NULL or len is 0.
 * @param ptr Buffer pointer to free.
 * @param len Length in bytes.
 */
void msi_buffer_free(uint8_t* ptr, size_t len);

/* ========================================================================= */
/* Package Builder API                                                       */
/* ========================================================================= */

/**
 * @brief Constructs a new package builder instance.
 * @param product_name Product name string.
 * @param manufacturer Manufacturer name string.
 * @param ver_major Major version component.
 * @param ver_minor Minor version component.
 * @param ver_build Build version component.
 * @param product_code ProductCode GUID in {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} format.
 * @param out_builder Pointer receiving the allocated builder handle.
 * @return MSI_SUCCESS on success, or an error code.
 */
int32_t msi_package_builder_create(
    const char* product_name,
    const char* manufacturer,
    uint8_t ver_major,
    uint8_t ver_minor,
    uint16_t ver_build,
    const char* product_code,
    MsiPackageBuilderHandle** out_builder
);

/**
 * @brief Sets the product name on the builder.
 */
int32_t msi_package_builder_set_product_name(MsiPackageBuilderHandle* builder, const char* name);

/**
 * @brief Sets the manufacturer name on the builder.
 */
int32_t msi_package_builder_set_manufacturer(MsiPackageBuilderHandle* builder, const char* manufacturer);

/**
 * @brief Sets the product version on the builder.
 */
int32_t msi_package_builder_set_version(MsiPackageBuilderHandle* builder, uint8_t ver_major, uint8_t ver_minor, uint16_t ver_build);

/**
 * @brief Sets the ProductCode GUID on the builder.
 */
int32_t msi_package_builder_set_product_code(MsiPackageBuilderHandle* builder, const char* product_code);

/**
 * @brief Sets the UpgradeCode GUID on the builder.
 */
int32_t msi_package_builder_set_upgrade_code(MsiPackageBuilderHandle* builder, const char* upgrade_code);

/**
 * @brief Adds a property to the package's Property table.
 */
int32_t msi_package_builder_add_property(MsiPackageBuilderHandle* builder, const char* name, const char* value);

/**
 * @brief Adds a directory definition to the package.
 */
int32_t msi_package_builder_add_directory(
    MsiPackageBuilderHandle* builder,
    const char* dir_id,
    const char* parent_id,
    const char* default_dir
);

/**
 * @brief Adds a component definition to the package.
 */
int32_t msi_package_builder_add_component(
    MsiPackageBuilderHandle* builder,
    const char* comp_id,
    const char* comp_guid,
    const char* dir_id,
    int16_t attributes,
    const char* condition,
    const char* keypath
);

/**
 * @brief Adds a feature definition to the package.
 */
int32_t msi_package_builder_add_feature(
    MsiPackageBuilderHandle* builder,
    const char* feat_id,
    const char* parent_id,
    const char* title,
    const char* description,
    int16_t display,
    int16_t level,
    const char* dir_id,
    int16_t attributes
);

/**
 * @brief Links a feature to a component in the FeatureComponents table.
 */
int32_t msi_package_builder_add_feature_component(
    MsiPackageBuilderHandle* builder,
    const char* feat_id,
    const char* comp_id
);

/**
 * @brief Adds a file definition to the File table.
 */
int32_t msi_package_builder_add_file(
    MsiPackageBuilderHandle* builder,
    const char* file_id,
    const char* comp_id,
    const char* file_name,
    uint32_t file_size,
    const char* version,
    const char* language,
    int16_t attributes,
    int16_t sequence
);

/**
 * @brief Adds a media entry to the Media table.
 */
int32_t msi_package_builder_add_media(
    MsiPackageBuilderHandle* builder,
    int16_t disk_id,
    uint32_t last_sequence,
    const char* disk_prompt,
    const char* cabinet,
    const char* volume_label,
    const char* source
);

/**
 * @brief Injects an embedded cabinet archive into the package.
 */
int32_t msi_package_builder_add_embedded_cabinet(
    MsiPackageBuilderHandle* builder,
    const char* cab_name,
    const uint8_t* cab_data,
    size_t cab_len
);

/**
 * @brief Automatically hashes files from disk, compresses them into an embedded cabinet,
 * and populates File, FileHash, Media, and cabinet streams.
 */
int32_t msi_package_builder_pack_files_from_disk(
    MsiPackageBuilderHandle* builder,
    const char** source_paths,
    const char** target_file_ids,
    const char** component_ids,
    size_t file_count,
    int32_t compression_type,
    const char* cabinet_name
);

/**
 * @brief Validates and compiles the package, returning an MsiPackageHandle.
 */
int32_t msi_package_builder_build(
    MsiPackageBuilderHandle* builder,
    MsiPackageHandle** out_package
);

/* ========================================================================= */
/* Package Serialization and Inspection                                      */
/* ========================================================================= */

/**
 * @brief Saves the package to a .msi file on disk.
 */
int32_t msi_package_save(const MsiPackageHandle* package, const char* output_path);

/**
 * @brief Serializes the package into an in-memory byte buffer.
 * The buffer must be freed using msi_buffer_free.
 */
int32_t msi_package_to_bytes(
    const MsiPackageHandle* package,
    uint8_t** out_bytes,
    size_t* out_len
);

/**
 * @brief Opens an existing .msi file from disk.
 */
int32_t msi_package_open(const char* path, MsiPackageHandle** out_package);

/**
 * @brief Queries a property value from the package.
 */
int32_t msi_package_get_property(
    const MsiPackageHandle* package,
    const char* property_name,
    char* buffer,
    size_t capacity,
    size_t* out_written
);

/**
 * @brief Extracts all files from an embedded cabinet stream to a local directory.
 */
int32_t msi_package_extract_cabinet(
    const MsiPackageHandle* package,
    const char* cabinet_name,
    const char* dest_dir
);

/* ========================================================================= */
/* WiX Compiler Pipeline                                                     */
/* ========================================================================= */

/**
 * @brief Compiles a WiX XML source string directly to an .msi file.
 */
int32_t msi_compile_wix_source(const char* wxs_content, const char* output_msi_path);

/**
 * @brief Compiles a WiX XML source file directly to an .msi file.
 */
int32_t msi_compile_wix_file(const char* wxs_path, const char* output_msi_path);

#ifdef __cplusplus
}
#endif

#endif /* MSI_RS_FFI_H */
