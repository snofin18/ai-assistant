//! Platform-private durable ownership journal for the Excel COM runner.
//!
//! Responsibilities:
//! - render a bounded schema-versioned journal from the complete owned identity;
//! - publish each lifecycle phase through one unique target path;
//! - keep failed cleanup journals on disk for later incident recovery.
//!
//! Boundaries:
//! - this module does not discover, terminate, or reap processes;
//! - it exposes no public API and does not serialize COM handles;
//! - watchdog consumption remains a follow-up task.
//!
//! Invariants:
//! 1. a journal contains PID, creation FILETIME, and executable path together;
//! 2. temporary and target paths are unique and never replace an existing file;
//! 3. a journal is deleted only after the runner reports successful cleanup.
//!
//! Related documents: ADR-0108 D8/D9, TASK-410, TASK-411.

use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
#[cfg(windows)]
use windows::core::PCWSTR;

use super::ExcelOwnedProcessIdentity;

const JOURNAL_SCHEMA_VERSION: u32 = 1;
const MAX_JOURNAL_ROOT_BYTES: usize = 1_024;
const MAX_JOURNAL_FILE_NAME_BYTES: usize = 192;
const MAX_JOURNAL_CONTENT_BYTES: usize = 131_072;
const MAX_OWNED_PATH_BYTES: usize = 32_768;
const MAX_ACTIVE_JOURNALS_PER_RUNNER: usize = 8;
#[cfg(all(windows, not(test)))]
const JOURNAL_DIRECTORY_NAME: &str = "excel-ownership-journal";
#[cfg(all(windows, not(test)))]
const APPLICATION_DIRECTORY_NAME: &str = "ai-assistant";
static NEXT_JOURNAL_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[derive(Debug)]
struct TestJournalRoot {
    path: PathBuf,
}

#[cfg(test)]
impl Drop for TestJournalRoot {
    fn drop(&mut self) {
        if let Err(_error) = fs::remove_dir_all(&self.path) {
            // Test cleanup is best effort after all assertions have completed.
        }
    }
}

/// Ordered runner phases that may publish ownership evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExcelOwnershipJournalPhase {
    OwnershipCaptured,
    Opening,
    Closing,
    Cleanup,
}

/// Inputs for one uniquely named journal publication.
#[derive(Clone, Copy)]
pub(super) struct ExcelOwnershipJournalPublication<'a> {
    pub(super) phase: ExcelOwnershipJournalPhase,
    pub(super) runner_id: u64,
    pub(super) session_id: u64,
    pub(super) workbook_path: Option<&'a Path>,
    pub(super) identity: &'a ExcelOwnedProcessIdentity,
}

impl ExcelOwnershipJournalPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::OwnershipCaptured => "ownership-captured",
            Self::Opening => "opening",
            Self::Closing => "closing",
            Self::Cleanup => "cleanup",
        }
    }
}

/// Repository-external directory that owns all production journal files.
#[derive(Debug, Clone)]
pub(super) struct ExcelOwnershipJournalStore {
    root: PathBuf,
    #[cfg(test)]
    _cleanup_root: Option<Arc<TestJournalRoot>>,
}

impl ExcelOwnershipJournalStore {
    /// Resolves and creates the production journal root from `LOCALAPPDATA`.
    #[cfg(all(windows, not(test)))]
    pub(super) fn from_environment() -> PlatformResult<Self> {
        let local_app_data = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            journal_error("LOCALAPPDATA is unavailable for the Excel ownership journal")
        })?;
        let root = PathBuf::from(local_app_data)
            .join(APPLICATION_DIRECTORY_NAME)
            .join(JOURNAL_DIRECTORY_NAME);
        Self::create(root)
    }

    /// Uses a test-owned root without touching the user's local application data.
    #[cfg(test)]
    pub(super) fn for_tests() -> PlatformResult<Self> {
        let sequence = NEXT_JOURNAL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "assistant-task411-tests-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).map_err(|error| {
            journal_io_error(
                "create test Excel ownership journal directory",
                &root,
                &error,
            )
        })?;
        let cleanup_root = Arc::new(TestJournalRoot { path: root.clone() });
        Ok(Self {
            root,
            _cleanup_root: Some(cleanup_root),
        })
    }

    #[cfg(test)]
    pub(super) fn for_tests_with_root(root: PathBuf) -> PlatformResult<Self> {
        Self::existing(root)
    }

    #[cfg(test)]
    pub(super) const fn for_tests_unchecked(root: PathBuf) -> Self {
        Self {
            root,
            _cleanup_root: None,
        }
    }

    #[cfg(all(windows, not(test)))]
    fn create(root: PathBuf) -> PlatformResult<Self> {
        validate_root(&root)?;
        fs::create_dir_all(&root).map_err(|error| {
            journal_io_error("create Excel ownership journal directory", &root, &error)
        })?;
        Self::existing(root)
    }

    fn existing(root: PathBuf) -> PlatformResult<Self> {
        validate_root(&root)?;
        let metadata = fs::metadata(&root).map_err(|error| {
            journal_io_error("inspect Excel ownership journal directory", &root, &error)
        })?;
        if !metadata.is_dir() {
            return Err(journal_error(format!(
                "Excel ownership journal root is not a directory: {}",
                root.display()
            )));
        }
        Ok(Self {
            root,
            #[cfg(test)]
            _cleanup_root: None,
        })
    }

    /// Publishes one uniquely named phase journal.
    pub(super) fn publish(
        &self,
        publication: ExcelOwnershipJournalPublication<'_>,
    ) -> PlatformResult<ExcelOwnershipJournal> {
        let record = JournalRecord::new(
            publication.phase,
            publication.runner_id,
            publication.session_id,
            publication.workbook_path,
            publication.identity.clone(),
        )?;
        let sequence = NEXT_JOURNAL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        if sequence == 0 {
            return Err(journal_error("Excel ownership journal sequence overflowed"));
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| journal_error(format!("system clock failed: {error}")))?
            .as_nanos();
        let file_name = format!(
            "excel-ownership-{}-{:016x}-{:016x}-{}-{sequence:016x}-{timestamp:032x}.json",
            std::process::id(),
            publication.runner_id,
            publication.session_id,
            publication.phase.as_str()
        );
        let target_path = self.root.join(file_name);
        self.publish_at(&target_path, &record)
    }

    fn publish_at(
        &self,
        target_path: &Path,
        record: &JournalRecord,
    ) -> PlatformResult<ExcelOwnershipJournal> {
        validate_target_path(&self.root, target_path)?;
        let content = record.render()?;
        let temporary_path = temporary_path(target_path)?;
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| {
                journal_io_error(
                    "create temporary Excel ownership journal",
                    &temporary_path,
                    &error,
                )
            })?;
        let write_result = temporary_file
            .write_all(content.as_bytes())
            .and_then(|()| temporary_file.sync_all());
        drop(temporary_file);
        if let Err(error) = write_result {
            return Err(cleanup_temporary_after_error(
                journal_io_error(
                    "write and sync temporary Excel ownership journal",
                    &temporary_path,
                    &error,
                ),
                &temporary_path,
            ));
        }
        if let Err(error) = publish_without_replacement(&temporary_path, target_path) {
            return Err(cleanup_temporary_after_error(
                journal_io_error("publish Excel ownership journal", target_path, &error),
                &temporary_path,
            ));
        }
        Ok(ExcelOwnershipJournal {
            path: target_path.to_path_buf(),
            phase: record.phase,
            session_id: record.session_id,
            identity: record.identity.clone(),
            is_active: true,
        })
    }
}

/// One durable journal file owned by the runner state.
#[derive(Debug)]
pub(super) struct ExcelOwnershipJournal {
    path: PathBuf,
    phase: ExcelOwnershipJournalPhase,
    session_id: u64,
    identity: ExcelOwnedProcessIdentity,
    is_active: bool,
}

impl ExcelOwnershipJournal {
    /// Rejects evidence that no longer describes the complete owned identity.
    pub(super) fn require_identity(
        &self,
        identity: &ExcelOwnedProcessIdentity,
    ) -> PlatformResult<()> {
        if self.identity == *identity {
            Ok(())
        } else {
            Err(PlatformError::new(
                ErrorCode::TargetNotFound,
                format!(
                    "Excel ownership journal identity changed: {}",
                    self.path.display()
                ),
            ))
        }
    }

    /// Removes one journal only after the runner reports cleanup success.
    pub(super) fn complete(&mut self) -> PlatformResult<()> {
        remove_journal_file(&self.path)?;
        self.is_active = false;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(test)]
    pub(super) const fn phase(&self) -> ExcelOwnershipJournalPhase {
        self.phase
    }
}

impl Drop for ExcelOwnershipJournal {
    fn drop(&mut self) {
        if self.is_active {
            // Retaining the file is the observable failure behavior. A later
            // watchdog or incident workflow consumes it instead of guessing.
        }
    }
}

/// Tracks a hard-bounded set of active journal files for one runner.
#[derive(Debug, Default)]
pub(super) struct ExcelOwnershipJournals {
    journals: Vec<ExcelOwnershipJournal>,
}

impl ExcelOwnershipJournals {
    /// Publishes one phase and enforces the per-runner active-file bound.
    pub(super) fn publish(
        &mut self,
        store: &ExcelOwnershipJournalStore,
        publication: ExcelOwnershipJournalPublication<'_>,
    ) -> PlatformResult<()> {
        if self.journals.len() >= MAX_ACTIVE_JOURNALS_PER_RUNNER {
            return Err(journal_error(format!(
                "Excel ownership journal count exceeded the per-runner maximum of {MAX_ACTIVE_JOURNALS_PER_RUNNER}"
            )));
        }
        let journal = store.publish(publication)?;
        self.journals.push(journal);
        Ok(())
    }

    /// Verifies every active file still carries the current complete identity.
    pub(super) fn require_identity(
        &self,
        identity: &ExcelOwnedProcessIdentity,
    ) -> PlatformResult<()> {
        for journal in &self.journals {
            journal.require_identity(identity)?;
        }
        Ok(())
    }

    /// Deletes all active files after successful cleanup.
    pub(super) fn complete(&mut self) -> PlatformResult<()> {
        let mut first_error = None;
        for journal in &mut self.journals {
            if let Err(error) = journal.complete()
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        self.journals.retain(|journal| journal.is_active);
        first_error.map_or(Ok(()), Err)
    }

    /// Deletes opening/closing files after one workbook session closes.
    pub(super) fn complete_session_closure(&mut self, session_id: u64) -> PlatformResult<()> {
        let mut first_error = None;
        for journal in &mut self.journals {
            if journal.session_id == session_id
                && matches!(
                    journal.phase,
                    ExcelOwnershipJournalPhase::Opening | ExcelOwnershipJournalPhase::Closing
                )
                && let Err(error) = journal.complete()
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        self.journals.retain(|journal| journal.is_active);
        first_error.map_or(Ok(()), Err)
    }

    #[cfg(test)]
    pub(super) const fn len(&self) -> usize {
        self.journals.len()
    }
}

#[derive(Debug)]
struct JournalRecord {
    phase: ExcelOwnershipJournalPhase,
    coordinator_process_id: u32,
    runner_id: u64,
    session_id: u64,
    workbook_path: Option<String>,
    identity: ExcelOwnedProcessIdentity,
}

impl JournalRecord {
    fn new(
        phase: ExcelOwnershipJournalPhase,
        runner_id: u64,
        session_id: u64,
        workbook_path: Option<&Path>,
        identity: ExcelOwnedProcessIdentity,
    ) -> PlatformResult<Self> {
        if runner_id == 0 {
            return Err(journal_error("Excel ownership journal runner id is zero"));
        }
        let workbook_path = workbook_path
            .map(|path| {
                let text = path.to_str().ok_or_else(|| {
                    journal_error("Excel ownership journal workbook path is not valid UTF-8")
                })?;
                validate_owned_path("workbook", text)?;
                Ok::<String, PlatformError>(text.to_owned())
            })
            .transpose()?;
        validate_owned_path("executable", identity.executable_path())?;
        Ok(Self {
            phase,
            coordinator_process_id: std::process::id(),
            runner_id,
            session_id,
            workbook_path,
            identity,
        })
    }

    fn render(&self) -> PlatformResult<String> {
        let workbook_path = self
            .workbook_path
            .as_deref()
            .map_or_else(|| "null".to_owned(), json_string);
        let content = format!(
            "{{\n  \"schema_version\": {JOURNAL_SCHEMA_VERSION},\n  \"coordinator_process_id\": {},\n  \"runner_id\": {},\n  \"session_id\": {},\n  \"phase\": {},\n  \"process_name\": \"EXCEL\",\n  \"process_id\": {},\n  \"start_time_100ns\": {},\n  \"executable_path\": {},\n  \"workbook_path\": {workbook_path}\n}}\n",
            self.coordinator_process_id,
            self.runner_id,
            self.session_id,
            json_string(self.phase.as_str()),
            self.identity.process_id(),
            self.identity.start_time_100ns(),
            json_string(self.identity.executable_path())
        );
        if content.len() > MAX_JOURNAL_CONTENT_BYTES {
            return Err(journal_error(format!(
                "Excel ownership journal exceeds {MAX_JOURNAL_CONTENT_BYTES} bytes"
            )));
        }
        Ok(content)
    }
}

fn validate_root(root: &Path) -> PlatformResult<()> {
    if !root.is_absolute() {
        return Err(journal_error(
            "Excel ownership journal root must be absolute",
        ));
    }
    let root_text = root
        .to_str()
        .ok_or_else(|| journal_error("Excel ownership journal root is not valid UTF-8"))?;
    if root_text.len() > MAX_JOURNAL_ROOT_BYTES {
        return Err(journal_error(format!(
            "Excel ownership journal root exceeds {MAX_JOURNAL_ROOT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_target_path(root: &Path, target_path: &Path) -> PlatformResult<()> {
    if !target_path.is_absolute() || target_path.parent() != Some(root) {
        return Err(journal_error(
            "Excel ownership journal target must be a direct child of the journal root",
        ));
    }
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| journal_error("Excel ownership journal file name is not valid UTF-8"))?;
    if file_name.len() > MAX_JOURNAL_FILE_NAME_BYTES
        || !file_name.starts_with("excel-ownership-")
        || !Path::new(file_name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        || !file_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(journal_error(
            "Excel ownership journal file name violates the bounded naming contract",
        ));
    }
    Ok(())
}

fn validate_owned_path(kind: &str, value: &str) -> PlatformResult<()> {
    if value.is_empty() || value.len() > MAX_OWNED_PATH_BYTES {
        return Err(journal_error(format!(
            "Excel ownership journal {kind} path must be 1..={MAX_OWNED_PATH_BYTES} bytes"
        )));
    }
    Ok(())
}

fn temporary_path(target_path: &Path) -> PlatformResult<PathBuf> {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| journal_error("Excel ownership journal target has no UTF-8 file name"))?;
    Ok(target_path.with_file_name(format!("{file_name}.tmp")))
}

fn publish_without_replacement(temporary_path: &Path, target_path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        let temporary = wide_path(temporary_path)?;
        let target = wide_path(target_path)?;
        // SAFETY: both paths are NUL-terminated UTF-16 buffers that remain alive
        // for this synchronous MoveFileExW call. Omitting REPLACE_EXISTING makes
        // the publish fail rather than overwrite another process's journal.
        unsafe {
            MoveFileExW(
                PCWSTR(temporary.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|error| io::Error::other(format!("MoveFileExW failed: {}", error.message())))
    }
    #[cfg(not(windows))]
    {
        fs::hard_link(temporary_path, target_path)?;
        fs::remove_file(temporary_path)
    }
}

#[cfg(windows)]
fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let text = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Excel ownership journal path is not UTF-8",
        )
    })?;
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    Ok(wide)
}

fn cleanup_temporary_after_error(error: PlatformError, temporary_path: &Path) -> PlatformError {
    match fs::remove_file(temporary_path) {
        Ok(()) => error,
        Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => error,
        Err(cleanup_error) => PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "{error}; temporary Excel ownership journal cleanup failed for {}: {cleanup_error}",
                temporary_path.display()
            ),
        ),
    }
}

fn remove_journal_file(path: &Path) -> PlatformResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(journal_io_error(
            "remove completed Excel ownership journal",
            path,
            &error,
        )),
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1F}' => {
                if write!(output, "\\u{:04x}", u32::from(character)).is_err() {
                    output.push('\u{FFFD}');
                }
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn journal_io_error(context: &str, path: &Path, error: &io::Error) -> PlatformError {
    PlatformError::new(
        ErrorCode::Fatal,
        format!("{context} failed for {}: {error}", path.display()),
    )
}

fn journal_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, message)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::cdp::JsonValue;
    use std::sync::atomic::AtomicU64;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    pub(super) struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "assistant-task411-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create test directory");
            Self { path }
        }

        fn cleanup(self) {
            fs::remove_dir_all(&self.path).expect("clean test directory");
        }
    }

    pub(super) fn identity(process_id: u32) -> ExcelOwnedProcessIdentity {
        ExcelOwnedProcessIdentity::new(
            process_id,
            u64::from(process_id) * 10,
            r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE".to_owned(),
        )
        .expect("test identity")
    }

    fn record(phase: ExcelOwnershipJournalPhase) -> JournalRecord {
        JournalRecord::new(
            phase,
            7,
            3,
            Some(Path::new(r"C:\data\book.xlsx")),
            identity(42),
        )
        .expect("journal record")
    }

    pub(super) fn publication<'a>(
        phase: ExcelOwnershipJournalPhase,
        session_id: u64,
        workbook_path: Option<&'a Path>,
        identity: &'a ExcelOwnedProcessIdentity,
    ) -> ExcelOwnershipJournalPublication<'a> {
        ExcelOwnershipJournalPublication {
            phase,
            runner_id: 1,
            session_id,
            workbook_path,
            identity,
        }
    }

    #[test]
    fn test_journal_publish_is_unique_and_bounded() {
        let directory = TestDirectory::new();
        let store = ExcelOwnershipJournalStore::existing(directory.path.clone()).expect("store");
        let mut first = store
            .publish(publication(
                ExcelOwnershipJournalPhase::OwnershipCaptured,
                0,
                None,
                &identity(42),
            ))
            .expect("first journal");
        let mut second = store
            .publish(publication(
                ExcelOwnershipJournalPhase::OwnershipCaptured,
                0,
                None,
                &identity(42),
            ))
            .expect("second journal");
        assert_ne!(first.path(), second.path());
        let content = fs::read_to_string(first.path()).expect("journal content");
        assert!(content.contains("\"schema_version\": 1"));
        assert!(content.contains("\"phase\": \"ownership-captured\""));
        assert!(content.contains("\"process_id\": 42"));
        assert!(content.contains("\"start_time_100ns\": 420"));
        first.complete().expect("complete first");
        second.complete().expect("complete second");
        directory.cleanup();
    }

    #[test]
    fn test_journal_io_failure_and_non_directory_root_fail_closed() {
        let directory = TestDirectory::new();
        let file_root = directory.path.join("not-a-directory");
        fs::write(&file_root, b"occupied").expect("write root file");
        let error = ExcelOwnershipJournalStore::existing(file_root).expect_err("file root");
        assert_eq!(error.code(), ErrorCode::Fatal);

        let missing = directory.path.join("missing");
        let store = ExcelOwnershipJournalStore::for_tests_unchecked(missing);
        let error = store
            .publish(publication(
                ExcelOwnershipJournalPhase::Cleanup,
                1,
                None,
                &identity(42),
            ))
            .expect_err("missing root");
        assert_eq!(error.code(), ErrorCode::Fatal);
        directory.cleanup();
    }

    #[test]
    fn test_journal_existing_target_and_temporary_collision_fail_closed() {
        let directory = TestDirectory::new();
        let store = ExcelOwnershipJournalStore::existing(directory.path.clone()).expect("store");
        let target = directory.path.join("excel-ownership-existing.json");
        fs::write(&target, b"existing").expect("existing target");
        let error = store
            .publish_at(&target, &record(ExcelOwnershipJournalPhase::Cleanup))
            .expect_err("existing target");
        assert_eq!(error.code(), ErrorCode::Fatal);
        assert_eq!(fs::read(&target).expect("target remains"), b"existing");
        assert!(
            !directory
                .path
                .join("excel-ownership-existing.json.tmp")
                .exists()
        );

        let target_with_temporary = directory.path.join("excel-ownership-temporary.json");
        fs::write(
            target_with_temporary.with_file_name("excel-ownership-temporary.json.tmp"),
            b"temporary",
        )
        .expect("temporary collision");
        let error = store
            .publish_at(
                &target_with_temporary,
                &record(ExcelOwnershipJournalPhase::Cleanup),
            )
            .expect_err("temporary collision");
        assert_eq!(error.code(), ErrorCode::Fatal);
        directory.cleanup();
    }

    #[test]
    fn test_journal_rejects_stale_identity_and_oversized_content() {
        let directory = TestDirectory::new();
        let store = ExcelOwnershipJournalStore::existing(directory.path.clone()).expect("store");
        let mut journal = store
            .publish(publication(
                ExcelOwnershipJournalPhase::Cleanup,
                1,
                Some(Path::new(r"C:\data\book.xlsx")),
                &identity(42),
            ))
            .expect("journal");
        journal
            .require_identity(&identity(42))
            .expect("matching identity");
        let stale = journal.require_identity(&identity(43)).expect_err("stale");
        assert_eq!(stale.code(), ErrorCode::TargetNotFound);
        journal.complete().expect("complete");

        let oversized = ExcelOwnedProcessIdentity::new(
            44,
            440,
            "x".repeat(MAX_OWNED_PATH_BYTES.saturating_add(1)),
        )
        .expect("complete oversized identity");
        let error = JournalRecord::new(ExcelOwnershipJournalPhase::Cleanup, 1, 1, None, oversized)
            .expect_err("oversized identity");
        assert_eq!(error.code(), ErrorCode::Fatal);
        directory.cleanup();
    }

    #[test]
    fn test_journal_retention_and_bounded_collection_cleanup() {
        let directory = TestDirectory::new();
        let store = ExcelOwnershipJournalStore::existing(directory.path.clone()).expect("store");
        let retained = store
            .publish(publication(
                ExcelOwnershipJournalPhase::Cleanup,
                1,
                None,
                &identity(42),
            ))
            .expect("retained journal");
        let retained_path = retained.path().to_path_buf();
        assert_eq!(retained.phase(), ExcelOwnershipJournalPhase::Cleanup);
        drop(retained);
        assert!(retained_path.exists());
        fs::remove_file(retained_path).expect("remove retained journal");

        let mut journals = ExcelOwnershipJournals::default();
        for index in 0..MAX_ACTIVE_JOURNALS_PER_RUNNER {
            let phase = match index {
                0 => ExcelOwnershipJournalPhase::OwnershipCaptured,
                1 => ExcelOwnershipJournalPhase::Opening,
                2 => ExcelOwnershipJournalPhase::Closing,
                _ => ExcelOwnershipJournalPhase::Cleanup,
            };
            journals
                .publish(&store, publication(phase, 1, None, &identity(42)))
                .expect("publish phase");
        }
        assert_eq!(journals.len(), MAX_ACTIVE_JOURNALS_PER_RUNNER);
        let overflow = journals
            .publish(
                &store,
                publication(ExcelOwnershipJournalPhase::Cleanup, 1, None, &identity(42)),
            )
            .expect_err("bounded collection");
        assert_eq!(overflow.code(), ErrorCode::Fatal);
        journals
            .require_identity(&identity(42))
            .expect("matching identity");
        journals.complete().expect("cleanup collection");
        assert_eq!(journals.len(), 0);
        directory.cleanup();
    }

    #[test]
    fn test_test_store_uses_private_directory_and_removes_it() {
        let store = ExcelOwnershipJournalStore::for_tests().expect("test store");
        let root = store.root.clone();
        assert_ne!(root, std::env::temp_dir());
        assert!(root.is_dir());
        drop(store);
        assert!(!root.exists());
    }

    #[cfg(windows)]
    #[test]
    fn test_journal_cleanup_failure_retains_active_file() {
        let directory = TestDirectory::new();
        let store = ExcelOwnershipJournalStore::existing(directory.path.clone()).expect("store");
        let mut journal = store
            .publish(publication(
                ExcelOwnershipJournalPhase::Cleanup,
                1,
                None,
                &identity(42),
            ))
            .expect("journal");
        let journal_path = journal.path().to_path_buf();
        fs::remove_file(&journal_path).expect("remove journal file");
        fs::create_dir(&journal_path).expect("replace journal with directory");
        let error = journal.complete().expect_err("cleanup failure");
        assert_eq!(error.code(), ErrorCode::Fatal);
        assert!(journal_path.exists());

        fs::remove_dir(&journal_path).expect("remove blocking directory");
        directory.cleanup();
    }

    #[path = "../excel_ownership_journal_json_tests.rs"]
    mod json_tests;
}
