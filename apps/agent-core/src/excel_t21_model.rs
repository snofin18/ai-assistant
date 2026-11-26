//! Domain models and parsers for the Excel T2.1 read-only workflow.
//!
//! Responsibilities:
//! - validate workbook paths, range identities, read modes, and session tokens;
//! - keep the bounded session registry independent from COM handles and GUI state;
//! - provide canonical decimal arithmetic for range summaries.
//!
//! Boundaries:
//! - this module never opens Excel, reads a workbook, or emits a `ToolBus` envelope;
//! - it does not decide policy or approval;
//! - it does not turn an untrusted application value into trusted content.
//!
//! Invariants:
//! 1. session state is bounded by one active session per task, eight globally, and a
//!    fifteen-minute TTL;
//! 2. opaque tokens are validated before lookup and are scoped to one `ToolBus` session
//!    and task;
//! 3. parsers reject ambiguous shapes instead of selecting a convenient branch.
//!
//! Related documents: ADR-0108, `docs/spec/excel-t21-readonly-workflow.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, PathBuf};
use std::sync::Arc;

use assistant_protocol::ErrorCode;
use serde_json::{Map, Value};

use super::SessionCapacity;

/// Workbook open tool name.
pub(crate) const TOOL_WORKBOOK_OPEN: &str = "excel.workbook.open";
/// Workbook metadata read tool name.
pub(crate) const TOOL_WORKBOOK_READ: &str = "excel.workbook.read";
/// Range read tool name.
pub(crate) const TOOL_RANGE_READ: &str = "excel.range.read";
/// Range summary tool name.
pub(crate) const TOOL_RANGE_SUMMARIZE: &str = "excel.range.summarize";

/// Maximum number of cells returned by one range read.
pub(crate) const MAX_RANGE_CELLS: u64 = 4096;
/// Maximum compact JSON payload returned by one range read.
pub(crate) const MAX_RANGE_PAYLOAD_BYTES: usize = 256 * 1024;
/// Maximum compact JSON payload returned by one workbook metadata read.
pub(crate) const MAX_WORKBOOK_PAYLOAD_BYTES: usize = 262_144;
/// Maximum worksheets returned by one workbook metadata read.
pub(crate) const MAX_WORKSHEETS: u64 = 1024;
/// Maximum workbook- and sheet-scoped named ranges returned by one metadata read.
pub(crate) const MAX_NAMED_RANGES: usize = 2048;
/// Maximum active sessions retained by one registry.
pub(crate) const MAX_ACTIVE_SESSIONS: usize = 8;
/// Session lifetime in milliseconds.
pub(crate) const SESSION_TTL_MS: i64 = 15 * 60 * 1000;
/// Minimum opaque session token length in UTF-8 bytes.
pub(crate) const MIN_SESSION_TOKEN_BYTES: usize = 32;
/// Maximum opaque session token length in UTF-8 bytes.
pub(crate) const MAX_SESSION_TOKEN_BYTES: usize = 128;
/// Maximum Excel worksheet name length used by the A1 selector.
const MAX_WORKSHEET_NAME_BYTES: usize = 255;
/// Maximum named-range name length.
const MAX_NAMED_RANGE_BYTES: usize = 255;

/// Structured failure with the existing protocol error category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExcelT21Error {
    code: ErrorCode,
    message: String,
}

impl ExcelT21Error {
    /// Creates a non-empty structured failure.
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let message = message.into();
        let message = if message.trim().is_empty() {
            format!("{code:?}")
        } else {
            message
        };
        Self { code, message }
    }

    /// Returns the existing protocol error category.
    pub(crate) const fn code(&self) -> ErrorCode {
        self.code
    }
}

impl fmt::Display for ExcelT21Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ExcelT21Error {}

pub(crate) fn invalid_arguments(message: impl Into<String>) -> ExcelT21Error {
    ExcelT21Error::new(ErrorCode::ToolInvalidArgs, message)
}

pub(crate) fn runtime_shape_error(message: impl Into<String>) -> ExcelT21Error {
    ExcelT21Error::new(ErrorCode::UserInteraction, message)
}

/// Opaque workbook handle retained only inside the fake/private runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkbookHandle(u64);

impl WorkbookHandle {
    /// Creates a runtime-local workbook handle.
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the runtime-local value.
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Owned process identity used for session ownership checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedProcessIdentity {
    /// Process identifier.
    pub(crate) process_id: u32,
    /// Process start ticks used to reject PID reuse.
    pub(crate) start_ticks: u64,
    /// Executable path observed at open time.
    pub(crate) executable_path: String,
}

impl OwnedProcessIdentity {
    fn validate(&self) -> Result<(), ExcelT21Error> {
        if self.process_id == 0 || self.start_ticks == 0 || self.executable_path.trim().is_empty() {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "owned process identity is incomplete",
            ));
        }
        Ok(())
    }
}

/// Workbook identity retained by a read-only session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkbookIdentity {
    /// Canonical local path.
    pub(crate) full_path: String,
    /// Read-only flag reported by the runtime.
    pub(crate) read_only: bool,
}

impl WorkbookIdentity {
    fn validate(&self) -> Result<(), ExcelT21Error> {
        if !self.read_only {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel session must be read-only",
            ));
        }
        parse_workbook_path(&self.full_path)?;
        Ok(())
    }
}

/// One worksheet exposed by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorksheetInfo {
    /// Exact worksheet name.
    pub(crate) name: String,
    /// One-based worksheet ordinal.
    pub(crate) ordinal: u32,
    /// Used range address, when the runtime can report it.
    pub(crate) used_range_address: Option<String>,
}

/// Workbook or worksheet scope of a named range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NamedRangeScope {
    /// Workbook-scoped name.
    Workbook,
    /// Worksheet-scoped name with the exact worksheet name.
    Worksheet(String),
}

/// One workbook-scoped or sheet-scoped named range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedRangeInfo {
    /// Exact name.
    pub(crate) name: String,
    /// Name scope.
    pub(crate) scope: NamedRangeScope,
    /// Formula text reported by Excel.
    pub(crate) refers_to: String,
}

/// Excel calculation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CalculationInfo {
    /// Calculation mode.
    pub(crate) mode: String,
    /// Calculation state.
    pub(crate) state: String,
}

/// Successful runtime open result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenedWorkbook {
    /// Runtime-local handle.
    pub(crate) handle: WorkbookHandle,
    /// Owned process identity.
    pub(crate) owned_process: OwnedProcessIdentity,
    /// Workbook identity.
    pub(crate) workbook: WorkbookIdentity,
    /// Worksheets visible to the read-only session.
    pub(crate) worksheets: Vec<WorksheetInfo>,
}

/// Workbook metadata returned by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkbookSnapshot {
    /// Workbook identity reported after the read.
    pub(crate) workbook: WorkbookIdentity,
    /// Worksheets in runtime order.
    pub(crate) worksheets: Vec<WorksheetInfo>,
    /// Named ranges in runtime order.
    pub(crate) named_ranges: Vec<NamedRangeInfo>,
    /// Calculation metadata.
    pub(crate) calculation: CalculationInfo,
}

/// Read mode for a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadMode {
    Value2,
    Formula,
    /// Excel `.Text`.
    Text,
}

impl ReadMode {
    /// Stable output token.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Value2 => "value2",
            Self::Formula => "formula",
            Self::Text => "text",
        }
    }
}

/// Closed cell value type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellValueType {
    /// Empty cell.
    Blank,
    /// Finite number represented by canonical text.
    Number,
    /// Text.
    Text,
    /// Boolean.
    Boolean,
    /// Known Excel error.
    Error,
}

impl CellValueType {
    /// Stable output token.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Blank => "blank",
            Self::Number => "number",
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Error => "error",
        }
    }
}

/// One runtime cell before coordinates and ordering are validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCell {
    /// One-based row.
    pub(crate) row: u32,
    /// One-based column.
    pub(crate) column: u32,
    /// Closed value type.
    pub(crate) value_type: CellValueType,
    /// Runtime value text.
    pub(crate) value_text: String,
}

/// Range result returned by the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RangeSnapshot {
    /// Address resolved by the runtime.
    pub(crate) address: String,
    /// Number of rows.
    pub(crate) rows: u32,
    /// Number of columns.
    pub(crate) columns: u32,
    /// Cells in any runtime order.
    pub(crate) cells: Vec<RuntimeCell>,
}

/// One worksheet selector in an A1 reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorksheetSelector {
    /// Exact worksheet name.
    Name(String),
    /// One-based worksheet ordinal.
    Ordinal(u32),
}

/// Closed range selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RangeSelector {
    /// Workbook or sheet-scoped named range.
    NamedRange(String),
    /// Explicit worksheet + bounded A1 reference.
    A1 {
        /// Worksheet selector.
        worksheet: WorksheetSelector,
        /// Start cell.
        start: A1Cell,
        /// End cell.
        end: A1Cell,
    },
}

impl RangeSelector {
    /// Returns the declared cell count when the selector is an A1 range.
    pub(crate) fn declared_cell_count(&self) -> Option<u64> {
        match self {
            Self::A1 { start, end, .. } => {
                let rows = u64::from(end.row - start.row + 1);
                let columns = u64::from(end.column - start.column + 1);
                rows.checked_mul(columns)
            }
            Self::NamedRange(_) => None,
        }
    }
}

/// One validated A1 cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct A1Cell {
    /// One-based row.
    pub(crate) row: u32,
    /// One-based column.
    pub(crate) column: u32,
}

/// Parses a workbook path and enforces the local `.xlsx` shape.
///
/// File existence, readability, canonicalization, and regular-file status are
/// checked by the runtime immediately before open.
pub(crate) fn parse_workbook_path(value: &str) -> Result<PathBuf, ExcelT21Error> {
    if value.is_empty()
        || value.len() > 32 * 1024
        || value.chars().any(char::is_control)
        || value.contains('\0')
        || value.contains("://")
        || value.starts_with("\\\\")
        || value.starts_with("//")
    {
        return Err(invalid_arguments(
            "workbook_path must be a local absolute .xlsx path",
        ));
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(invalid_arguments("workbook_path must be absolute"));
    }
    for component in path.components() {
        match component {
            Component::CurDir | Component::ParentDir => {
                return Err(invalid_arguments(
                    "workbook_path must not contain dot or parent-directory components",
                ));
            }
            Component::Prefix(prefix) => {
                let prefix_text = prefix.as_os_str().to_string_lossy();
                if prefix_text.starts_with("\\\\") || prefix_text.contains("UNC") {
                    return Err(invalid_arguments(
                        "workbook_path must not use a UNC or device path",
                    ));
                }
            }
            Component::RootDir | Component::Normal(_) => {}
        }
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("xlsx") {
        return Err(invalid_arguments(
            "workbook_path extension must be exactly .xlsx",
        ));
    }
    Ok(path)
}

/// Parses one closed range selector.
pub(crate) fn parse_range_selector(value: &Value) -> Result<RangeSelector, ExcelT21Error> {
    let object = value.as_object().ok_or_else(|| {
        invalid_arguments("range must be a closed object with exactly one selector kind")
    })?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_arguments("range.kind must be a string"))?;
    match kind {
        "named_range" => {
            reject_unknown_keys(object, &["kind", "name"], "named_range")?;
            let name = required_non_empty_string(object, "name", MAX_NAMED_RANGE_BYTES)?;
            Ok(RangeSelector::NamedRange(name.to_owned()))
        }
        "a1" => parse_a1_selector(object),
        _ => Err(invalid_arguments(
            "range.kind must be `named_range` or `a1`",
        )),
    }
}

fn parse_a1_selector(object: &Map<String, Value>) -> Result<RangeSelector, ExcelT21Error> {
    reject_unknown_keys(object, &["kind", "worksheet", "range"], "a1")?;
    let worksheet = object
        .get("worksheet")
        .ok_or_else(|| invalid_arguments("range.worksheet is required for a1"))?;
    let worksheet_object = worksheet
        .as_object()
        .ok_or_else(|| invalid_arguments("range.worksheet must be an object"))?;
    reject_unknown_keys(worksheet_object, &["kind", "value"], "worksheet")?;
    let worksheet_kind = worksheet_object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_arguments("range.worksheet.kind must be a string"))?;
    let worksheet_selector = match worksheet_kind {
        "name" => WorksheetSelector::Name(
            required_non_empty_string(worksheet_object, "value", MAX_WORKSHEET_NAME_BYTES)?
                .to_owned(),
        ),
        "ordinal" => {
            let ordinal = worksheet_object
                .get("value")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid_arguments("range.worksheet.value must be an integer"))?;
            if !(1..=MAX_WORKSHEETS).contains(&ordinal) {
                return Err(invalid_arguments(
                    "range.worksheet ordinal must be between 1 and 1024",
                ));
            }
            WorksheetSelector::Ordinal(
                u32::try_from(ordinal)
                    .map_err(|_| invalid_arguments("range.worksheet ordinal does not fit u32"))?,
            )
        }
        _ => {
            return Err(invalid_arguments(
                "range.worksheet.kind must be `name` or `ordinal`",
            ));
        }
    };
    let address = required_non_empty_string(object, "range", 32)?;
    let (start, end) = parse_a1_range(address)?;
    Ok(RangeSelector::A1 {
        worksheet: worksheet_selector,
        start,
        end,
    })
}

/// Parses a bounded A1 or A1:B2 reference.
pub(crate) fn parse_a1_range(value: &str) -> Result<(A1Cell, A1Cell), ExcelT21Error> {
    let mut parts = value.split(':');
    let first = parts
        .next()
        .ok_or_else(|| invalid_arguments("A1 range is empty"))?;
    let second = parts.next();
    if parts.next().is_some() {
        return Err(invalid_arguments("A1 range must contain at most one colon"));
    }
    let start = parse_a1_cell(first)?;
    let end = match second {
        Some(second) => parse_a1_cell(second)?,
        None => start,
    };
    if start.row > end.row || start.column > end.column {
        return Err(invalid_arguments(
            "A1 range start must be above and to the left of its end",
        ));
    }
    Ok((start, end))
}

fn parse_a1_cell(value: &str) -> Result<A1Cell, ExcelT21Error> {
    let bytes = value.as_bytes();
    let mut position = 0;
    if bytes.get(position) == Some(&b'$') {
        position += 1;
    }
    let letters_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_uppercase) {
        position += 1;
    }
    if position == letters_start || position - letters_start > 3 {
        return Err(invalid_arguments(
            "A1 cell must start with 1..=3 uppercase letters",
        ));
    }
    let mut column = 0_u32;
    for byte in bytes
        .iter()
        .skip(letters_start)
        .take(position.saturating_sub(letters_start))
    {
        column = column
            .checked_mul(26)
            .and_then(|value| value.checked_add(u32::from(byte - b'A' + 1)))
            .ok_or_else(|| invalid_arguments("A1 column overflows"))?;
    }
    if bytes.get(position) == Some(&b'$') {
        position += 1;
    }
    let digits_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        position += 1;
    }
    if position == digits_start || position != bytes.len() {
        return Err(invalid_arguments("A1 cell must end with a row number"));
    }
    if bytes.get(digits_start) == Some(&b'0') {
        return Err(invalid_arguments("A1 row numbers are one-based"));
    }
    let mut row = 0_u32;
    for byte in bytes
        .iter()
        .skip(digits_start)
        .take(position.saturating_sub(digits_start))
    {
        row = row
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(byte - b'0')))
            .ok_or_else(|| invalid_arguments("A1 row overflows"))?;
    }
    if column == 0 || column > 16_384 || row == 0 || row > 1_048_576 {
        return Err(invalid_arguments(
            "A1 cell must be within Excel bounds XFD1048576",
        ));
    }
    Ok(A1Cell { row, column })
}

fn reject_unknown_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    shape: &str,
) -> Result<(), ExcelT21Error> {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    if let Some(unknown) = object.keys().find(|key| !allowed.contains(key.as_str())) {
        return Err(invalid_arguments(format!(
            "{shape} contains unknown field `{unknown}`"
        )));
    }
    Ok(())
}

fn required_non_empty_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    maximum_bytes: usize,
) -> Result<&'a str, ExcelT21Error> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_arguments(format!("{field} must be a string")))?;
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        return Err(invalid_arguments(format!(
            "{field} must be 1..={maximum_bytes} bytes without control characters"
        )));
    }
    Ok(value)
}

/// Clock abstraction for deterministic TTL tests.
pub(crate) trait SessionClock: Send + Sync {
    fn now_unix_ms(&self) -> i64;
}

/// Opaque token source injected into a registry.
pub(crate) trait TokenSource: Send + Sync {
    fn next_token(&self) -> Result<String, ExcelT21Error>;
}

/// One validated session access snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionAccess {
    /// Runtime-local workbook handle.
    pub(crate) handle: WorkbookHandle,
    /// Owned process identity.
    pub(crate) owned_process: OwnedProcessIdentity,
    /// Workbook identity.
    pub(crate) workbook: WorkbookIdentity,
    /// Absolute expiry in Unix milliseconds.
    pub(crate) expires_at_unix_ms: i64,
    /// Remaining lifetime at snapshot time.
    pub(crate) remaining_ms: u64,
    /// Worksheets observed at open time.
    pub(crate) worksheets: Vec<WorksheetInfo>,
}

/// Output from a successful registry insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredSession {
    /// Opaque token.
    pub(crate) token: String,
    /// Absolute expiry in Unix milliseconds.
    pub(crate) expires_at_unix_ms: i64,
}

/// Session returned when task cleanup starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClosingSession {
    /// Runtime-local workbook handle.
    pub(crate) handle: WorkbookHandle,
}

#[derive(Debug, Clone)]
pub(super) struct SessionRecord {
    tool_bus_session_id: String,
    task_id: String,
    access: SessionAccess,
}

/// ToolBus-isolated, bounded, in-memory session registry.
pub(crate) struct SessionRegistry {
    pub(crate) tool_bus_session_id: String,
    clock: Arc<dyn SessionClock>,
    token_source: Arc<dyn TokenSource>,
    capacity: SessionCapacity,
    sessions: BTreeMap<String, SessionRecord>,
    pub(super) closing: BTreeMap<u64, SessionRecord>,
    active_by_task: BTreeMap<String, String>,
}

impl SessionRegistry {
    /// Creates a registry for one `ToolBus` session.
    pub(crate) fn new(
        tool_bus_session_id: impl Into<String>,
        clock: Arc<dyn SessionClock>,
        token_source: Arc<dyn TokenSource>,
    ) -> Result<Self, ExcelT21Error> {
        Self::with_capacity(
            tool_bus_session_id,
            clock,
            token_source,
            SessionCapacity::default(),
        )
    }

    /// Creates a registry sharing the process-wide active-session capacity.
    pub(crate) fn with_capacity(
        tool_bus_session_id: impl Into<String>,
        clock: Arc<dyn SessionClock>,
        token_source: Arc<dyn TokenSource>,
        capacity: SessionCapacity,
    ) -> Result<Self, ExcelT21Error> {
        let tool_bus_session_id = tool_bus_session_id.into();
        if tool_bus_session_id.trim().is_empty() {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "tool_bus_session_id must not be empty",
            ));
        }
        Ok(Self {
            tool_bus_session_id,
            clock,
            token_source,
            capacity,
            sessions: BTreeMap::new(),
            closing: BTreeMap::new(),
            active_by_task: BTreeMap::new(),
        })
    }

    /// Inserts one opened workbook or rejects without evicting active sessions.
    pub(crate) fn register(
        &mut self,
        task_id: &str,
        opened: OpenedWorkbook,
    ) -> Result<RegisteredSession, ExcelT21Error> {
        if task_id.trim().is_empty() {
            return Err(invalid_arguments("task_id must not be empty"));
        }
        let now = self.clock.now_unix_ms();
        if self.active_by_task.contains_key(task_id) {
            return Err(invalid_arguments(
                "the task already has one active Excel session",
            ));
        }
        opened.owned_process.validate()?;
        opened.workbook.validate()?;
        validate_worksheets(&opened.worksheets)?;
        let token = self.token_source.next_token()?;
        validate_session_token(&token)?;
        if self.sessions.contains_key(&token) {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "token source produced a duplicate active session token",
            ));
        }
        let expires_at_unix_ms = now
            .checked_add(SESSION_TTL_MS)
            .ok_or_else(|| ExcelT21Error::new(ErrorCode::Fatal, "session expiry overflowed"))?;
        if !self.capacity.try_reserve() {
            return Err(ExcelT21Error::new(
                ErrorCode::UserInteraction,
                "the global Excel session limit is active; close a task before opening another",
            ));
        }
        self.sessions.insert(
            token.clone(),
            SessionRecord {
                tool_bus_session_id: self.tool_bus_session_id.clone(),
                task_id: task_id.to_owned(),
                access: SessionAccess {
                    handle: opened.handle,
                    owned_process: opened.owned_process,
                    workbook: opened.workbook,
                    expires_at_unix_ms,
                    remaining_ms: 0,
                    worksheets: opened.worksheets,
                },
            },
        );
        self.active_by_task
            .insert(task_id.to_owned(), token.clone());
        Ok(RegisteredSession {
            token,
            expires_at_unix_ms,
        })
    }

    /// Looks up one token and verifies `ToolBus` + task isolation.
    pub(crate) fn lookup(
        &self,
        tool_bus_session_id: &str,
        task_id: &str,
        token: &str,
    ) -> Result<SessionAccess, ExcelT21Error> {
        validate_session_token(token)?;
        let now = self.clock.now_unix_ms();
        let record = self
            .sessions
            .get(token)
            .ok_or_else(|| invalid_arguments("Excel session is unknown or expired"))?;
        if record.access.expires_at_unix_ms <= now {
            return Err(invalid_arguments("Excel session is unknown or expired"));
        }
        if record.tool_bus_session_id != self.tool_bus_session_id
            || record.tool_bus_session_id != tool_bus_session_id
            || record.task_id != task_id
        {
            return Err(invalid_arguments(
                "Excel session is not valid for this task and ToolBus session",
            ));
        }
        let mut access = record.access.clone();
        let remaining = access.expires_at_unix_ms.saturating_sub(now).max(0);
        access.remaining_ms = u64::try_from(remaining).map_err(|_| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                "session remaining lifetime does not fit u64",
            )
        })?;
        Ok(access)
    }

    /// Revalidates a token after a runtime call and returns its current access.
    pub(crate) fn revalidate(
        &self,
        tool_bus_session_id: &str,
        task_id: &str,
        token: &str,
    ) -> Result<SessionAccess, ExcelT21Error> {
        self.lookup(tool_bus_session_id, task_id, token)
    }

    /// Removes the task's session immediately and returns its runtime handle.
    pub(crate) fn begin_close_task(&mut self, task_id: &str) -> Option<ClosingSession> {
        let token = self.active_by_task.remove(task_id)?;
        let record = self.sessions.remove(&token)?;
        let handle = record.access.handle;
        self.closing.insert(handle.get(), record);
        Some(ClosingSession { handle })
    }

    /// Moves every active session, plus retained closing sessions, to task-end cleanup.
    pub(crate) fn begin_close_all(&mut self) -> Vec<ClosingSession> {
        for (_, token) in std::mem::take(&mut self.active_by_task) {
            if let Some(record) = self.sessions.remove(&token) {
                self.closing.insert(record.access.handle.get(), record);
            }
        }
        self.closing
            .keys()
            .copied()
            .map(WorkbookHandle::new)
            .map(|handle| ClosingSession { handle })
            .collect()
    }

    /// Moves expired records into closing state and returns every handle awaiting close.
    pub(crate) fn expire_sessions(&mut self) -> Vec<ClosingSession> {
        let now = self.clock.now_unix_ms();
        let expired = self
            .sessions
            .iter()
            .filter(|(_, record)| record.access.expires_at_unix_ms <= now)
            .map(|(token, _)| token.clone())
            .collect::<Vec<_>>();
        for token in expired {
            if let Some(record) = self.sessions.remove(&token) {
                self.active_by_task.remove(&record.task_id);
                self.closing.insert(record.access.handle.get(), record);
            }
        }
        self.closing
            .keys()
            .copied()
            .map(WorkbookHandle::new)
            .map(|handle| ClosingSession { handle })
            .collect()
    }

    /// Completes cleanup and releases capacity only after the runtime close succeeds.
    pub(crate) fn finish_close(&mut self, handle: WorkbookHandle) -> Result<(), ExcelT21Error> {
        if self.closing.remove(&handle.get()).is_some() {
            self.capacity.release();
            Ok(())
        } else {
            Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel closing session was not registered",
            ))
        }
    }

    /// Releases every closing session after the runtime confirms full convergence.
    pub(crate) fn finish_all_closing(&mut self) -> usize {
        let released = self.closing.len();
        self.closing.clear();
        for _ in 0..released {
            self.capacity.release();
        }
        released
    }

    /// Number of active sessions retained by this registry.
    pub(crate) fn active_session_count(&self) -> usize {
        self.sessions.len()
    }
}

fn validate_session_token(token: &str) -> Result<(), ExcelT21Error> {
    if token.len() < MIN_SESSION_TOKEN_BYTES
        || token.len() > MAX_SESSION_TOKEN_BYTES
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(invalid_arguments(
            "session_id must be 32..=128 opaque ASCII token bytes",
        ));
    }
    Ok(())
}

fn validate_worksheets(worksheets: &[WorksheetInfo]) -> Result<(), ExcelT21Error> {
    let worksheet_count = u64::try_from(worksheets.len())
        .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "worksheet count does not fit u64"))?;
    if worksheets.is_empty() || worksheet_count > MAX_WORKSHEETS {
        return Err(runtime_shape_error(
            "workbook must expose 1..=1024 worksheets",
        ));
    }
    let mut ordinals = BTreeSet::new();
    let mut names = BTreeSet::new();
    for worksheet in worksheets {
        if worksheet.name.is_empty()
            || worksheet.name.chars().any(char::is_control)
            || worksheet.ordinal == 0
            || !ordinals.insert(worksheet.ordinal)
            || !names.insert(worksheet.name.as_str())
        {
            return Err(runtime_shape_error(
                "worksheet names and ordinals must be non-empty and unique",
            ));
        }
    }
    Ok(())
}
