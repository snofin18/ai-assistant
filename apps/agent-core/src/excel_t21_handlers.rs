//! Private pure-logic handlers for the Excel T2.1 read-only workflow.
//!
//! Responsibilities:
//! - parse and validate model-shaped arguments;
//! - acquire workbook-scoped sessions from the isolated registry;
//! - invoke the private runtime boundary and revalidate all returned shapes;
//! - produce bounded JSON payloads and pre/post workbook fingerprints.
//!
//! Boundaries:
//! - these handlers are not registered in a production `ToolBus`;
//! - the module does not open Excel, create COM objects, or save workbooks;
//! - the module returns [`ExcelT21Error`] because the current public
//!   `ToolBusError` cannot faithfully represent every ADR-0108 error category.
//!
//! Invariants:
//! 1. a runtime result is never trusted until ownership, identity, shape, and bounds
//!    pass a second validation;
//! 2. a read publishes equal pre/post fingerprints or fails;
//! 3. range payloads and workbook metadata are rejected, not truncated, above bounds.
//!
//! Related documents: ADR-0108, `docs/spec/excel-t21-readonly-workflow.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use assistant_protocol::ErrorCode;
use serde_json::{Map, Value, json};

use super::{
    CanonicalDecimal, CellValueType, ExcelT21Error, MAX_RANGE_CELLS, MAX_RANGE_PAYLOAD_BYTES,
    MAX_WORKBOOK_PAYLOAD_BYTES, OpenedWorkbook, RangeSelector, RangeSnapshot, ReadMode,
    RuntimeCell, SessionAccess, SessionRegistry, TOOL_RANGE_READ, TOOL_RANGE_SUMMARIZE,
    TOOL_WORKBOOK_OPEN, TOOL_WORKBOOK_READ, WorkbookHandle, WorkbookIdentity, WorkbookSnapshot,
    canonical_named_ranges, canonical_worksheets, invalid_arguments, open_worksheet_values,
    parse_a1_range, parse_range_selector, parse_workbook_path, runtime_shape_error,
    worksheet_layout_matches,
};

/// Private runtime boundary implemented by the fake now and by a COM runner later.
pub(crate) trait ExcelT21Runtime: Send + Sync {
    fn open_read_only(&self, path: &Path) -> Result<OpenedWorkbook, ExcelT21Error>;

    fn close(&self, handle: WorkbookHandle) -> Result<(), ExcelT21Error>;

    fn verify_ownership(
        &self,
        handle: WorkbookHandle,
        workbook: &WorkbookIdentity,
    ) -> Result<(), ExcelT21Error>;

    fn fingerprint(&self, handle: WorkbookHandle) -> Result<String, ExcelT21Error>;

    fn read_workbook(&self, handle: WorkbookHandle) -> Result<WorkbookSnapshot, ExcelT21Error>;

    fn read_range(
        &self,
        handle: WorkbookHandle,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<RangeSnapshot, ExcelT21Error>;
}

/// Call identity injected by assembly; no public `CallContext` field is added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcelCallContext {
    tool_bus_session_id: String,
    task_id: String,
}

impl ExcelCallContext {
    /// Creates a private call identity.
    pub(crate) fn new(
        tool_bus_session_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, ExcelT21Error> {
        let tool_bus_session_id = tool_bus_session_id.into();
        let task_id = task_id.into();
        if tool_bus_session_id.trim().is_empty() || task_id.trim().is_empty() {
            return Err(invalid_arguments(
                "tool_bus_session_id and task_id must not be empty",
            ));
        }
        Ok(Self {
            tool_bus_session_id,
            task_id,
        })
    }
}

/// Pure handler result prepared for a future `ToolBus` wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcelToolResult {
    /// Serializable payload.
    pub(crate) data: Value,
    /// Whether the payload contains application-derived content.
    pub(crate) untrusted: bool,
}

/// Four Excel T2.1 handlers over one private runtime and one isolated registry.
pub(crate) struct ExcelT21Handlers<R> {
    runtime: Arc<R>,
    pub(crate) sessions: Mutex<SessionRegistry>,
}

impl<R> ExcelT21Handlers<R>
where
    R: ExcelT21Runtime + 'static,
{
    /// Creates a private handler set; production assembly does not call this.
    pub(crate) const fn new(runtime: Arc<R>, sessions: SessionRegistry) -> Self {
        Self {
            runtime,
            sessions: Mutex::new(sessions),
        }
    }

    /// Handles `excel.workbook.open`.
    pub(crate) fn open(
        &self,
        call: &ExcelCallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        reject_unknown_keys(
            arguments,
            &["workbook_path", "read_only"],
            TOOL_WORKBOOK_OPEN,
        )?;
        let workbook_path = required_string(arguments, "workbook_path", TOOL_WORKBOOK_OPEN)?;
        if arguments.get("read_only").and_then(Value::as_bool) != Some(true) {
            return Err(invalid_arguments("read_only must be explicitly true"));
        }
        let path = parse_workbook_path(workbook_path)?;
        let opened = self.runtime.open_read_only(&path)?;
        let handle = opened.handle;
        if !workbook_path_matches(&path, &opened.workbook.full_path) {
            return self.fail_open_after_cleanup(
                handle,
                ExcelT21Error::new(
                    ErrorCode::Fatal,
                    "runtime opened a workbook different from the requested path",
                ),
            );
        }
        if let Err(error) = self.runtime.verify_ownership(handle, &opened.workbook) {
            return self
                .fail_open_after_cleanup(handle, ownership_error(error, TOOL_WORKBOOK_OPEN));
        }
        if let Err(error) = self.cleanup_expired_sessions() {
            return self.fail_open_after_cleanup(handle, error);
        }
        let registered = {
            let mut sessions = self.lock_sessions(TOOL_WORKBOOK_OPEN)?;
            sessions.register(&call.task_id, opened)
        };
        let registered = match registered {
            Ok(registered) => registered,
            Err(error) => {
                self.runtime.close(handle).map_err(|close_error| {
                    ExcelT21Error::new(
                        ErrorCode::Fatal,
                        format!(
                            "{TOOL_WORKBOOK_OPEN}: open rejected and cleanup failed: {close_error}"
                        ),
                    )
                })?;
                return Err(error);
            }
        };
        let expires_at_utc = format_unix_ms_utc(registered.expires_at_unix_ms)?;
        let access = {
            let sessions = self.lock_sessions(TOOL_WORKBOOK_OPEN)?;
            sessions.lookup(&call.tool_bus_session_id, &call.task_id, &registered.token)?
        };
        let worksheets = open_worksheet_values(&access)?;
        Ok(ExcelToolResult {
            data: json!({
                "session_id": registered.token,
                "expires_at_utc": expires_at_utc,
                "workbook": {
                    "identity_kind": "full_path",
                    "identity_value": access.workbook.full_path,
                    "read_only": true,
                },
                "worksheets": worksheets,
            }),
            untrusted: true,
        })
    }

    /// Handles `excel.workbook.read`.
    pub(crate) fn workbook_read(
        &self,
        call: &ExcelCallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        reject_unknown_keys(arguments, &["session_id"], TOOL_WORKBOOK_READ)?;
        let token = required_string(arguments, "session_id", TOOL_WORKBOOK_READ)?;
        let access = self.lookup(call, token, TOOL_WORKBOOK_READ)?;
        let before = self.fingerprint(&access, TOOL_WORKBOOK_READ)?;
        let snapshot = self.runtime.read_workbook(access.handle)?;
        let after = self.fingerprint(&access, TOOL_WORKBOOK_READ)?;
        ensure_unchanged(&before, &after, TOOL_WORKBOOK_READ)?;
        self.publish_workbook_snapshot(call, token, &access, &snapshot, &after)
    }

    /// Handles `excel.range.read`.
    pub(crate) fn range_read(
        &self,
        call: &ExcelCallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        reject_unknown_keys(arguments, &["session_id", "range", "mode"], TOOL_RANGE_READ)?;
        let token = required_string(arguments, "session_id", TOOL_RANGE_READ)?;
        let selector = arguments
            .get("range")
            .ok_or_else(|| invalid_arguments("range is required"))
            .and_then(parse_range_selector)?;
        ensure_declared_range_bound(&selector)?;
        let mode = parse_read_mode(required_string(arguments, "mode", TOOL_RANGE_READ)?)?;
        let (access, snapshot, fingerprint) =
            self.read_range(call, token, &selector, mode, TOOL_RANGE_READ)?;
        let normalized =
            normalize_cells(&selector, &snapshot, mode, fingerprint, &access.worksheets)?;
        let data = json!({
            "range": {
                "address": normalized.address,
                "rows": normalized.rows,
                "columns": normalized.columns,
            },
            "cells": normalized.payload,
            "fingerprint_scope": "workbook",
            "fingerprint": normalized.fingerprint,
            "previous_fingerprint": normalized.previous_fingerprint,
        });
        ensure_payload_bound(&data, MAX_RANGE_PAYLOAD_BYTES, TOOL_RANGE_READ)?;
        self.publish_after_revalidation(call, token, &access, data)
    }

    /// Handles `excel.range.summarize`.
    pub(crate) fn summarize(
        &self,
        call: &ExcelCallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        reject_unknown_keys(
            arguments,
            &["session_id", "range", "mode"],
            TOOL_RANGE_SUMMARIZE,
        )?;
        let token = required_string(arguments, "session_id", TOOL_RANGE_SUMMARIZE)?;
        let selector = arguments
            .get("range")
            .ok_or_else(|| invalid_arguments("range is required"))
            .and_then(parse_range_selector)?;
        ensure_declared_range_bound(&selector)?;
        let mode = parse_read_mode(required_string(arguments, "mode", TOOL_RANGE_SUMMARIZE)?)?;
        if mode != ReadMode::Value2 {
            return Err(invalid_arguments(
                "range summarize mode must be exactly value2",
            ));
        }
        let (access, snapshot, fingerprint) =
            self.read_range(call, token, &selector, mode, TOOL_RANGE_SUMMARIZE)?;
        let normalized =
            normalize_cells(&selector, &snapshot, mode, fingerprint, &access.worksheets)?;
        let summary = summarize_numbers(&normalized.cells)?;
        let data = json!({
            "range": {
                "address": normalized.address,
                "rows": normalized.rows,
                "columns": normalized.columns,
            },
            "count": normalized.cell_count,
            "non_numeric_count": summary.non_numeric_count,
            "sum": summary.sum,
            "min": summary.min,
            "max": summary.max,
            "fingerprint_scope": "workbook",
            "fingerprint": normalized.fingerprint,
            "previous_fingerprint": normalized.previous_fingerprint,
        });
        ensure_payload_bound(&data, MAX_RANGE_PAYLOAD_BYTES, TOOL_RANGE_SUMMARIZE)?;
        self.publish_after_revalidation(call, token, &access, data)
    }

    /// Invalidates the task token and closes its runtime handle.
    pub(crate) fn cleanup_task(&self, task_id: &str) -> Result<(), ExcelT21Error> {
        self.cleanup_expired_sessions()?;
        let closing = self
            .sessions
            .lock()
            .map_err(|_| {
                ExcelT21Error::new(ErrorCode::Fatal, "Excel session registry is poisoned")
            })?
            .begin_close_task(task_id);
        if let Some(closing) = closing {
            self.runtime.close(closing.handle)?;
            self.lock_sessions("excel.t21.cleanup")?
                .finish_close(closing.handle)?;
        }
        Ok(())
    }

    fn fail_open_after_cleanup(
        &self,
        handle: WorkbookHandle,
        error: ExcelT21Error,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        self.runtime.close(handle).map_err(|close_error| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                format!(
                    "{TOOL_WORKBOOK_OPEN}: open validation failed and cleanup failed: {close_error}"
                ),
            )
        })?;
        Err(error)
    }

    fn cleanup_expired_sessions(&self) -> Result<(), ExcelT21Error> {
        let expired = self.lock_sessions("excel.t21.cleanup")?.expire_sessions();
        for session in expired {
            self.runtime.close(session.handle).map_err(|error| {
                ExcelT21Error::new(
                    ErrorCode::Fatal,
                    format!("expired Excel session cleanup failed: {error}"),
                )
            })?;
            self.lock_sessions("excel.t21.cleanup")?
                .finish_close(session.handle)?;
        }
        Ok(())
    }

    fn publish_workbook_snapshot(
        &self,
        call: &ExcelCallContext,
        token: &str,
        access: &SessionAccess,
        snapshot: &WorkbookSnapshot,
        fingerprint: &str,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        if snapshot.workbook != access.workbook {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "workbook identity drifted during metadata read",
            ));
        }
        let snapshot_worksheets = canonical_worksheets(&snapshot.worksheets)?;
        if !worksheet_layout_matches(&access.worksheets, &snapshot.worksheets) {
            return Err(runtime_shape_error(
                "worksheet metadata changed without a matching workbook identity",
            ));
        }
        let named_ranges = canonical_named_ranges(&snapshot.named_ranges)?;
        let remaining_ms = access.remaining_ms;
        let data = json!({
            "workbook": {
                "identity_kind": "full_path",
                "identity_value": snapshot.workbook.full_path,
                "read_only": true,
            },
            "worksheets": snapshot_worksheets,
            "named_ranges": named_ranges,
            "calculation": {
                "mode": snapshot.calculation.mode,
                "state": snapshot.calculation.state,
            },
            "session": {
                "expires_at_utc": format_unix_ms_utc(access.expires_at_unix_ms)?,
                "remaining_ms": remaining_ms,
            },
            "fingerprint_scope": "workbook",
            "fingerprint": fingerprint,
            "previous_fingerprint": fingerprint,
        });
        ensure_payload_bound(&data, MAX_WORKBOOK_PAYLOAD_BYTES, TOOL_WORKBOOK_READ)?;
        self.publish_after_revalidation(call, token, access, data)
    }

    fn read_range(
        &self,
        call: &ExcelCallContext,
        token: &str,
        selector: &RangeSelector,
        mode: ReadMode,
        tool: &str,
    ) -> Result<(SessionAccess, RangeSnapshot, String), ExcelT21Error> {
        let access = self.lookup(call, token, tool)?;
        let before = self.fingerprint(&access, tool)?;
        let snapshot = self.runtime.read_range(access.handle, selector, mode)?;
        let after = self.fingerprint(&access, tool)?;
        ensure_unchanged(&before, &after, tool)?;
        Ok((access, snapshot, after))
    }

    fn publish_after_revalidation(
        &self,
        call: &ExcelCallContext,
        token: &str,
        access: &SessionAccess,
        data: Value,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        self.cleanup_expired_sessions()?;
        let current = {
            let sessions = self.lock_sessions("excel.t21")?;
            let result = sessions.revalidate(&call.tool_bus_session_id, &call.task_id, token);
            drop(sessions);
            if result.is_err() {
                self.cleanup_expired_sessions()?;
            }
            result?
        };
        if current.handle != access.handle || current.workbook != access.workbook {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel session identity changed during the read",
            ));
        }
        Ok(ExcelToolResult {
            data,
            untrusted: true,
        })
    }

    fn lookup(
        &self,
        call: &ExcelCallContext,
        token: &str,
        tool: &str,
    ) -> Result<SessionAccess, ExcelT21Error> {
        self.cleanup_expired_sessions()?;
        let access = {
            let sessions = self.lock_sessions(tool)?;
            let result = sessions.lookup(&call.tool_bus_session_id, &call.task_id, token);
            drop(sessions);
            if result.is_err() {
                self.cleanup_expired_sessions()?;
            }
            result?
        };
        self.runtime
            .verify_ownership(access.handle, &access.workbook)
            .map_err(|error| ownership_error(error, tool))?;
        Ok(access)
    }

    fn fingerprint(&self, access: &SessionAccess, tool: &str) -> Result<String, ExcelT21Error> {
        let fingerprint = self.runtime.fingerprint(access.handle)?;
        if fingerprint.trim().is_empty() {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                format!("{tool}: runtime returned an empty workbook fingerprint"),
            ));
        }
        Ok(fingerprint)
    }

    fn lock_sessions(
        &self,
        tool: &str,
    ) -> Result<std::sync::MutexGuard<'_, SessionRegistry>, ExcelT21Error> {
        self.sessions.lock().map_err(|_| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                format!("{tool}: Excel session registry is poisoned"),
            )
        })
    }
}

#[derive(Debug)]
struct NormalizedCells {
    address: String,
    rows: u64,
    columns: u64,
    cell_count: u64,
    cells: Vec<NormalizedCell>,
    payload: Vec<Value>,
    fingerprint: String,
    previous_fingerprint: String,
}

#[derive(Debug)]
struct NormalizedCell {
    value_type: CellValueType,
    value_text: String,
}

#[derive(Debug)]
struct Summary {
    non_numeric_count: u64,
    sum: String,
    min: String,
    max: String,
}

fn normalize_cells(
    selector: &RangeSelector,
    snapshot: &RangeSnapshot,
    mode: ReadMode,
    fingerprint: String,
    worksheets: &[super::WorksheetInfo],
) -> Result<NormalizedCells, ExcelT21Error> {
    let (range_start, range_end) = validate_range_anchor(selector, snapshot, worksheets)?;
    let rows = u64::from(snapshot.rows);
    let columns = u64::from(snapshot.columns);
    let cell_count = rows
        .checked_mul(columns)
        .ok_or_else(|| invalid_arguments("range cell count overflows"))?;
    if cell_count > MAX_RANGE_CELLS {
        return Err(invalid_arguments(format!(
            "range has {cell_count} cells; maximum is {MAX_RANGE_CELLS}"
        )));
    }
    if let Some(expected) = selector.declared_cell_count()
        && expected != cell_count
    {
        return Err(runtime_shape_error(
            "runtime range dimensions differ from the requested A1 range",
        ));
    }
    let returned_cells = u64::try_from(snapshot.cells.len()).map_err(|_| {
        ExcelT21Error::new(ErrorCode::Fatal, "returned cell count does not fit u64")
    })?;
    if returned_cells != cell_count {
        return Err(runtime_shape_error(
            "runtime range returned a partial cell set",
        ));
    }
    let mut normalized = BTreeMap::new();
    for cell in &snapshot.cells {
        if cell.row < range_start.row
            || cell.column < range_start.column
            || cell.row > range_end.row
            || cell.column > range_end.column
        {
            return Err(runtime_shape_error(
                "runtime cell coordinate is outside the returned range",
            ));
        }
        let value_text = normalize_cell_value(cell)?;
        let previous = normalized.insert(
            (cell.row, cell.column),
            NormalizedCell {
                value_type: cell.value_type,
                value_text,
            },
        );
        if previous.is_some() {
            return Err(runtime_shape_error(
                "runtime range returned a duplicate cell coordinate",
            ));
        }
    }
    let payload = normalized
        .iter()
        .map(|((row, column), cell)| -> Result<Value, ExcelT21Error> {
            Ok(json!({
                "row": row,
                "column": column,
                "address": cell_address(*row, *column)?,
                "mode": mode.as_str(),
                "value_type": cell.value_type.as_str(),
                "value_text": cell.value_text,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NormalizedCells {
        address: snapshot.address.clone(),
        rows,
        columns,
        cell_count,
        cells: normalized.into_values().collect(),
        payload,
        previous_fingerprint: fingerprint.clone(),
        fingerprint,
    })
}

fn snapshot_range_bounds(address: &str) -> Result<(super::A1Cell, super::A1Cell), ExcelT21Error> {
    let range = address.rsplit_once('!').map_or(address, |(_, range)| range);
    parse_a1_range(range)
}

fn validate_range_anchor(
    selector: &RangeSelector,
    snapshot: &RangeSnapshot,
    worksheets: &[super::WorksheetInfo],
) -> Result<(super::A1Cell, super::A1Cell), ExcelT21Error> {
    if snapshot.rows == 0 || snapshot.columns == 0 || snapshot.address.trim().is_empty() {
        return Err(runtime_shape_error(
            "runtime range shape must have a non-empty address and positive dimensions",
        ));
    }
    let (range_start, range_end) = snapshot_range_bounds(&snapshot.address)?;
    let declared_rows = range_end
        .row
        .checked_sub(range_start.row)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| runtime_shape_error("runtime range row bounds overflow"))?;
    let declared_columns = range_end
        .column
        .checked_sub(range_start.column)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| runtime_shape_error("runtime range column bounds overflow"))?;
    if declared_rows != snapshot.rows || declared_columns != snapshot.columns {
        return Err(runtime_shape_error(
            "runtime range dimensions differ from its returned address",
        ));
    }
    if let RangeSelector::A1 { start, end, .. } = selector
        && (*start != range_start || *end != range_end)
    {
        return Err(runtime_shape_error(
            "runtime range address does not match the requested A1 range",
        ));
    }
    if let RangeSelector::A1 { worksheet, .. } = selector {
        let expected = match worksheet {
            super::WorksheetSelector::Name(name) => {
                worksheets.iter().find(|candidate| candidate.name == *name)
            }
            super::WorksheetSelector::Ordinal(ordinal) => worksheets
                .iter()
                .find(|candidate| candidate.ordinal == *ordinal),
        }
        .ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "requested worksheet is not present in the open session",
            )
        })?;
        if let Some((reported_name, _)) = snapshot.address.rsplit_once('!')
            && reported_name != expected.name
        {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "runtime range address identifies a different worksheet",
            ));
        }
    }
    Ok((range_start, range_end))
}

fn normalize_cell_value(cell: &RuntimeCell) -> Result<String, ExcelT21Error> {
    match cell.value_type {
        CellValueType::Blank => {
            if cell.value_text.is_empty() {
                Ok(String::new())
            } else {
                Err(runtime_shape_error("blank cell value_text must be empty"))
            }
        }
        CellValueType::Number => {
            CanonicalDecimal::parse(&cell.value_text).and_then(|number| number.as_text())
        }
        CellValueType::Text => Ok(cell.value_text.clone()),
        CellValueType::Boolean => match cell.value_text.as_str() {
            "true" | "false" => Ok(cell.value_text.clone()),
            _ => Err(runtime_shape_error(
                "boolean cell value_text must be true or false",
            )),
        },
        CellValueType::Error => {
            if is_known_excel_error(&cell.value_text) {
                Ok(cell.value_text.clone())
            } else {
                Err(runtime_shape_error("cell contains an unknown Excel error"))
            }
        }
    }
}

fn summarize_numbers(cells: &[NormalizedCell]) -> Result<Summary, ExcelT21Error> {
    let mut numbers = Vec::new();
    let mut non_numeric_count = 0_u64;
    for cell in cells {
        if cell.value_type == CellValueType::Number {
            numbers.push(CanonicalDecimal::parse(&cell.value_text)?);
        } else {
            non_numeric_count = non_numeric_count.saturating_add(1);
        }
    }
    let Some(first) = numbers.first() else {
        return Err(invalid_arguments(
            "range summarize requires at least one finite numeric cell",
        ));
    };
    let mut sum = CanonicalDecimal::parse("0")?;
    let mut minimum = first.clone();
    let mut maximum = first.clone();
    for number in &numbers {
        sum = sum.checked_add(number)?;
        if number.cmp_value(&minimum) == std::cmp::Ordering::Less {
            minimum = number.clone();
        }
        if number.cmp_value(&maximum) == std::cmp::Ordering::Greater {
            maximum = number.clone();
        }
    }
    Ok(Summary {
        non_numeric_count,
        sum: sum.as_text()?,
        min: minimum.as_text()?,
        max: maximum.as_text()?,
    })
}

fn parse_read_mode(value: &str) -> Result<ReadMode, ExcelT21Error> {
    match value {
        "value2" => Ok(ReadMode::Value2),
        "formula" => Ok(ReadMode::Formula),
        "text" => Ok(ReadMode::Text),
        _ => Err(invalid_arguments(
            "mode must be `value2`, `formula`, or `text`",
        )),
    }
}

fn ensure_declared_range_bound(selector: &RangeSelector) -> Result<(), ExcelT21Error> {
    if let Some(count) = selector.declared_cell_count()
        && count > MAX_RANGE_CELLS
    {
        return Err(invalid_arguments(format!(
            "A1 range has {count} cells; maximum is {MAX_RANGE_CELLS}"
        )));
    }
    Ok(())
}

fn ensure_unchanged(before: &str, after: &str, tool: &str) -> Result<(), ExcelT21Error> {
    if before != after {
        return Err(ExcelT21Error::new(
            ErrorCode::VerifyFailed,
            format!("{tool}: workbook fingerprint changed during a read"),
        ));
    }
    Ok(())
}
fn ensure_payload_bound(data: &Value, maximum: usize, tool: &str) -> Result<(), ExcelT21Error> {
    let code = if tool == TOOL_WORKBOOK_READ {
        ErrorCode::UserInteraction
    } else {
        ErrorCode::ToolInvalidArgs
    };
    let bytes = serde_json::to_vec(data).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("{tool}: payload serialization failed: {error}"),
        )
    })?;
    if bytes.len() > maximum {
        return Err(ExcelT21Error::new(
            code,
            format!("{tool}: payload exceeds {maximum} bytes"),
        ));
    }
    Ok(())
}
fn ownership_error(error: ExcelT21Error, tool: &str) -> ExcelT21Error {
    if error.code() == ErrorCode::TargetNotFound {
        error
    } else {
        ExcelT21Error::new(
            ErrorCode::TargetNotFound,
            format!("{tool}: owned workbook identity check failed: {error}"),
        )
    }
}
fn cell_address(row: u32, column: u32) -> Result<String, ExcelT21Error> {
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
fn reject_unknown_keys(
    arguments: &Map<String, Value>,
    allowed: &[&str],
    tool: &str,
) -> Result<(), ExcelT21Error> {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    if let Some(unknown) = arguments.keys().find(|key| !allowed.contains(key.as_str())) {
        return Err(invalid_arguments(format!(
            "{tool} contains unknown argument `{unknown}`"
        )));
    }
    Ok(())
}

fn required_string<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<&'a str, ExcelT21Error> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_arguments(format!("{tool}: {field} must be a non-empty string")))
}

/// Creates one deterministic runtime cell for handler tests.
pub(crate) fn runtime_cell(
    row: u32,
    column: u32,
    value_type: CellValueType,
    value_text: &str,
) -> RuntimeCell {
    RuntimeCell {
        row,
        column,
        value_type,
        value_text: value_text.to_owned(),
    }
}

fn workbook_path_matches(requested: &Path, reported: &str) -> bool {
    let Ok(reported) = parse_workbook_path(reported) else {
        return false;
    };
    normalize_windows_path(requested) == normalize_windows_path(&reported)
}

fn normalize_windows_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
        .trim_end_matches('\\')
        .to_owned()
}

/// Formats Unix milliseconds as a second-precision UTC timestamp.
fn format_unix_ms_utc(unix_ms: i64) -> Result<String, ExcelT21Error> {
    let seconds = unix_ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let hour = seconds_of_day / 3600;
    let minute = (seconds_of_day % 3600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_from_days(days)?;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_from_days(days_since_epoch: i64) -> Result<(i64, i64, i64), ExcelT21Error> {
    let shifted = days_since_epoch
        .checked_add(719_468)
        .ok_or_else(|| ExcelT21Error::new(ErrorCode::Fatal, "UTC day conversion overflowed"))?;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    Ok((year, month, day))
}
