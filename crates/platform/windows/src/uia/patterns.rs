//! 读 / 写文本（`read_text` / `set_value` / `edit_text`）与它们的**回读后置条件**（铁律 4）。
//!
//! 职责：用 UIA pattern 做原子取值与写入，并在写入后**回读**确认生效。
//! 边界：**不做**键盘模拟（TASK-018）、**不做**撤销（TASK-024）、**不做**后置断言引擎（TASK-023）。
//!
//! ## 为什么不返回写入内容的明文
//! `VerifyFailed` 的 message 只报**长度**与差异位置，不回显内容 —— 目标里可能是密码、
//! 令牌或用户隐私，错误信息会进审计与模型上下文（AGENTS.md §7 的密钥红线同一条精神）。
//!
//! ## 不变量
//! 1. **写后必读**：`set_value` 回读并**逐字符相等**才返回 `Ok`；不一致 → `VerifyFailed`。
//! 2. `edit_text` 的偏移量是**字符**偏移（不是字节），越界 → `ToolInvalidArgs`（不是静默截断）。
//! 3. `edit_text` 的落地形态是「读全量 → 计算新全量 → `SetValue` 原子替换」；
//!    它**不**保留光标 / 选区（UIA `ValuePattern` 没有插入点概念），这一点写进 README「已知限制」。
//!
//! 相关：架构 v2 §13.1.1、ADR-0022 D5、`docs/memory/apps/notepad.md` §3（`RichEditD2DPT` 支持 `ValuePattern`）。

use assistant_platform_api::{PlatformResult, ResolvedElement, TextEditOp};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationValuePattern, UIA_TextPatternId,
    UIA_ValuePatternId,
};
use windows::core::BSTR;

use crate::error;
use crate::handles;

use super::bstr_to_string;

/// 读文本：优先 `ValuePattern`，退回 `TextPattern`（架构 v2 §13.1.1 的 `read_text`）。
///
/// # Errors
/// - 两种 pattern 都没有 → `CapabilityMissing`
/// - 句柄不在本线程的元素表里 / 元素已消失 → `TargetNotFound`
pub fn read_text(element: &ResolvedElement) -> PlatformResult<String> {
    handles::with_element(element.id(), read_text_of)
}

/// 在一个已解析的 UIA 元素上读文本。
fn read_text_of(element: &IUIAutomationElement) -> PlatformResult<String> {
    // SAFETY: 只读地取 pattern；`GetCurrentPatternAs` 用返回类型决定请求的 IID，两者不可能不一致。
    if let Ok(value_pattern) =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
    {
        // SAFETY: 只读属性；返回的 BSTR 由本进程持有。
        return unsafe { value_pattern.CurrentValue() }
            .map(|value| bstr_to_string(&value))
            .map_err(|failure| {
                error::error_from_hresult(failure.code().0, "ValuePattern::CurrentValue")
            });
    }
    // SAFETY: 同上（`TextPattern` 是富文本 / 只读文档的兜底）。
    if let Ok(text_pattern) =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
    {
        // SAFETY: `DocumentRange` 返回文档全文范围，调用方拥有它。
        let range = unsafe { text_pattern.DocumentRange() }.map_err(|failure| {
            error::error_from_hresult(failure.code().0, "TextPattern::DocumentRange")
        })?;
        // `-1` = 不限制长度（UIA 约定）。
        // SAFETY: 只读；返回的 BSTR 由本进程持有。
        let text = unsafe { range.GetText(-1) }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "TextRange::GetText"))?;
        return Ok(bstr_to_string(&text));
    }
    Err(error::capability_missing(
        "read_text: element exposes neither ValuePattern nor TextPattern",
    ))
}

/// 设置取值（**优先于键盘模拟**，架构 v2 §13.1.1），并回读验证（铁律 4）。
///
/// # Errors
/// - 不支持 `ValuePattern` → `CapabilityMissing`
/// - 只读元素 → `CapabilityMissing`
/// - 平台拒绝写入 → `PlatformPermission`
/// - **回读与写入不一致 → `VerifyFailed`**（绝不返回 `Ok`）
pub fn set_value(element: &ResolvedElement, value: &str) -> PlatformResult<()> {
    handles::with_element(element.id(), |uia| set_value_of(uia, value))
}

/// 在一个已解析的 UIA 元素上写取值并回读。
fn set_value_of(element: &IUIAutomationElement, value: &str) -> PlatformResult<()> {
    let pattern = value_pattern(element, "set_value")?;
    // SAFETY: 只读属性。
    let is_read_only = unsafe { pattern.CurrentIsReadOnly() }
        .map_err(|failure| {
            error::error_from_hresult(failure.code().0, "ValuePattern::CurrentIsReadOnly")
        })?
        .as_bool();
    if is_read_only {
        return Err(error::capability_missing(
            "set_value: element is read-only (ValuePattern.CurrentIsReadOnly = true)",
        ));
    }
    // SAFETY: `BSTR` 借用只在本次调用期间有效，UIA 会复制内容。
    unsafe { pattern.SetValue(&BSTR::from(value)) }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "ValuePattern::SetValue"))?;
    // 后置条件：回读必须与写入**逐字符相等**。
    // SAFETY: 只读属性；返回的 BSTR 由本进程持有。
    let read_back = unsafe { pattern.CurrentValue() }
        .map(|value| bstr_to_string(&value))
        .map_err(|failure| {
            error::error_from_hresult(failure.code().0, "ValuePattern::CurrentValue")
        })?;
    if read_back == value {
        return Ok(());
    }
    Err(error::verify_failed(format!(
        "set_value postcondition failed: wrote {} chars, read back {} chars (content differs; \
         values are intentionally not echoed to avoid leaking target data into logs)",
        value.chars().count(),
        read_back.chars().count()
    )))
}

/// 编辑文本（插入 / 删除 / 替换）：读全量 → 计算新全量 → `SetValue` 原子替换 → 回读。
///
/// # Errors
/// - 偏移越界 / 区间反向 → `ToolInvalidArgs`
/// - 元素不支持 `ValuePattern` → `CapabilityMissing`
/// - 回读不一致 → `VerifyFailed`
pub fn edit_text(element: &ResolvedElement, operation: &TextEditOp) -> PlatformResult<()> {
    let current = read_text(element)?;
    let updated = apply_text_edit(&current, operation)?;
    set_value(element, &updated)
}

/// **纯函数**：把一次文本编辑应用到 `current` 上（可单测、不碰 COM）。
///
/// 偏移量按**字符**计（与 `TextEditOp` 的文档一致）；越界 / 反向区间一律 `ToolInvalidArgs`。
pub fn apply_text_edit(current: &str, operation: &TextEditOp) -> PlatformResult<String> {
    match operation {
        TextEditOp::Insert { at, text } => {
            let index = byte_offset_for_chars(current, *at)?;
            Ok(format!(
                "{}{}{}",
                current.get(..index).unwrap_or_default(),
                text,
                current.get(index..).unwrap_or_default()
            ))
        }
        TextEditOp::Delete { start, end } => {
            let (start_index, end_index) = byte_range_for_chars(current, *start, *end)?;
            Ok(format!(
                "{}{}",
                current.get(..start_index).unwrap_or_default(),
                current.get(end_index..).unwrap_or_default()
            ))
        }
        TextEditOp::Replace { start, end, text } => {
            let (start_index, end_index) = byte_range_for_chars(current, *start, *end)?;
            Ok(format!(
                "{}{}{}",
                current.get(..start_index).unwrap_or_default(),
                text,
                current.get(end_index..).unwrap_or_default()
            ))
        }
        _ => Err(error::invalid_args(
            "unknown text edit operation: refusing to guess (fail closed)",
        )),
    }
}

/// 字符偏移 → 字节偏移（`offset == 字符数` 时返回 `len()`，即"末尾"）。
fn byte_offset_for_chars(text: &str, offset: usize) -> PlatformResult<usize> {
    if offset == text.chars().count() {
        return Ok(text.len());
    }
    text.char_indices()
        .nth(offset)
        .map(|(byte, _)| byte)
        .ok_or_else(|| {
            error::invalid_args(format!(
                "character offset {offset} is out of range (text has {} chars)",
                text.chars().count()
            ))
        })
}

/// 字符区间 → 字节区间（要求 `start <= end`，两者都必须在 `[0, 字符数]` 内）。
fn byte_range_for_chars(text: &str, start: usize, end: usize) -> PlatformResult<(usize, usize)> {
    if start > end {
        return Err(error::invalid_args(format!(
            "text edit range is inverted: start {start} > end {end}"
        )));
    }
    Ok((
        byte_offset_for_chars(text, start)?,
        byte_offset_for_chars(text, end)?,
    ))
}

/// 取 `ValuePattern`，不支持时给出**语义明确**的 `CapabilityMissing`（不是裸 HRESULT）。
fn value_pattern(
    element: &IUIAutomationElement,
    context: &str,
) -> PlatformResult<IUIAutomationValuePattern> {
    // SAFETY: 只读地取 pattern；IID 由返回类型决定。
    unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
        .map_err(|failure| pattern_missing(&failure, context, "ValuePattern"))
}

/// 把「控件不支持该 pattern」翻译成 `CapabilityMissing`（而不是裸 HRESULT 的 `Fatal`）。
///
/// 为什么必须单独处理：`GetCurrentPatternAs` 对不支持的 pattern 返回 `E_NOINTERFACE`，
/// 若走通用映射会变成 `Fatal`（"未识别的 COM 失败"）—— 而这是**完全正常**的能力缺失，
/// 调用方据此换通道（例如改用 `TextPattern`），不是致命错误。
pub(super) fn pattern_missing(
    failure: &windows::core::Error,
    context: &str,
    pattern_name: &str,
) -> assistant_platform_api::PlatformError {
    let code = failure.code().0;
    if code == crate::error::E_NOINTERFACE {
        return error::capability_missing(format!(
            "{context}: element does not support {pattern_name}"
        ));
    }
    error::error_from_hresult(code, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_at_character_offset_handles_multibyte_text() {
        // "中文abc" 有 5 个字符；在第 2 个字符后插入 —— 偏移按**字符**计，不是字节。
        let updated = apply_text_edit(
            "中文abc",
            &TextEditOp::Insert {
                at: 2,
                text: "X".to_string(),
            },
        );
        assert_eq!(updated, Ok("中文Xabc".to_string()));
    }

    #[test]
    fn test_insert_at_end_is_allowed() {
        let updated = apply_text_edit(
            "abc",
            &TextEditOp::Insert {
                at: 3,
                text: "!".to_string(),
            },
        );
        assert_eq!(updated, Ok("abc!".to_string()));
    }

    #[test]
    fn test_insert_beyond_end_is_rejected() {
        // 负向用例（ADR-0019 N1）：越界必须报错，不得静默截断到末尾。
        let updated = apply_text_edit(
            "abc",
            &TextEditOp::Insert {
                at: 4,
                text: "!".to_string(),
            },
        );
        assert_eq!(
            updated.map_err(|error| error.code()),
            Err(assistant_platform_api::ErrorCode::ToolInvalidArgs)
        );
    }

    #[test]
    fn test_delete_range_is_half_open() {
        let updated = apply_text_edit("abcdef", &TextEditOp::Delete { start: 1, end: 4 });
        assert_eq!(updated, Ok("aef".to_string()));
    }

    #[test]
    fn test_replace_range_keeps_surrounding_text() {
        let updated = apply_text_edit(
            "hello world",
            &TextEditOp::Replace {
                start: 6,
                end: 11,
                text: "Rust".to_string(),
            },
        );
        assert_eq!(updated, Ok("hello Rust".to_string()));
    }

    #[test]
    fn test_inverted_range_is_rejected() {
        // 负向用例：start > end 不得被"自动纠正"（那会静默改语义）。
        let updated = apply_text_edit("abc", &TextEditOp::Delete { start: 3, end: 1 });
        assert_eq!(
            updated.map_err(|error| error.code()),
            Err(assistant_platform_api::ErrorCode::ToolInvalidArgs)
        );
    }

    #[test]
    fn test_delete_whole_text_is_allowed() {
        let updated = apply_text_edit("abc", &TextEditOp::Delete { start: 0, end: 3 });
        assert_eq!(updated, Ok(String::new()));
    }
}
