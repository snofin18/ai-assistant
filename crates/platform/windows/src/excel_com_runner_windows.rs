//! Windows-only Excel COM backend and `IDispatch` helpers.
//!
//! The public handle lives in `excel_com_runner`; this module owns every
//! Windows COM object on the dedicated worker thread.

#[cfg(windows)]
use super::api::{calculation_mode, calculation_state};
use super::{
    ErrorCode, ExcelCalculationInfo, ExcelCellValueType, ExcelComBackend, ExcelNamedRangeInfo,
    ExcelNamedRangeScope, ExcelOwnedProcessIdentity, ExcelRangeSelector, ExcelRangeSnapshot,
    ExcelReadMode, ExcelRuntimeCell, ExcelWorkbookSnapshot, ExcelWorksheetInfo,
    ExcelWorksheetSelector, MAX_EXCEL_NAMED_RANGES, MAX_EXCEL_RANGE_CELLS, MAX_EXCEL_WORKSHEETS,
    Path, PlatformError, PlatformResult, runner_protocol_error,
};
use std::path::PathBuf;
#[cfg(windows)]
use variants::{collection_count, variant_from_bool, variant_from_i4, variant_from_string};

#[cfg(windows)]
#[path = "excel_com_runner_windows_startup.rs"]
mod startup;
#[cfg(windows)]
pub(super) use startup::spawn_windows_excel_runner;

#[cfg(windows)]
struct WindowsExcelComBackend {
    workbook: Option<windows::Win32::System::Com::IDispatch>,
    application: Option<windows::Win32::System::Com::IDispatch>,
    canonical_path: Option<PathBuf>,
}

#[cfg(windows)]
impl WindowsExcelComBackend {
    const fn new() -> Self {
        Self {
            workbook: None,
            application: None,
            canonical_path: None,
        }
    }

    fn open_workbook(&mut self, path: &Path, read_only: bool) -> PlatformResult<()> {
        if self.workbook.is_some() {
            return Err(runner_protocol_error(
                "Windows Excel backend already owns an open workbook",
            ));
        }
        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        let workbooks = get_dispatch_property(application, "Workbooks")?;
        let path_text = path.to_str().ok_or_else(|| {
            PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel workbook path is not valid UTF-8",
            )
        })?;
        let workbook = invoke_method_returning_dispatch(
            &workbooks,
            "Open",
            vec![
                variant_from_string(path_text),
                variant_from_i4(0),
                variant_from_bool(read_only),
            ],
        )?;
        self.workbook = Some(workbook);
        self.canonical_path = Some(path.to_path_buf());
        Ok(())
    }
}

#[cfg(windows)]
impl ExcelComBackend for WindowsExcelComBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        use windows::Win32::System::Com::{
            CLSCTX_LOCAL_SERVER, CLSIDFromProgID, CoCreateInstance, IDispatch,
        };
        use windows::core::w;

        if self.application.is_some() {
            return Err(runner_protocol_error(
                "Windows Excel backend already owns an application",
            ));
        }
        crate::excel_process::ensure_no_existing_excel_process()?;
        // SAFETY: the worker thread holds an STA, the ProgID is static and
        // NUL-terminated, and the returned interface is retained in this thread.
        let class_id = unsafe { CLSIDFromProgID(w!("Excel.Application")) }.map_err(|error| {
            crate::error::error_from_hresult(error.code().0, "CLSIDFromProgID(Excel.Application)")
        })?;
        // SAFETY: CLSID resolution succeeded, no aggregation is used, and the
        // returned IDispatch remains owned by the dedicated STA worker.
        let application: IDispatch =
            unsafe { CoCreateInstance(&raw const class_id, None, CLSCTX_LOCAL_SERVER) }.map_err(
                |error| {
                    crate::error::error_from_hresult(
                        error.code().0,
                        "CoCreateInstance(Excel.Application)",
                    )
                },
            )?;
        self.application = Some(application);
        Ok(())
    }

    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        let hwnd_value = get_i4_property(application, "Hwnd")?;
        let pointer_value = isize::try_from(hwnd_value)
            .map_err(|_| runner_protocol_error("Excel Application.Hwnd did not fit in isize"))?;
        let hwnd = HWND(pointer_value as *mut core::ffi::c_void);
        let mut process_id = 0_u32;
        // SAFETY: the HWND came from the owned Excel application and is only
        // queried; the out-parameter points to a live stack value.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process_id)) };
        if process_id == 0 {
            return Err(runner_protocol_error(
                "Excel Application.Hwnd did not map to an owned process id",
            ));
        }
        let identity = crate::excel_process::process_identity(process_id)?.ok_or_else(|| {
            runner_protocol_error("owned Excel process disappeared before identity capture")
        })?;
        ExcelOwnedProcessIdentity::new(
            identity.process_id,
            identity.start_time_100ns,
            identity.executable_path,
        )
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        invoke_method(application, "Quit")?;
        self.workbook = None;
        self.application = None;
        Ok(())
    }

    fn open_read_only(&mut self, path: &Path) -> PlatformResult<()> {
        self.open_workbook(path, true)
    }

    fn read_active_sheet_text(&self, address: &str) -> PlatformResult<String> {
        let workbook = self
            .workbook
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no open workbook"))?;
        let active_sheet = get_dispatch_property(workbook, "ActiveSheet")?;
        let range = get_dispatch_property_with_string_argument(&active_sheet, "Range", address)?;
        get_string_property(&range, "Text")
    }

    fn read_workbook(&mut self, path: &Path) -> PlatformResult<ExcelWorkbookSnapshot> {
        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        let workbook = self
            .workbook
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no open workbook"))?;
        let worksheets_dispatch = get_dispatch_property(workbook, "Worksheets")?;
        let worksheet_count = collection_count(&worksheets_dispatch, "Worksheets")?;
        if worksheet_count == 0 || worksheet_count > MAX_EXCEL_WORKSHEETS {
            return Err(PlatformError::new(
                ErrorCode::UserInteraction,
                format!(
                    "Excel workbook worksheet count is {worksheet_count}; maximum is {MAX_EXCEL_WORKSHEETS}"
                ),
            ));
        }
        let mut worksheets = Vec::with_capacity(worksheet_count);
        for ordinal in 1..=worksheet_count {
            let ordinal_i32 = i32::try_from(ordinal)
                .map_err(|_| runner_protocol_error("worksheet ordinal does not fit i32"))?;
            let worksheet =
                get_dispatch_property_with_i4_argument(&worksheets_dispatch, "Item", ordinal_i32)?;
            let name = get_string_property(&worksheet, "Name")?;
            let used_range = get_dispatch_property(&worksheet, "UsedRange")?;
            let used_range_address = get_string_property(&used_range, "Address")?;
            worksheets.push(ExcelWorksheetInfo::new(
                name,
                u32::try_from(ordinal)
                    .map_err(|_| runner_protocol_error("worksheet ordinal does not fit u32"))?,
                Some(used_range_address),
            )?);
        }
        let names_dispatch = get_dispatch_property(workbook, "Names")?;
        let name_count = collection_count(&names_dispatch, "Names")?;
        if name_count > MAX_EXCEL_NAMED_RANGES {
            return Err(PlatformError::new(
                ErrorCode::UserInteraction,
                format!(
                    "Excel workbook named range count is {name_count}; maximum is {MAX_EXCEL_NAMED_RANGES}"
                ),
            ));
        }
        let mut named_ranges = Vec::with_capacity(name_count);
        for ordinal in 1..=name_count {
            let ordinal_i32 = i32::try_from(ordinal)
                .map_err(|_| runner_protocol_error("name ordinal does not fit i32"))?;
            let range_definition =
                get_dispatch_property_with_i4_argument(&names_dispatch, "Item", ordinal_i32)?;
            let qualified_name = get_string_property(&range_definition, "Name")?;
            let refers_to = get_string_property(&range_definition, "RefersTo")?;
            let (scope, name) = parse_named_range_scope(&qualified_name)?;
            named_ranges.push(ExcelNamedRangeInfo::new(name, scope, refers_to)?);
        }
        let calculation = ExcelCalculationInfo::new(
            calculation_mode(get_i4_property(application, "Calculation")?),
            calculation_state(get_i4_property(application, "CalculationState")?),
        );
        ExcelWorkbookSnapshot::new(
            path.to_string_lossy().into_owned(),
            true,
            worksheets,
            named_ranges,
            calculation,
        )
    }

    fn read_range(
        &mut self,
        selector: &ExcelRangeSelector,
        mode: ExcelReadMode,
    ) -> PlatformResult<ExcelRangeSnapshot> {
        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        let workbook = self
            .workbook
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no open workbook"))?;
        let range = match selector {
            ExcelRangeSelector::NamedRange(name) => {
                get_dispatch_property_with_string_argument(application, "Range", name)?
            }
            ExcelRangeSelector::A1 { worksheet, .. } => {
                let worksheets = get_dispatch_property(workbook, "Worksheets")?;
                let worksheet = match worksheet {
                    ExcelWorksheetSelector::Name(name) => {
                        get_dispatch_property_with_string_argument(&worksheets, "Item", name)?
                    }
                    ExcelWorksheetSelector::Ordinal(ordinal) => {
                        let ordinal = i32::try_from(*ordinal).map_err(|_| {
                            PlatformError::new(
                                ErrorCode::ToolInvalidArgs,
                                "worksheet ordinal does not fit i32",
                            )
                        })?;
                        get_dispatch_property_with_i4_argument(&worksheets, "Item", ordinal)?
                    }
                };
                let address = super::api::range_address(selector)?;
                get_dispatch_property_with_string_argument(&worksheet, "Range", &address)?
            }
        };
        let address = get_string_property(&range, "Address")?;
        let rows_dispatch = get_dispatch_property(&range, "Rows")?;
        let columns_dispatch = get_dispatch_property(&range, "Columns")?;
        let rows = u32::try_from(get_i4_property(&rows_dispatch, "Count")?)
            .map_err(|_| runner_protocol_error("Excel range row count is invalid"))?;
        let columns = u32::try_from(get_i4_property(&columns_dispatch, "Count")?)
            .map_err(|_| runner_protocol_error("Excel range column count is invalid"))?;
        let cell_count = u64::from(rows)
            .checked_mul(u64::from(columns))
            .ok_or_else(|| runner_protocol_error("Excel range cell count overflowed"))?;
        if rows == 0
            || columns == 0
            || cell_count > u64::try_from(MAX_EXCEL_RANGE_CELLS).unwrap_or(u64::MAX)
        {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("Excel range has {cell_count} cells; maximum is {MAX_EXCEL_RANGE_CELLS}"),
            ));
        }
        let mut cells = Vec::with_capacity(usize::try_from(cell_count).unwrap_or(0));
        for row in 1..=rows {
            for column in 1..=columns {
                let cell = get_dispatch_property_with_two_i4_arguments(
                    &range,
                    "Cells",
                    i32::try_from(row)
                        .map_err(|_| runner_protocol_error("range row does not fit i32"))?,
                    i32::try_from(column)
                        .map_err(|_| runner_protocol_error("range column does not fit i32"))?,
                )?;
                let (value_type, value_text) = read_cell_value(&cell, mode)?;
                cells.push(ExcelRuntimeCell::new(row, column, value_type, value_text)?);
            }
        }
        ExcelRangeSnapshot::new(address, rows, columns, mode, cells)
    }

    fn verify_ownership(&self, identity: &ExcelOwnedProcessIdentity) -> PlatformResult<()> {
        if self.workbook.is_none() {
            return Err(PlatformError::new(
                ErrorCode::TargetNotFound,
                "Excel workbook is no longer open",
            ));
        }
        if !crate::excel_process::process_matches(
            identity.process_id(),
            identity.start_time_100ns(),
            identity.executable_path(),
        )? {
            return Err(PlatformError::new(
                ErrorCode::TargetNotFound,
                "owned Excel process identity changed",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn execute_write_acceptance(
        &mut self,
        request: &super::ExcelWriteAcceptanceRequest,
    ) -> PlatformResult<()> {
        self.execute_write_acceptance_request(request)
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        let workbook = self
            .workbook
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no open workbook"))?;
        invoke_method_with_arguments(workbook, "Close", vec![variant_from_bool(false)])?;
        self.workbook = None;
        self.canonical_path = None;
        Ok(())
    }
}

#[cfg(windows)]
pub(super) fn parse_named_range_scope(
    qualified_name: &str,
) -> PlatformResult<(ExcelNamedRangeScope, String)> {
    if let Some((scope_name, range_name)) = qualified_name.split_once('!') {
        let scope_name = scope_name.trim().trim_matches('\'');
        let range_name = range_name.trim();
        if scope_name.is_empty() || range_name.is_empty() {
            return Err(runner_protocol_error(
                "Excel named range has an invalid worksheet-qualified name",
            ));
        }
        Ok((
            ExcelNamedRangeScope::Worksheet(scope_name.to_owned()),
            range_name.to_owned(),
        ))
    } else if qualified_name.trim().is_empty() {
        Err(runner_protocol_error("Excel named range has an empty name"))
    } else {
        Ok((
            ExcelNamedRangeScope::Workbook,
            qualified_name.trim().to_owned(),
        ))
    }
}

#[cfg(test)]
#[path = "excel_com_runner_windows_write.rs"]
mod write_acceptance;

#[cfg(windows)]
#[path = "excel_com_runner_windows_variants.rs"]
mod variants;

#[cfg(windows)]
fn get_i4_property(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
) -> PlatformResult<i32> {
    use windows::Win32::System::Com::DISPATCH_PROPERTYGET;
    use windows::Win32::System::Variant::{VARIANT, VT_I4, VariantClear};

    let dispatch_id = member_dispatch_id(dispatch, property_name)?;
    let parameters = windows::Win32::System::Com::DISPPARAMS::default();
    let mut result = VARIANT::default();
    // SAFETY: both the dispatch id and parameter block belong to this STA
    // worker; the result points to a live VARIANT cleared below.
    unsafe {
        dispatch.Invoke(
            dispatch_id,
            &windows::core::GUID::zeroed(),
            0,
            DISPATCH_PROPERTYGET,
            &raw const parameters,
            Some(&raw mut result),
            None,
            None,
        )
    }
    .map_err(|error| {
        crate::error::error_from_hresult(
            error.code().0,
            &format!("IDispatch.Invoke({property_name})"),
        )
    })?;

    // SAFETY: Invoke initialized the VARIANT; the union is read only after
    // checking the variant tag, and VariantClear runs exactly once.
    let value = unsafe {
        let inner = &result.Anonymous.Anonymous;
        if inner.vt != VT_I4 {
            let clear = VariantClear(&raw mut result);
            return match clear {
                Ok(()) => Err(runner_protocol_error(format!(
                    "Excel property {property_name} did not return VT_I4"
                ))),
                Err(error) => Err(crate::error::error_from_hresult(
                    error.code().0,
                    "VariantClear",
                )),
            };
        }
        inner.Anonymous.lVal
    };
    // SAFETY: paired with the successful Invoke above; no reference escapes.
    unsafe { VariantClear(&raw mut result) }
        .map_err(|error| crate::error::error_from_hresult(error.code().0, "VariantClear"))?;
    Ok(value)
}

#[cfg(windows)]
type ComVariant = windows::Win32::System::Variant::VARIANT;

#[cfg(windows)]
fn get_dispatch_property(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    let result = invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET,
        Vec::new(),
        true,
    )?;
    convert_dispatch_result(result, property_name)
}

#[cfg(windows)]
fn get_dispatch_property_with_i4_argument(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
    argument: i32,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    get_dispatch_property_with_variant_argument(dispatch, property_name, variant_from_i4(argument))
}

#[cfg(windows)]
fn get_dispatch_property_with_two_i4_arguments(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
    first: i32,
    second: i32,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    // Excel marks `Range.Cells` and collection `Item` inconsistently as
    // property-get or method. Both flags let the server choose the declared
    // member kind without changing argument order.
    let result = invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET
            | windows::Win32::System::Com::DISPATCH_METHOD,
        vec![variant_from_i4(first), variant_from_i4(second)],
        true,
    )?;
    convert_dispatch_result(result, property_name)
}

#[cfg(windows)]
fn get_dispatch_property_with_variant_argument(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
    argument: ComVariant,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    let result = invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET
            | windows::Win32::System::Com::DISPATCH_METHOD,
        vec![argument],
        true,
    )?;
    convert_dispatch_result(result, property_name)
}

#[cfg(windows)]
fn get_dispatch_property_with_string_argument(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
    argument: &str,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    let result = invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET
            | windows::Win32::System::Com::DISPATCH_METHOD,
        vec![variant_from_string(argument)],
        true,
    )?;
    convert_dispatch_result(result, property_name)
}

#[cfg(windows)]
fn get_variant_property(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
) -> PlatformResult<ComVariant> {
    invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET,
        Vec::new(),
        true,
    )?
    .ok_or_else(|| {
        runner_protocol_error(format!("Excel property {property_name} returned no value"))
    })
}

#[cfg(windows)]
fn read_cell_value(
    dispatch: &windows::Win32::System::Com::IDispatch,
    mode: ExcelReadMode,
) -> PlatformResult<(ExcelCellValueType, String)> {
    use windows::Win32::System::Variant::{
        VT_BOOL, VT_BSTR, VT_EMPTY, VT_ERROR, VT_I4, VT_NULL, VT_R8, VariantClear,
    };

    let mut variant = get_variant_property(dispatch, mode.property_name())?;
    // SAFETY: Invoke initialized the VARIANT; fields are read only after checking
    // the tag, and VariantClear runs exactly once below.
    let result = unsafe {
        let inner = &variant.Anonymous.Anonymous;
        match inner.vt {
            VT_EMPTY | VT_NULL => Ok((ExcelCellValueType::Blank, String::new())),
            VT_R8 => {
                let value = inner.Anonymous.dblVal;
                if value.is_finite() {
                    Ok((ExcelCellValueType::Number, value.to_string()))
                } else {
                    Err(runner_protocol_error(
                        "Excel Value2 returned a non-finite number",
                    ))
                }
            }
            VT_I4 => Ok((ExcelCellValueType::Number, inner.Anonymous.lVal.to_string())),
            VT_BOOL => Ok((
                ExcelCellValueType::Boolean,
                if inner.Anonymous.boolVal.0 == 0 {
                    "false".to_owned()
                } else {
                    "true".to_owned()
                },
            )),
            VT_BSTR => {
                let text = inner.Anonymous.bstrVal.to_string();
                if text.is_empty() {
                    Ok((ExcelCellValueType::Blank, text))
                } else {
                    Ok((ExcelCellValueType::Text, text))
                }
            }
            VT_ERROR => Ok((
                ExcelCellValueType::Error,
                excel_error_text(inner.Anonymous.scode)?,
            )),
            _ => Err(runner_protocol_error(format!(
                "Excel cell value used unsupported VARIANT type {}",
                inner.vt.0
            ))),
        }
    };
    // SAFETY: paired with the successful Invoke above.
    unsafe { VariantClear(&raw mut variant) }
        .map_err(|error| crate::error::error_from_hresult(error.code().0, "VariantClear(cell)"))?;
    result
}

#[cfg(windows)]
fn excel_error_text(scode: i32) -> PlatformResult<String> {
    let code = u32::from_ne_bytes(scode.to_ne_bytes()) & 0xFFFF;
    let text = match code {
        2000 => "#NULL!",
        2007 => "#DIV/0!",
        2015 => "#VALUE!",
        2023 => "#REF!",
        2029 => "#NAME?",
        2036 => "#NUM!",
        2042 => "#N/A",
        _ => {
            return Err(runner_protocol_error(format!(
                "Excel cell returned unknown error code {scode:#010x}"
            )));
        }
    };
    Ok(text.to_owned())
}

#[cfg(windows)]
fn get_string_property(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
) -> PlatformResult<String> {
    let result = invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET,
        Vec::new(),
        true,
    )?;
    convert_string_result(result, property_name)
}

#[cfg(windows)]
fn invoke_method_returning_dispatch(
    dispatch: &windows::Win32::System::Com::IDispatch,
    method_name: &str,
    arguments: Vec<ComVariant>,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    let result = invoke_dispatch(
        dispatch,
        method_name,
        windows::Win32::System::Com::DISPATCH_METHOD,
        arguments,
        true,
    )?;
    convert_dispatch_result(result, method_name)
}

#[cfg(windows)]
fn invoke_method_with_arguments(
    dispatch: &windows::Win32::System::Com::IDispatch,
    method_name: &str,
    arguments: Vec<ComVariant>,
) -> PlatformResult<()> {
    invoke_dispatch(
        dispatch,
        method_name,
        windows::Win32::System::Com::DISPATCH_METHOD,
        arguments,
        false,
    )
    .map(|_result| ())
}

#[cfg(windows)]
fn invoke_dispatch(
    dispatch: &windows::Win32::System::Com::IDispatch,
    member_name: &str,
    flags: windows::Win32::System::Com::DISPATCH_FLAGS,
    arguments: Vec<ComVariant>,
    expects_result: bool,
) -> PlatformResult<Option<ComVariant>> {
    use windows::Win32::System::Com::DISPPARAMS;
    use windows::Win32::System::Variant::{VARIANT, VariantClear};

    let argument_count = u32::try_from(arguments.len()).map_err(|_| {
        runner_protocol_error("Excel COM argument count does not fit the IDispatch ABI")
    })?;
    let mut dispatch_arguments: Vec<ComVariant> = arguments.into_iter().rev().collect();
    let parameter_pointer = if dispatch_arguments.is_empty() {
        std::ptr::null_mut()
    } else {
        dispatch_arguments.as_mut_ptr()
    };
    let parameters = DISPPARAMS {
        rgvarg: parameter_pointer,
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: argument_count,
        cNamedArgs: 0,
    };
    let dispatch_id = member_dispatch_id(dispatch, member_name)?;
    let mut result = VARIANT::default();
    // SAFETY: all VARIANT arguments and the optional result point to live
    // storage on this worker thread for the synchronous Invoke call.
    let invoke_result = unsafe {
        dispatch.Invoke(
            dispatch_id,
            &windows::core::GUID::zeroed(),
            0,
            flags,
            &raw const parameters,
            if expects_result {
                Some(&raw mut result)
            } else {
                None
            },
            None,
            None,
        )
    };
    let mut clear_error = None;
    for argument in &mut dispatch_arguments {
        // SAFETY: each VARIANT was initialized above and is cleared exactly once.
        if let Err(error) = unsafe { VariantClear(argument) } {
            clear_error.get_or_insert_with(|| {
                crate::error::error_from_hresult(error.code().0, "VariantClear(arguments)")
            });
        }
    }
    if let Some(error) = clear_error {
        if expects_result {
            // SAFETY: Invoke initialized this VARIANT; the return path must not
            // leak it even when an argument cleanup also failed.
            if let Err(result_error) = unsafe { VariantClear(&raw mut result) } {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!(
                        "{error}; result VariantClear also failed: {}",
                        result_error.message()
                    ),
                ));
            }
        }
        return Err(error);
    }
    invoke_result.map_err(|error| {
        crate::error::error_from_hresult(
            error.code().0,
            &format!("IDispatch.Invoke({member_name})"),
        )
    })?;
    Ok(expects_result.then_some(result))
}

#[cfg(windows)]
fn convert_dispatch_result(
    result: Option<ComVariant>,
    member_name: &str,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    use windows::Win32::System::Com::IDispatch;
    use windows::Win32::System::Variant::VariantClear;

    let mut result = result.ok_or_else(|| {
        runner_protocol_error(format!(
            "Excel property or method {member_name} returned no IDispatch value"
        ))
    })?;
    let converted = IDispatch::try_from(&result);
    // SAFETY: Invoke initialized the VARIANT and this is its single clear.
    let cleared = unsafe { VariantClear(&raw mut result) };
    match (converted, cleared) {
        (Ok(dispatch), Ok(())) => Ok(dispatch),
        (Err(error), _) => Err(PlatformError::new(
            ErrorCode::Fatal,
            format!("Excel member {member_name} did not return VT_DISPATCH: {error}"),
        )),
        (Ok(_), Err(error)) => Err(crate::error::error_from_hresult(
            error.code().0,
            "VariantClear(IDispatch result)",
        )),
    }
}

#[cfg(windows)]
fn convert_string_result(result: Option<ComVariant>, member_name: &str) -> PlatformResult<String> {
    use windows::Win32::System::Variant::{VT_BSTR, VariantClear};

    let mut result = result.ok_or_else(|| {
        runner_protocol_error(format!("Excel property {member_name} returned no value"))
    })?;
    // SAFETY: Invoke initialized the VARIANT; the union is read only after the
    // tag check, and VariantClear runs exactly once below.
    let converted = unsafe {
        let inner = &result.Anonymous.Anonymous;
        if inner.vt == VT_BSTR {
            Ok(inner.Anonymous.bstrVal.to_string())
        } else {
            Err(runner_protocol_error(format!(
                "Excel property {member_name} did not return VT_BSTR"
            )))
        }
    };
    // SAFETY: Invoke initialized the VARIANT and this is its single clear.
    let cleared = unsafe { VariantClear(&raw mut result) };
    match (converted, cleared) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(crate::error::error_from_hresult(
            error.code().0,
            "VariantClear(BSTR result)",
        )),
    }
}

#[cfg(windows)]
fn invoke_method(
    dispatch: &windows::Win32::System::Com::IDispatch,
    method_name: &str,
) -> PlatformResult<()> {
    invoke_method_with_arguments(dispatch, method_name, Vec::new())
}

#[cfg(windows)]
fn member_dispatch_id(
    dispatch: &windows::Win32::System::Com::IDispatch,
    member_name: &str,
) -> PlatformResult<i32> {
    use windows::core::PCWSTR;

    let mut wide: Vec<u16> = member_name.encode_utf16().collect();
    wide.push(0);
    let name = PCWSTR(wide.as_ptr());
    let mut dispatch_id = 0_i32;
    // SAFETY: `name` points to NUL-terminated UTF-16 storage alive for the
    // synchronous call, and `dispatch_id` is a live out-parameter.
    unsafe {
        dispatch.GetIDsOfNames(
            &windows::core::GUID::zeroed(),
            &raw const name,
            1,
            0,
            &raw mut dispatch_id,
        )
    }
    .map_err(|error| {
        crate::error::error_from_hresult(error.code().0, &format!("GetIDsOfNames({member_name})"))
    })?;
    Ok(dispatch_id)
}
