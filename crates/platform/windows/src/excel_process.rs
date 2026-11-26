//! Excel process discovery and identity helpers for the COM runner boundary.
//!
//! Responsibilities:
//! - inspect process names before Excel COM creation;
//! - capture a conservative PID/start-time/executable identity for cleanup;
//! - reject mapped network drives as local workbook paths.
//!
//! Boundaries:
//! - this module never starts or terminates a process;
//! - it never treats a PID alone as ownership evidence.

use std::io;
use std::path::{Path, PathBuf};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};
#[cfg(windows)]
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, ERROR_NO_MORE_FILES, HANDLE,
};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
#[cfg(windows)]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
#[cfg(windows)]
use windows::core::PCWSTR;

#[cfg(windows)]
use windows::Win32::Foundation::{FILETIME, STILL_ACTIVE};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
#[cfg(windows)]
use windows::core::PWSTR;

#[cfg(windows)]
const DRIVE_REMOTE: u32 = 4;
#[cfg(windows)]
const MAX_PROCESS_PATH_UNITS: usize = 32_768;
const MAX_CELL_ADDRESS_BYTES: usize = 32;
#[cfg(windows)]
const MAX_PROCESS_SNAPSHOT_ENTRIES: usize = 65_536;

/// Stable process ownership evidence captured after creating Excel.
#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelProcessIdentity {
    /// Process id observed from `Application.Hwnd`.
    pub process_id: u32,
    /// Creation time in 100 ns Windows FILETIME units.
    pub start_time_100ns: u64,
    /// Canonical executable path.
    pub executable_path: String,
}

/// One bounded entry from the system process snapshot.
#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessTreeEntry {
    /// Process id from Toolhelp.
    pub process_id: u32,
    /// Immediate parent process id from the same snapshot.
    pub parent_process_id: u32,
    /// Executable file name from the same snapshot.
    pub process_name: String,
}

/// Fails closed when any Excel process already exists.
#[cfg(windows)]
pub fn ensure_no_existing_excel_process() -> PlatformResult<()> {
    let process_ids = excel_process_ids()?;
    fail_if_existing_excel_process(&process_ids)
}

#[cfg(windows)]
fn fail_if_existing_excel_process(process_ids: &[u32]) -> PlatformResult<()> {
    if process_ids.is_empty() {
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::UserInteraction,
            format!(
                "existing EXCEL.EXE process detected before owned COM creation: {process_ids:?}"
            ),
        ))
    }
}

/// Returns all current `EXCEL.EXE` process ids.
#[cfg(windows)]
pub fn excel_process_ids() -> PlatformResult<Vec<u32>> {
    Ok(process_tree_snapshot()?
        .into_iter()
        .filter(|entry| entry.process_name.eq_ignore_ascii_case("EXCEL.EXE"))
        .map(|entry| entry.process_id)
        .collect())
}

/// Returns a hard-bounded snapshot of process ids, parent ids, and image names.
#[cfg(windows)]
pub fn process_tree_snapshot() -> PlatformResult<Vec<ProcessTreeEntry>> {
    // SAFETY: the snapshot request has no pointer inputs; the returned handle is
    // owned by SnapshotHandle and closed exactly once.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.map_err(|error| {
        process_error(format!(
            "CreateToolhelp32Snapshot failed: {}",
            error.message()
        ))
    })?;
    let snapshot = SnapshotHandle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>())
            .map_err(|_| process_error("PROCESSENTRY32W size does not fit u32"))?,
        ..PROCESSENTRY32W::default()
    };
    let mut entries = Vec::new();
    // SAFETY: the snapshot handle is live and entry points to initialized writable
    // storage for the duration of this synchronous call.
    match unsafe { Process32FirstW(snapshot.0, &raw mut entry) } {
        Ok(()) => {}
        Err(error) => {
            classify_first_process_error(&error)?;
            return Ok(entries);
        }
    }
    loop {
        if entries.len() >= MAX_PROCESS_SNAPSHOT_ENTRIES {
            return Err(process_error(format!(
                "process snapshot exceeded {MAX_PROCESS_SNAPSHOT_ENTRIES} entries"
            )));
        }
        entries.push(ProcessTreeEntry {
            process_id: entry.th32ProcessID,
            parent_process_id: entry.th32ParentProcessID,
            process_name: process_name(&entry),
        });
        // SAFETY: the snapshot handle remains live and entry is the same
        // initialized PROCESSENTRY32W buffer used by Process32FirstW.
        let next = unsafe { Process32NextW(snapshot.0, &raw mut entry) };
        match next {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
            Err(error) => {
                return Err(process_error(format!(
                    "Process32NextW failed: {}",
                    error.message()
                )));
            }
        }
    }
    Ok(entries)
}

/// Captures the exact identity of one live process.
#[cfg(windows)]
pub fn process_identity(process_id: u32) -> PlatformResult<Option<ExcelProcessIdentity>> {
    let Some(process) = open_process_handle(process_id, PROCESS_QUERY_LIMITED_INFORMATION)? else {
        return Ok(None);
    };
    let process = ProcessHandle(process);
    read_process_identity_from_handle(process.0, process_id)
}

/// Opens one process handle and distinguishes missing processes from failures.
#[cfg(windows)]
pub fn open_process_handle(
    process_id: u32,
    access: PROCESS_ACCESS_RIGHTS,
) -> PlatformResult<Option<HANDLE>> {
    // SAFETY: the caller supplies a closed access mask and the returned handle is
    // transferred to the caller for exactly one CloseHandle responsibility.
    match unsafe { OpenProcess(access, false, process_id) } {
        Ok(process) => Ok(Some(process)),
        Err(error) => {
            classify_open_process_error(process_id, &error)?;
            Ok(None)
        }
    }
}

/// Reads complete identity from an already-opened process handle.
#[cfg(windows)]
pub fn read_process_identity_from_handle(
    process: HANDLE,
    process_id: u32,
) -> PlatformResult<Option<ExcelProcessIdentity>> {
    let mut exit_code = 0_u32;
    // SAFETY: the process handle is live and exit_code points to writable storage
    // for the duration of this synchronous call.
    if let Err(error) = unsafe { GetExitCodeProcess(process, &raw mut exit_code) } {
        return Err(process_error(format!(
            "GetExitCodeProcess failed for pid {process_id}: {}",
            error.message()
        )));
    }
    if exit_code
        != u32::try_from(STILL_ACTIVE.0)
            .map_err(|_| process_error("STILL_ACTIVE does not fit u32"))?
    {
        return Ok(None);
    }
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all FILETIME outputs point to distinct live stack values and the
    // process handle grants query access.
    if let Err(error) = unsafe {
        GetProcessTimes(
            process,
            &raw mut creation,
            &raw mut exit,
            &raw mut kernel,
            &raw mut user,
        )
    } {
        return Err(process_error(format!(
            "GetProcessTimes failed for pid {process_id}: {}",
            error.message()
        )));
    }
    let mut path_buffer = vec![0_u16; MAX_PROCESS_PATH_UNITS];
    let mut path_length = u32::try_from(path_buffer.len())
        .map_err(|_| process_error("process path buffer length does not fit u32"))?;
    // SAFETY: the process handle is live, the path buffer is writable, and its
    // capacity is passed through a live u32 out-parameter.
    if let Err(error) = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path_buffer.as_mut_ptr()),
            &raw mut path_length,
        )
    } {
        return Err(process_error(format!(
            "QueryFullProcessImageNameW failed for pid {process_id}: {}",
            error.message()
        )));
    }
    let path_length = usize::try_from(path_length)
        .map_err(|_| process_error("process path length does not fit usize"))?;
    let executable_path = String::from_utf16_lossy(
        path_buffer
            .get(..path_length)
            .ok_or_else(|| process_error("process path length exceeded its buffer"))?,
    );
    Ok(Some(ExcelProcessIdentity {
        process_id,
        start_time_100ns: filetime_value(creation),
        executable_path,
    }))
}

#[cfg(windows)]
fn classify_open_process_error(
    process_id: u32,
    error: &windows::core::Error,
) -> PlatformResult<()> {
    if error.code() == ERROR_INVALID_PARAMETER.to_hresult() {
        return Ok(());
    }
    let code = if error.code() == ERROR_ACCESS_DENIED.to_hresult() {
        ErrorCode::PlatformPermission
    } else {
        ErrorCode::Fatal
    };
    Err(PlatformError::new(
        code,
        format!(
            "OpenProcess failed for pid {process_id}: {}",
            error.message()
        ),
    ))
}

/// Verifies that a live process still matches the captured identity fields.
#[cfg(windows)]
pub fn process_matches(
    process_id: u32,
    start_time_100ns: u64,
    executable_path: &str,
) -> PlatformResult<bool> {
    Ok(process_identity(process_id)?.is_some_and(|current| {
        current.start_time_100ns == start_time_100ns
            && current
                .executable_path
                .eq_ignore_ascii_case(executable_path)
    }))
}

/// Rejects mapped network drives while allowing ordinary local paths.
#[cfg(windows)]
pub fn ensure_local_drive(path: &Path) -> PlatformResult<()> {
    let path_text = path.to_string_lossy();
    let bytes = path_text.as_bytes();
    if bytes.len() < 3
        || bytes.get(1).is_none_or(|byte| *byte != b':')
        || bytes.get(2).is_none_or(|byte| *byte != b'\\')
    {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must be on a local drive",
        ));
    }
    let root = format!("{}\\", &path_text[..2]);
    let mut wide: Vec<u16> = root.encode_utf16().collect();
    wide.push(0);
    // SAFETY: wide is NUL-terminated and remains alive for this synchronous call.
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) };
    if drive_type == DRIVE_REMOTE {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must not use a mapped network drive",
        ));
    }
    Ok(())
}

/// Validates and canonicalizes one local `.xlsx` workbook path.
pub fn validate_workbook_path(path: &Path) -> PlatformResult<PathBuf> {
    if !path.is_absolute() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must be absolute",
        ));
    }
    if !has_xlsx_extension(path) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must use the .xlsx extension",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must not contain . or .. components",
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::NotFound {
            ErrorCode::TargetNotFound
        } else if error.kind() == io::ErrorKind::PermissionDenied {
            ErrorCode::PlatformPermission
        } else {
            ErrorCode::ToolInvalidArgs
        };
        PlatformError::new(
            code,
            format!("canonicalize Excel workbook path failed: {error}"),
        )
    })?;
    if !canonical.is_file() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must identify a regular file",
        ));
    }
    if !has_xlsx_extension(&canonical) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "canonical Excel workbook target must use the .xlsx extension",
        ));
    }
    let canonical_text = canonical.to_string_lossy();
    if canonical_text.starts_with(r"\\?\UNC\") {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must be local, not a UNC share",
        ));
    }
    let canonical = canonical_text
        .strip_prefix(r"\\?\")
        .map_or_else(|| canonical.clone(), PathBuf::from);
    #[cfg(windows)]
    ensure_local_drive(&canonical)?;
    Ok(canonical)
}

/// Validates one local temporary directory before any workbook fixture copy.
#[cfg(all(windows, test))]
pub fn validate_local_workbook_directory(path: &Path) -> PlatformResult<PathBuf> {
    if !path.is_absolute() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook directory must be absolute",
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            format!("canonicalize Excel workbook directory failed: {error}"),
        )
    })?;
    if !canonical.is_dir() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook directory must identify a directory",
        ));
    }
    let canonical_text = canonical.to_string_lossy();
    if canonical_text.starts_with(r"\\?\UNC\") {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook directory must be local, not a UNC share",
        ));
    }
    let canonical = canonical_text
        .strip_prefix(r"\\?\")
        .map_or_else(|| canonical.clone(), PathBuf::from);
    ensure_local_drive(&canonical)?;
    Ok(canonical)
}

/// Validates and normalizes one bounded A1 cell reference.
pub fn validate_cell_address(address: &str) -> PlatformResult<String> {
    let normalized = address.trim().to_ascii_uppercase();
    if normalized.is_empty()
        || normalized.len() > MAX_CELL_ADDRESS_BYTES
        || !is_single_a1_cell(&normalized)
    {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel cell address must be a bounded single A1 cell reference",
        ));
    }
    Ok(normalized)
}

fn has_xlsx_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx"))
}

fn is_single_a1_cell(address: &str) -> bool {
    let bytes = address.as_bytes();
    let mut position = 0;
    if bytes.get(position) == Some(&b'$') {
        position += 1;
    }
    let column_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_uppercase) {
        position += 1;
    }
    let column_length = position.saturating_sub(column_start);
    if !(1..=3).contains(&column_length) {
        return false;
    }
    let column = bytes
        .get(column_start..position)
        .unwrap_or_default()
        .iter()
        .fold(0_u32, |value, byte| {
            value
                .saturating_mul(26)
                .saturating_add(u32::from(byte.saturating_sub(b'A') + 1))
        });
    if !(1..=16_384).contains(&column) {
        return false;
    }
    if bytes.get(position) == Some(&b'$') {
        position += 1;
    }
    let row_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        position += 1;
    }
    let row_length = position.saturating_sub(row_start);
    if !(1..=7).contains(&row_length)
        || bytes.get(row_start) == Some(&b'0')
        || position != bytes.len()
    {
        return false;
    }
    bytes
        .get(row_start..position)
        .and_then(|digits| std::str::from_utf8(digits).ok())
        .and_then(|digits| digits.parse::<u32>().ok())
        .is_some_and(|value| (1..=1_048_576).contains(&value))
}

#[cfg(windows)]
fn process_name(entry: &PROCESSENTRY32W) -> String {
    let name = entry
        .szExeFile
        .iter()
        .take_while(|unit| **unit != 0)
        .copied()
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&name)
}

#[cfg(windows)]
fn classify_first_process_error(error: &windows::core::Error) -> PlatformResult<()> {
    if error.code() == ERROR_NO_MORE_FILES.to_hresult() {
        Ok(())
    } else {
        Err(process_error(format!(
            "Process32FirstW failed: {}",
            error.message()
        )))
    }
}

#[cfg(windows)]
fn filetime_value(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

#[cfg(windows)]
fn process_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, message)
}

#[cfg(windows)]
struct SnapshotHandle(HANDLE);

#[cfg(windows)]
impl Drop for SnapshotHandle {
    fn drop(&mut self) {
        // SAFETY: this handle is owned by the wrapper and has not been closed.
        if let Err(_error) = unsafe { CloseHandle(self.0) } {
            // Process discovery remains a read-only best effort. The caller sees
            // snapshot/iteration failures; a handle close failure has no safe retry.
        }
    }
}

#[cfg(windows)]
struct ProcessHandle(HANDLE);

#[cfg(windows)]
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: this handle is owned by the wrapper and has not been closed.
        if let Err(_error) = unsafe { CloseHandle(self.0) } {
            // The identity result is already determined; no safe recovery exists.
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn test_existing_excel_processes_fail_closed_before_com_creation() {
        fail_if_existing_excel_process(&[]).expect("empty process list");
        let error = fail_if_existing_excel_process(&[42]).expect_err("existing process must fail");
        assert_eq!(error.code(), ErrorCode::UserInteraction);
    }

    #[test]
    fn test_process_snapshot_first_error_is_empty_only_for_no_more_files() {
        let no_more = windows::core::Error::from_hresult(ERROR_NO_MORE_FILES.to_hresult());
        classify_first_process_error(&no_more).expect("empty snapshot");
        let denied = windows::core::Error::from_hresult(windows::Win32::Foundation::E_ACCESSDENIED);
        let error = classify_first_process_error(&denied).expect_err("enumeration failure");
        assert_eq!(error.code(), ErrorCode::Fatal);
    }

    #[test]
    fn test_open_process_error_classification_is_explicit() {
        let missing = windows::core::Error::from_hresult(ERROR_INVALID_PARAMETER.to_hresult());
        classify_open_process_error(42, &missing).expect("missing process");
        let denied = windows::core::Error::from_hresult(ERROR_ACCESS_DENIED.to_hresult());
        let denied = classify_open_process_error(42, &denied).expect_err("access denied");
        assert_eq!(denied.code(), ErrorCode::PlatformPermission);
        let failed = windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL);
        let failed = classify_open_process_error(42, &failed).expect_err("unexpected error");
        assert_eq!(failed.code(), ErrorCode::Fatal);
    }
}
