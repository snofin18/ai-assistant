//! Test-only filesystem shadow-copy port for Excel T2.2 L1 anchors.
//! Copies exact workbook bytes into a repository-external owned root, binds them to an undo
//! `ShadowCopy`, and restores only after verifying both the staged and restored digests.
//! Boundary: no Excel/COM/GUI/ToolBus; restore is test-local, not production atomic replacement.
//! Related: ADR-0062, ADR-0107, TASK-404, TASK-406, TASK-414.

use assistant_protocol::ErrorCode;
use assistant_storage::BlobId;
use assistant_undo::{ContentDigest, ShadowCopy, ShadowCopyPath};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    ExcelL1AnchorHandle, ExcelL1AnchorStore, ExcelL1CaptureRequest, ExcelL1RollbackReadiness,
    ExcelL1WorkbookIdentity, ExcelT21Error, MAX_EXCEL_L1_SHADOW_BYTES,
};

static NEXT_FILE_SHADOW_TEMP: AtomicU64 = AtomicU64::new(1);
const ROOT_MARKER_NAME: &str = ".task414-file-shadow-root";

/// One physically published shadow file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelL1FileShadowCapture {
    target_path: PathBuf,
    shadow_copy: ShadowCopy,
    relative_path: PathBuf,
    byte_count: usize,
}

/// Result of one verified file restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelL1FileRestoreReceipt {
    restored_digest: ContentDigest,
    byte_count: usize,
}

/// Outcome of the anchor-bound file restore path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExcelL1FileRestoreOutcome {
    /// The target was restored and the anchor released.
    Restored(ExcelL1FileRestoreReceipt),
    /// The workbook was already at its pre-write state.
    AlreadyAtAnchor,
}

/// Bounded filesystem shadow-copy store rooted in one test-owned temporary tree.
#[derive(Debug)]
pub struct ExcelL1FileShadowStore {
    root: PathBuf,
    max_total_bytes: usize,
    active_bytes: usize,
    captures: BTreeMap<ShadowCopyPath, ExcelL1FileShadowCapture>,
}

impl ExcelL1FileShadowStore {
    /// Opens a repository-external empty root with typed path/filesystem errors.
    pub(crate) fn open(
        root: impl AsRef<Path>,
        max_total_bytes: usize,
    ) -> Result<Self, ExcelT21Error> {
        if max_total_bytes == 0 || max_total_bytes > MAX_EXCEL_L1_SHADOW_BYTES {
            return Err(invalid_arguments(
                "Excel file-shadow root byte budget is outside the L1 bound",
            ));
        }
        let requested_root = validate_local_absolute_path(root.as_ref(), false)?;
        let repository_root = resolve_repository_root()?;
        let canonical_root = if requested_root.exists() {
            let canonical = canonicalize_local_path(&requested_root)?;
            validate_root_location(&canonical, &repository_root)?;
            require_empty_directory(&canonical)?;
            canonical
        } else {
            let parent = requested_root.parent().ok_or_else(|| {
                invalid_arguments("Excel file-shadow root has no parent directory")
            })?;
            let canonical_parent = canonicalize_local_path(parent)?;
            validate_root_location(&canonical_parent, &repository_root)?;
            fs::create_dir(&requested_root).map_err(|error| {
                io_error(
                    "create Excel file-shadow root",
                    Some(&requested_root),
                    &error,
                )
            })?;
            canonicalize_local_path(&requested_root)?
        };
        if !fs::metadata(&canonical_root)
            .map_err(|error| {
                io_error(
                    "read file-shadow root metadata",
                    Some(&canonical_root),
                    &error,
                )
            })?
            .is_dir()
        {
            return Err(invalid_arguments(
                "Excel file-shadow root must be a directory",
            ));
        }
        let marker_sequence = NEXT_FILE_SHADOW_TEMP.fetch_add(1, Ordering::Relaxed);
        let marker = format!(
            "task414-file-shadow-root/v1\npid={}\nsequence={marker_sequence}\n",
            std::process::id()
        );
        publish_new_file(&canonical_root.join(ROOT_MARKER_NAME), marker.as_bytes())?;
        Ok(Self {
            root: canonical_root,
            max_total_bytes,
            active_bytes: 0,
            captures: BTreeMap::new(),
        })
    }

    fn read_source(source: &Path) -> Result<(PathBuf, Vec<u8>), ExcelT21Error> {
        let canonical_source = canonicalize_local_path(source)?;
        validate_local_absolute_path(&canonical_source, true)?;
        if !fs::metadata(&canonical_source)
            .map_err(|error| {
                io_error(
                    "read Excel file-shadow source metadata",
                    Some(&canonical_source),
                    &error,
                )
            })?
            .is_file()
        {
            return Err(invalid_arguments(
                "Excel file-shadow source must be a regular file",
            ));
        }
        let bytes = read_file_bounded(
            &canonical_source,
            MAX_EXCEL_L1_SHADOW_BYTES,
            "read Excel file-shadow source",
        )?;
        Ok((canonical_source, bytes))
    }

    pub(super) fn publish_bytes(
        &mut self,
        target_path: PathBuf,
        shadow_copy: &ShadowCopy,
        bytes: &[u8],
    ) -> Result<ExcelL1FileShadowCapture, ExcelT21Error> {
        if bytes.is_empty() {
            return Err(invalid_arguments(
                "Excel file-shadow bytes must not be empty",
            ));
        }
        if bytes.len() > MAX_EXCEL_L1_SHADOW_BYTES {
            return Err(invalid_arguments(
                "Excel file-shadow source exceeds the L1 byte bound",
            ));
        }
        let actual_digest = content_digest(bytes)?;
        if actual_digest != shadow_copy.expected_digest {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel file-shadow bytes differ from the anchor digest",
            ));
        }
        if self.captures.contains_key(&shadow_copy.path) {
            return Err(ExcelT21Error::new(
                ErrorCode::UserInteraction,
                "Excel file-shadow path is already active",
            ));
        }
        let next_total = self.active_bytes.checked_add(bytes.len()).ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::Fatal, "Excel file-shadow byte total overflowed")
        })?;
        if next_total > self.max_total_bytes {
            return Err(ExcelT21Error::new(
                ErrorCode::UserInteraction,
                "Excel file-shadow total exceeds the configured bound",
            ));
        }
        let relative_path = validate_relative_shadow_path(&shadow_copy.path)?;
        let destination = self.root.join(&relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                io_error("create Excel file-shadow parent", Some(parent), &error)
            })?;
            let canonical_parent = canonicalize_local_path(parent)?;
            if !canonical_parent.starts_with(&self.root) {
                return Err(invalid_arguments(
                    "Excel file-shadow destination escaped its root",
                ));
            }
        }
        publish_new_file(&destination, bytes)?;
        let capture = ExcelL1FileShadowCapture {
            target_path,
            shadow_copy: shadow_copy.clone(),
            relative_path,
            byte_count: bytes.len(),
        };
        self.active_bytes = next_total;
        self.captures
            .insert(shadow_copy.path.clone(), capture.clone());
        Ok(capture)
    }

    /// Returns one active physical capture by validated anchor metadata.
    pub(crate) fn capture(
        &self,
        shadow_copy: &ShadowCopy,
    ) -> Result<&ExcelL1FileShadowCapture, ExcelT21Error> {
        let capture = self.captures.get(&shadow_copy.path).ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel file-shadow path is unknown or already cleaned",
            )
        })?;
        if capture.shadow_copy.expected_digest != shadow_copy.expected_digest {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel file-shadow digest differs from the active capture",
            ));
        }
        Ok(capture)
    }
    /// Restores one active shadow after verifying the expected current target digest.
    pub(crate) fn restore(
        &self,
        shadow_copy: &ShadowCopy,
        expected_current_digest: &ContentDigest,
    ) -> Result<ExcelL1FileRestoreReceipt, ExcelT21Error> {
        let capture = self.capture(shadow_copy)?;
        let current_target = canonicalize_local_path(&capture.target_path)?;
        if !same_path(&current_target, &capture.target_path) {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel file-shadow restore target identity drifted",
            ));
        }
        let shadow_path = self.root.join(&capture.relative_path);
        let shadow_bytes = read_file_bounded(
            &shadow_path,
            MAX_EXCEL_L1_SHADOW_BYTES,
            "read Excel file-shadow restoration source",
        )?;
        let shadow_digest = content_digest(&shadow_bytes)?;
        if shadow_digest != capture.shadow_copy.expected_digest {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel file-shadow source digest no longer matches its anchor",
            ));
        }
        replace_file_verified(
            &current_target,
            expected_current_digest,
            &shadow_bytes,
            &shadow_digest,
        )?;
        Ok(ExcelL1FileRestoreReceipt {
            restored_digest: shadow_digest,
            byte_count: shadow_bytes.len(),
        })
    }
    /// Returns the physical shadow path for an active capture.
    pub(crate) fn physical_path(&self, shadow_copy: &ShadowCopy) -> Result<PathBuf, ExcelT21Error> {
        self.capture(shadow_copy)
            .map(|capture| self.root.join(&capture.relative_path))
    }
    /// Removes only registered files, the marker, and empty owned directories.
    pub(crate) fn cleanup(&mut self) -> Result<(), ExcelT21Error> {
        let mut directories = BTreeSet::new();
        for capture in self.captures.values() {
            let path = self.root.join(&capture.relative_path);
            fs::remove_file(&path).map_err(|error| {
                io_error("remove Excel file-shadow capture", Some(&path), &error)
            })?;
            let mut parent = path.parent();
            while let Some(directory) = parent {
                if directory == self.root {
                    break;
                }
                directories.insert(directory.to_path_buf());
                parent = directory.parent();
            }
        }
        let marker = self.root.join(ROOT_MARKER_NAME);
        fs::remove_file(&marker)
            .map_err(|error| io_error("remove Excel file-shadow marker", Some(&marker), &error))?;
        directories.insert(self.root.clone());
        let mut ordered_directories = directories.into_iter().collect::<Vec<_>>();
        ordered_directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for directory in ordered_directories {
            fs::remove_dir(&directory).map_err(|error| {
                io_error(
                    "remove Excel file-shadow directory",
                    Some(&directory),
                    &error,
                )
            })?;
        }
        self.captures.clear();
        self.active_bytes = 0;
        Ok(())
    }
}

impl ExcelL1AnchorStore {
    /// Captures an anchor from the owned path and rolls back memory if publishing fails.
    pub(crate) fn capture_with_file_shadow(
        &mut self,
        mut request: ExcelL1CaptureRequest,
        file_shadow_store: &mut ExcelL1FileShadowStore,
    ) -> Result<ExcelL1AnchorHandle, ExcelT21Error> {
        let source_path = super::validate_workbook_identity(&request.workbook, None)?;
        let (canonical_source, source_bytes) = ExcelL1FileShadowStore::read_source(&source_path)?;
        request.file_bytes = source_bytes;
        let handle = self.capture(request)?;
        let shadow_copy = self.anchor(handle)?.shadow_copy().cloned().ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel L1 anchor unexpectedly has no shadow-copy metadata",
            )
        })?;
        let publish_result = self.shadow_bytes(handle).and_then(|bytes| {
            file_shadow_store
                .publish_bytes(canonical_source, &shadow_copy, bytes)
                .map(|_capture| ())
        });
        match publish_result {
            Ok(()) => Ok(handle),
            Err(publish_error) => match self.release(handle) {
                Ok(()) => Err(publish_error),
                Err(release_error) => Err(ExcelT21Error::new(
                    ErrorCode::Fatal,
                    format!(
                        "Excel file-shadow capture failed: {publish_error}; anchor rollback failed: {release_error}"
                    ),
                )),
            },
        }
    }
    /// Restores after the fingerprint gate; success releases the active anchor.
    pub(crate) fn restore_with_file_shadow(
        &mut self,
        handle: ExcelL1AnchorHandle,
        observed: &ExcelL1WorkbookIdentity,
        expected_current_file_digest: &ContentDigest,
        file_shadow_store: &ExcelL1FileShadowStore,
    ) -> Result<ExcelL1FileRestoreOutcome, ExcelT21Error> {
        match self.rollback_readiness(handle, observed)? {
            ExcelL1RollbackReadiness::AlreadyAtAnchor => {
                self.release(handle)?;
                return Ok(ExcelL1FileRestoreOutcome::AlreadyAtAnchor);
            }
            ExcelL1RollbackReadiness::ReadyToRestore => {}
        }
        let shadow_copy = self.anchor(handle)?.shadow_copy().cloned().ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel L1 anchor unexpectedly has no shadow-copy metadata",
            )
        })?;
        let receipt = file_shadow_store.restore(&shadow_copy, expected_current_file_digest)?;
        self.release(handle)?;
        Ok(ExcelL1FileRestoreOutcome::Restored(receipt))
    }
}

fn validate_root_location(root: &Path, current_directory: &Path) -> Result<(), ExcelT21Error> {
    if root.starts_with(current_directory) || current_directory.starts_with(root) {
        return Err(invalid_arguments(
            "Excel file-shadow root must be disjoint from the repository",
        ));
    }
    Ok(())
}

fn resolve_repository_root() -> Result<PathBuf, ExcelT21Error> {
    let mut current = canonicalize_local_path(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    loop {
        if current.join(".git").exists() {
            return Ok(current);
        }
        current = current.parent().map(Path::to_path_buf).ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel file-shadow could not locate the repository root",
            )
        })?;
    }
}

fn require_empty_directory(directory: &Path) -> Result<(), ExcelT21Error> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error("read Excel file-shadow root", Some(directory), &error))?;
    match entries.next() {
        None => Ok(()),
        Some(Ok(_entry)) => Err(invalid_arguments(
            "existing Excel file-shadow root must be empty",
        )),
        Some(Err(error)) => Err(io_error(
            "read Excel file-shadow root entry",
            Some(directory),
            &error,
        )),
    }
}
fn validate_local_absolute_path(path: &Path, require_xlsx: bool) -> Result<PathBuf, ExcelT21Error> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(invalid_arguments("Excel file-shadow path must be absolute"));
    }
    let path_text = path.to_string_lossy();
    if path_text.len() > 32 * 1024
        || path_text.contains("://")
        || path_text.starts_with("\\\\")
        || path_text.starts_with("//")
        || path_text.starts_with("\\\\?\\")
        || path_text.chars().any(char::is_control)
    {
        return Err(invalid_arguments(
            "Excel file-shadow path must be local and non-device",
        ));
    }
    for component in path.components() {
        match component {
            Component::CurDir | Component::ParentDir => {
                return Err(invalid_arguments(
                    "Excel file-shadow path must not contain dot components",
                ));
            }
            Component::Prefix(prefix) => {
                let prefix_text = prefix.as_os_str().to_string_lossy();
                if prefix_text.starts_with("\\\\") || prefix_text.contains("UNC") {
                    return Err(invalid_arguments(
                        "Excel file-shadow path must not use a UNC/device prefix",
                    ));
                }
            }
            Component::RootDir | Component::Normal(_) => {}
        }
    }
    if require_xlsx && path.extension().and_then(|extension| extension.to_str()) != Some("xlsx") {
        return Err(invalid_arguments(
            "Excel file-shadow workbook extension must be exactly .xlsx",
        ));
    }
    Ok(path.to_path_buf())
}
fn validate_relative_shadow_path(path: &ShadowCopyPath) -> Result<PathBuf, ExcelT21Error> {
    let relative = PathBuf::from(path.as_str());
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(invalid_arguments(
            "Excel file-shadow relative path must not be empty or absolute",
        ));
    }
    for component in relative.components() {
        match component {
            Component::Normal(segment) if !segment.is_empty() => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(invalid_arguments(
                    "Excel file-shadow relative path contains an unsafe component",
                ));
            }
            Component::Normal(_) => {
                return Err(invalid_arguments(
                    "Excel file-shadow relative path contains an empty component",
                ));
            }
        }
    }
    Ok(relative)
}
fn canonicalize_local_path(path: &Path) -> Result<PathBuf, ExcelT21Error> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| io_error("canonicalize Excel file-shadow path", Some(path), &error))?;
    let text = canonical.to_string_lossy();
    let stripped = text.strip_prefix("\\\\?\\").map_or_else(
        || text.to_string(),
        |remainder| {
            if remainder.starts_with("UNC\\") {
                "\\\\".to_owned() + remainder
            } else {
                remainder.to_owned()
            }
        },
    );
    let normalized = PathBuf::from(stripped);
    validate_local_absolute_path(&normalized, false)
}

fn read_file_bounded(
    path: &Path,
    maximum_bytes: usize,
    context: &str,
) -> Result<Vec<u8>, ExcelT21Error> {
    let file = File::open(path).map_err(|error| io_error(context, Some(path), &error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(&format!("{context} metadata"), Some(path), &error))?;
    let length = usize::try_from(metadata.len()).map_err(|_| {
        ExcelT21Error::new(
            ErrorCode::UserInteraction,
            format!("{context} size does not fit the platform address space"),
        )
    })?;
    if length > maximum_bytes {
        return Err(ExcelT21Error::new(
            ErrorCode::UserInteraction,
            format!("{context} exceeds the configured byte bound"),
        ));
    }
    let limit = u64::try_from(maximum_bytes)
        .map_err(|_| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                format!("{context} byte bound does not fit u64"),
            )
        })?
        .checked_add(1)
        .ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::Fatal, format!("{context} byte limit overflowed"))
        })?;
    let mut bytes = Vec::with_capacity(length);
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(context, Some(path), &error))?;
    if bytes.len() > maximum_bytes {
        return Err(ExcelT21Error::new(
            ErrorCode::UserInteraction,
            format!("{context} grew beyond the configured byte bound"),
        ));
    }
    Ok(bytes)
}

fn publish_new_file(destination: &Path, bytes: &[u8]) -> Result<(), ExcelT21Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| {
            io_error(
                "create Excel file-shadow create-new",
                Some(destination),
                &error,
            )
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let write_error = io_error("write Excel file-shadow", Some(destination), &error);
        let cleanup_result = cleanup_temporary(destination);
        return Err(combine_cleanup_error(write_error, cleanup_result));
    }
    Ok(())
}

fn write_new_synced_file(path: &Path, bytes: &[u8]) -> Result<(), ExcelT21Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create Excel file-shadow temporary", Some(path), &error))?;
    file.write_all(bytes)
        .map_err(|error| io_error("write Excel file-shadow temporary", Some(path), &error))?;
    file.sync_all()
        .map_err(|error| io_error("sync Excel file-shadow temporary", Some(path), &error))
}

fn replace_file_verified(
    target: &Path,
    expected_current_digest: &ContentDigest,
    bytes: &[u8],
    expected_restored_digest: &ContentDigest,
) -> Result<(), ExcelT21Error> {
    let temporary = temporary_sibling(target, "restore-tmp")?;
    let backup = temporary_sibling(target, "restore-backup")?;
    write_new_synced_file(&temporary, bytes)?;
    if let Err(error) = fs::rename(target, &backup) {
        let stage_error = io_error(
            "stage Excel file-shadow restore target",
            Some(target),
            &error,
        );
        return Err(combine_cleanup_error(
            stage_error,
            cleanup_temporary(&temporary),
        ));
    }
    verify_staged_target(target, &backup, &temporary, expected_current_digest)?;
    if let Err(error) = fs::hard_link(&temporary, target) {
        let temporary_cleanup = cleanup_temporary(&temporary);
        if error.kind() == io::ErrorKind::AlreadyExists {
            return Err(combine_cleanup_error(
                ExcelT21Error::new(
                    ErrorCode::UserInteraction,
                    format!(
                        "Excel file-shadow restore target reappeared; backup retained at {}",
                        backup.to_string_lossy()
                    ),
                ),
                temporary_cleanup,
            ));
        }
        let rollback_result = restore_backup_without_replacing(target, &backup);
        return Err(combine_restore_and_cleanup_failure(
            io_error(
                "publish Excel file-shadow restore target",
                Some(target),
                &error,
            ),
            rollback_result,
            temporary_cleanup,
        ));
    }
    if let Err(error) = fs::remove_file(&temporary) {
        let rollback_result = remove_owned_target_and_restore_backup(target, &backup);
        let cleanup_result = cleanup_temporary(&temporary);
        return Err(combine_restore_and_cleanup_failure(
            io_error(
                "remove Excel file-shadow restore temporary",
                Some(&temporary),
                &error,
            ),
            rollback_result,
            cleanup_result,
        ));
    }
    verify_restored_target(target, &backup, expected_restored_digest)?;
    fs::remove_file(&backup).map_err(|error| {
        io_error(
            "remove Excel file-shadow restore backup",
            Some(&backup),
            &error,
        )
    })
}

fn verify_staged_target(
    target: &Path,
    backup: &Path,
    temporary: &Path,
    expected_current_digest: &ContentDigest,
) -> Result<(), ExcelT21Error> {
    let verification = read_file_bounded(
        backup,
        MAX_EXCEL_L1_SHADOW_BYTES,
        "read staged Excel file-shadow target",
    )
    .and_then(|bytes| content_digest(&bytes))
    .and_then(|digest| {
        if &digest == expected_current_digest {
            Ok(())
        } else {
            Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel file-shadow target changed after the recorded write",
            ))
        }
    });
    match verification {
        Ok(()) => Ok(()),
        Err(error) => {
            let rollback_result = restore_backup_without_replacing(target, backup);
            let cleanup_result = cleanup_temporary(temporary);
            Err(combine_restore_and_cleanup_failure(
                error,
                rollback_result,
                cleanup_result,
            ))
        }
    }
}

fn verify_restored_target(
    target: &Path,
    backup: &Path,
    expected_digest: &ContentDigest,
) -> Result<(), ExcelT21Error> {
    let sync_result = OpenOptions::new()
        .write(true)
        .open(target)
        .and_then(|file| file.sync_all());
    if let Err(error) = sync_result {
        let sync_error = io_error(
            "sync restored Excel file-shadow target",
            Some(target),
            &error,
        );
        let rollback_result = remove_owned_target_and_restore_backup(target, backup);
        return combine_restore_failure(sync_error, rollback_result);
    }
    let restored_bytes = match read_file_bounded(
        target,
        MAX_EXCEL_L1_SHADOW_BYTES,
        "read restored Excel file-shadow target",
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            let rollback_result = remove_owned_target_and_restore_backup(target, backup);
            return combine_restore_failure(error, rollback_result);
        }
    };
    let restored_digest = match content_digest(&restored_bytes) {
        Ok(digest) => digest,
        Err(error) => {
            let rollback_result = remove_owned_target_and_restore_backup(target, backup);
            return combine_restore_failure(error, rollback_result);
        }
    };
    if &restored_digest != expected_digest {
        let rollback_result = remove_owned_target_and_restore_backup(target, backup);
        return combine_restore_failure(
            ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel file-shadow restored digest differs from the anchor",
            ),
            rollback_result,
        );
    }
    Ok(())
}

fn restore_backup_without_replacing(target: &Path, backup: &Path) -> Result<(), ExcelT21Error> {
    if let Err(error) = fs::hard_link(backup, target) {
        if error.kind() == io::ErrorKind::AlreadyExists {
            return Err(ExcelT21Error::new(
                ErrorCode::UserInteraction,
                format!(
                    "Excel file-shadow rollback target reappeared; backup retained at {}",
                    backup.to_string_lossy()
                ),
            ));
        }
        return Err(io_error(
            "restore Excel file-shadow target backup",
            Some(target),
            &error,
        ));
    }
    if let Err(error) = fs::remove_file(backup) {
        let error = io_error(
            "remove Excel file-shadow restored backup link",
            Some(backup),
            &error,
        );
        let cleanup = fs::remove_file(target).map_err(|target_error| {
            io_error(
                "remove duplicate Excel file-shadow rollback target",
                Some(target),
                &target_error,
            )
        });
        return Err(combine_cleanup_error(error, cleanup));
    }
    Ok(())
}

fn remove_owned_target_and_restore_backup(
    target: &Path,
    backup: &Path,
) -> Result<(), ExcelT21Error> {
    fs::remove_file(target).map_err(|error| {
        io_error(
            "remove owned Excel file-shadow restore target",
            Some(target),
            &error,
        )
    })?;
    restore_backup_without_replacing(target, backup)
}

fn combine_restore_failure(
    original: ExcelT21Error,
    rollback_result: Result<(), ExcelT21Error>,
) -> Result<(), ExcelT21Error> {
    match rollback_result {
        Ok(()) => Err(original),
        Err(rollback_error) => Err(ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("{original}; target rollback also failed: {rollback_error}"),
        )),
    }
}

fn combine_restore_and_cleanup_failure(
    original: ExcelT21Error,
    rollback_result: Result<(), ExcelT21Error>,
    cleanup_result: Result<(), ExcelT21Error>,
) -> ExcelT21Error {
    match (rollback_result, cleanup_result) {
        (Ok(()), Ok(())) => original,
        (Err(rollback_error), Ok(())) if rollback_error.code() == ErrorCode::UserInteraction => {
            rollback_error
        }
        (rollback, cleanup) => ExcelT21Error::new(
            ErrorCode::Fatal,
            format!(
                "{original}; target rollback: {}; temporary cleanup: {}",
                result_text(rollback),
                result_text(cleanup)
            ),
        ),
    }
}

fn temporary_sibling(destination: &Path, suffix: &str) -> Result<PathBuf, ExcelT21Error> {
    let parent = destination.parent().ok_or_else(|| {
        invalid_arguments("Excel file-shadow destination has no parent directory")
    })?;
    let file_name = destination
        .file_name()
        .ok_or_else(|| invalid_arguments("Excel file-shadow destination has no file name"))?;
    let sequence = NEXT_FILE_SHADOW_TEMP.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{}.{}.{}.{suffix}",
        file_name.to_string_lossy(),
        std::process::id(),
        sequence
    )))
}

fn cleanup_temporary(path: &Path) -> Result<(), ExcelT21Error> {
    fs::remove_file(path).map_or_else(
        |error| {
            if error.kind() == io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(io_error(
                    "remove Excel file-shadow temporary",
                    Some(path),
                    &error,
                ))
            }
        },
        |()| Ok(()),
    )
}

fn combine_cleanup_error(
    original: ExcelT21Error,
    cleanup_result: Result<(), ExcelT21Error>,
) -> ExcelT21Error {
    match cleanup_result {
        Ok(()) => original,
        Err(cleanup_error) => ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("{original}; temporary cleanup also failed: {cleanup_error}"),
        ),
    }
}

fn result_text(result: Result<(), ExcelT21Error>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(error) => error.to_string(),
    }
}

fn content_digest(bytes: &[u8]) -> Result<ContentDigest, ExcelT21Error> {
    let blob_id = BlobId::of_content(bytes);
    ContentDigest::parse(blob_id.as_str()).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel file-shadow SHA-256 digest was invalid: {error}"),
        )
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn invalid_arguments(message: impl Into<String>) -> ExcelT21Error {
    ExcelT21Error::new(ErrorCode::ToolInvalidArgs, message)
}

fn io_error(context: &str, path: Option<&Path>, error: &io::Error) -> ExcelT21Error {
    let code = match error.kind() {
        io::ErrorKind::NotFound => ErrorCode::TargetNotFound,
        io::ErrorKind::PermissionDenied => ErrorCode::PlatformPermission,
        io::ErrorKind::AlreadyExists => ErrorCode::UserInteraction,
        _ => ErrorCode::Fatal,
    };
    let path_suffix = path.map_or_else(String::new, |path| {
        format!(" for {}", path.to_string_lossy())
    });
    ExcelT21Error::new(code, format!("{context}{path_suffix}: {error}"))
}

#[cfg(test)]
#[path = "excel_t21_l1_file_shadow_tests.rs"]
mod tests;
