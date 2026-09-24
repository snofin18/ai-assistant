//! Win32 / COM 失败 → `PlatformError` 的**纯函数**映射（铁律 1）。
//!
//! 职责：把 HRESULT / Win32 错误码翻译成 `assistant_protocol::ErrorCode`，使上层能按**类别**
//! 决定重试 / 升级 / 报错（架构 v2 §8.7）。
//! 边界：**不做重试**（重试策略由调用方或 `ResolutionPolicy` 决定）；**不新增错误码**。
//!
//! ## 不变量
//! 1. 映射是**纯函数**（同样的输入必得同样的输出）→ 单测不需要真机
//! 2. 每条 `message` 都带**原始码**，使失败可从时间线定位（阶段 1 `DoD`）
//! 3. 三类「定位失败」**必须可区分**：`TargetNotFound` / `TargetAmbiguous` / `TargetUnresponsive`
//! 4. 未识别的码一律 `Fatal` + 原始码 —— **不得**降级成 `Transient`（那会诱导无意义重试）
//!
//! 相关：架构 v2 §8.7、`docs/spec/error-codes.md`、ADR-0019 N1。

use assistant_platform_api::{ErrorCode, PlatformError};

/// `E_ACCESSDENIED`（Win32 `ERROR_ACCESS_DENIED` 的 HRESULT 形态）。
pub const E_ACCESSDENIED: i32 = -2_147_024_891;
/// `E_INVALIDARG`。
pub const E_INVALIDARG: i32 = -2_147_024_715;
/// `E_NOINTERFACE`（该控件不支持被请求的 pattern）。
pub const E_NOINTERFACE: i32 = -2_147_467_262;
/// `E_OUTOFMEMORY`。
pub const E_OUTOFMEMORY: i32 = -2_147_024_688;
/// `RPC_E_DISCONNECTED`（跨进程 COM 服务已断开）。
pub const RPC_E_DISCONNECTED: i32 = -2_147_410_176;
/// `RPC_E_CHANGED_MODE`（线程已按**另一个** COM 单元模型初始化过）。
pub const RPC_E_CHANGED_MODE: i32 = -2_147_417_850;
/// `UIA_E_ELEMENTNOTAVAILABLE`（元素已消失）。
pub const UIA_E_ELEMENTNOTAVAILABLE: i32 = -2_147_380_735;
/// `UIA_E_ELEMENTNOTENABLED`。
pub const UIA_E_ELEMENTNOTENABLED: i32 = -2_147_380_736;
/// `UIA_E_NOTSUPPORTED`（该控件不支持请求的 pattern）。
pub const UIA_E_NOTSUPPORTED: i32 = -2_147_380_732;
/// `UIA_E_NOCLICKABLEPOINT`。
pub const UIA_E_NOCLICKABLEPOINT: i32 = -2_147_380_730;

/// HRESULT → `PlatformError`（**纯函数**）。
///
/// `context` 是人类可读的操作名（如 `"FindAll(automation_id=TextEditor)"`），会出现在 message 里 ——
/// 没有它，时间线上只会剩一个十六进制码，无法定位（阶段 1 DoD「所有失败可从时间线定位原因」）。
#[must_use]
pub fn error_from_hresult(code: i32, context: &str) -> PlatformError {
    let (error_code, kind) = match code {
        E_ACCESSDENIED => (
            ErrorCode::PlatformPermission,
            "access denied (UIPI / integrity level)",
        ),
        E_INVALIDARG => (ErrorCode::ToolInvalidArgs, "invalid argument"),
        E_NOINTERFACE => (
            ErrorCode::CapabilityMissing,
            "the control does not support the requested interface / pattern",
        ),
        E_OUTOFMEMORY => (ErrorCode::Fatal, "out of memory"),
        RPC_E_DISCONNECTED => (
            ErrorCode::TargetUnresponsive,
            "COM server disconnected (target process gone or busy)",
        ),
        UIA_E_ELEMENTNOTAVAILABLE | UIA_E_ELEMENTNOTENABLED => {
            (ErrorCode::TargetNotFound, "element is no longer available")
        }
        RPC_E_CHANGED_MODE => (
            ErrorCode::Fatal,
            "thread already initialized with a different COM apartment model (cannot use UIA here)",
        ),
        UIA_E_NOTSUPPORTED => (
            ErrorCode::CapabilityMissing,
            "the control does not support the requested pattern",
        ),
        UIA_E_NOCLICKABLEPOINT => (
            ErrorCode::TargetNotFound,
            "element has no clickable point (offscreen or zero-sized)",
        ),
        _ => (ErrorCode::Fatal, "unrecognized COM failure"),
    };
    PlatformError::new(
        error_code,
        format!("{context}: {kind} (HRESULT 0x{code:08X})"),
    )
}

/// Win32 错误码（`GetLastError` 的值）→ `PlatformError`（**纯函数**）。
#[must_use]
pub fn error_from_win32(code: u32, context: &str) -> PlatformError {
    let (error_code, kind) = match code {
        5 => (ErrorCode::PlatformPermission, "access denied"),
        8 => (ErrorCode::Fatal, "not enough memory"),
        170 => (ErrorCode::TargetUnresponsive, "resource is busy"),
        1400 => (ErrorCode::TargetNotFound, "invalid window handle"),
        1460 => (ErrorCode::Transient, "operation timed out"),
        _ => (ErrorCode::Fatal, "unrecognized Win32 failure"),
    };
    PlatformError::new(
        error_code,
        format!("{context}: {kind} (Win32 error {code})"),
    )
}

/// Win32 API 失败（`windows` crate 的 `Error`）→ `PlatformError`（**纯函数**）。
///
/// `windows` crate 把 Win32 API 的 `GetLastError` 值包成 `HRESULT_FROM_WIN32(code)`（`0x8007_xxxx`）。
/// 若一律走 `error_from_hresult`，像 1400（`ERROR_INVALID_WINDOW_HANDLE`）这种码会落进"未识别"
/// 分支被误判成 `Fatal` —— 而它其实是 `TargetNotFound`。因此**只在本 crate 确认是 Win32 API**
/// 的调用点用这个入口（COM / UIA 调用点仍走 `error_from_hresult`，那里 `E_INVALIDARG` 等
/// 是 COM 语义，不能被当成 Win32 码解释）。
#[must_use]
pub fn error_from_win32_failure(failure: &windows::core::Error, context: &str) -> PlatformError {
    error_from_win32(win32_code_from_hresult(failure.code().0), context)
}

/// `HRESULT_FROM_WIN32(x)`（`x` 为正）的逆：`0x8007_0000 | (x & 0xFFFF)` → `x`。
///
/// 用 `from_ne_bytes` 做**位重解释**而不是 `as` 数值转换：`clippy::cast_sign_loss` 在本 workspace
/// 是 warn（`-D warnings` 下即错误）。
const fn win32_code_from_hresult(code: i32) -> u32 {
    u32::from_ne_bytes(code.to_ne_bytes()) & 0x0000_FFFF
}

/// 「本平台 / 本通道不提供该能力」的标准错误。
#[must_use]
pub fn capability_missing(context: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::CapabilityMissing, context)
}

/// 「还没实现，且已排了卡」的标准错误（源码里**占位实现标记**的配套运行时形态；
/// 标记格式见 `docs/spec/naming.md` §8）。
#[must_use]
pub fn stub_not_implemented(card: &str, context: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::CapabilityMissing,
        format!("{context} is not implemented yet (scheduled as {card})"),
    )
}

/// 输入不合法（如未知 action 名）。
#[must_use]
pub fn invalid_args(context: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::ToolInvalidArgs, context)
}

/// 「目标不存在 / 已失效」的标准错误（`TargetNotFound`）。
#[must_use]
pub fn target_not_found(context: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::TargetNotFound, context)
}

/// 「目标无法安全择一」的标准错误（`TargetAmbiguous`，铁律 1：不猜）。
#[must_use]
pub fn target_ambiguous(context: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::TargetAmbiguous, context)
}

/// 「目标不响应 / 已断开」的标准错误（`TargetUnresponsive`）。
#[must_use]
pub fn target_unresponsive(context: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::TargetUnresponsive, context)
}

/// 「后置条件回读失败」的标准错误（`VerifyFailed`，铁律 4）。
///
/// 写操作**只有**回读验证通过才返回 `Ok`；回读不一致 = 静默失败的前兆，必须当场报错。
#[must_use]
pub fn verify_failed(context: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::VerifyFailed, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_denied_maps_to_platform_permission() {
        let error = error_from_hresult(E_ACCESSDENIED, "ElementFromHandle");
        assert_eq!(error.code(), ErrorCode::PlatformPermission);
        assert!(error.message().contains("ElementFromHandle"));
        assert!(error.message().contains("80070005"));
    }

    #[test]
    fn test_changed_apartment_model_maps_to_fatal() {
        // 负向用例（ADR-0019 N1）：单元模型冲突**不得**被降级成可重试的 `Transient`。
        let error = error_from_hresult(RPC_E_CHANGED_MODE, "CoInitializeEx");
        assert_eq!(error.code(), ErrorCode::Fatal);
        assert!(error.message().contains("apartment"));
    }

    #[test]
    fn test_unknown_hresult_outside_known_table_is_fatal() {
        let error = error_from_hresult(-2_000_000_000, "mystery");
        assert_eq!(error.code(), ErrorCode::Fatal);
    }

    #[test]
    fn test_verify_failed_helper_carries_code_and_message() {
        let error = verify_failed("set_value postcondition: expected `abc`, read back `abd`");
        assert_eq!(error.code(), ErrorCode::VerifyFailed);
        assert!(error.message().contains("postcondition"));
    }

    #[test]
    fn test_element_not_available_maps_to_target_not_found() {
        let error = error_from_hresult(UIA_E_ELEMENTNOTAVAILABLE, "read_text");
        assert_eq!(error.code(), ErrorCode::TargetNotFound);
    }

    #[test]
    fn test_not_supported_maps_to_capability_missing() {
        let error = error_from_hresult(UIA_E_NOTSUPPORTED, "set_value");
        assert_eq!(error.code(), ErrorCode::CapabilityMissing);
    }

    #[test]
    fn test_disconnected_com_maps_to_target_unresponsive() {
        let error = error_from_hresult(RPC_E_DISCONNECTED, "FindAll");
        assert_eq!(error.code(), ErrorCode::TargetUnresponsive);
    }

    #[test]
    fn test_unknown_hresult_is_fatal_not_transient() {
        // 负向用例（ADR-0019 N1）：未识别的码**不得**被当成可重试的 Transient。
        // message 里的十六进制按 i32 的**补码位模式**打印：`-1_073_741_824` = `0xC000_0000`，
        // 所以断言 "C0000000"（"40000000" 是 +1_073_741_824 的位模式，见 DRIFT-017-6）。
        let error = error_from_hresult(-1_073_741_824, "mystery");
        assert_eq!(error.code(), ErrorCode::Fatal);
        assert!(error.message().contains("C0000000"));
    }

    #[test]
    fn test_win32_api_failure_restores_the_raw_win32_code() {
        // 回归（ADR-0019 N1）：Win32 API 的失败必须按**原始 Win32 码**分类。
        // 1400（`ERROR_INVALID_WINDOW_HANDLE`）走 `error_from_hresult` 会落进"未识别"分支变 `Fatal`。
        let hresult = windows::core::HRESULT::from_win32(1400);
        let failure = windows::core::Error::from_hresult(hresult);
        let error = error_from_win32_failure(&failure, "GetWindowRect");
        assert_eq!(error.code(), ErrorCode::TargetNotFound);
        assert!(error.message().contains("1400"), "message 必须带原始码");
        assert_eq!(win32_code_from_hresult(hresult.0), 1400);
    }

    #[test]
    fn test_com_only_hresult_is_not_reinterpreted_as_win32() {
        // 负向用例：`RPC_E_DISCONNECTED`（0x8001_0108）**不是** `HRESULT_FROM_WIN32` 形态。
        // 它的低 16 位是 0x0108=264，若被当成 Win32 码会得到 "unrecognized Win32 failure" ——
        // 所以 COM 路径必须继续用 `error_from_hresult`（本用例把这条边界钉住）。
        let error = error_from_hresult(RPC_E_DISCONNECTED, "FindAll");
        assert_eq!(error.code(), ErrorCode::TargetUnresponsive);
        assert_ne!(win32_code_from_hresult(RPC_E_DISCONNECTED), 0x8001_0108);
    }

    #[test]
    fn test_win32_codes_map_by_kind() {
        assert_eq!(
            error_from_win32(5, "OpenProcess").code(),
            ErrorCode::PlatformPermission
        );
        assert_eq!(
            error_from_win32(1400, "IsIconic").code(),
            ErrorCode::TargetNotFound
        );
        assert_eq!(
            error_from_win32(1460, "WaitForInputIdle").code(),
            ErrorCode::Transient
        );
        assert_eq!(
            error_from_win32(170, "SetForegroundWindow").code(),
            ErrorCode::TargetUnresponsive
        );
        assert_eq!(error_from_win32(9999, "unknown").code(), ErrorCode::Fatal);
    }

    #[test]
    fn test_every_mapped_message_is_non_blank() {
        // 铁律 1：message 不得为空。
        for code in [
            E_ACCESSDENIED,
            E_INVALIDARG,
            E_OUTOFMEMORY,
            RPC_E_DISCONNECTED,
            0,
        ] {
            let error = error_from_hresult(code, "op");
            assert!(!error.message().trim().is_empty());
            assert!(!error.evidence_ref().trim().is_empty());
        }
    }
}
