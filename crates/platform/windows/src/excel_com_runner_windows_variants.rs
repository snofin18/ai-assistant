//! Closed VARIANT constructors and collection helpers for Excel COM.

use super::{ComVariant, PlatformResult, get_i4_property, runner_protocol_error};

pub(super) fn variant_from_string(value: &str) -> ComVariant {
    use windows::Win32::System::Variant::VARIANT;
    use windows::core::BSTR;

    VARIANT::from(BSTR::from(value))
}

pub(super) fn variant_from_i4(value: i32) -> ComVariant {
    windows::Win32::System::Variant::VARIANT::from(value)
}

pub(super) fn variant_from_bool(value: bool) -> ComVariant {
    windows::Win32::System::Variant::VARIANT::from(value)
}

pub(super) fn collection_count(
    dispatch: &windows::Win32::System::Com::IDispatch,
    collection_name: &str,
) -> PlatformResult<usize> {
    let count = get_i4_property(dispatch, "Count")?;
    usize::try_from(count).map_err(|_| {
        runner_protocol_error(format!("Excel {collection_name}.Count did not fit usize"))
    })
}
