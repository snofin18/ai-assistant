//! Test-only Excel T2.1 pure logic and fake-runtime scaffolding.
//!
//! Responsibilities:
//! - provide the private model, isolated session registry, and four handler paths;
//! - compile only in tests until a COM runner and real workbook acceptance land.
//!
//! Boundaries:
//! - this module is absent from production assembly and the model-visible `ToolBus`;
//! - no adapter declaration is read or changed by this module.
//!
//! Invariants:
//! 1. `adapters/com.microsoft.excel/tools/tools.json` remains empty while this
//!    scaffolding is pending real COM and human acceptance;
//! 2. external `ToolBus` registration requires a separate error-mapping decision.
//!
//! Related documents: TASK-395, ADR-0108.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use assistant_protocol::ErrorCode;
use serde_json::{Map, Value, json};

#[path = "excel_t21_handlers.rs"]
pub mod handlers;
#[path = "excel_t21_model.rs"]
pub mod model;

pub(crate) use handlers::{ExcelCallContext, ExcelT21Handlers, ExcelT21Runtime, ExcelToolResult};
pub(crate) use model::*;

/// Maximum accepted canonical-number source text.
pub(crate) const MAX_NUMERIC_TEXT_BYTES: usize = 256;
/// Maximum decimal exponent magnitude accepted from a runtime value.
const MAX_DECIMAL_EXPONENT: i32 = 1000;
/// Maximum digit count retained while summing.
const MAX_DECIMAL_DIGITS: usize = 4096;

/// Canonical finite decimal implemented without floating-point conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanonicalDecimal {
    negative: bool,
    digits: Vec<u8>,
    scale: u32,
}

impl CanonicalDecimal {
    /// Parses and canonicalizes a finite decimal string.
    pub(crate) fn parse(value: &str) -> Result<Self, ExcelT21Error> {
        if value.is_empty() || value.len() > MAX_NUMERIC_TEXT_BYTES {
            return Err(runtime_shape_error(
                "numeric value text must be 1..=256 bytes",
            ));
        }
        let bytes = value.as_bytes();
        let mut position = 0;
        let negative = bytes.get(position) == Some(&b'-');
        if negative || bytes.get(position) == Some(&b'+') {
            position += 1;
        }
        let mut digits = Vec::new();
        let mut fraction_digits = 0_u32;
        let mut saw_digit = false;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            if let Some(digit) = bytes.get(position) {
                digits.push(digit - b'0');
            }
            position += 1;
            saw_digit = true;
        }
        if bytes.get(position) == Some(&b'.') {
            position += 1;
            while bytes.get(position).is_some_and(u8::is_ascii_digit) {
                if let Some(digit) = bytes.get(position) {
                    digits.push(digit - b'0');
                }
                fraction_digits = fraction_digits
                    .checked_add(1)
                    .ok_or_else(|| runtime_shape_error("numeric fractional scale overflows"))?;
                position += 1;
                saw_digit = true;
            }
        }
        if !saw_digit {
            return Err(runtime_shape_error("numeric value has no digits"));
        }
        let exponent = parse_decimal_exponent(bytes, &mut position)?;
        if position != bytes.len() {
            return Err(runtime_shape_error(
                "numeric value contains unsupported characters",
            ));
        }
        let mut scale = i32::try_from(fraction_digits)
            .map_err(|_| runtime_shape_error("numeric scale does not fit i32"))?
            .checked_sub(exponent)
            .ok_or_else(|| runtime_shape_error("numeric scale overflows"))?;
        if scale < 0 {
            let zeros = usize::try_from(-scale)
                .map_err(|_| runtime_shape_error("numeric zero padding overflows"))?;
            digits.resize(digits.len().saturating_add(zeros), 0);
            scale = 0;
        }
        if digits.len() > MAX_DECIMAL_DIGITS {
            return Err(runtime_shape_error("canonical decimal exceeds digit bound"));
        }
        let scale = u32::try_from(scale)
            .map_err(|_| runtime_shape_error("numeric scale does not fit u32"))?;
        Ok(Self::normalized(negative, digits, scale))
    }

    fn normalized(mut negative: bool, mut digits: Vec<u8>, mut scale: u32) -> Self {
        let leading_zeros = digits.iter().take_while(|digit| **digit == 0).count();
        if leading_zeros > 0 {
            digits.drain(..leading_zeros);
        }
        while digits.last() == Some(&0) && scale > 0 {
            digits.pop();
            scale -= 1;
        }
        if digits.is_empty() {
            negative = false;
            scale = 0;
        }
        Self {
            negative,
            digits,
            scale,
        }
    }

    /// Returns canonical decimal text.
    pub(crate) fn as_text(&self) -> Result<String, ExcelT21Error> {
        if self.digits.is_empty() {
            return Ok("0".to_owned());
        }
        let mut output = String::new();
        if self.negative {
            output.push('-');
        }
        let scale = usize::try_from(self.scale).map_err(|_| {
            ExcelT21Error::new(ErrorCode::Fatal, "decimal scale does not fit usize")
        })?;
        if scale >= self.digits.len() {
            output.push_str("0.");
            for _ in 0..scale.saturating_sub(self.digits.len()) {
                output.push('0');
            }
            for digit in &self.digits {
                output.push(char::from(b'0' + digit));
            }
            return Ok(output);
        }
        let split = self.digits.len() - scale;
        for digit in self.digits.iter().take(split) {
            output.push(char::from(b'0' + digit));
        }
        if scale > 0 {
            output.push('.');
            for digit in self.digits.iter().skip(split) {
                output.push(char::from(b'0' + digit));
            }
        }
        Ok(output)
    }

    /// Adds two finite canonical decimals.
    pub(crate) fn checked_add(&self, other: &Self) -> Result<Self, ExcelT21Error> {
        let scale = self.scale.max(other.scale);
        let left = self.aligned_digits(scale);
        let right = other.aligned_digits(scale);
        if self.negative == other.negative {
            let output = add_absolute_digits(&left, &right)?;
            Ok(Self::normalized(self.negative, output, scale))
        } else {
            match left.cmp(&right) {
                std::cmp::Ordering::Greater => {
                    let output = subtract_absolute_digits(&left, &right)?;
                    Ok(Self::normalized(self.negative, output, scale))
                }
                std::cmp::Ordering::Less => {
                    let output = subtract_absolute_digits(&right, &left)?;
                    Ok(Self::normalized(other.negative, output, scale))
                }
                std::cmp::Ordering::Equal => Ok(Self::normalized(false, Vec::new(), 0)),
            }
        }
    }

    /// Compares absolute values.
    pub(crate) fn cmp_absolute(&self, other: &Self) -> std::cmp::Ordering {
        let scale = self.scale.max(other.scale);
        self.aligned_digits(scale).cmp(&other.aligned_digits(scale))
    }

    /// Compares signed values.
    pub(crate) fn cmp_value(&self, other: &Self) -> std::cmp::Ordering {
        match (self.negative, other.negative) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => self.cmp_absolute(other),
            (true, true) => self.cmp_absolute(other).reverse(),
        }
    }

    fn aligned_len(&self, scale: u32) -> usize {
        self.digits.len() + usize::try_from(scale.saturating_sub(self.scale)).unwrap_or(usize::MAX)
    }

    fn aligned_digits(&self, scale: u32) -> Vec<u8> {
        let mut digits = self.digits.clone();
        digits.resize(self.aligned_len(scale), 0);
        digits
    }
}

fn parse_decimal_exponent(bytes: &[u8], position: &mut usize) -> Result<i32, ExcelT21Error> {
    if !matches!(bytes.get(*position), Some(b'e' | b'E')) {
        return Ok(0);
    }
    *position += 1;
    let exponent_negative = match bytes.get(*position) {
        Some(b'-') => {
            *position += 1;
            true
        }
        Some(b'+') => {
            *position += 1;
            false
        }
        _ => false,
    };
    let exponent_start = *position;
    let mut exponent = 0_i32;
    while bytes.get(*position).is_some_and(u8::is_ascii_digit) {
        exponent = exponent
            .checked_mul(10)
            .and_then(|value| {
                bytes
                    .get(*position)
                    .and_then(|byte| i32::from(*byte - b'0').checked_add(value))
            })
            .ok_or_else(|| runtime_shape_error("numeric exponent overflows"))?;
        *position += 1;
    }
    if exponent_start == *position || exponent > MAX_DECIMAL_EXPONENT {
        return Err(runtime_shape_error(
            "numeric exponent is missing or outside the supported bound",
        ));
    }
    Ok(if exponent_negative {
        -exponent
    } else {
        exponent
    })
}

fn add_absolute_digits(left: &[u8], right: &[u8]) -> Result<Vec<u8>, ExcelT21Error> {
    let mut output = Vec::with_capacity(left.len().max(right.len()).saturating_add(1));
    let mut left = left.iter().rev();
    let mut right = right.iter().rev();
    let mut carry = 0_u8;
    for _ in 0..left.len().max(right.len()) {
        let left_digit = left.next().copied().unwrap_or(0);
        let right_digit = right.next().copied().unwrap_or(0);
        let sum = left_digit
            .checked_add(right_digit)
            .and_then(|value| value.checked_add(carry))
            .ok_or_else(|| runtime_shape_error("decimal addition overflowed"))?;
        output.push(sum % 10);
        carry = sum / 10;
    }
    if carry > 0 {
        output.push(carry);
    }
    if output.len() > MAX_DECIMAL_DIGITS {
        return Err(runtime_shape_error(
            "decimal sum exceeds the supported digit bound",
        ));
    }
    output.reverse();
    Ok(output)
}

fn subtract_absolute_digits(larger: &[u8], smaller: &[u8]) -> Result<Vec<u8>, ExcelT21Error> {
    let mut output = Vec::with_capacity(larger.len());
    let mut smaller = smaller.iter().rev();
    let mut borrow = 0_i8;
    for larger_digit in larger.iter().rev() {
        let smaller_digit = smaller.next().copied().unwrap_or(0);
        let mut difference =
            i16::from(*larger_digit) - i16::from(smaller_digit) - i16::from(borrow);
        if difference < 0 {
            difference += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        output.push(
            u8::try_from(difference).map_err(|_| {
                runtime_shape_error("decimal subtraction produced an invalid digit")
            })?,
        );
    }
    output.reverse();
    Ok(output)
}

fn worksheet_layout_matches(opened: &[WorksheetInfo], current: &[WorksheetInfo]) -> bool {
    if opened.len() != current.len() {
        return false;
    }
    let mut opened = opened
        .iter()
        .map(|worksheet| (worksheet.ordinal, worksheet.name.as_str()))
        .collect::<Vec<_>>();
    let mut current = current
        .iter()
        .map(|worksheet| (worksheet.ordinal, worksheet.name.as_str()))
        .collect::<Vec<_>>();
    opened.sort_unstable();
    current.sort_unstable();
    opened == current
}

fn open_worksheet_values(access: &SessionAccess) -> Result<Vec<Value>, ExcelT21Error> {
    if access.workbook.full_path.is_empty() {
        return Err(runtime_shape_error("workbook path must not be empty"));
    }
    let mut worksheets = access.worksheets.clone();
    worksheets.sort_by_key(|worksheet| worksheet.ordinal);
    worksheets
        .into_iter()
        .map(|worksheet| {
            if worksheet.name.is_empty() || worksheet.ordinal == 0 {
                return Err(runtime_shape_error(
                    "worksheet names and ordinals must be non-empty",
                ));
            }
            Ok(json!({
                "name": worksheet.name,
                "ordinal": worksheet.ordinal,
            }))
        })
        .collect()
}

fn canonical_worksheets(worksheets: &[WorksheetInfo]) -> Result<Vec<Value>, ExcelT21Error> {
    let worksheet_count = u64::try_from(worksheets.len())
        .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "worksheet count does not fit u64"))?;
    if worksheets.is_empty() || worksheet_count > MAX_WORKSHEETS {
        return Err(runtime_shape_error(
            "workbook must expose 1..=1024 worksheets",
        ));
    }
    let mut sorted = worksheets.to_vec();
    sorted.sort_by_key(|worksheet| worksheet.ordinal);
    let mut names = BTreeSet::new();
    let mut ordinals = BTreeSet::new();
    let mut output = Vec::with_capacity(sorted.len());
    for worksheet in sorted {
        if worksheet.name.is_empty()
            || worksheet.ordinal == 0
            || !names.insert(worksheet.name.clone())
            || !ordinals.insert(worksheet.ordinal)
        {
            return Err(runtime_shape_error(
                "worksheet names and ordinals must be non-empty and unique",
            ));
        }
        let mut value = Map::new();
        value.insert("name".to_owned(), Value::String(worksheet.name));
        value.insert("ordinal".to_owned(), Value::from(worksheet.ordinal));
        if let Some(address) = worksheet.used_range_address {
            value.insert("used_range_address".to_owned(), Value::String(address));
        }
        output.push(Value::Object(value));
    }
    Ok(output)
}

fn canonical_named_ranges(named_ranges: &[NamedRangeInfo]) -> Result<Vec<Value>, ExcelT21Error> {
    if named_ranges.len() > MAX_NAMED_RANGES {
        return Err(ExcelT21Error::new(
            ErrorCode::UserInteraction,
            format!("workbook exposes more than {MAX_NAMED_RANGES} named ranges"),
        ));
    }
    let mut sorted = named_ranges.to_vec();
    sorted.sort_by(|left, right| {
        scope_sort_key(&left.scope)
            .cmp(&scope_sort_key(&right.scope))
            .then_with(|| left.name.cmp(&right.name))
    });
    let mut keys = BTreeSet::new();
    let mut output = Vec::with_capacity(sorted.len());
    for named_range in sorted {
        if named_range.name.is_empty() || named_range.refers_to.is_empty() {
            return Err(runtime_shape_error(
                "named range name and refers_to must not be empty",
            ));
        }
        let (scope_kind, scope_name) = match &named_range.scope {
            NamedRangeScope::Workbook => ("workbook", None),
            NamedRangeScope::Worksheet(name) if !name.is_empty() => {
                ("worksheet", Some(name.clone()))
            }
            NamedRangeScope::Worksheet(_) => {
                return Err(runtime_shape_error(
                    "worksheet-scoped named range must name its worksheet",
                ));
            }
        };
        if !keys.insert((scope_kind, scope_name.clone(), named_range.name.clone())) {
            return Err(runtime_shape_error(
                "workbook returned duplicate scoped named ranges",
            ));
        }
        output.push(json!({
            "name": named_range.name,
            "scope_kind": scope_kind,
            "scope_name": scope_name,
            "refers_to": named_range.refers_to,
        }));
    }
    Ok(output)
}

fn scope_sort_key(scope: &NamedRangeScope) -> (u8, &str) {
    match scope {
        NamedRangeScope::Workbook => (0, ""),
        NamedRangeScope::Worksheet(name) => (1, name.as_str()),
    }
}

/// Process-wide capacity shared by every ToolBus-isolated registry.
#[derive(Clone)]
pub(crate) struct SessionCapacity {
    active: Arc<AtomicUsize>,
}

impl SessionCapacity {
    fn try_reserve(&self) -> bool {
        let mut current = self.active.load(Ordering::Acquire);
        loop {
            if current >= MAX_ACTIVE_SESSIONS {
                return false;
            }
            match self.active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(observed) => current = observed,
            }
        }
    }

    fn release(&self) {
        let _ = self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.saturating_sub(1))
            });
    }

    /// Returns the process-wide active session count.
    pub(crate) fn active_count(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }
}

impl Default for SessionCapacity {
    fn default() -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[derive(Default)]
struct FakeClock {
    now: std::sync::atomic::AtomicI64,
}

impl FakeClock {
    fn set(&self, value: i64) {
        self.now.store(value, std::sync::atomic::Ordering::SeqCst);
    }

    fn advance(&self, milliseconds: i64) {
        self.now
            .fetch_add(milliseconds, std::sync::atomic::Ordering::SeqCst);
    }
}

impl SessionClock for FakeClock {
    fn now_unix_ms(&self) -> i64 {
        self.now.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Default)]
struct SequenceTokenSource {
    next: std::sync::atomic::AtomicU64,
}

impl TokenSource for SequenceTokenSource {
    fn next_token(&self) -> Result<String, ExcelT21Error> {
        let value = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!("fake-session-token-{value:016}-0000000000000000"))
    }
}

struct FixedTokenSource(&'static str);

impl TokenSource for FixedTokenSource {
    fn next_token(&self) -> Result<String, ExcelT21Error> {
        Ok(self.0.to_owned())
    }
}

#[derive(Default)]
struct FakeRuntime {
    state: std::sync::Mutex<FakeState>,
}

#[derive(Default)]
struct FakeState {
    next_handle: u64,
    workbooks: std::collections::BTreeMap<u64, FakeWorkbook>,
    opened_paths: Vec<String>,
    closed_handles: Vec<u64>,
    fingerprint_calls: u64,
    drift_fingerprint: bool,
    open_error: Option<ExcelT21Error>,
    close_error_handle: Option<u64>,
    opened_identity_override: Option<String>,
    ownership_error: bool,
    workbook_response: Option<WorkbookSnapshot>,
    range_response: Option<RangeSnapshot>,
}

#[derive(Clone)]
struct FakeWorkbook {
    workbook: WorkbookIdentity,
    owned_process: OwnedProcessIdentity,
    worksheets: Vec<WorksheetInfo>,
    named_ranges: Vec<NamedRangeInfo>,
}

impl ExcelT21Runtime for FakeRuntime {
    fn open_read_only(&self, path: &std::path::Path) -> Result<OpenedWorkbook, ExcelT21Error> {
        let mut state = self.lock_state()?;
        if let Some(error) = state.open_error.clone() {
            return Err(error);
        }
        state.next_handle = state
            .next_handle
            .checked_add(1)
            .ok_or_else(|| ExcelT21Error::new(ErrorCode::Fatal, "fake handle overflowed"))?;
        let handle = WorkbookHandle::new(state.next_handle);
        let workbook = WorkbookIdentity {
            full_path: state
                .opened_identity_override
                .clone()
                .unwrap_or_else(|| path.to_string_lossy().to_string()),
            read_only: true,
        };
        let owned_process = OwnedProcessIdentity {
            process_id: 4000 + case_id(handle),
            start_ticks: 7000 + u64::from(case_id(handle)),
            executable_path: "C:\\Program Files\\Microsoft Office\\root\\Office16\\EXCEL.EXE"
                .to_owned(),
        };
        let worksheets = vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: Some("A1:C3".to_owned()),
        }];
        state.opened_paths.push(path.to_string_lossy().to_string());
        state.workbooks.insert(
            handle.get(),
            FakeWorkbook {
                workbook: workbook.clone(),
                owned_process: owned_process.clone(),
                worksheets: worksheets.clone(),
                named_ranges: vec![
                    NamedRangeInfo {
                        name: "SalesData".to_owned(),
                        scope: NamedRangeScope::Workbook,
                        refers_to: "=Sheet1!$A$1:$C$1".to_owned(),
                    },
                    NamedRangeInfo {
                        name: "LocalData".to_owned(),
                        scope: NamedRangeScope::Worksheet("Sheet1".to_owned()),
                        refers_to: "=Sheet1!$A$2:$A$2".to_owned(),
                    },
                ],
            },
        );
        drop(state);
        Ok(OpenedWorkbook {
            handle,
            owned_process,
            workbook,
            worksheets,
        })
    }

    fn close(&self, handle: WorkbookHandle) -> Result<(), ExcelT21Error> {
        let mut state = self.lock_state()?;
        if state.close_error_handle == Some(handle.get()) {
            state.close_error_handle = None;
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "scripted close failure",
            ));
        }
        if state.workbooks.remove(&handle.get()).is_none() {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "fake close received an unknown handle",
            ));
        }
        state.closed_handles.push(handle.get());
        drop(state);
        Ok(())
    }

    fn verify_ownership(
        &self,
        handle: WorkbookHandle,
        workbook: &WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        let state = self.lock_state()?;
        if state.ownership_error {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "scripted ownership failure",
            ));
        }
        let opened = state.workbooks.get(&handle.get()).ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "fake workbook handle disappeared",
            )
        })?;
        if &opened.workbook != workbook {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "fake workbook identity drifted",
            ));
        }
        if opened.owned_process.process_id == 0 {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "fake owned process is missing",
            ));
        }
        drop(state);
        Ok(())
    }

    fn fingerprint(&self, _handle: WorkbookHandle) -> Result<String, ExcelT21Error> {
        let mut state = self.lock_state()?;
        state.fingerprint_calls = state.fingerprint_calls.saturating_add(1);
        let suffix = if state.drift_fingerprint {
            state.fingerprint_calls
        } else {
            1
        };
        Ok(format!("sha256:workbook-{suffix:016x}"))
    }

    fn read_workbook(&self, handle: WorkbookHandle) -> Result<WorkbookSnapshot, ExcelT21Error> {
        let state = self.lock_state()?;
        if let Some(response) = &state.workbook_response {
            let response = response.clone();
            drop(state);
            return Ok(response);
        }
        let workbook = state.workbooks.get(&handle.get()).ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::TargetNotFound, "fake workbook disappeared")
        })?;
        let snapshot = WorkbookSnapshot {
            workbook: workbook.workbook.clone(),
            worksheets: workbook.worksheets.clone(),
            named_ranges: workbook.named_ranges.clone(),
            calculation: CalculationInfo {
                mode: "automatic".to_owned(),
                state: "done".to_owned(),
            },
        };
        drop(state);
        Ok(snapshot)
    }

    fn read_range(
        &self,
        _handle: WorkbookHandle,
        selector: &RangeSelector,
        _mode: ReadMode,
    ) -> Result<RangeSnapshot, ExcelT21Error> {
        let state = self.lock_state()?;
        if let Some(response) = &state.range_response {
            let response = response.clone();
            drop(state);
            return Ok(response);
        }
        drop(state);
        match selector {
            RangeSelector::NamedRange(name) if name == "SalesData" => Ok(RangeSnapshot {
                address: "Sheet1!A1:C1".to_owned(),
                rows: 1,
                columns: 3,
                cells: vec![
                    RuntimeCell {
                        row: 1,
                        column: 1,
                        value_type: CellValueType::Number,
                        value_text: "1".to_owned(),
                    },
                    RuntimeCell {
                        row: 1,
                        column: 2,
                        value_type: CellValueType::Number,
                        value_text: "2".to_owned(),
                    },
                    RuntimeCell {
                        row: 1,
                        column: 3,
                        value_type: CellValueType::Number,
                        value_text: "3".to_owned(),
                    },
                ],
            }),
            RangeSelector::A1 { start, end, .. } => {
                let rows = end.row - start.row + 1;
                let columns = end.column - start.column + 1;
                let mut cells = Vec::new();
                for row in 0..rows {
                    for column in 0..columns {
                        let value = u64::from(row) * u64::from(columns) + u64::from(column) + 1;
                        cells.push(RuntimeCell {
                            row: start.row + row,
                            column: start.column + column,
                            value_type: CellValueType::Number,
                            value_text: value.to_string(),
                        });
                    }
                }
                Ok(RangeSnapshot {
                    address: format!(
                        "{}{}:{}{}",
                        column_name(start.column)?,
                        start.row,
                        column_name(end.column)?,
                        end.row
                    ),
                    rows,
                    columns,
                    cells,
                })
            }
            RangeSelector::NamedRange(_) => Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "fake named range not found",
            )),
        }
    }
}

impl FakeRuntime {
    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, FakeState>, ExcelT21Error> {
        self.state
            .lock()
            .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "fake runtime mutex is poisoned"))
    }

    fn set_range_response(&self, response: RangeSnapshot) -> Result<(), ExcelT21Error> {
        self.lock_state()?.range_response = Some(response);
        Ok(())
    }

    fn set_workbook_response(&self, response: WorkbookSnapshot) -> Result<(), ExcelT21Error> {
        self.lock_state()?.workbook_response = Some(response);
        Ok(())
    }

    fn set_fingerprint_drift(&self, enabled: bool) -> Result<(), ExcelT21Error> {
        self.lock_state()?.drift_fingerprint = enabled;
        Ok(())
    }

    fn set_open_error(&self, error: ExcelT21Error) -> Result<(), ExcelT21Error> {
        self.lock_state()?.open_error = Some(error);
        Ok(())
    }

    fn set_close_error_handle(&self, handle: Option<u64>) -> Result<(), ExcelT21Error> {
        self.lock_state()?.close_error_handle = handle;
        Ok(())
    }

    fn set_opened_identity_override(&self, path: Option<String>) -> Result<(), ExcelT21Error> {
        self.lock_state()?.opened_identity_override = path;
        Ok(())
    }

    fn set_ownership_error(&self, enabled: bool) -> Result<(), ExcelT21Error> {
        self.lock_state()?.ownership_error = enabled;
        Ok(())
    }

    fn snapshot(&self) -> Result<(Vec<String>, Vec<u64>, usize), ExcelT21Error> {
        let state = self.lock_state()?;
        Ok((
            state.opened_paths.clone(),
            state.closed_handles.clone(),
            state.workbooks.len(),
        ))
    }
}

fn case_id(handle: WorkbookHandle) -> u32 {
    u32::try_from(handle.get()).unwrap_or(u32::MAX)
}

fn column_name(column: u32) -> Result<String, ExcelT21Error> {
    let mut value = column;
    let mut letters = Vec::new();
    while value > 0 {
        let digit = (value - 1) % 26;
        letters.push(
            u8::try_from(digit).map_err(|_| invalid_arguments("column digit does not fit u8"))?
                + b'A',
        );
        value = (value - 1) / 26;
    }
    letters.reverse();
    String::from_utf8(letters).map_err(|_| invalid_arguments("column name conversion failed"))
}

fn test_context(bus: &str, task: &str) -> Result<ExcelCallContext, ExcelT21Error> {
    ExcelCallContext::new(bus, task)
}

fn test_registry(
    bus: &str,
) -> Result<(SessionRegistry, Arc<FakeClock>, Arc<SequenceTokenSource>), ExcelT21Error> {
    let clock = Arc::new(FakeClock::default());
    clock.set(1_700_000_000_000);
    let tokens = Arc::new(SequenceTokenSource::default());
    let registry = SessionRegistry::new(bus, clock.clone(), tokens.clone())?;
    Ok((registry, clock, tokens))
}

fn opened(handle: u64) -> OpenedWorkbook {
    OpenedWorkbook {
        handle: WorkbookHandle::new(handle),
        owned_process: OwnedProcessIdentity {
            process_id: 4000 + u32::try_from(handle).unwrap_or(u32::MAX),
            start_ticks: 7000 + handle,
            executable_path: "C:\\Program Files\\Microsoft Office\\root\\Office16\\EXCEL.EXE"
                .to_owned(),
        },
        workbook: WorkbookIdentity {
            full_path: "C:\\data\\book.xlsx".to_owned(),
            read_only: true,
        },
        worksheets: vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: None,
        }],
    }
}

fn arguments(value: Value) -> Result<Map<String, Value>, ExcelT21Error> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(invalid_arguments("test arguments must be an object")),
    }
}

fn token_from(result: &ExcelToolResult) -> Result<&str, ExcelT21Error> {
    result
        .data
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ExcelT21Error::new(ErrorCode::Fatal, "open result has no session_id"))
}

#[cfg(test)]
#[path = "excel_t21_tests.rs"]
mod tests;
