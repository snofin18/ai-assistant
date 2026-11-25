//! Windows-only Excel COM backend and `IDispatch` helpers.
//!
//! The public handle lives in `excel_com_runner`; this module owns every
//! Windows COM object on the dedicated worker thread.

use super::{
    ErrorCode, ExcelComBackend, ExcelComRunner, Path, PlatformError, PlatformResult,
    REQUEST_QUEUE_CAPACITY, REQUEST_TIMEOUT, WORKER_THREAD_NAME, join_error, mpsc, next_runner_id,
    run_worker, runner_protocol_error, thread, thread_spawn_error,
};

#[cfg(windows)]
pub(super) fn spawn_windows_excel_runner() -> PlatformResult<ExcelComRunner> {
    let runner_id = next_runner_id();
    let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let mut ready_sender = Some(ready_sender);
    let worker = thread::Builder::new()
        .name(WORKER_THREAD_NAME.to_owned())
        .spawn(move || {
            let initialization = crate::com::with_com_apartment(|_guard| {
                if let Some(sender) = ready_sender.take() {
                    sender.send(Ok(())).map_err(|_| {
                        runner_protocol_error("Excel COM runner readiness receiver disappeared")
                    })?;
                }
                run_worker(WindowsExcelComBackend::new(), &receiver, runner_id)
            });
            match initialization {
                Ok(_apartment) => Ok(()),
                Err(error) => {
                    if let Some(sender) = ready_sender.take() {
                        // The receiver disappearing means startup already returned;
                        // the worker is exiting and no further notification is needed.
                        sender.send(Err(error.clone())).unwrap_or(());
                    }
                    Err(error)
                }
            }
        })
        .map_err(|error| thread_spawn_error("Excel COM runner", &error))?;

    let initialization = ready_receiver
        .recv_timeout(REQUEST_TIMEOUT)
        .map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!(
                    "Excel COM runner initialization failed before readiness: {error}; worker was detached and requires process-level cleanup"
                ),
            )
        })?;
    if let Err(error) = initialization {
        let joined = worker.join().map_err(join_error);
        joined??;
        return Err(error);
    }
    Ok(ExcelComRunner {
        sender,
        worker: Some(worker),
        shutdown_started: false,
    })
}

#[cfg(windows)]
struct WindowsExcelComBackend {
    workbook: Option<windows::Win32::System::Com::IDispatch>,
    application: Option<windows::Win32::System::Com::IDispatch>,
}

#[cfg(windows)]
impl WindowsExcelComBackend {
    const fn new() -> Self {
        Self {
            workbook: None,
            application: None,
        }
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

    fn application_process_id(&mut self) -> PlatformResult<u32> {
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
        Ok(process_id)
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
                variant_from_bool(true),
            ],
        )?;
        self.workbook = Some(workbook);
        Ok(())
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

    fn close_workbook(&mut self) -> PlatformResult<()> {
        let workbook = self
            .workbook
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no open workbook"))?;
        invoke_method_with_arguments(workbook, "Close", vec![variant_from_bool(false)])?;
        self.workbook = None;
        Ok(())
    }
}

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
fn variant_from_string(value: &str) -> ComVariant {
    use windows::Win32::System::Variant::VARIANT;
    use windows::core::BSTR;

    VARIANT::from(BSTR::from(value))
}

#[cfg(windows)]
fn variant_from_i4(value: i32) -> ComVariant {
    windows::Win32::System::Variant::VARIANT::from(value)
}

#[cfg(windows)]
fn variant_from_bool(value: bool) -> ComVariant {
    windows::Win32::System::Variant::VARIANT::from(value)
}

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
fn get_dispatch_property_with_string_argument(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
    argument: &str,
) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
    let result = invoke_dispatch(
        dispatch,
        property_name,
        windows::Win32::System::Com::DISPATCH_PROPERTYGET,
        vec![variant_from_string(argument)],
        true,
    )?;
    convert_dispatch_result(result, property_name)
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
