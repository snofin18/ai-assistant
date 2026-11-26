//! Windows-only implementation of the test-only Excel write calibration.
//!
//! Every method here executes on the dedicated Excel STA worker. The only
//! property put is the closed scalar assignment to `Range.Value2`; arbitrary
//! member names, formulas, and COM values are not accepted.

use super::super::{ExcelScalarValue, ExcelWriteAcceptanceRequest};
use super::{
    ComVariant, ErrorCode, ExcelRangeSelector, ExcelWorksheetSelector, PlatformError,
    PlatformResult, WindowsExcelComBackend, get_dispatch_property,
    get_dispatch_property_with_i4_argument, get_dispatch_property_with_string_argument,
    invoke_method_with_arguments, member_dispatch_id, runner_protocol_error, variant_from_bool,
    variant_from_string,
};
use std::mem::ManuallyDrop;

impl WindowsExcelComBackend {
    pub(super) fn execute_write_acceptance_request(
        &mut self,
        request: &ExcelWriteAcceptanceRequest,
    ) -> PlatformResult<()> {
        match request {
            ExcelWriteAcceptanceRequest::OpenWritable { path } => self.open_workbook(path, false),
            ExcelWriteAcceptanceRequest::WriteScalar {
                selector, scalar, ..
            } => {
                scalar.validate()?;
                let range = self.resolve_range(selector)?;
                put_variant_property(&range, "Value2", variant_from_scalar(scalar))
            }
            ExcelWriteAcceptanceRequest::Recalculate { .. } => {
                let application = self.application.as_ref().ok_or_else(|| {
                    runner_protocol_error("Windows Excel backend has no application")
                })?;
                invoke_method_with_arguments(application, "Calculate", Vec::new())
            }
            ExcelWriteAcceptanceRequest::Save { .. } => {
                let workbook = self.workbook.as_ref().ok_or_else(|| {
                    runner_protocol_error("Windows Excel backend has no open workbook")
                })?;
                invoke_method_with_arguments(workbook, "Save", Vec::new())
            }
        }
    }

    fn resolve_range(
        &self,
        selector: &ExcelRangeSelector,
    ) -> PlatformResult<windows::Win32::System::Com::IDispatch> {
        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        let workbook = self
            .workbook
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no open workbook"))?;
        match selector {
            ExcelRangeSelector::NamedRange(name) => {
                get_dispatch_property_with_string_argument(application, "Range", name)
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
                let address = super::super::api::range_address(selector)?;
                get_dispatch_property_with_string_argument(&worksheet, "Range", &address)
            }
        }
    }
}

fn variant_from_scalar(scalar: &ExcelScalarValue) -> ComVariant {
    match scalar {
        ExcelScalarValue::Blank => ComVariant::default(),
        ExcelScalarValue::Boolean(value) => variant_from_bool(*value),
        ExcelScalarValue::Number(value) => ComVariant::from(*value),
        ExcelScalarValue::Text(value) => variant_from_string(value),
    }
}

fn put_variant_property(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
    value: ComVariant,
) -> PlatformResult<()> {
    use windows::Win32::System::Com::{DISPATCH_PROPERTYPUT, DISPPARAMS};
    use windows::Win32::System::Ole::DISPID_PROPERTYPUT;
    use windows::Win32::System::Variant::VariantClear;

    let dispatch_id = member_dispatch_id(dispatch, property_name)?;
    // VARIANT owns a Drop implementation. Suppress it while the explicit
    // VariantClear below enforces exactly one cleanup on success and failure.
    let mut value = ManuallyDrop::new(value);
    let mut named_argument = DISPID_PROPERTYPUT;
    let parameters = DISPPARAMS {
        rgvarg: &raw mut *value,
        rgdispidNamedArgs: &raw mut named_argument,
        cArgs: 1,
        cNamedArgs: 1,
    };
    // SAFETY: the dispatch id, value, and named DISPID live for the synchronous
    // Invoke call. No result VARIANT is requested.
    let invoke_result = unsafe {
        dispatch.Invoke(
            dispatch_id,
            &windows::core::GUID::zeroed(),
            0,
            DISPATCH_PROPERTYPUT,
            &raw const parameters,
            None,
            None,
            None,
        )
    };
    // SAFETY: the VARIANT was initialized above and is cleared exactly once.
    let clear_result = unsafe { VariantClear(&raw mut *value) };
    match (invoke_result, clear_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(crate::error::error_from_hresult(
            error.code().0,
            &format!("IDispatch.Invoke({property_name}=scalar)"),
        )),
        (Ok(()), Err(error)) => Err(crate::error::error_from_hresult(
            error.code().0,
            "VariantClear(scalar property put)",
        )),
        (Err(invoke_error), Err(clear_error)) => Err(PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "IDispatch.Invoke({property_name}=scalar) failed: {invoke_error}; VariantClear also failed: {clear_error}"
            ),
        )),
    }
}
