//! Execution Engine and `msiexec` CLI Implementation.
//!
//! Grounded directly in official Microsoft Windows Installer execution specifications:
//! - Command-line parsing matching `msiexec.exe` actions, UI levels, and logging modes.
//! - Property scoping, formatted string expansion, and rich condition expression evaluation.

pub mod cli_parser;
pub mod costing;
pub mod custom_action;
pub mod native_action;
pub mod properties;
pub mod script;
pub mod script_engine;
pub mod transaction;
pub mod worker;

pub use cli_parser::{
    ActionMode, AdvertiseScope, LoggingOptions, MsiExecOptions, RepairFlags, UiLevel,
};
pub use costing::{DiskCostEngine, VolumeCost, DEFAULT_CLUSTER_SIZE};
pub use custom_action::{
    global_handles, CustomActionDefinition, CustomActionExecutionMode, CustomActionExecutor,
    CustomActionSourceType, HandleManager, InScriptMode, InstallSession, MsiCloseHandle,
    MsiCreateRecord, MsiDoActionW, MsiEvaluateConditionW, MsiGetActiveDatabase, MsiGetPropertyW,
    MsiProcessMessage, MsiRecordGetInteger, MsiRecordGetStringW, MsiRecordSetInteger,
    MsiRecordSetStringW, MsiSetPropertyW, ERROR_FUNCTION_FAILED, ERROR_INVALID_HANDLE,
    ERROR_INVALID_PARAMETER, ERROR_MORE_DATA, MSICONDITION_ERROR, MSICONDITION_FALSE,
    MSICONDITION_NONE, MSICONDITION_TRUE, MSIDB_CUSTOM_ACTION_TYPE_ASYNC,
    MSIDB_CUSTOM_ACTION_TYPE_CLIENT_REPEAT, MSIDB_CUSTOM_ACTION_TYPE_COMMIT,
    MSIDB_CUSTOM_ACTION_TYPE_CONTINUE, MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY,
    MSIDB_CUSTOM_ACTION_TYPE_DLL, MSIDB_CUSTOM_ACTION_TYPE_EXE,
    MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE, MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_DLL,
    MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_EXE, MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT,
    MSIDB_CUSTOM_ACTION_TYPE_JSCRIPT, MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE,
    MSIDB_CUSTOM_ACTION_TYPE_ONCE_PER_PROCESS, MSIDB_CUSTOM_ACTION_TYPE_PROPERTY,
    MSIDB_CUSTOM_ACTION_TYPE_ROLLBACK, MSIDB_CUSTOM_ACTION_TYPE_TEXT_DATA,
    MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT, MSIHANDLE,
};
pub use native_action::{
    MsiCustomActionFn, NativeLibraryLoader, SubprocessResult, SubprocessRunner,
    DEFAULT_ACTION_TIMEOUT_MS, ERROR_SUCCESS_REBOOT_REQUIRED,
};
pub use properties::{EvaluationContext, InstallState};
pub use script::{InstallScript, RollbackOp, RollbackScript, ScriptOp, IBS_MAGIC, RBS_MAGIC};
pub use script_engine::{
    JScriptEngine, JsValue, ScriptDatabase, ScriptEngine, ScriptLanguage, ScriptSession,
    ScriptValue, SessionMessage, VBScriptEngine, Variant, VbErr, DEFAULT_SCRIPT_FUEL,
    MSICONDITION_ERROR_CODE, MSICONDITION_FALSE_CODE, MSICONDITION_NONE_CODE,
    MSICONDITION_TRUE_CODE, MSIRUNMODE_ADMIN, MSIRUNMODE_ADVERTISE, MSIRUNMODE_CABPATH,
    MSIRUNMODE_COMMIT, MSIRUNMODE_LOGENABLED, MSIRUNMODE_MAINTENANCE, MSIRUNMODE_OPERATIONS,
    MSIRUNMODE_REBOOTATEND, MSIRUNMODE_REBOOTNOW, MSIRUNMODE_ROLLBACK, MSIRUNMODE_ROLLBACKENABLED,
    MSIRUNMODE_SCHEDULED, MSIRUNMODE_SOURCESHORTNAMES, MSIRUNMODE_TARGETSHORTNAMES,
    MSIRUNMODE_WINDOWS9X, MSIRUNMODE_ZAWENABLED,
};
pub use transaction::{
    Committed, Executed, Prepared, RolledBack, Transaction, Uninitialized, WorkerContext,
    WorkerIpcMessage, ERROR_INSTALL_FAILURE, ERROR_SUCCESS,
};
pub use worker::{
    compute_crc32, CommandSpec, EscalationMethod, InstalledFileRecord, IpcFrame, IpcSocketEndpoint,
    LiveWorkerExecutor, PrivilegeEscalator, WorkerMessage, IPC_FRAME_HEADER_SIZE, IPC_FRAME_MAGIC,
};
