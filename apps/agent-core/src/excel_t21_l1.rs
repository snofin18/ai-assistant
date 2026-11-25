//! Test-only Excel T2.2 L1 anchor preparation.
//!
//! Responsibilities:
//! - bind a bounded `value2` range snapshot to an existing undo `ShadowCopy` anchor;
//! - retain an in-memory copy of caller-provided workbook bytes and its content digest;
//! - validate pre-write, post-write, and pre-rollback workbook fingerprints.
//!
//! Boundaries:
//! - no real workbook is opened, read, written, saved, or restored;
//! - no filesystem shadow-copy directory or persistent blob store is created;
//! - the module is test-only and is not reachable from production assembly or `ToolBus`.
//!
//! Invariants:
//! 1. anchors and shadow bytes are bounded and are released explicitly;
//! 2. `value2` payloads above 4096 cells or 256 KiB are rejected, never truncated;
//! 3. a write is prepared only at the captured pre-fingerprint, and rollback requires the
//!    recorded post-fingerprint plus an unchanged current fingerprint.
//!
//! Related documents: ADR-0062, ADR-0107, ADR-0108, Stage 2 batch 5.
use super::{
    CanonicalDecimal, CellValueType, ExcelT21Error, MAX_RANGE_CELLS, MAX_RANGE_PAYLOAD_BYTES,
    OpenedWorkbook, OwnedProcessIdentity, RangeSelector, RangeSnapshot, ReadMode, RuntimeCell,
    WorkbookHandle, WorkbookIdentity, WorksheetInfo, WorksheetSelector, invalid_arguments,
    parse_a1_range, parse_workbook_path, runtime_shape_error,
};
use assistant_platform_api::Fingerprint;
use assistant_protocol::ErrorCode;
use assistant_storage::BlobId;
use assistant_undo::{
    Anchor, AnchorId, AnchorKind, ContentDigest, Reversibility, RollbackConflict, ShadowCopy,
    ShadowCopyPath, StepId, TargetId, detect_conflict,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::path::{Path, PathBuf};
/// Maximum number of active anchors retained by one private Excel L1 store.
pub(crate) const MAX_EXCEL_L1_ANCHORS: usize = 64;
/// Maximum total workbook bytes retained as in-memory shadow copies.
pub(crate) const MAX_EXCEL_L1_SHADOW_BYTES: usize = 32 * 1024 * 1024;
/// Process-, workbook-, task-, and ToolBus-session identity bound to one L1 anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcelL1WorkbookIdentity {
    tool_bus_session_id: String,
    task_id: String,
    workbook_handle: WorkbookHandle,
    workbook: WorkbookIdentity,
    owned_process: OwnedProcessIdentity,
    worksheets: Vec<WorksheetInfo>,
    fingerprint: String,
}
impl ExcelL1WorkbookIdentity {
    /// Derives the binding from the validated result of the owned workbook open.
    pub(crate) fn from_opened(
        tool_bus_session_id: String,
        task_id: String,
        opened: &OpenedWorkbook,
        fingerprint: String,
    ) -> Self {
        Self {
            tool_bus_session_id,
            task_id,
            workbook_handle: opened.handle,
            workbook: opened.workbook.clone(),
            owned_process: opened.owned_process.clone(),
            worksheets: opened.worksheets.clone(),
            fingerprint,
        }
    }
}
/// One capture request assembled immediately before an Excel write step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcelL1CaptureRequest {
    /// Stable anchor identifier.
    pub(crate) anchor_id: String,
    /// Plan step identifier.
    pub(crate) step_id: String,
    /// Owned Excel process, workbook, task, session, and pre-write fingerprint.
    pub(crate) workbook: ExcelL1WorkbookIdentity,
    /// Range being snapshotted.
    pub(crate) selector: RangeSelector,
    /// `value2` result captured for the range.
    pub(crate) snapshot: RangeSnapshot,
    /// Read mode; L1 capture accepts only `Value2`.
    pub(crate) mode: ReadMode,
    /// Original workbook bytes supplied by the future file-shadow-copy port.
    pub(crate) file_bytes: Vec<u8>,
}
/// Opaque handle for one in-memory L1 anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExcelL1AnchorHandle(u64);
/// Result of checking whether an L1 rollback may restore the captured bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExcelL1RollbackReadiness {
    /// The workbook is at the recorded post-write fingerprint and may be restored.
    ReadyToRestore,
    /// The workbook already matches the pre-write anchor, so no restore is needed.
    AlreadyAtAnchor,
}
/// Validated value snapshot retained alongside the file shadow copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcelL1ValueSnapshot {
    address: String,
    rows: u32,
    columns: u32,
    selector: Value,
    cells: Vec<RuntimeCell>,
    payload_bytes: usize,
}
impl ExcelL1ValueSnapshot {
    /// Returns the canonical address captured for the range.
    pub(crate) fn address(&self) -> &str {
        &self.address
    }
    /// Returns the captured row count.
    pub(crate) const fn rows(&self) -> u32 {
        self.rows
    }
    /// Returns the captured column count.
    pub(crate) const fn columns(&self) -> u32 {
        self.columns
    }
    /// Returns the canonical range selector recorded with the cells.
    pub(crate) const fn selector(&self) -> &Value {
        &self.selector
    }
    /// Returns the number of captured cells.
    pub(crate) fn cell_count(&self) -> usize {
        self.cells.len()
    }
    /// Returns normalized cells with absolute one-based workbook coordinates.
    pub(crate) fn cells(&self) -> &[RuntimeCell] {
        &self.cells
    }
    /// Returns the compact JSON payload size.
    pub(crate) const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}
#[derive(Debug)]
struct ExcelL1AnchorRecord {
    anchor: Anchor,
    value_snapshot: ExcelL1ValueSnapshot,
    workbook: ExcelL1WorkbookIdentity,
    shadow_bytes: Vec<u8>,
    write_prepared: bool,
}
/// Bounded in-memory L1 anchor store used only by Excel T2.2 tests.
#[derive(Debug)]
pub(crate) struct ExcelL1AnchorStore {
    next_handle: u64,
    anchors: BTreeMap<u64, ExcelL1AnchorRecord>,
    active_shadow_bytes: usize,
}
impl ExcelL1AnchorStore {
    /// Creates an empty store.
    pub(crate) fn new() -> Self {
        Self {
            next_handle: 0,
            anchors: BTreeMap::new(),
            active_shadow_bytes: 0,
        }
    }
    /// Captures a bounded `value2` snapshot and in-memory workbook shadow copy.
    ///
    /// The anchor is inserted only after every input, bound, digest, and undo-shape check
    /// succeeds, so a failed capture does not consume capacity or expose a partial anchor.
    pub(crate) fn capture(
        &mut self,
        request: ExcelL1CaptureRequest,
    ) -> Result<ExcelL1AnchorHandle, ExcelT21Error> {
        if self.anchors.len() >= MAX_EXCEL_L1_ANCHORS {
            return Err(ExcelT21Error::new(
                ErrorCode::UserInteraction,
                "Excel L1 store full",
            ));
        }
        let workbook_path = validate_workbook_identity(&request.workbook, None)?;
        if request.mode != ReadMode::Value2 {
            return Err(invalid_arguments("Excel L1 snapshots must use value2"));
        }
        let pre_fingerprint = parse_request_fingerprint(&request.workbook.fingerprint)?;
        let value_snapshot = validate_value_snapshot(
            &request.selector,
            &request.snapshot,
            &request.workbook.worksheets,
        )?;
        if request.file_bytes.is_empty() {
            return Err(invalid_arguments("Excel L1 shadow bytes are required"));
        }
        let next_shadow_bytes = self
            .active_shadow_bytes
            .checked_add(request.file_bytes.len())
            .ok_or_else(|| {
                ExcelT21Error::new(ErrorCode::Fatal, "Excel L1 shadow-byte count overflowed")
            })?;
        if next_shadow_bytes > MAX_EXCEL_L1_SHADOW_BYTES {
            return Err(ExcelT21Error::new(
                ErrorCode::UserInteraction,
                "shadow bytes exceed bound",
            ));
        }
        let blob_id = BlobId::of_content(&request.file_bytes);
        let digest = ContentDigest::parse(blob_id.as_str()).map_err(|error| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                format!("Excel L1 content digest was invalid: {error}"),
            )
        })?;
        let handle_value = self.next_handle.checked_add(1).ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::Fatal, "Excel L1 anchor handle overflowed")
        })?;
        let anchor = create_shadow_anchor(
            handle_value,
            &request,
            &workbook_path,
            pre_fingerprint,
            digest,
        )?;
        match self.anchors.entry(handle_value) {
            Entry::Vacant(entry) => {
                entry.insert(ExcelL1AnchorRecord {
                    anchor,
                    value_snapshot,
                    workbook: request.workbook,
                    shadow_bytes: request.file_bytes,
                    write_prepared: false,
                });
                self.next_handle = handle_value;
                self.active_shadow_bytes = next_shadow_bytes;
            }
            Entry::Occupied(_) => {
                return Err(ExcelT21Error::new(
                    ErrorCode::Fatal,
                    "Excel L1 anchor handle collided with an active entry",
                ));
            }
        }
        Ok(ExcelL1AnchorHandle(handle_value))
    }
    /// Checks the current workbook fingerprint immediately before a write.
    pub(crate) fn prepare_write(
        &mut self,
        handle: ExcelL1AnchorHandle,
        observed: &ExcelL1WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        validate_workbook_identity(observed, None)?;
        let current = parse_runtime_fingerprint(&observed.fingerprint)?;
        let record = self.record_mut(handle)?;
        ensure_same_workbook(&record.workbook, observed)?;
        if record.write_prepared {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel L1 write preparation was already completed",
            ));
        }
        if &current != record.anchor.pre_fingerprint() {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel workbook changed after the L1 anchor was captured",
            ));
        }
        record.write_prepared = true;
        Ok(())
    }
    /// Records the post-write fingerprint exactly once after a prepared write.
    pub(crate) fn record_post_write(
        &mut self,
        handle: ExcelL1AnchorHandle,
        observed: &ExcelL1WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        validate_workbook_identity(observed, None)?;
        let post = parse_runtime_fingerprint(&observed.fingerprint)?;
        let record = self.record_mut(handle)?;
        ensure_same_workbook(&record.workbook, observed)?;
        if !record.write_prepared {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel L1 post-fingerprint cannot be recorded before write preparation",
            ));
        }
        if record.anchor.post_fingerprint().is_some() {
            return Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel L1 post-fingerprint was already recorded",
            ));
        }
        record
            .anchor
            .record_post_fingerprint(post)
            .map_err(|error| undo_shape_error(&error))
    }
    /// Classifies the current fingerprint before an L1 shadow-copy restore.
    pub(crate) fn rollback_readiness(
        &self,
        handle: ExcelL1AnchorHandle,
        observed: &ExcelL1WorkbookIdentity,
    ) -> Result<ExcelL1RollbackReadiness, ExcelT21Error> {
        validate_workbook_identity(observed, None)?;
        let current = parse_runtime_fingerprint(&observed.fingerprint)?;
        let record = self.record(handle)?;
        ensure_same_workbook(&record.workbook, observed)?;
        match detect_conflict(&record.anchor, Some(&current)) {
            RollbackConflict::NoConflict => Ok(ExcelL1RollbackReadiness::ReadyToRestore),
            RollbackConflict::AlreadyAtAnchor { .. } => {
                Ok(ExcelL1RollbackReadiness::AlreadyAtAnchor)
            }
            RollbackConflict::UserChanged { .. } => Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                "Excel workbook changed after the agent write; L1 rollback is blocked",
            )),
            RollbackConflict::EvidenceMissing { reason } => Err(ExcelT21Error::new(
                ErrorCode::VerifyFailed,
                format!("Excel L1 rollback evidence is missing: {reason}"),
            )),
        }
    }
    /// Returns the validated value snapshot for an active anchor.
    pub(crate) fn value_snapshot(
        &self,
        handle: ExcelL1AnchorHandle,
    ) -> Result<&ExcelL1ValueSnapshot, ExcelT21Error> {
        self.record(handle).map(|record| &record.value_snapshot)
    }
    /// Returns the owned workbook binding retained by an active anchor.
    pub(crate) fn workbook(
        &self,
        handle: ExcelL1AnchorHandle,
    ) -> Result<&ExcelL1WorkbookIdentity, ExcelT21Error> {
        self.record(handle).map(|record| &record.workbook)
    }
    /// Returns the existing undo anchor for an active handle.
    pub(crate) fn anchor(&self, handle: ExcelL1AnchorHandle) -> Result<&Anchor, ExcelT21Error> {
        self.record(handle).map(|record| &record.anchor)
    }
    /// Returns the exact in-memory shadow bytes retained for an active handle.
    pub(crate) fn shadow_bytes(&self, handle: ExcelL1AnchorHandle) -> Result<&[u8], ExcelT21Error> {
        self.record(handle)
            .map(|record| record.shadow_bytes.as_slice())
    }
    /// Returns the number of active anchors.
    pub(crate) fn active_anchor_count(&self) -> usize {
        self.anchors.len()
    }
    /// Returns the total number of retained shadow bytes.
    pub(crate) const fn active_shadow_bytes(&self) -> usize {
        self.active_shadow_bytes
    }
    /// Releases one anchor and its shadow bytes.
    pub(crate) fn release(&mut self, handle: ExcelL1AnchorHandle) -> Result<(), ExcelT21Error> {
        let record = self.anchors.remove(&handle.0).ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel L1 anchor handle is unknown or already released",
            )
        })?;
        self.active_shadow_bytes = self
            .active_shadow_bytes
            .checked_sub(record.shadow_bytes.len())
            .ok_or_else(|| {
                ExcelT21Error::new(
                    ErrorCode::Fatal,
                    "Excel L1 shadow-byte accounting underflowed",
                )
            })?;
        Ok(())
    }
    /// Releases every active anchor and resets shadow-byte accounting.
    pub(crate) fn shutdown(&mut self) {
        self.anchors.clear();
        self.active_shadow_bytes = 0;
    }
    fn record(&self, handle: ExcelL1AnchorHandle) -> Result<&ExcelL1AnchorRecord, ExcelT21Error> {
        self.anchors.get(&handle.0).ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel L1 anchor handle is unknown or already released",
            )
        })
    }
    fn record_mut(
        &mut self,
        handle: ExcelL1AnchorHandle,
    ) -> Result<&mut ExcelL1AnchorRecord, ExcelT21Error> {
        self.anchors.get_mut(&handle.0).ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel L1 anchor handle is unknown or already released",
            )
        })
    }
}
fn create_shadow_anchor(
    handle_value: u64,
    request: &ExcelL1CaptureRequest,
    workbook_path: &Path,
    pre_fingerprint: Fingerprint,
    digest: ContentDigest,
) -> Result<Anchor, ExcelT21Error> {
    let anchor_id =
        AnchorId::parse(request.anchor_id.clone()).map_err(|error| undo_shape_error(&error))?;
    let step_id =
        StepId::parse(request.step_id.clone()).map_err(|error| undo_shape_error(&error))?;
    let target_id = TargetId::parse(format!(
        "{}:{}:{}",
        request.workbook.tool_bus_session_id,
        request.workbook.task_id,
        workbook_path.to_string_lossy()
    ))
    .map_err(|error| undo_shape_error(&error))?;
    let shadow_path = ShadowCopyPath::parse(format!(
        "excel-l1/{handle_value}/{}.shadow",
        digest.as_str()
    ))
    .map_err(|error| undo_shape_error(&error))?;
    Anchor::new(
        anchor_id,
        step_id,
        target_id,
        Reversibility::L1Snapshot,
        pre_fingerprint,
        AnchorKind::ShadowCopy {
            shadow_copy: ShadowCopy::new(shadow_path, digest),
        },
    )
    .map_err(|error| undo_shape_error(&error))
}
fn validate_value_snapshot(
    selector: &RangeSelector,
    snapshot: &RangeSnapshot,
    worksheets: &[WorksheetInfo],
) -> Result<ExcelL1ValueSnapshot, ExcelT21Error> {
    let (expected_cells, address_start) = validate_snapshot_bounds(selector, snapshot, worksheets)?;
    let cells = normalize_snapshot_cells(snapshot, expected_cells, address_start)?;
    let address = write_port::canonical_snapshot_address(selector, snapshot, worksheets)?;
    let selector = selector_value(selector)?;
    let payload = value_snapshot_payload(&selector, snapshot, &cells)?;
    let payload_bytes = serde_json::to_vec(&payload).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel L1 value snapshot serialization failed: {error}"),
        )
    })?;
    if payload_bytes.len() > MAX_RANGE_PAYLOAD_BYTES {
        return Err(invalid_arguments(format!(
            "Excel L1 value snapshot exceeds {MAX_RANGE_PAYLOAD_BYTES} bytes"
        )));
    }
    Ok(ExcelL1ValueSnapshot {
        address,
        rows: snapshot.rows,
        columns: snapshot.columns,
        selector,
        cells,
        payload_bytes: payload_bytes.len(),
    })
}

fn validate_snapshot_bounds(
    selector: &RangeSelector,
    snapshot: &RangeSnapshot,
    worksheets: &[WorksheetInfo],
) -> Result<(u64, super::A1Cell), ExcelT21Error> {
    if snapshot.address.trim().is_empty() || snapshot.rows == 0 || snapshot.columns == 0 {
        return Err(runtime_shape_error(
            "Excel L1 snapshot requires a non-empty address and positive dimensions",
        ));
    }
    let expected_cells = u64::from(snapshot.rows)
        .checked_mul(u64::from(snapshot.columns))
        .ok_or_else(|| invalid_arguments("Excel L1 cell count overflowed"))?;
    if expected_cells > MAX_RANGE_CELLS {
        return Err(invalid_arguments(format!(
            "Excel L1 snapshot has {expected_cells} cells; maximum is {MAX_RANGE_CELLS}"
        )));
    }
    let (address_start, address_end) = snapshot_address_bounds(&snapshot.address)?;
    let declared_rows = address_end
        .row
        .checked_sub(address_start.row)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| runtime_shape_error("Excel L1 range row bounds are inverted"))?;
    let declared_columns = address_end
        .column
        .checked_sub(address_start.column)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| runtime_shape_error("Excel L1 range column bounds are inverted"))?;
    if declared_rows != snapshot.rows || declared_columns != snapshot.columns {
        return Err(runtime_shape_error(
            "Excel L1 address dimensions differ from snapshot dimensions",
        ));
    }
    validate_a1_selector(selector, snapshot, address_start, address_end, worksheets)?;
    Ok((expected_cells, address_start))
}
fn validate_a1_selector(
    selector: &RangeSelector,
    snapshot: &RangeSnapshot,
    address_start: super::A1Cell,
    address_end: super::A1Cell,
    worksheets: &[WorksheetInfo],
) -> Result<(), ExcelT21Error> {
    let RangeSelector::A1 {
        worksheet,
        start,
        end,
    } = selector
    else {
        return Ok(());
    };
    let selector_rows = end
        .row
        .checked_sub(start.row)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| invalid_arguments("Excel L1 selector row bounds are inverted"))?;
    let selector_columns = end
        .column
        .checked_sub(start.column)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| invalid_arguments("Excel L1 selector column bounds are inverted"))?;
    if *start != address_start
        || *end != address_end
        || selector_rows != snapshot.rows
        || selector_columns != snapshot.columns
    {
        return Err(runtime_shape_error(
            "Excel L1 address differs from the requested A1 range",
        ));
    }
    let reported_worksheet = snapshot.address.rsplit_once('!').map(|(name, _)| name);
    match worksheet {
        WorksheetSelector::Name(name) => {
            if !worksheets.iter().any(|worksheet| worksheet.name == *name) {
                return Err(ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel L1 named worksheet is absent from the open workbook",
                ));
            }
            if reported_worksheet.is_some_and(|reported| reported != name.as_str()) {
                return Err(ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel L1 snapshot identifies a different worksheet",
                ));
            }
            Ok(())
        }
        WorksheetSelector::Ordinal(0) => Err(invalid_arguments(
            "Excel L1 worksheet ordinal must be positive",
        )),
        WorksheetSelector::Ordinal(ordinal) => {
            let expected_name = worksheets
                .iter()
                .find(|worksheet| worksheet.ordinal == *ordinal)
                .ok_or_else(|| {
                    ExcelT21Error::new(
                        ErrorCode::TargetNotFound,
                        "Excel L1 ordinal selector is absent from the open workbook",
                    )
                })?;
            if reported_worksheet.is_some_and(|reported| reported != expected_name.name.as_str()) {
                Err(ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel L1 snapshot identifies a different worksheet",
                ))
            } else {
                Ok(())
            }
        }
    }
}
fn normalize_snapshot_cells(
    snapshot: &RangeSnapshot,
    expected_cells: u64,
    address_start: super::A1Cell,
) -> Result<Vec<RuntimeCell>, ExcelT21Error> {
    let expected_cells_usize = usize::try_from(expected_cells)
        .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "cell count does not fit usize"))?;
    if snapshot.cells.len() > expected_cells_usize {
        return Err(runtime_shape_error(
            "Excel L1 snapshot contains too many cells",
        ));
    }
    let mut cells = BTreeMap::new();
    for cell in &snapshot.cells {
        if cell.row == 0
            || cell.column == 0
            || cell.row > snapshot.rows
            || cell.column > snapshot.columns
        {
            return Err(runtime_shape_error(
                "Excel L1 cell coordinate is outside the captured range",
            ));
        }
        let normalized = normalize_cell_value(cell)?;
        let absolute_row = address_start
            .row
            .checked_add(cell.row - 1)
            .ok_or_else(|| runtime_shape_error("Excel L1 absolute row overflowed"))?;
        let absolute_column = address_start
            .column
            .checked_add(cell.column - 1)
            .ok_or_else(|| runtime_shape_error("Excel L1 absolute column overflowed"))?;
        let previous = cells.insert(
            (cell.row, cell.column),
            RuntimeCell {
                row: absolute_row,
                column: absolute_column,
                value_type: cell.value_type,
                value_text: normalized,
            },
        );
        if previous.is_some() {
            return Err(runtime_shape_error(
                "Excel L1 snapshot contains duplicate cell coordinates",
            ));
        }
    }
    let returned_cells = u64::try_from(cells.len())
        .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "cell count does not fit u64"))?;
    if returned_cells != expected_cells {
        return Err(runtime_shape_error(
            "Excel L1 snapshot is partial or contains extra cells",
        ));
    }
    Ok(cells.into_values().collect())
}
fn value_snapshot_payload(
    selector: &Value,
    snapshot: &RangeSnapshot,
    cells: &[RuntimeCell],
) -> Result<Value, ExcelT21Error> {
    let payload_cells = cells
        .iter()
        .map(|cell| {
            Ok(json!({
                "row": cell.row,
                "column": cell.column,
                "address": cell_address(cell.row, cell.column)?,
                "mode": ReadMode::Value2.as_str(),
                "value_type": cell.value_type.as_str(),
                "value_text": cell.value_text,
            }))
        })
        .collect::<Result<Vec<_>, ExcelT21Error>>()?;
    Ok(json!({
        "range": selector,
        "address": snapshot.address,
        "rows": snapshot.rows,
        "columns": snapshot.columns,
        "cells": payload_cells,
    }))
}
fn selector_value(selector: &RangeSelector) -> Result<Value, ExcelT21Error> {
    match selector {
        RangeSelector::NamedRange(name) if !name.trim().is_empty() => {
            Ok(json!({"kind": "named_range", "name": name}))
        }
        RangeSelector::NamedRange(_) => {
            Err(invalid_arguments("Excel L1 named range must not be empty"))
        }
        RangeSelector::A1 {
            worksheet,
            start,
            end,
        } => {
            let worksheet = match worksheet {
                WorksheetSelector::Name(name) if !name.trim().is_empty() => {
                    json!({"kind": "name", "value": name})
                }
                WorksheetSelector::Name(_) => {
                    return Err(invalid_arguments(
                        "Excel L1 worksheet name must not be empty",
                    ));
                }
                WorksheetSelector::Ordinal(ordinal) if *ordinal > 0 => {
                    json!({"kind": "ordinal", "value": ordinal})
                }
                WorksheetSelector::Ordinal(_) => {
                    return Err(invalid_arguments(
                        "Excel L1 worksheet ordinal must be positive",
                    ));
                }
            };
            Ok(json!({
                "kind": "a1",
                "worksheet": worksheet,
                "range": format!(
                    "{}:{}",
                    cell_address(start.row, start.column)?,
                    cell_address(end.row, end.column)?
                ),
            }))
        }
    }
}
fn snapshot_address_bounds(address: &str) -> Result<(super::A1Cell, super::A1Cell), ExcelT21Error> {
    let range = address.rsplit_once('!').map_or(address, |(_, range)| range);
    parse_a1_range(range)
}
fn normalize_cell_value(cell: &RuntimeCell) -> Result<String, ExcelT21Error> {
    match cell.value_type {
        CellValueType::Blank if cell.value_text.is_empty() => Ok(String::new()),
        CellValueType::Blank => Err(runtime_shape_error(
            "Excel L1 blank cell value_text must be empty",
        )),
        CellValueType::Number => {
            CanonicalDecimal::parse(&cell.value_text).and_then(|number| number.as_text())
        }
        CellValueType::Text => Ok(cell.value_text.clone()),
        CellValueType::Boolean => match cell.value_text.as_str() {
            "true" | "false" => Ok(cell.value_text.clone()),
            _ => Err(runtime_shape_error(
                "Excel L1 boolean cell value_text must be true or false",
            )),
        },
        CellValueType::Error if is_known_excel_error(&cell.value_text) => {
            Ok(cell.value_text.clone())
        }
        CellValueType::Error => Err(runtime_shape_error(
            "Excel L1 cell contains an unknown Excel error",
        )),
    }
}
fn cell_address(row: u32, column: u32) -> Result<String, ExcelT21Error> {
    if row == 0 || column == 0 {
        return Err(runtime_shape_error(
            "Excel L1 A1 coordinates must be one-based",
        ));
    }
    let mut value = column;
    let mut letters = Vec::new();
    while value > 0 {
        let digit = (value - 1) % 26;
        letters.push(
            u8::try_from(digit)
                .map_err(|_| runtime_shape_error("A1 column digit does not fit u8"))?
                + b'A',
        );
        value = (value - 1) / 26;
    }
    letters.reverse();
    let letters = String::from_utf8(letters)
        .map_err(|_| runtime_shape_error("A1 column conversion produced invalid ASCII"))?;
    Ok(format!("{letters}{row}"))
}
fn is_known_excel_error(value: &str) -> bool {
    matches!(
        value,
        "#NULL!"
            | "#DIV/0!"
            | "#VALUE!"
            | "#REF!"
            | "#NAME?"
            | "#NUM!"
            | "#N/A"
            | "#GETTING_DATA"
            | "#SPILL!"
            | "#CALC!"
            | "#FIELD!"
            | "#BLOCKED!"
            | "#UNKNOWN!"
            | "#CONNECT!"
            | "#BUSY!"
    )
}
fn validate_workbook_identity(
    identity: &ExcelL1WorkbookIdentity,
    expected_path: Option<&Path>,
) -> Result<PathBuf, ExcelT21Error> {
    for (field, value) in [
        ("tool_bus_session_id", identity.tool_bus_session_id.as_str()),
        ("task_id", identity.task_id.as_str()),
    ] {
        if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(invalid_arguments(format!("Excel L1 {field} is invalid")));
        }
    }
    let workbook_path = parse_workbook_path(&identity.workbook.full_path)?;
    if expected_path.is_some_and(|expected| expected != workbook_path) {
        return Err(invalid_arguments(
            "Excel L1 workbook identity differs from the requested path",
        ));
    }
    let executable_path = PathBuf::from(&identity.owned_process.executable_path);
    if identity.owned_process.process_id == 0
        || identity.owned_process.start_ticks == 0
        || !executable_path.is_absolute()
        || identity.owned_process.executable_path.contains("://")
        || identity.owned_process.executable_path.starts_with("\\\\")
    {
        return Err(invalid_arguments(
            "Excel L1 owned process identity is incomplete",
        ));
    }
    if identity.worksheets.is_empty()
        || identity.worksheets.len() > 1024
        || identity
            .worksheets
            .iter()
            .any(|worksheet| worksheet.name.trim().is_empty() || worksheet.ordinal == 0)
        || identity
            .worksheets
            .iter()
            .map(|worksheet| worksheet.name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != identity.worksheets.len()
        || identity
            .worksheets
            .iter()
            .map(|worksheet| worksheet.ordinal)
            .collect::<BTreeSet<_>>()
            .len()
            != identity.worksheets.len()
    {
        return Err(invalid_arguments(
            "Excel L1 workbook worksheet list is invalid",
        ));
    }
    Ok(workbook_path)
}
fn ensure_same_workbook(
    expected: &ExcelL1WorkbookIdentity,
    observed: &ExcelL1WorkbookIdentity,
) -> Result<(), ExcelT21Error> {
    if expected.tool_bus_session_id != observed.tool_bus_session_id
        || expected.task_id != observed.task_id
        || expected.workbook_handle != observed.workbook_handle
        || expected.workbook != observed.workbook
        || expected.owned_process != observed.owned_process
        || expected.worksheets != observed.worksheets
    {
        return Err(ExcelT21Error::new(
            ErrorCode::VerifyFailed,
            "Excel L1 owned workbook/session identity drifted",
        ));
    }
    Ok(())
}
fn parse_request_fingerprint(value: &str) -> Result<Fingerprint, ExcelT21Error> {
    Fingerprint::parse(value).map_err(|error| {
        invalid_arguments(format!("Excel L1 workbook fingerprint is invalid: {error}"))
    })
}
fn parse_runtime_fingerprint(value: &str) -> Result<Fingerprint, ExcelT21Error> {
    Fingerprint::parse(value).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel runtime returned an invalid fingerprint: {error}"),
        )
    })
}
fn undo_shape_error(error: &assistant_undo::UndoError) -> ExcelT21Error {
    invalid_arguments(format!("Excel L1 undo anchor shape is invalid: {error}"))
}

#[cfg(test)]
#[path = "excel_t21_l1_write.rs"]
pub(crate) mod write;

#[cfg(test)]
#[path = "excel_t21_l1_write_port.rs"]
pub(crate) mod write_port;

#[cfg(test)]
#[path = "excel_t21_l1_tests.rs"]
mod tests;
