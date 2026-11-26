//! Windows process-tree operations for the platform-private Excel watchdog.
//!
//! Responsibilities:
//! - capture and retain complete descendant identities before risky COM calls;
//! - wait for the full tracked tree, not only the original root PID;
//! - verify exact identities on open handles before terminating them.
//!
//! Boundaries:
//! - no COM call and no `taskkill` process launch;
//! - no PID-only termination or snapshot-only ownership claim;
//! - no recovery after the Host process itself has exited.
//!
//! Invariants:
//! 1. root and descendants are killed only from handles matching full identity;
//! 2. tracked descendants, snapshots, handles, and waits all have hard bounds;
//! 3. a reused PID with a different creation time or path is treated as exited.
//!
//! Related documents: ADR-0108 D8/D9, TASK-409, TASK-410, TASK-412.

use std::cmp::Reverse;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    TerminateProcess, WaitForSingleObject,
};

use super::ExcelOwnedProcessIdentity;
use super::process_watchdog::{ExcelProcessTerminationReport, ExcelProcessWatchdogBackend};
use crate::excel_process::{
    ExcelProcessIdentity, ProcessTreeEntry, open_process_handle, process_identity,
    process_tree_snapshot, read_process_identity_from_handle,
};

const MAX_PROCESS_TREE_HANDLES: usize = 256;
const MAX_PROCESS_TREE_SNAPSHOTS: usize = 8;
const PROCESS_EXIT_POLL: Duration = Duration::from_millis(25);
const TREE_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const FILETIME_UNIX_EPOCH_100NS: u64 = 116_444_736_000_000_000;

/// Production backend backed only by Win32 process handles and Toolhelp snapshots.
pub(super) struct WindowsProcessTreeBackend {
    tracked_descendants: Vec<TrackedProcess>,
}

impl WindowsProcessTreeBackend {
    pub(super) const fn new() -> Self {
        Self {
            tracked_descendants: Vec::new(),
        }
    }

    fn merge_tracked(&mut self, discovered: Vec<TrackedProcess>) -> PlatformResult<()> {
        for process in discovered {
            if let Some(existing) = self
                .tracked_descendants
                .iter_mut()
                .find(|existing| existing.identity == process.identity)
            {
                existing.depth = existing.depth.min(process.depth);
                continue;
            }
            if self.tracked_descendants.len() >= MAX_PROCESS_TREE_HANDLES {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!(
                        "Excel process tree exceeded {MAX_PROCESS_TREE_HANDLES} tracked descendants"
                    ),
                ));
            }
            self.tracked_descendants.push(process);
        }
        Ok(())
    }

    fn has_live_descendants(&self) -> PlatformResult<bool> {
        for tracked in &self.tracked_descendants {
            if process_identity(tracked.identity.process_id)?
                .is_some_and(|current| current == tracked.identity)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn handle_processes(
        &self,
        root: Option<(ProcessHandle, ExcelProcessIdentity)>,
    ) -> PlatformResult<Vec<TerminatingProcess>> {
        let mut processes = Vec::new();
        for tracked in &self.tracked_descendants {
            let Some(handle) = open_process_handle(
                tracked.identity.process_id,
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
            )?
            else {
                continue;
            };
            let handle = ProcessHandle(handle);
            let Some(current) =
                read_process_identity_from_handle(handle.0, tracked.identity.process_id)?
            else {
                continue;
            };
            if current != tracked.identity {
                continue;
            }
            processes.push(TerminatingProcess {
                handle,
                identity: current,
                depth: tracked.depth,
            });
        }
        if let Some((handle, identity)) = root {
            processes.push(TerminatingProcess {
                handle,
                identity,
                depth: 0,
            });
        }
        processes.sort_by_key(|process| Reverse(process.depth));
        Ok(processes)
    }
}

impl ExcelProcessWatchdogBackend for WindowsProcessTreeBackend {
    fn capture_verified_tree(
        &mut self,
        identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<()> {
        let Some(root) = verified_live_root(identity)? else {
            return Ok(());
        };
        let descendants = collect_descendants(root.process_id, identity.start_time_100ns())?;
        self.merge_tracked(descendants)
    }

    fn wait_for_identity_exit(
        &mut self,
        identity: &ExcelOwnedProcessIdentity,
        timeout: Duration,
    ) -> PlatformResult<bool> {
        let deadline = Instant::now() + timeout;
        let mut next_tree_refresh = Instant::now();
        loop {
            let root = verified_live_root(identity)?;
            let now = Instant::now();
            if let Some(root) = root.as_ref()
                && now >= next_tree_refresh
            {
                let descendants =
                    collect_descendants(root.process_id, identity.start_time_100ns())?;
                self.merge_tracked(descendants)?;
                next_tree_refresh = now + TREE_REFRESH_INTERVAL;
            }
            if root.is_none() && !self.has_live_descendants()? {
                self.tracked_descendants.clear();
                return Ok(true);
            }
            if now >= deadline {
                return Ok(false);
            }
            std::thread::sleep(PROCESS_EXIT_POLL);
        }
    }

    fn terminate_verified_tree(
        &mut self,
        identity: &ExcelOwnedProcessIdentity,
        timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport> {
        let deadline = Instant::now() + timeout;
        let root = verified_live_root(identity)?;
        if let Some(root) = root.as_ref() {
            let descendants = collect_descendants(root.process_id, identity.start_time_100ns())?;
            self.merge_tracked(descendants)?;
        }
        let root_handle = match root {
            None => None,
            Some(_) => match open_process_handle(
                identity.process_id(),
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
            )? {
                None => None,
                Some(handle) => {
                    let handle = ProcessHandle(handle);
                    let current =
                        read_process_identity_from_handle(handle.0, identity.process_id())?;
                    current
                        .filter(|current| identity_matches_owned(identity, current))
                        .map(|current| (handle, current))
                }
            },
        };
        let root_was_alive = root_handle.is_some();
        let processes = self.handle_processes(root_handle)?;
        if processes.is_empty() {
            self.tracked_descendants.clear();
            return Ok(ExcelProcessTerminationReport::new(false, false, 0, true));
        }

        let mut terminated_count = 0;
        for candidate in &processes {
            let Some(current) = read_process_identity_from_handle(
                candidate.handle.0,
                candidate.identity.process_id,
            )?
            else {
                continue;
            };
            if current != candidate.identity {
                continue;
            }
            // SAFETY: the handle was opened with PROCESS_TERMINATE and the exact
            // identity was re-read from that same handle immediately above.
            let wait = unsafe { WaitForSingleObject(candidate.handle.0, 0) };
            if wait == WAIT_OBJECT_0 {
                continue;
            }
            // SAFETY: the live handle grants PROCESS_TERMINATE.
            unsafe { TerminateProcess(candidate.handle.0, 1) }.map_err(|error| {
                PlatformError::new(
                    ErrorCode::Fatal,
                    format!(
                        "TerminateProcess failed for verified Excel tree process {}: {}",
                        candidate.identity.process_id,
                        error.message()
                    ),
                )
            })?;
            terminated_count += 1;
        }

        let converged = wait_for_handles(&processes, deadline);
        if converged {
            self.tracked_descendants.clear();
        }
        Ok(ExcelProcessTerminationReport::new(
            root_was_alive,
            true,
            terminated_count,
            converged,
        ))
    }
}

fn verified_live_root(
    identity: &ExcelOwnedProcessIdentity,
) -> PlatformResult<Option<ExcelProcessIdentity>> {
    Ok(process_identity(identity.process_id())?
        .filter(|current| identity_matches_owned(identity, current)))
}

fn collect_descendants(
    root_process_id: u32,
    root_start_time_100ns: u64,
) -> PlatformResult<Vec<TrackedProcess>> {
    let mut candidates = Vec::new();
    let mut known_process_ids = Vec::new();
    for _snapshot_index in 0..MAX_PROCESS_TREE_SNAPSHOTS {
        let snapshot_time_100ns = current_filetime_100ns()?;
        let entries = process_tree_snapshot()?;
        let ordered = descendant_order(root_process_id, &entries)?;
        let mut added = 0_usize;
        for node in ordered {
            if known_process_ids.contains(&node.process_id) {
                continue;
            }
            if known_process_ids.len() >= MAX_PROCESS_TREE_HANDLES {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!(
                        "Excel process tree exceeded {MAX_PROCESS_TREE_HANDLES} snapshot entries"
                    ),
                ));
            }
            known_process_ids.push(node.process_id);
            let Some(handle) = open_process_handle(
                node.process_id,
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            )?
            else {
                continue;
            };
            let handle = ProcessHandle(handle);
            let Some(identity) = read_process_identity_from_handle(handle.0, node.process_id)?
            else {
                continue;
            };
            if identity.start_time_100ns < root_start_time_100ns
                || identity.start_time_100ns > snapshot_time_100ns
                || !image_name_matches(&identity.executable_path, &node.process_name)
            {
                continue;
            }
            candidates.push(TrackedProcess {
                identity,
                depth: node.depth,
                parent_process_id: node.parent_process_id,
            });
            added += 1;
        }
        if added == 0 {
            let stable_entries = process_tree_snapshot()?;
            candidates.retain(|candidate| {
                stable_entries.iter().any(|entry| {
                    entry.process_id == candidate.identity.process_id
                        && entry.parent_process_id == candidate.parent_process_id
                        && entry.process_name.eq_ignore_ascii_case(&process_file_name(
                            &candidate.identity.executable_path,
                        ))
                })
            });
            return Ok(candidates);
        }
    }
    Err(PlatformError::new(
        ErrorCode::Fatal,
        format!(
            "Excel process tree did not stabilize within {MAX_PROCESS_TREE_SNAPSHOTS} snapshots"
        ),
    ))
}

fn current_filetime_100ns() -> PlatformResult<u64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("system clock is before the Unix epoch: {error}"),
            )
        })?;
    let seconds = elapsed
        .as_secs()
        .checked_mul(10_000_000)
        .ok_or_else(|| PlatformError::new(ErrorCode::Fatal, "system clock overflowed FILETIME"))?;
    seconds
        .checked_add(u64::from(elapsed.subsec_nanos() / 100))
        .and_then(|value| value.checked_add(FILETIME_UNIX_EPOCH_100NS))
        .ok_or_else(|| {
            PlatformError::new(
                ErrorCode::Fatal,
                "system clock overflowed FILETIME epoch offset",
            )
        })
}

fn descendant_order(
    root_process_id: u32,
    entries: &[ProcessTreeEntry],
) -> PlatformResult<Vec<TreeOrderNode>> {
    if root_process_id == 0 {
        return Err(PlatformError::new(
            ErrorCode::Fatal,
            "Excel process-tree root id is zero",
        ));
    }
    let mut ordered = Vec::new();
    let mut frontier = vec![(root_process_id, 0_usize)];
    while let Some((parent_process_id, depth)) = frontier.pop() {
        for entry in entries
            .iter()
            .filter(|entry| entry.parent_process_id == parent_process_id)
        {
            if entry.process_id == root_process_id
                || ordered
                    .iter()
                    .any(|node: &TreeOrderNode| node.process_id == entry.process_id)
            {
                continue;
            }
            if ordered.len() >= MAX_PROCESS_TREE_HANDLES {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!("Excel process tree exceeded {MAX_PROCESS_TREE_HANDLES} descendants"),
                ));
            }
            let node = TreeOrderNode {
                process_id: entry.process_id,
                parent_process_id: entry.parent_process_id,
                process_name: entry.process_name.clone(),
                depth: depth.saturating_add(1),
            };
            let process_id = node.process_id;
            let depth = node.depth;
            ordered.push(node);
            frontier.push((process_id, depth));
        }
    }
    Ok(ordered)
}

fn image_name_matches(executable_path: &str, process_name: &str) -> bool {
    process_file_name(executable_path).eq_ignore_ascii_case(process_name)
}

fn process_file_name(executable_path: &str) -> String {
    Path::new(executable_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

fn wait_for_handles(processes: &[TerminatingProcess], deadline: Instant) -> bool {
    for process in processes {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let milliseconds = u32::try_from(remaining.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: every handle carries PROCESS_SYNCHRONIZE and stays live.
        let wait = unsafe { WaitForSingleObject(process.handle.0, milliseconds) };
        if wait != WAIT_OBJECT_0 {
            return false;
        }
        let mut exit_code = 0_u32;
        // SAFETY: the same live handle grants query access and exit_code points
        // to writable stack storage.
        if unsafe { GetExitCodeProcess(process.handle.0, &raw mut exit_code) }.is_err() {
            return false;
        }
    }
    true
}

fn identity_matches_owned(
    expected: &ExcelOwnedProcessIdentity,
    observed: &ExcelProcessIdentity,
) -> bool {
    expected.process_id() == observed.process_id
        && expected.start_time_100ns() == observed.start_time_100ns
        && expected
            .executable_path()
            .eq_ignore_ascii_case(&observed.executable_path)
}

struct TreeOrderNode {
    process_id: u32,
    parent_process_id: u32,
    process_name: String,
    depth: usize,
}

struct TrackedProcess {
    identity: ExcelProcessIdentity,
    depth: usize,
    parent_process_id: u32,
}

struct TerminatingProcess {
    handle: ProcessHandle,
    identity: ExcelProcessIdentity,
    depth: usize,
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the process handle exactly once.
        if let Err(_error) = unsafe { CloseHandle(self.0) } {
            // Cleanup evidence is already determined; the OS owns no alternate
            // reverse mapping that could make a retry safer.
        }
    }
}
