//! Test-only Excel T2.2 scalar range write transaction.
//!
//! Responsibilities:
//! - bind a validated scalar literal to the selector captured by an L1 anchor;
//! - execute one fake write, recalculation, and complete `value2`/`formula` readback;
//! - record the post-write fingerprint only after every readback postcondition passes.
//!
//! Boundaries:
//! - no Excel, COM object, GUI, filesystem shadow copy, save, or restore is touched;
//! - the port accepts only self-contained workbook identities and closed selectors;
//! - this module is test-only and is not reachable from production assembly or `ToolBus`.
//!
//! Invariants:
//! 1. scalar parsing and selector binding complete before the anchor is consumed;
//! 2. a prepared write is single-use, so any port failure cannot be retried on the same anchor;
//! 3. post-fingerprint is recorded only after both read modes pass complete-shape validation.
//!
//! Related documents: ADR-0062, ADR-0107, ADR-0108, TASK-404, TASK-405.

use assistant_protocol::ErrorCode;
use std::sync::{Arc, Mutex};

use super::super::{
    CellValueType, ExcelT21Error, MAX_RANGE_CELLS, MAX_RANGE_PAYLOAD_BYTES, RangeSelector,
    RangeSnapshot, ReadMode, RuntimeCell, WorksheetInfo, WorksheetSelector, invalid_arguments,
    parse_a1_range, runtime_shape_error,
};
use super::write::ExcelL1ScalarLiteral;
use super::{
    ExcelL1AnchorHandle, ExcelL1AnchorStore, ExcelL1ValueSnapshot, ExcelL1WorkbookIdentity,
    cell_address, selector_value,
};

/// One readback carrying the mode requested from the private port.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExcelL1RangeReadback {
    mode: ReadMode,
    snapshot: RangeSnapshot,
}

pub(super) fn canonical_snapshot_address(
    selector: &RangeSelector,
    snapshot: &RangeSnapshot,
    worksheets: &[WorksheetInfo],
) -> Result<String, ExcelT21Error> {
    let RangeSelector::A1 {
        worksheet,
        start,
        end,
    } = selector
    else {
        return Ok(snapshot.address.trim().to_owned());
    };
    let worksheet_name = match worksheet {
        WorksheetSelector::Name(name) => worksheets
            .iter()
            .find(|candidate| candidate.name == *name)
            .map(|candidate| candidate.name.as_str())
            .ok_or_else(|| {
                ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel L1 named worksheet is absent from the open workbook",
                )
            })?,
        WorksheetSelector::Ordinal(ordinal) => worksheets
            .iter()
            .find(|candidate| candidate.ordinal == *ordinal)
            .map(|candidate| candidate.name.as_str())
            .ok_or_else(|| {
                ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel L1 ordinal selector is absent from the open workbook",
                )
            })?,
    };
    let start_address = cell_address(start.row, start.column)?;
    let end_address = cell_address(end.row, end.column)?;
    Ok(if start_address == end_address {
        format!("{worksheet_name}!{start_address}")
    } else {
        format!("{worksheet_name}!{start_address}:{end_address}")
    })
}

/// Successful result of one fake scalar range write transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExcelL1ScalarWriteReceipt {
    value_type: CellValueType,
    value_text: String,
    formula_text: String,
    cell_count: usize,
    post_fingerprint: String,
}

impl ExcelL1ScalarWriteReceipt {
    const fn value_type(&self) -> CellValueType {
        self.value_type
    }

    fn value_text(&self) -> &str {
        &self.value_text
    }

    fn formula_text(&self) -> &str {
        &self.formula_text
    }

    const fn cell_count(&self) -> usize {
        self.cell_count
    }

    fn post_fingerprint(&self) -> &str {
        &self.post_fingerprint
    }
}

/// Private port implemented by fake tests now and by a future COM adapter separately.
trait ExcelL1WritePort {
    fn write_scalar(
        &mut self,
        workbook: &ExcelL1WorkbookIdentity,
        selector: &RangeSelector,
        literal: &ExcelL1ScalarLiteral,
    ) -> Result<(), ExcelT21Error>;

    fn recalculate(
        &mut self,
        workbook: &ExcelL1WorkbookIdentity,
        selector: &RangeSelector,
    ) -> Result<(), ExcelT21Error>;

    fn read_range(
        &mut self,
        workbook: &ExcelL1WorkbookIdentity,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<ExcelL1RangeReadback, ExcelT21Error>;

    fn fingerprint(&mut self, workbook: &ExcelL1WorkbookIdentity) -> Result<String, ExcelT21Error>;
}

/// Runs one fully checked scalar write transaction.
///
/// The scalar is parsed before the anchor is consumed. After preparation, every side effect is
/// irreversible for this anchor: a write, recalculation, readback, verification, or fingerprint
/// failure returns explicitly and cannot be converted into a successful retry.
fn execute_scalar_write<P>(
    store: &mut ExcelL1AnchorStore,
    handle: ExcelL1AnchorHandle,
    observed: &ExcelL1WorkbookIdentity,
    selector: &RangeSelector,
    input_json: &str,
    port: &mut P,
) -> Result<ExcelL1ScalarWriteReceipt, ExcelT21Error>
where
    P: ExcelL1WritePort,
{
    let literal = ExcelL1ScalarLiteral::parse(input_json)?;
    let anchor_snapshot = validate_anchor_selector(store, handle, selector)?;
    store.prepare_write(handle, observed)?;

    port.write_scalar(observed, selector, &literal)?;
    port.recalculate(observed, selector)?;
    let value_readback = port.read_range(observed, selector, ReadMode::Value2)?;
    let formula_readback = port.read_range(observed, selector, ReadMode::Formula)?;
    let cell_count = validate_readback_pair(
        selector,
        observed,
        &anchor_snapshot,
        &literal,
        &value_readback,
        &formula_readback,
    )?;

    let post_fingerprint = port.fingerprint(observed)?;
    if post_fingerprint == observed.fingerprint {
        return Err(ExcelT21Error::new(
            ErrorCode::VerifyFailed,
            "Excel scalar write did not change the workbook fingerprint",
        ));
    }
    let mut post_observed = observed.clone();
    post_observed.fingerprint = post_fingerprint.clone();
    store.record_post_write(handle, &post_observed)?;

    Ok(ExcelL1ScalarWriteReceipt {
        value_type: literal.value_type(),
        value_text: literal.value_text().to_owned(),
        formula_text: expected_formula_text(&literal),
        cell_count,
        post_fingerprint,
    })
}

fn validate_anchor_selector(
    store: &ExcelL1AnchorStore,
    handle: ExcelL1AnchorHandle,
    selector: &RangeSelector,
) -> Result<ExcelL1ValueSnapshot, ExcelT21Error> {
    validate_selector_shape(selector)?;
    let expected = selector_value(selector)?;
    let captured = store.value_snapshot(handle)?.clone();
    if captured.selector() != &expected {
        return Err(ExcelT21Error::new(
            ErrorCode::TargetNotFound,
            "Excel scalar write selector differs from the captured L1 range",
        ));
    }
    Ok(captured)
}

fn validate_selector_shape(selector: &RangeSelector) -> Result<(), ExcelT21Error> {
    match selector {
        RangeSelector::NamedRange(name) if !name.trim().is_empty() => Ok(()),
        RangeSelector::NamedRange(_) => {
            Err(invalid_arguments("Excel scalar write named range is empty"))
        }
        RangeSelector::A1 {
            worksheet,
            start,
            end,
        } => {
            match worksheet {
                WorksheetSelector::Name(name) if name.trim().is_empty() => {
                    return Err(invalid_arguments(
                        "Excel scalar write worksheet name is empty",
                    ));
                }
                WorksheetSelector::Ordinal(0) => {
                    return Err(invalid_arguments(
                        "Excel scalar write worksheet ordinal must be positive",
                    ));
                }
                WorksheetSelector::Name(_) | WorksheetSelector::Ordinal(_) => {}
            }
            if start.row == 0
                || start.column == 0
                || end.row == 0
                || end.column == 0
                || end.row < start.row
                || end.column < start.column
            {
                return Err(invalid_arguments(
                    "Excel scalar write A1 bounds are invalid",
                ));
            }
            Ok(())
        }
    }
}

fn validate_readback_pair(
    selector: &RangeSelector,
    workbook: &ExcelL1WorkbookIdentity,
    anchor_snapshot: &ExcelL1ValueSnapshot,
    literal: &ExcelL1ScalarLiteral,
    value_readback: &ExcelL1RangeReadback,
    formula_readback: &ExcelL1RangeReadback,
) -> Result<usize, ExcelT21Error> {
    if value_readback.mode != ReadMode::Value2 {
        return Err(runtime_shape_error(
            "Excel scalar write value2 readback returned a different mode",
        ));
    }
    if formula_readback.mode != ReadMode::Formula {
        return Err(runtime_shape_error(
            "Excel scalar write formula readback returned a different mode",
        ));
    }

    let value_shape = validate_readback_shape(
        selector,
        workbook,
        anchor_snapshot,
        &value_readback.snapshot,
    )?;
    let formula_shape = validate_readback_shape(
        selector,
        workbook,
        anchor_snapshot,
        &formula_readback.snapshot,
    )?;
    if value_shape != formula_shape {
        return Err(runtime_shape_error(
            "Excel scalar write readbacks describe different ranges",
        ));
    }

    for cell in &value_readback.snapshot.cells {
        validate_value_cell(literal, cell)?;
    }
    for cell in &formula_readback.snapshot.cells {
        validate_formula_cell(literal, cell)?;
    }
    Ok(value_shape.cell_count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadbackShape {
    address: String,
    rows: u32,
    columns: u32,
    cell_count: usize,
}

fn validate_readback_shape(
    selector: &RangeSelector,
    workbook: &ExcelL1WorkbookIdentity,
    anchor_snapshot: &ExcelL1ValueSnapshot,
    snapshot: &RangeSnapshot,
) -> Result<ReadbackShape, ExcelT21Error> {
    if snapshot.address.trim().is_empty() || snapshot.rows == 0 || snapshot.columns == 0 {
        return Err(runtime_shape_error(
            "Excel scalar write readback has an empty address or zero dimensions",
        ));
    }
    let expected_cells = u64::from(snapshot.rows)
        .checked_mul(u64::from(snapshot.columns))
        .ok_or_else(|| runtime_shape_error("Excel scalar write readback cell count overflowed"))?;
    if expected_cells > MAX_RANGE_CELLS
        || expected_cells
            != u64::try_from(snapshot.cells.len())
                .map_err(|_| runtime_shape_error("readback cell count does not fit u64"))?
    {
        return Err(runtime_shape_error(
            "Excel scalar write readback is partial or exceeds the cell bound",
        ));
    }
    if snapshot.rows != anchor_snapshot.rows()
        || snapshot.columns != anchor_snapshot.columns()
        || snapshot.cells.len() != anchor_snapshot.cell_count()
    {
        return Err(runtime_shape_error(
            "Excel scalar write readback dimensions differ from the captured L1 range",
        ));
    }

    let mut seen = std::collections::BTreeSet::new();
    for cell in &snapshot.cells {
        if cell.row == 0
            || cell.column == 0
            || cell.row > snapshot.rows
            || cell.column > snapshot.columns
        {
            return Err(runtime_shape_error(
                "Excel scalar write readback cell is outside its range",
            ));
        }
        if !seen.insert((cell.row, cell.column)) {
            return Err(runtime_shape_error(
                "Excel scalar write readback contains duplicate cells",
            ));
        }
    }
    let payload_cells = snapshot
        .cells
        .iter()
        .map(|cell| {
            serde_json::json!({
                "row": cell.row,
                "column": cell.column,
                "value_type": cell.value_type.as_str(),
                "value_text": cell.value_text,
            })
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "address": snapshot.address,
        "rows": snapshot.rows,
        "columns": snapshot.columns,
        "cells": payload_cells,
    });
    let payload_bytes = serde_json::to_vec(&payload).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel scalar write readback serialization failed: {error}"),
        )
    })?;
    if payload_bytes.len() > MAX_RANGE_PAYLOAD_BYTES {
        return Err(invalid_arguments(format!(
            "Excel scalar write readback exceeds {MAX_RANGE_PAYLOAD_BYTES} bytes"
        )));
    }
    validate_readback_address(selector, workbook, anchor_snapshot.address(), snapshot)?;
    Ok(ReadbackShape {
        address: snapshot.address.clone(),
        rows: snapshot.rows,
        columns: snapshot.columns,
        cell_count: snapshot.cells.len(),
    })
}

fn validate_readback_address(
    selector: &RangeSelector,
    workbook: &ExcelL1WorkbookIdentity,
    captured_address: &str,
    snapshot: &RangeSnapshot,
) -> Result<(), ExcelT21Error> {
    let actual_address = match selector {
        RangeSelector::A1 {
            worksheet,
            start,
            end,
        } => {
            let worksheet_name = worksheet_name(workbook, worksheet)?;
            let (reported_worksheet, range) = snapshot
                .address
                .rsplit_once('!')
                .map_or((None, snapshot.address.as_str()), |(name, range)| {
                    (Some(name), range)
                });
            if reported_worksheet.is_some_and(|name| name != worksheet_name) {
                return Err(runtime_shape_error(
                    "Excel scalar write readback identifies a different worksheet",
                ));
            }
            let (reported_start, reported_end) = parse_a1_range(range)?;
            if reported_start != *start || reported_end != *end {
                return Err(runtime_shape_error(
                    "Excel scalar write readback address differs from the requested A1 range",
                ));
            }
            let start_address = cell_address(reported_start.row, reported_start.column)?;
            let end_address = cell_address(reported_end.row, reported_end.column)?;
            if start_address == end_address {
                format!("{worksheet_name}!{start_address}")
            } else {
                format!("{worksheet_name}!{start_address}:{end_address}")
            }
        }
        RangeSelector::NamedRange(_) => snapshot.address.trim().to_owned(),
    };
    if actual_address != captured_address {
        return Err(runtime_shape_error(
            "Excel scalar write readback address differs from the captured L1 range",
        ));
    }
    Ok(())
}

fn worksheet_name<'a>(
    workbook: &'a ExcelL1WorkbookIdentity,
    selector: &WorksheetSelector,
) -> Result<&'a str, ExcelT21Error> {
    match selector {
        WorksheetSelector::Name(name) => workbook
            .worksheets
            .iter()
            .find(|worksheet| worksheet.name == *name)
            .map(|worksheet| worksheet.name.as_str())
            .ok_or_else(|| {
                ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel scalar write worksheet name is absent from the open workbook",
                )
            }),
        WorksheetSelector::Ordinal(ordinal) => workbook
            .worksheets
            .iter()
            .find(|worksheet| worksheet.ordinal == *ordinal)
            .map(|worksheet| worksheet.name.as_str())
            .ok_or_else(|| {
                ExcelT21Error::new(
                    ErrorCode::TargetNotFound,
                    "Excel scalar write worksheet ordinal is absent from the open workbook",
                )
            }),
    }
}

fn validate_value_cell(
    literal: &ExcelL1ScalarLiteral,
    cell: &RuntimeCell,
) -> Result<(), ExcelT21Error> {
    if cell.value_type != literal.value_type() || cell.value_text != literal.value_text() {
        return Err(ExcelT21Error::new(
            ErrorCode::VerifyFailed,
            "Excel scalar write value2 readback differs from the written literal",
        ));
    }
    Ok(())
}

fn validate_formula_cell(
    literal: &ExcelL1ScalarLiteral,
    cell: &RuntimeCell,
) -> Result<(), ExcelT21Error> {
    let expected_type = match literal.value_type() {
        CellValueType::Blank => CellValueType::Blank,
        CellValueType::Number | CellValueType::Text | CellValueType::Boolean => CellValueType::Text,
        CellValueType::Error => {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel scalar literal resolved to an unsupported error type",
            ));
        }
    };
    let expected_text = expected_formula_text(literal);
    if cell.value_type != expected_type
        || cell.value_text != expected_text
        || (literal.value_type() == CellValueType::Text && has_formula_prefix(&cell.value_text))
    {
        return Err(ExcelT21Error::new(
            ErrorCode::VerifyFailed,
            "Excel scalar write formula readback differs from the literal or contains a formula",
        ));
    }
    Ok(())
}

fn expected_formula_text(literal: &ExcelL1ScalarLiteral) -> String {
    match (literal.value_type(), literal.value_text()) {
        (CellValueType::Boolean, "true") => "TRUE".to_owned(),
        (CellValueType::Boolean, "false") => "FALSE".to_owned(),
        (CellValueType::Number | CellValueType::Text, value) => value.to_owned(),
        (CellValueType::Blank | CellValueType::Boolean | CellValueType::Error, _) => String::new(),
    }
}

fn has_formula_prefix(value: &str) -> bool {
    matches!(
        value
            .trim_start_matches(|character: char| {
                character.is_whitespace() || character.is_control()
            })
            .chars()
            .next(),
        Some('=' | '+' | '-' | '@')
    )
}

#[derive(Debug, Default)]
struct FakeState {
    calls: Vec<&'static str>,
    written_value_type: Option<CellValueType>,
    written_text: Option<String>,
    last_selector: Option<RangeSelector>,
    fail_write: Option<ErrorCode>,
    fail_recalculate: Option<ErrorCode>,
    fail_read_value2: Option<ErrorCode>,
    fail_read_formula: Option<ErrorCode>,
    fail_fingerprint: Option<ErrorCode>,
    value_mode_override: Option<ReadMode>,
    formula_mode_override: Option<ReadMode>,
    value_snapshot_override: Option<RangeSnapshot>,
    formula_snapshot_override: Option<RangeSnapshot>,
    post_fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
struct FakePort {
    state: Arc<Mutex<FakeState>>,
}

impl FakePort {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState::default())),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, FakeState>, ExcelT21Error> {
        self.state.lock().map_err(|_| {
            ExcelT21Error::new(ErrorCode::Fatal, "fake Excel write-port state is poisoned")
        })
    }

    fn call_names(&self) -> Result<Vec<&'static str>, String> {
        self.lock()
            .map(|state| state.calls.clone())
            .map_err(|error| error.to_string())
    }
}

impl ExcelL1WritePort for FakePort {
    fn write_scalar(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
        selector: &RangeSelector,
        literal: &ExcelL1ScalarLiteral,
    ) -> Result<(), ExcelT21Error> {
        let mut state = self.lock()?;
        state.calls.push("write");
        state.last_selector = Some(selector.clone());
        state.written_value_type = Some(literal.value_type());
        state.written_text = Some(literal.value_text().to_owned());
        state.fail_write.map_or(Ok(()), |code| {
            Err(ExcelT21Error::new(code, "scripted write failure"))
        })
    }

    fn recalculate(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
        _selector: &RangeSelector,
    ) -> Result<(), ExcelT21Error> {
        let mut state = self.lock()?;
        state.calls.push("recalculate");
        state.fail_recalculate.map_or(Ok(()), |code| {
            Err(ExcelT21Error::new(code, "scripted recalculation failure"))
        })
    }

    fn read_range(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<ExcelL1RangeReadback, ExcelT21Error> {
        let mut state = self.lock()?;
        state.calls.push(match mode {
            ReadMode::Value2 => "read:value2",
            ReadMode::Formula => "read:formula",
            ReadMode::Text => "read:text",
        });
        let failure = match mode {
            ReadMode::Value2 => state.fail_read_value2,
            ReadMode::Formula => state.fail_read_formula,
            ReadMode::Text => None,
        };
        if let Some(code) = failure {
            return Err(ExcelT21Error::new(code, "scripted range read failure"));
        }
        let written_value_type = state.written_value_type.ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::Fatal, "fake write did not record a value type")
        })?;
        let written_text = state.written_text.clone().ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::Fatal, "fake write did not record value text")
        })?;
        let (mode_override, snapshot_override) = match mode {
            ReadMode::Value2 => (
                state.value_mode_override,
                state.value_snapshot_override.clone(),
            ),
            ReadMode::Formula => (
                state.formula_mode_override,
                state.formula_snapshot_override.clone(),
            ),
            ReadMode::Text => (None, None),
        };
        let readback_mode = mode_override.unwrap_or(mode);
        drop(state);
        let snapshot = snapshot_override.unwrap_or_else(|| {
            fake_snapshot_for_write(selector, mode, written_value_type, &written_text)
        });
        Ok(ExcelL1RangeReadback {
            mode: readback_mode,
            snapshot,
        })
    }

    fn fingerprint(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
    ) -> Result<String, ExcelT21Error> {
        let mut state = self.lock()?;
        state.calls.push("fingerprint");
        if let Some(code) = state.fail_fingerprint {
            return Err(ExcelT21Error::new(code, "scripted fingerprint failure"));
        }
        Ok(state
            .post_fingerprint
            .clone()
            .unwrap_or_else(|| fake_fingerprint('b')))
    }
}

fn fake_fingerprint(value: char) -> String {
    format!("sha256:{}", value.to_string().repeat(64))
}

fn fake_snapshot_for_write(
    selector: &RangeSelector,
    mode: ReadMode,
    literal_type: CellValueType,
    literal_text: &str,
) -> RangeSnapshot {
    let (address, rows, columns) = fake_selector_dimensions(selector);
    let cells = (1..=rows)
        .flat_map(|row| {
            (1..=columns).map(move |column| RuntimeCell {
                row,
                column,
                value_type: fake_formula_value_type(mode, literal_type),
                value_text: fake_formula_value_text(mode, literal_type, literal_text),
            })
        })
        .collect();
    RangeSnapshot {
        address,
        rows,
        columns,
        cells,
    }
}

fn fake_selector_dimensions(selector: &RangeSelector) -> (String, u32, u32) {
    match selector {
        RangeSelector::NamedRange(_) => ("Sheet1!A1".to_owned(), 1, 1),
        RangeSelector::A1 { start, end, .. } => {
            let rows = end.row.saturating_sub(start.row).saturating_add(1);
            let columns = end.column.saturating_sub(start.column).saturating_add(1);
            let start_address =
                cell_address(start.row, start.column).unwrap_or_else(|_| "A1".to_owned());
            let end_address = cell_address(end.row, end.column).unwrap_or_else(|_| "A1".to_owned());
            let address = if start_address == end_address {
                format!("Sheet1!{start_address}")
            } else {
                format!("Sheet1!{start_address}:{end_address}")
            };
            (address, rows, columns)
        }
    }
}

fn fake_formula_value_type(mode: ReadMode, literal_type: CellValueType) -> CellValueType {
    match (mode, literal_type) {
        (
            ReadMode::Formula,
            CellValueType::Number | CellValueType::Text | CellValueType::Boolean,
        ) => CellValueType::Text,
        (_, value_type) => value_type,
    }
}

fn fake_formula_value_text(
    mode: ReadMode,
    literal_type: CellValueType,
    literal_text: &str,
) -> String {
    if mode != ReadMode::Formula {
        return literal_text.to_owned();
    }
    match (literal_type, literal_text) {
        (CellValueType::Boolean, "true") => "TRUE".to_owned(),
        (CellValueType::Boolean, "false") => "FALSE".to_owned(),
        (CellValueType::Blank, _) => String::new(),
        _ => literal_text.to_owned(),
    }
}

#[cfg(test)]
#[path = "excel_t21_l1_write_port_tests.rs"]
mod tests;
