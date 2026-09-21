//! Error types and results for the MSI library.

use derive_more::{Display, Error};

/// Primary error enum for all MSI operations.
#[derive(Debug, Display, Error, PartialEq, Eq)]
pub enum Error {
    /// An I/O error occurred during an operation.
    #[display("I/O error: {_0}")]
    #[error(ignore)]
    Io(String),

    /// An invalid argument was provided to an operation.
    #[display("Invalid argument '{argument}': {reason}")]
    InvalidArgument {
        /// Name of the parameter that was invalid.
        argument: String,
        /// Detail regarding why the argument was rejected.
        reason: String,
    },

    /// A required database table is absent.
    #[display("Missing required table: {name}")]
    MissingTable {
        /// Name of the missing database table.
        name: String,
    },

    /// A semantic validation rule was violated.
    #[display("Validation error on {element}: {reason}")]
    Validation {
        /// Target element or record that failed validation.
        element: String,
        /// Reason for validation failure.
        reason: String,
    },

    /// An unsupported or unrecognized feature was encountered.
    #[display("Unsupported feature: {name}")]
    Unsupported {
        /// Name or identifier of the unsupported feature.
        name: String,
    },

    /// A SQL parsing or execution error occurred.
    #[display("SQL query error: {message}")]
    Sql {
        /// Detail regarding the SQL error.
        message: String,
    },

    /// The CFB header signature is invalid.
    #[display("Invalid CFB header signature: {found:02X?}")]
    InvalidCfbSignature {
        /// The byte sequence found in the header signature field.
        found: [u8; 8],
    },

    /// The CFB header CLSID is invalid (must be all zeroes).
    #[display("Invalid CFB header CLSID: {found:02X?}")]
    InvalidCfbClsid {
        /// The byte sequence found in the header CLSID field.
        found: [u8; 16],
    },

    /// The CFB minor version is invalid (must be 0x003E).
    #[display("Invalid CFB minor version: 0x{found:04X}")]
    InvalidCfbMinorVersion {
        /// The minor version found.
        found: u16,
    },

    /// The CFB major version is invalid (must be 0x0003 or 0x0004).
    #[display("Invalid CFB major version: 0x{found:04X}")]
    InvalidCfbMajorVersion {
        /// The major version found.
        found: u16,
    },

    /// The CFB byte order marker is invalid (must be 0xFFFE).
    #[display("Invalid CFB byte order marker: 0x{found:04X}")]
    InvalidCfbByteOrder {
        /// The byte order marker found.
        found: u16,
    },

    /// The CFB sector shift does not match the major version.
    #[display("Invalid CFB sector shift {shift} for major version {major_version}")]
    InvalidCfbSectorShift {
        /// Major version of the CFB file.
        major_version: u16,
        /// Sector shift found in the header.
        shift: u16,
    },

    /// The CFB mini sector shift is invalid (must be 0x0006).
    #[display("Invalid CFB mini sector shift: 0x{shift:04X}")]
    InvalidCfbMiniSectorShift {
        /// Mini sector shift found in the header.
        shift: u16,
    },

    /// The CFB header reserved bytes are non-zero.
    #[display("Invalid CFB header reserved bytes: {found:02X?}")]
    InvalidCfbReserved {
        /// Reserved byte sequence found.
        found: [u8; 6],
    },

    /// The CFB directory sector count is invalid for the major version.
    #[display("Invalid CFB directory sector count {count} for major version {major_version}")]
    InvalidCfbDirectorySectors {
        /// Major version of the CFB file.
        major_version: u16,
        /// Count of directory sectors in the header.
        count: u32,
    },

    /// The CFB mini stream cutoff size is invalid (must be 4096 bytes).
    #[display("Invalid CFB mini stream cutoff size: {cutoff}")]
    InvalidCfbMiniStreamCutoff {
        /// Cutoff size found in the header.
        cutoff: u32,
    },

    /// A CFB sector number is invalid or out of range.
    #[display("Invalid sector index 0x{sector:08X}: {reason}")]
    InvalidSector {
        /// Sector index that was invalid.
        sector: u32,
        /// Reason the sector is invalid.
        reason: String,
    },

    /// A cycle was detected in a CFB sector chain.
    #[display("Cycle detected in CFB sector chain starting at sector 0x{sector:08X}")]
    SectorChainCycle {
        /// Starting sector index of the chain where a loop occurred.
        sector: u32,
    },

    /// A CFB directory entry is malformed or invalid.
    #[display("Invalid directory entry at index {index}: {reason}")]
    InvalidDirectoryEntry {
        /// Index of the directory entry.
        index: u32,
        /// Reason the directory entry is invalid.
        reason: String,
    },

    /// A requested stream was not found in the CFB container.
    #[display("Stream '{name}' was not found in container")]
    StreamNotFound {
        /// Name of the requested stream.
        name: String,
    },

    /// A duplicate directory entry name was found in the same storage.
    #[display("Duplicate directory entry name '{name}'")]
    DuplicateDirectoryEntry {
        /// Name of the duplicated entry.
        name: String,
    },

    /// Corruption was encountered in the CFB container structure.
    #[display("CFB corruption at byte offset {offset}: {reason}")]
    CfbCorrupted {
        /// Byte offset where corruption was identified.
        offset: u64,
        /// Description of the corruption.
        reason: String,
    },

    /// An MSI stream name is invalid or cannot be decoded.
    #[display("Invalid stream name '{name}': {reason}")]
    InvalidStreamName {
        /// Name of the invalid stream.
        name: String,
        /// Reason the stream name is invalid.
        reason: String,
    },

    /// Unexpected stream size encountered.
    #[display("Stream size mismatch: expected {expected} bytes, got {actual} bytes")]
    StreamSizeMismatch {
        /// Expected byte length.
        expected: u64,
        /// Actual byte length.
        actual: u64,
    },

    /// The Cabinet file signature is invalid (must be MSCF).
    #[display("Invalid Cabinet signature: {found:02X?}")]
    InvalidCabSignature {
        /// Bytes found in signature field.
        found: [u8; 4],
    },

    /// The Cabinet format version is unsupported.
    #[display("Unsupported Cabinet version {major}.{minor}")]
    InvalidCabVersion {
        /// Format major version.
        major: u8,
        /// Format minor version.
        minor: u8,
    },

    /// A Cabinet block checksum verification failed.
    #[display("Cabinet checksum mismatch: expected 0x{expected:08X}, actual 0x{actual:08X}")]
    InvalidCabChecksum {
        /// Expected checksum recorded in header.
        expected: u32,
        /// Actual checksum computed from block data.
        actual: u32,
    },

    /// Malformed or corrupted Cabinet container structure encountered.
    #[display("Invalid Cabinet data: {reason}")]
    InvalidCabData {
        /// Description of the corruption or invalid field.
        reason: String,
    },

    /// Decompression of a Cabinet block failed.
    #[display("Decompression failed using {method}: {reason}")]
    DecompressionFailed {
        /// Name of compression algorithm (e.g., MSZIP, LZX).
        method: String,
        /// Reason decompression failed.
        reason: String,
    },

    /// Compression of a Cabinet block failed.
    #[display("Compression failed using {method}: {reason}")]
    CompressionFailed {
        /// Name of compression algorithm.
        method: String,
        /// Reason compression failed.
        reason: String,
    },

    /// A requested file was not found within the Cabinet archive.
    #[display("File '{name}' was not found in Cabinet archive")]
    CabinetFileNotFound {
        /// Name of the missing file.
        name: String,
    },

    /// The column type bitmask in an MSI table schema is invalid.
    #[display("Invalid MSI column type bitmask: 0x{raw:04X}")]
    InvalidColumnType {
        /// Raw 16-bit column type integer.
        raw: u16,
    },

    /// The MSI string pool data or pool header stream is invalid.
    #[display("Invalid MSI string pool: {reason}")]
    InvalidStringPool {
        /// Description of string pool corruption.
        reason: String,
    },

    /// A string pool index was out of range.
    #[display("String pool index {index} out of bounds (max {max})")]
    StringPoolIndexOutOfBounds {
        /// Attempted index.
        index: u32,
        /// Maximum valid index in current string pool.
        max: u32,
    },

    /// An OLE Summary Information property stream is invalid.
    #[display("Invalid Summary Information stream: {reason}")]
    InvalidSummaryInfo {
        /// Description of property set corruption.
        reason: String,
    },

    /// A record byte or field count did not match the table schema.
    #[display("Record length mismatch: expected {expected}, actual {actual}")]
    RecordLengthMismatch {
        /// Expected size or count.
        expected: usize,
        /// Actual size or count.
        actual: usize,
    },

    /// A `WiX` preprocessor error occurred.
    #[display("WiX preprocessor error at {line}:{column}: {message}")]
    Preprocessor {
        /// Line number (1-based).
        line: usize,
        /// Column number (1-based).
        column: usize,
        /// Diagnostic message.
        message: String,
    },

    /// A `WiX` XML parsing error occurred.
    #[display("WiX XML parse error at {line}:{column}: {message}")]
    XmlParse {
        /// Line number (1-based).
        line: usize,
        /// Column number (1-based).
        column: usize,
        /// Diagnostic message.
        message: String,
    },

    /// A `WiX` compiler error occurred while compiling elements to intermediate representation.
    #[display("WiX compiler error in element '{element}': {message}")]
    WixCompiler {
        /// Enclosing or offending element name.
        element: String,
        /// Diagnostic message.
        message: String,
    },

    /// A `WiX` intermediate object file (`.wixobj`) is invalid or corrupted.
    #[display("Invalid WiX intermediate object file: {reason}")]
    InvalidWixObject {
        /// Diagnostic reason.
        reason: String,
    },

    /// A `WiX` linker error occurred during symbol resolution or linking.
    #[display("WiX linker error: {message}")]
    WixLinker {
        /// Diagnostic message.
        message: String,
    },

    /// An Internal Consistency Evaluator (ICE) validation failed.
    #[display("ICE validation failure [{ice}]: {message}")]
    IceValidation {
        /// Identifier of the ICE rule (e.g. "ICE01", "ICE03").
        ice: String,
        /// Diagnostic failure message.
        message: String,
    },

    /// An action execution failure occurred during the installation transaction.
    #[display("Action '{action}' failed with return code {return_code}: {message}")]
    ExecutionFailed {
        /// Action or script command that failed.
        action: String,
        /// Execution or Win32 return code (e.g. 1603 for fatal failure).
        return_code: u32,
        /// Diagnostic message.
        message: String,
    },

    /// A rollback command failed while undoing transaction changes.
    #[display("Rollback failed during action '{action}': {reason}")]
    RollbackFailed {
        /// The rollback action that encountered a failure.
        action: String,
        /// Detail regarding the failure.
        reason: String,
    },

    /// A transaction state machine transition failed or was attempted in an invalid state.
    #[display("Transaction state mismatch: expected '{expected}', found '{actual}'")]
    TransactionStateMismatch {
        /// Expected state description.
        expected: String,
        /// Actual state encountered.
        actual: String,
    },

    /// Required disk space exceeds available space on target volume.
    #[display("Insufficient disk space on volume '{volume}': required {required_bytes} bytes, available {available_bytes} bytes")]
    DiskCostExceeded {
        /// Volume name or path identifier.
        volume: String,
        /// Number of bytes required.
        required_bytes: u64,
        /// Number of bytes available.
        available_bytes: u64,
    },

    /// An error occurred during custom action execution.
    #[display("Custom action '{action}' failed: {reason}")]
    CustomActionFailed {
        /// The name of the custom action.
        action: String,
        /// Reason for failure.
        reason: String,
    },

    /// An error occurred while parsing or processing an execution or rollback script.
    #[display("Script processing error on opcode '{opcode}': {reason}")]
    ScriptError {
        /// The script opcode or instruction name.
        opcode: String,
        /// Diagnostic reason.
        reason: String,
    },

    /// An error occurred during UI dialog or control evaluation.
    #[display("UI error in dialog '{dialog}' control '{control}': {reason}")]
    UiError {
        /// The dialog identifier.
        dialog: String,
        /// The control identifier.
        control: String,
        /// Diagnostic message.
        reason: String,
    },

    /// An error occurred during script engine execution (`JScript` or `VBScript`).
    #[display("Script runtime error at line {line}, col {col}: {message}")]
    ScriptRuntimeError {
        /// 1-based line number where the error occurred.
        line: usize,
        /// 1-based column number where the error occurred.
        col: usize,
        /// Diagnostic error message or stack trace.
        message: String,
    },

    /// An error occurred during privileged worker IPC transport or frame verification.
    #[display("Worker IPC error: {reason}")]
    WorkerIpcError {
        /// Diagnostic detail.
        reason: String,
    },
}

impl From<std::io::Error> for Error {
    /// Converts a standard [`std::io::Error`] into an [`Error::Io`].
    ///
    /// # Arguments
    ///
    /// * `err` - The underlying standard I/O error.
    ///
    /// # Returns
    ///
    /// An [`Error::Io`] containing the string description of the I/O error.
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// A specialized [`Result`] type for MSI package operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests core error variant display formatting.
    #[test]
    fn test_error_display_core() {
        let err_io = Error::Io("file not found".to_string());
        assert_eq!(format!("{err_io}"), "I/O error: file not found");

        let err_arg = Error::InvalidArgument {
            argument: "name".to_string(),
            reason: "cannot be empty".to_string(),
        };
        assert_eq!(
            format!("{err_arg}"),
            "Invalid argument 'name': cannot be empty"
        );

        let err_table = Error::MissingTable {
            name: "Property".to_string(),
        };
        assert_eq!(format!("{err_table}"), "Missing required table: Property");

        let err_val = Error::Validation {
            element: "ProductCode".to_string(),
            reason: "must be a valid GUID".to_string(),
        };
        assert_eq!(
            format!("{err_val}"),
            "Validation error on ProductCode: must be a valid GUID"
        );

        let err_unsup = Error::Unsupported {
            name: "ARM64X".to_string(),
        };
        assert_eq!(format!("{err_unsup}"), "Unsupported feature: ARM64X");
    }

    /// Tests CFB container error variant display formatting.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_error_display_cfb() {
        let err_sig = Error::InvalidCfbSignature { found: [0; 8] };
        assert_eq!(
            format!("{err_sig}"),
            "Invalid CFB header signature: [00, 00, 00, 00, 00, 00, 00, 00]"
        );

        let err_clsid = Error::InvalidCfbClsid { found: [1; 16] };
        assert_eq!(
            format!("{err_clsid}"),
            "Invalid CFB header CLSID: [01, 01, 01, 01, 01, 01, 01, 01, 01, 01, 01, 01, 01, 01, 01, 01]"
        );

        let err_min_v = Error::InvalidCfbMinorVersion { found: 0x003F };
        assert_eq!(format!("{err_min_v}"), "Invalid CFB minor version: 0x003F");

        let err_maj_v = Error::InvalidCfbMajorVersion { found: 0x0002 };
        assert_eq!(format!("{err_maj_v}"), "Invalid CFB major version: 0x0002");

        let err_order = Error::InvalidCfbByteOrder { found: 0x1234 };
        assert_eq!(
            format!("{err_order}"),
            "Invalid CFB byte order marker: 0x1234"
        );

        let err_shift = Error::InvalidCfbSectorShift {
            major_version: 3,
            shift: 12,
        };
        assert_eq!(
            format!("{err_shift}"),
            "Invalid CFB sector shift 12 for major version 3"
        );

        let err_mini_shift = Error::InvalidCfbMiniSectorShift { shift: 7 };
        assert_eq!(
            format!("{err_mini_shift}"),
            "Invalid CFB mini sector shift: 0x0007"
        );

        let err_res = Error::InvalidCfbReserved {
            found: [1, 2, 3, 4, 5, 6],
        };
        assert_eq!(
            format!("{err_res}"),
            "Invalid CFB header reserved bytes: [01, 02, 03, 04, 05, 06]"
        );

        let err_dir_sec = Error::InvalidCfbDirectorySectors {
            major_version: 3,
            count: 5,
        };
        assert_eq!(
            format!("{err_dir_sec}"),
            "Invalid CFB directory sector count 5 for major version 3"
        );

        let err_cutoff = Error::InvalidCfbMiniStreamCutoff { cutoff: 2048 };
        assert_eq!(
            format!("{err_cutoff}"),
            "Invalid CFB mini stream cutoff size: 2048"
        );

        let err_sec = Error::InvalidSector {
            sector: 0x10,
            reason: "out of range".to_string(),
        };
        assert_eq!(
            format!("{err_sec}"),
            "Invalid sector index 0x00000010: out of range"
        );

        let err_cycle = Error::SectorChainCycle { sector: 0x20 };
        assert_eq!(
            format!("{err_cycle}"),
            "Cycle detected in CFB sector chain starting at sector 0x00000020"
        );

        let err_dir = Error::InvalidDirectoryEntry {
            index: 2,
            reason: "bad name".to_string(),
        };
        assert_eq!(
            format!("{err_dir}"),
            "Invalid directory entry at index 2: bad name"
        );

        let err_st_not = Error::StreamNotFound {
            name: "test".to_string(),
        };
        assert_eq!(
            format!("{err_st_not}"),
            "Stream 'test' was not found in container"
        );

        let err_dup = Error::DuplicateDirectoryEntry {
            name: "dup".to_string(),
        };
        assert_eq!(format!("{err_dup}"), "Duplicate directory entry name 'dup'");

        let err_corr = Error::CfbCorrupted {
            offset: 512,
            reason: "truncated sector".to_string(),
        };
        assert_eq!(
            format!("{err_corr}"),
            "CFB corruption at byte offset 512: truncated sector"
        );

        let err_name = Error::InvalidStreamName {
            name: "bad!name".to_string(),
            reason: "unsupported character".to_string(),
        };
        assert_eq!(
            format!("{err_name}"),
            "Invalid stream name 'bad!name': unsupported character"
        );

        let err_sz = Error::StreamSizeMismatch {
            expected: 100,
            actual: 90,
        };
        assert_eq!(
            format!("{err_sz}"),
            "Stream size mismatch: expected 100 bytes, got 90 bytes"
        );
    }

    /// Tests Cabinet format error variant display formatting.
    #[test]
    fn test_error_display_cab() {
        let err_sig = Error::InvalidCabSignature {
            found: [1, 2, 3, 4],
        };
        assert_eq!(
            format!("{err_sig}"),
            "Invalid Cabinet signature: [01, 02, 03, 04]"
        );

        let err_ver = Error::InvalidCabVersion { major: 2, minor: 0 };
        assert_eq!(format!("{err_ver}"), "Unsupported Cabinet version 2.0");

        let err_csum = Error::InvalidCabChecksum {
            expected: 0x1234,
            actual: 0x5678,
        };
        assert_eq!(
            format!("{err_csum}"),
            "Cabinet checksum mismatch: expected 0x00001234, actual 0x00005678"
        );

        let err_data = Error::InvalidCabData {
            reason: "truncated header".to_string(),
        };
        assert_eq!(
            format!("{err_data}"),
            "Invalid Cabinet data: truncated header"
        );

        let err_decomp = Error::DecompressionFailed {
            method: "MSZIP".to_string(),
            reason: "bad frame".to_string(),
        };
        assert_eq!(
            format!("{err_decomp}"),
            "Decompression failed using MSZIP: bad frame"
        );

        let err_comp = Error::CompressionFailed {
            method: "LZX".to_string(),
            reason: "window overflow".to_string(),
        };
        assert_eq!(
            format!("{err_comp}"),
            "Compression failed using LZX: window overflow"
        );

        let err_fnf = Error::CabinetFileNotFound {
            name: "test.dll".to_string(),
        };
        assert_eq!(
            format!("{err_fnf}"),
            "File 'test.dll' was not found in Cabinet archive"
        );
    }

    /// Tests Database and String Pool error variant display formatting.
    #[test]
    fn test_error_display_db() {
        let err_col = Error::InvalidColumnType { raw: 0xFFFF };
        assert_eq!(
            format!("{err_col}"),
            "Invalid MSI column type bitmask: 0xFFFF"
        );

        let err_sp = Error::InvalidStringPool {
            reason: "corrupted codepage".to_string(),
        };
        assert_eq!(
            format!("{err_sp}"),
            "Invalid MSI string pool: corrupted codepage"
        );

        let err_sql = Error::Sql {
            message: "syntax error".to_string(),
        };
        assert_eq!(format!("{err_sql}"), "SQL query error: syntax error");

        let err_idx = Error::StringPoolIndexOutOfBounds { index: 50, max: 40 };
        assert_eq!(
            format!("{err_idx}"),
            "String pool index 50 out of bounds (max 40)"
        );

        let err_sum = Error::InvalidSummaryInfo {
            reason: "bad property type".to_string(),
        };
        assert_eq!(
            format!("{err_sum}"),
            "Invalid Summary Information stream: bad property type"
        );

        let err_rec = Error::RecordLengthMismatch {
            expected: 4,
            actual: 3,
        };
        assert_eq!(
            format!("{err_rec}"),
            "Record length mismatch: expected 4, actual 3"
        );
    }

    /// Tests formatting of WiX-related error variants.
    #[test]
    fn test_error_display_wix() {
        let err_prep = Error::Preprocessor {
            line: 10,
            column: 5,
            message: "undefined variable $(var.FOO)".to_string(),
        };
        assert_eq!(
            format!("{err_prep}"),
            "WiX preprocessor error at 10:5: undefined variable $(var.FOO)"
        );

        let err_xml = Error::XmlParse {
            line: 12,
            column: 1,
            message: "unclosed tag <Product>".to_string(),
        };
        assert_eq!(
            format!("{err_xml}"),
            "WiX XML parse error at 12:1: unclosed tag <Product>"
        );

        let err_wix = Error::WixCompiler {
            element: "Component".to_string(),
            message: "missing Guid attribute".to_string(),
        };
        assert_eq!(
            format!("{err_wix}"),
            "WiX compiler error in element 'Component': missing Guid attribute"
        );

        let err_obj = Error::InvalidWixObject {
            reason: "bad magic signature".to_string(),
        };
        assert_eq!(
            format!("{err_obj}"),
            "Invalid WiX intermediate object file: bad magic signature"
        );

        let err_link = Error::WixLinker {
            message: "unresolved symbol Component:Comp1".to_string(),
        };
        assert_eq!(
            format!("{err_link}"),
            "WiX linker error: unresolved symbol Component:Comp1"
        );

        let err_ice = Error::IceValidation {
            ice: "ICE03".to_string(),
            message: "Table 'File' column 'Sequence' cannot be null".to_string(),
        };
        assert_eq!(
            format!("{err_ice}"),
            "ICE validation failure [ICE03]: Table 'File' column 'Sequence' cannot be null"
        );
    }

    /// Tests formatting of execution and transaction error variants.
    #[test]
    fn test_error_display_execution() {
        let err_exec = Error::ExecutionFailed {
            action: "InstallFiles".to_string(),
            return_code: 1603,
            message: "Access denied".to_string(),
        };
        assert_eq!(
            format!("{err_exec}"),
            "Action 'InstallFiles' failed with return code 1603: Access denied"
        );

        let err_rb = Error::RollbackFailed {
            action: "DeleteFile".to_string(),
            reason: "file is locked".to_string(),
        };
        assert_eq!(
            format!("{err_rb}"),
            "Rollback failed during action 'DeleteFile': file is locked"
        );

        let err_state = Error::TransactionStateMismatch {
            expected: "Prepared".to_string(),
            actual: "Uninitialized".to_string(),
        };
        assert_eq!(
            format!("{err_state}"),
            "Transaction state mismatch: expected 'Prepared', found 'Uninitialized'"
        );

        let err_cost = Error::DiskCostExceeded {
            volume: "C:\\".to_string(),
            required_bytes: 1_048_576,
            available_bytes: 524_288,
        };
        assert_eq!(
            format!("{err_cost}"),
            "Insufficient disk space on volume 'C:\\': required 1048576 bytes, available 524288 bytes"
        );

        let err_ca = Error::CustomActionFailed {
            action: "CheckPreReqs".to_string(),
            reason: "missing .NET runtime".to_string(),
        };
        assert_eq!(
            format!("{err_ca}"),
            "Custom action 'CheckPreReqs' failed: missing .NET runtime"
        );

        let err_script = Error::ScriptError {
            opcode: "InstallFile".to_string(),
            reason: "corrupted stream payload".to_string(),
        };
        assert_eq!(
            format!("{err_script}"),
            "Script processing error on opcode 'InstallFile': corrupted stream payload"
        );

        let err_ui = Error::UiError {
            dialog: "InstallDlg".to_string(),
            control: "NextButton".to_string(),
            reason: "invalid target event".to_string(),
        };
        assert_eq!(
            format!("{err_ui}"),
            "UI error in dialog 'InstallDlg' control 'NextButton': invalid target event"
        );

        let err_script_rt = Error::ScriptRuntimeError {
            line: 42,
            col: 10,
            message: "undefined identifier 'Foo'".to_string(),
        };
        assert_eq!(
            format!("{err_script_rt}"),
            "Script runtime error at line 42, col 10: undefined identifier 'Foo'"
        );

        let err_ipc = Error::WorkerIpcError {
            reason: "CRC32 frame checksum mismatch".to_string(),
        };
        assert_eq!(
            format!("{err_ipc}"),
            "Worker IPC error: CRC32 frame checksum mismatch"
        );
    }

    /// Tests standard I/O error conversion via `From`.
    #[test]
    fn test_io_error_conversion() {
        let std_err = std::io::Error::new(std::io::ErrorKind::NotFound, "disk read failure");
        let msi_err = Error::from(std_err);
        assert_eq!(msi_err, Error::Io("disk read failure".to_string()));
    }
}
