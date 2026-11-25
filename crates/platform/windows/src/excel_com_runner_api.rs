//! Public self-contained Excel T2.1 read-only runner types and operations.
//!
//! This module never owns a COM object. Requests cross the runner channel and
//! all results are copied into the plain data types below before returning.

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

use super::{
    ExcelComRunner, ExcelWorkbookHandle, RunnerCommand, RunnerValue, expect_unit,
    runner_protocol_error,
};

/// Maximum worksheets returned for one workbook.
pub const MAX_EXCEL_WORKSHEETS: usize = 1024;
/// Maximum named ranges returned for one workbook.
pub const MAX_EXCEL_NAMED_RANGES: usize = 2048;
/// Maximum cells returned for one range request.
pub const MAX_EXCEL_RANGE_CELLS: usize = 4096;
/// Maximum UTF-8 bytes across one range payload.
pub const MAX_EXCEL_RANGE_BYTES: usize = 262_144;
/// Largest Excel worksheet row.
pub const MAX_EXCEL_ROW: u32 = 1_048_576;
/// Largest Excel worksheet column.
pub const MAX_EXCEL_COLUMN: u32 = 16_384;

/// Process ownership evidence retained with a workbook handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelOwnedProcessIdentity {
    process_id: u32,
    start_time_100ns: u64,
    executable_path: String,
}

impl ExcelOwnedProcessIdentity {
    pub(super) fn new(
        process_id: u32,
        start_time_100ns: u64,
        executable_path: String,
    ) -> PlatformResult<Self> {
        if process_id == 0 || start_time_100ns == 0 || executable_path.trim().is_empty() {
            return Err(runner_protocol_error(
                "Excel owned process identity is incomplete",
            ));
        }
        Ok(Self {
            process_id,
            start_time_100ns,
            executable_path,
        })
    }

    /// Returns the owned process id.
    #[must_use]
    pub const fn process_id(&self) -> u32 {
        self.process_id
    }

    /// Returns the creation FILETIME in 100 ns units.
    #[must_use]
    pub const fn start_time_100ns(&self) -> u64 {
        self.start_time_100ns
    }

    /// Returns the executable path.
    #[must_use]
    pub fn executable_path(&self) -> &str {
        &self.executable_path
    }
}

/// Worksheet metadata returned by the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelWorksheetInfo {
    name: String,
    ordinal: u32,
    used_range_address: Option<String>,
}

impl ExcelWorksheetInfo {
    pub(super) fn new(
        name: String,
        ordinal: u32,
        used_range_address: Option<String>,
    ) -> PlatformResult<Self> {
        if name.is_empty() || name.len() > 255 || ordinal == 0 {
            return Err(runner_protocol_error(
                "Excel worksheet metadata has an invalid name or ordinal",
            ));
        }
        Ok(Self {
            name,
            ordinal,
            used_range_address,
        })
    }

    /// Exact worksheet name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// One-based worksheet ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Used range address when Excel reports one.
    #[must_use]
    pub fn used_range_address(&self) -> Option<&str> {
        self.used_range_address.as_deref()
    }
}

/// Scope of a named range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExcelNamedRangeScope {
    /// Workbook-scoped name.
    Workbook,
    /// Worksheet-scoped name with exact sheet name.
    Worksheet(String),
}

/// Named range metadata returned by the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelNamedRangeInfo {
    name: String,
    scope: ExcelNamedRangeScope,
    refers_to: String,
}

impl ExcelNamedRangeInfo {
    pub(super) fn new(
        name: String,
        scope: ExcelNamedRangeScope,
        refers_to: String,
    ) -> PlatformResult<Self> {
        if name.is_empty() || name.len() > 255 || refers_to.is_empty() || refers_to.len() > 8192 {
            return Err(runner_protocol_error(
                "Excel named range metadata is empty or exceeds its bound",
            ));
        }
        if matches!(&scope, ExcelNamedRangeScope::Worksheet(sheet) if sheet.is_empty()) {
            return Err(runner_protocol_error(
                "Excel worksheet-scoped name has an empty sheet name",
            ));
        }
        Ok(Self {
            name,
            scope,
            refers_to,
        })
    }

    /// Exact name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Workbook or worksheet scope.
    #[must_use]
    pub const fn scope(&self) -> &ExcelNamedRangeScope {
        &self.scope
    }

    /// Formula text reported by Excel.
    #[must_use]
    pub fn refers_to(&self) -> &str {
        &self.refers_to
    }
}

/// Excel calculation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelCalculationInfo {
    mode: String,
    state: String,
}

impl ExcelCalculationInfo {
    pub(super) const fn new(mode: String, state: String) -> Self {
        Self { mode, state }
    }

    /// Stable calculation mode.
    #[must_use]
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Stable calculation state.
    #[must_use]
    pub fn state(&self) -> &str {
        &self.state
    }
}

/// Read-only workbook metadata snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelWorkbookSnapshot {
    canonical_path: String,
    read_only: bool,
    worksheets: Vec<ExcelWorksheetInfo>,
    named_ranges: Vec<ExcelNamedRangeInfo>,
    calculation: ExcelCalculationInfo,
}

impl ExcelWorkbookSnapshot {
    pub(super) fn new(
        canonical_path: String,
        read_only: bool,
        worksheets: Vec<ExcelWorksheetInfo>,
        named_ranges: Vec<ExcelNamedRangeInfo>,
        calculation: ExcelCalculationInfo,
    ) -> PlatformResult<Self> {
        if canonical_path.is_empty()
            || !read_only
            || worksheets.is_empty()
            || worksheets.len() > MAX_EXCEL_WORKSHEETS
            || named_ranges.len() > MAX_EXCEL_NAMED_RANGES
        {
            return Err(runner_protocol_error(
                "Excel workbook metadata is incomplete or exceeds its bound",
            ));
        }
        Ok(Self {
            canonical_path,
            read_only,
            worksheets,
            named_ranges,
            calculation,
        })
    }

    /// Canonical workbook path.
    #[must_use]
    pub fn canonical_path(&self) -> &str {
        &self.canonical_path
    }

    /// Always true for the T2.1 read-only surface.
    #[must_use]
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Worksheets in Excel order.
    #[must_use]
    pub fn worksheets(&self) -> &[ExcelWorksheetInfo] {
        &self.worksheets
    }

    /// Named ranges in Excel order.
    #[must_use]
    pub fn named_ranges(&self) -> &[ExcelNamedRangeInfo] {
        &self.named_ranges
    }

    /// Calculation metadata.
    #[must_use]
    pub const fn calculation(&self) -> &ExcelCalculationInfo {
        &self.calculation
    }
}

/// Cell read mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcelReadMode {
    /// `.Value2`.
    Value2,
    /// `.Formula`.
    Formula,
    /// `.Text`.
    Text,
}

impl ExcelReadMode {
    pub(super) const fn property_name(self) -> &'static str {
        match self {
            Self::Value2 => "Value2",
            Self::Formula => "Formula",
            Self::Text => "Text",
        }
    }
}

/// Closed cell value type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcelCellValueType {
    /// Empty or null cell.
    Blank,
    /// Finite numeric value.
    Number,
    /// Text value.
    Text,
    /// Boolean value.
    Boolean,
    /// Known Excel error.
    Error,
}

/// One range cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelRuntimeCell {
    row: u32,
    column: u32,
    value_type: ExcelCellValueType,
    value_text: String,
}

impl ExcelRuntimeCell {
    pub(super) fn new(
        row: u32,
        column: u32,
        value_type: ExcelCellValueType,
        value_text: String,
    ) -> PlatformResult<Self> {
        if row == 0
            || row > MAX_EXCEL_ROW
            || column == 0
            || column > MAX_EXCEL_COLUMN
            || value_text.len() > MAX_EXCEL_RANGE_BYTES
        {
            return Err(runner_protocol_error("Excel range cell is invalid"));
        }
        Ok(Self {
            row,
            column,
            value_type,
            value_text,
        })
    }

    /// One-based row relative to the range.
    #[must_use]
    pub const fn row(&self) -> u32 {
        self.row
    }

    /// One-based column relative to the range.
    #[must_use]
    pub const fn column(&self) -> u32 {
        self.column
    }

    /// Closed value type.
    #[must_use]
    pub const fn value_type(&self) -> ExcelCellValueType {
        self.value_type
    }

    /// Value text.
    #[must_use]
    pub fn value_text(&self) -> &str {
        &self.value_text
    }
}

/// Range snapshot returned by the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelRangeSnapshot {
    address: String,
    rows: u32,
    columns: u32,
    mode: ExcelReadMode,
    cells: Vec<ExcelRuntimeCell>,
}

impl ExcelRangeSnapshot {
    pub(super) fn new(
        address: String,
        rows: u32,
        columns: u32,
        mode: ExcelReadMode,
        cells: Vec<ExcelRuntimeCell>,
    ) -> PlatformResult<Self> {
        let expected = u64::from(rows)
            .checked_mul(u64::from(columns))
            .ok_or_else(|| runner_protocol_error("Excel range cell count overflowed"))?;
        if address.is_empty()
            || rows == 0
            || columns == 0
            || expected != u64::try_from(cells.len()).unwrap_or(u64::MAX)
            || expected > u64::try_from(MAX_EXCEL_RANGE_CELLS).unwrap_or(u64::MAX)
        {
            return Err(runner_protocol_error(
                "Excel range snapshot is incomplete or exceeds its bound",
            ));
        }
        let total_bytes = cells
            .iter()
            .try_fold(0_usize, |total, cell| {
                total.checked_add(cell.value_text.len())
            })
            .ok_or_else(|| runner_protocol_error("Excel range payload size overflowed"))?;
        if total_bytes > MAX_EXCEL_RANGE_BYTES {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!(
                    "Excel range payload is {total_bytes} bytes; maximum is {MAX_EXCEL_RANGE_BYTES}"
                ),
            ));
        }
        Ok(Self {
            address,
            rows,
            columns,
            mode,
            cells,
        })
    }

    /// Address reported by Excel.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Row count.
    #[must_use]
    pub const fn rows(&self) -> u32 {
        self.rows
    }

    /// Column count.
    #[must_use]
    pub const fn columns(&self) -> u32 {
        self.columns
    }

    /// Requested read mode.
    #[must_use]
    pub const fn mode(&self) -> ExcelReadMode {
        self.mode
    }

    /// Cells in row-major order.
    #[must_use]
    pub fn cells(&self) -> &[ExcelRuntimeCell] {
        &self.cells
    }
}

/// Worksheet selector in an A1 reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExcelWorksheetSelector {
    /// Exact worksheet name.
    Name(String),
    /// One-based worksheet ordinal.
    Ordinal(u32),
}

/// Closed range selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExcelRangeSelector {
    /// Workbook or worksheet-scoped name.
    NamedRange(String),
    /// Explicit worksheet and inclusive A1 bounds.
    A1 {
        /// Worksheet selector.
        worksheet: ExcelWorksheetSelector,
        /// One-based first row.
        start_row: u32,
        /// One-based first column.
        start_column: u32,
        /// One-based final row.
        end_row: u32,
        /// One-based final column.
        end_column: u32,
    },
}

impl ExcelRangeSelector {
    pub(super) fn validate(&self) -> PlatformResult<()> {
        match self {
            Self::NamedRange(name) if name.is_empty() || name.len() > 255 => {
                Err(PlatformError::new(
                    ErrorCode::ToolInvalidArgs,
                    "Excel named range must contain 1..=255 bytes",
                ))
            }
            Self::NamedRange(_) => Ok(()),
            Self::A1 {
                worksheet,
                start_row,
                start_column,
                end_row,
                end_column,
            } => {
                if matches!(worksheet, ExcelWorksheetSelector::Name(name) if name.is_empty() || name.len() > 255)
                    || matches!(worksheet, ExcelWorksheetSelector::Ordinal(0))
                    || !(1..=MAX_EXCEL_ROW).contains(start_row)
                    || !(1..=MAX_EXCEL_ROW).contains(end_row)
                    || !(1..=MAX_EXCEL_COLUMN).contains(start_column)
                    || !(1..=MAX_EXCEL_COLUMN).contains(end_column)
                    || end_row < start_row
                    || end_column < start_column
                {
                    return Err(PlatformError::new(
                        ErrorCode::ToolInvalidArgs,
                        "Excel A1 selector is outside the workbook bounds",
                    ));
                }
                let cells = u64::from(end_row - start_row + 1)
                    .checked_mul(u64::from(end_column - start_column + 1));
                if cells.is_none_or(|count| {
                    count > u64::try_from(MAX_EXCEL_RANGE_CELLS).unwrap_or(u64::MAX)
                }) {
                    return Err(PlatformError::new(
                        ErrorCode::ToolInvalidArgs,
                        "Excel A1 selector exceeds 4096 cells",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl ExcelComRunner {
    /// Reads the complete workbook metadata snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed platform failure when the handle is stale, the workbook
    /// is no longer owned, or metadata exceeds the hard bounds.
    pub fn read_workbook(
        &self,
        workbook: &ExcelWorkbookHandle,
    ) -> PlatformResult<ExcelWorkbookSnapshot> {
        match self.dispatch(|response| RunnerCommand::ReadWorkbook {
            runner_id: workbook.runner_id(),
            session_id: workbook.session_id(),
            response,
        })? {
            RunnerValue::WorkbookSnapshot(snapshot) => Ok(snapshot),
            _ => Err(runner_protocol_error(
                "Excel COM runner returned no workbook snapshot",
            )),
        }
    }

    /// Reads one bounded workbook range.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for invalid selectors or payload bounds and a
    /// typed backend failure for unknown ranges or stale sessions.
    pub fn read_range(
        &self,
        workbook: &ExcelWorkbookHandle,
        selector: ExcelRangeSelector,
        mode: ExcelReadMode,
    ) -> PlatformResult<ExcelRangeSnapshot> {
        selector.validate()?;
        match self.dispatch(|response| RunnerCommand::ReadRange {
            runner_id: workbook.runner_id(),
            session_id: workbook.session_id(),
            selector,
            mode,
            response,
        })? {
            RunnerValue::RangeSnapshot(snapshot) => Ok(snapshot),
            _ => Err(runner_protocol_error(
                "Excel COM runner returned no range snapshot",
            )),
        }
    }

    /// Returns a deterministic fingerprint of workbook metadata.
    ///
    /// This is intentionally not a file-content digest.
    ///
    /// # Errors
    ///
    /// Returns the workbook metadata read failure unchanged.
    pub fn fingerprint_workbook(&self, workbook: &ExcelWorkbookHandle) -> PlatformResult<String> {
        let snapshot = self.read_workbook(workbook)?;
        Ok(crate::digest::sha256_fingerprint(&fingerprint_material(
            snapshot.canonical_path(),
            snapshot.worksheets(),
            snapshot.named_ranges(),
            snapshot.calculation(),
        )?))
    }

    /// Revalidates the owned process identity for one open workbook.
    ///
    /// # Errors
    ///
    /// Returns `TargetNotFound` when the process identity or workbook ownership
    /// can no longer be verified.
    pub fn verify_ownership(&self, workbook: &ExcelWorkbookHandle) -> PlatformResult<()> {
        expect_unit(self.dispatch(|response| RunnerCommand::VerifyOwnership {
            runner_id: workbook.runner_id(),
            session_id: workbook.session_id(),
            response,
        }))
    }
}

fn fingerprint_material(
    canonical_path: &str,
    worksheets: &[ExcelWorksheetInfo],
    named_ranges: &[ExcelNamedRangeInfo],
    calculation: &ExcelCalculationInfo,
) -> PlatformResult<String> {
    use std::fmt::Write;

    let mut output = format!(
        "path={canonical_path}\ncalc={}:{}\n",
        calculation.mode(),
        calculation.state()
    );
    for worksheet in worksheets {
        writeln!(
            output,
            "sheet={}:{}:{}",
            worksheet.ordinal(),
            worksheet.name(),
            worksheet.used_range_address().unwrap_or("")
        )
        .map_err(|error| {
            runner_protocol_error(format!("Excel fingerprint worksheet write failed: {error}"))
        })?;
    }
    for named_range in named_ranges {
        let scope = match named_range.scope() {
            ExcelNamedRangeScope::Workbook => "workbook".to_owned(),
            ExcelNamedRangeScope::Worksheet(sheet) => format!("sheet:{sheet}"),
        };
        writeln!(
            output,
            "name={}:{}:{}",
            scope,
            named_range.name(),
            named_range.refers_to()
        )
        .map_err(|error| {
            runner_protocol_error(format!("Excel fingerprint name write failed: {error}"))
        })?;
    }
    Ok(output)
}

pub(super) fn range_address(selector: &ExcelRangeSelector) -> PlatformResult<String> {
    match selector {
        ExcelRangeSelector::NamedRange(name) => Ok(name.clone()),
        ExcelRangeSelector::A1 {
            start_row,
            start_column,
            end_row,
            end_column,
            ..
        } => {
            let start = format!("{}{}", column_name(*start_column)?, start_row);
            let end = format!("{}{}", column_name(*end_column)?, end_row);
            Ok(if start == end {
                start
            } else {
                format!("{start}:{end}")
            })
        }
    }
}

fn column_name(column: u32) -> PlatformResult<String> {
    if !(1..=MAX_EXCEL_COLUMN).contains(&column) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel column is outside 1..=16384",
        ));
    }
    let mut value = column;
    let mut output = Vec::new();
    while value > 0 {
        let remainder = (value - 1) % 26;
        output.push((b'A' + u8::try_from(remainder).unwrap_or(0)) as char);
        value = (value - 1) / 26;
    }
    output.reverse();
    Ok(output.into_iter().collect())
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn calculation_mode(value: i32) -> String {
    match value {
        -4105 => "automatic".to_owned(),
        -4135 => "manual".to_owned(),
        2 => "automatic_except_tables".to_owned(),
        _ => format!("unknown_{value}"),
    }
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn calculation_state(value: i32) -> String {
    match value {
        0 => "done".to_owned(),
        1 => "pending".to_owned(),
        2 => "calculating".to_owned(),
        _ => format!("unknown_{value}"),
    }
}

#[cfg(all(test, windows))]
#[allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::missing_const_for_fn,
    clippy::print_stderr,
    clippy::redundant_pub_crate
)]
pub(crate) mod real_com_support {
    use super::*;

    pub(crate) struct RealComAcceptanceGuard {
        runner: Option<ExcelComRunner>,
        owned_process: Option<crate::excel_process::ExcelProcessIdentity>,
    }

    impl RealComAcceptanceGuard {
        pub(crate) fn new(runner: ExcelComRunner) -> Self {
            Self {
                runner: Some(runner),
                owned_process: None,
            }
        }

        pub(crate) fn runner(&self) -> &ExcelComRunner {
            self.runner.as_ref().expect("acceptance runner")
        }

        pub(crate) fn set_owned_process(
            &mut self,
            identity: crate::excel_process::ExcelProcessIdentity,
        ) {
            self.owned_process = Some(identity);
        }

        pub(crate) fn shutdown(&mut self) -> PlatformResult<()> {
            self.runner
                .take()
                .expect("acceptance runner before shutdown")
                .shutdown()
        }
    }

    #[cfg(all(test, windows))]
    impl Drop for RealComAcceptanceGuard {
        fn drop(&mut self) {
            if let Some(mut runner) = self.runner.take()
                && runner.shutdown().is_err()
            {
                eprintln!("real-COM acceptance runner shutdown failed during Drop");
            }
            let Some(identity) = self.owned_process.as_ref() else {
                return;
            };
            match crate::excel_process::process_matches(
                identity.process_id,
                identity.start_time_100ns,
                &identity.executable_path,
            ) {
                Ok(true) => {
                    let process_id = identity.process_id;
                    match std::process::Command::new("taskkill.exe")
                        .args(["/PID", &process_id.to_string(), "/T", "/F"])
                        .status()
                    {
                        Ok(status) if status.success() => {
                            eprintln!(
                                "real-COM acceptance force-terminated owned Excel {process_id}"
                            );
                        }
                        Ok(status) => {
                            eprintln!(
                                "real-COM acceptance taskkill for owned Excel {process_id} exited with {status}"
                            );
                        }
                        Err(error) => {
                            eprintln!(
                                "real-COM acceptance taskkill for owned Excel {process_id} failed: {error}"
                            );
                        }
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    eprintln!(
                        "real-COM acceptance could not verify owned Excel {}: {error}",
                        identity.process_id
                    );
                }
            }
        }
    }

    pub(crate) fn verify_real_com_read_surface(
        runner: &ExcelComRunner,
        workbook: &ExcelWorkbookHandle,
    ) {
        let metadata = runner
            .read_workbook(workbook)
            .expect("read workbook metadata");
        assert_eq!(metadata.worksheets().len(), 1);
        assert_eq!(metadata.worksheets()[0].name(), "Sheet1");
        assert!(
            metadata
                .named_ranges()
                .iter()
                .any(|name| name.name() == "SalesData")
        );
        runner
            .read_active_sheet_text(workbook, "A1")
            .expect("read active sheet A1 text");
        let value_range = runner
            .read_range(
                workbook,
                ExcelRangeSelector::A1 {
                    worksheet: ExcelWorksheetSelector::Name("Sheet1".to_owned()),
                    start_row: 1,
                    start_column: 1,
                    end_row: 2,
                    end_column: 3,
                },
                ExcelReadMode::Value2,
            )
            .expect("read typed value range");
        assert_eq!(
            value_range.cells()[2].value_type(),
            ExcelCellValueType::Boolean
        );
        assert_eq!(
            value_range.cells()[5].value_type(),
            ExcelCellValueType::Error
        );
        assert_eq!(value_range.cells()[5].value_text(), "#DIV/0!");
        for mode in [
            ExcelReadMode::Value2,
            ExcelReadMode::Formula,
            ExcelReadMode::Text,
        ] {
            let range = runner
                .read_range(
                    workbook,
                    ExcelRangeSelector::A1 {
                        worksheet: ExcelWorksheetSelector::Name("Sheet1".to_owned()),
                        start_row: 1,
                        start_column: 1,
                        end_row: 2,
                        end_column: 3,
                    },
                    mode,
                )
                .expect("read A1 range");
            assert_eq!(range.cells().len(), 6);
        }
        let named = runner
            .read_range(
                workbook,
                ExcelRangeSelector::NamedRange("SalesData".to_owned()),
                ExcelReadMode::Value2,
            )
            .expect("read named range");
        assert_eq!(named.cells().len(), 4);
        let fingerprint = runner
            .fingerprint_workbook(workbook)
            .expect("workbook fingerprint");
        assert_eq!(
            fingerprint,
            runner
                .fingerprint_workbook(workbook)
                .expect("stable workbook fingerprint")
        );
        runner.verify_ownership(workbook).expect("verify ownership");
    }
}
