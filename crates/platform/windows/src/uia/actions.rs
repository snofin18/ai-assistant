//! 动作类写操作（`invoke_action` / `select` / `scroll`）与它们的**回读后置条件**（铁律 4）。
//!
//! 职责：把动作名映射到 UIA pattern，调用后**回读状态**确认生效。
//! 边界：**不做**指针 / 键盘合成（TASK-018）、**不做**撤销锚点（TASK-024）、
//! **不做**跨步骤断言（TASK-023）。
//!
//! ## 为什么 `invoke` 的后置条件是"弱"的（并且如实说明）
//! `InvokePattern::Invoke()` 的效果是**应用语义**（打开对话框、提交表单、切页…），
//! UIA 侧没有通用的"这次 Invoke 生效了"判据 —— 通用判据属于后置断言引擎（TASK-023）。
//! 本层能验证的是：**调用没有把目标弄没**（元素仍可达）。其余动作
//! （`toggle` / `expand` / `collapse` / `select` / `focus` / `scroll_into_view` / `scroll`）
//! 都有**强**后置条件（状态确实变了）。这条差异写进 README「已知限制」与 §9 审阅者关注点。
//!
//! ## 不变量
//! 1. 未知动作名 → `ToolInvalidArgs` 并列出支持集合（不猜、不静默 no-op）。
//! 2. `scroll` 的百分比必须在 `[0, 1]` 且有限；不可滚动的方向按 UIA 约定传 `-1`（不是 0）。
//! 3. `select(ByStableValue)` 用 **`AutomationId`** 作为"稳定值"（非本地化，ADR-0022 D4）。
//!
//! 相关：架构 v2 §13.1.1、ADR-0022 D4、`docs/memory/apps/notepad.md` §3。

use assistant_platform_api::{PlatformResult, ResolvedElement, ScrollTarget, Selection};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded,
    ExpandCollapseState_PartiallyExpanded, IUIAutomationElement,
    IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern, IUIAutomationScrollItemPattern,
    IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, IUIAutomationTogglePattern,
    TreeScope_Descendants, UIA_AutomationIdPropertyId, UIA_ExpandCollapsePatternId,
    UIA_InvokePatternId, UIA_ScrollItemPatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId,
    UIA_TogglePatternId,
};
use windows::core::HRESULT;

use crate::error;
use crate::handles;

use super::patterns::pattern_missing;
use super::tree::walk_step_for_actions;
use crate::com;

/// 支持的动作名（进 `ToolInvalidArgs` 的 message，使调用方一眼看到可用集合）。
const SUPPORTED_ACTIONS: &str = "invoke, toggle, expand, collapse, select, focus, scroll_into_view";

/// `scroll` 回读的容差（UIA 的百分比是 `f64`，不同提供程序会有亚百分点差异）。
const SCROLL_TOLERANCE_PERCENT: f64 = 1.0;

/// 触发动作（`invoke_action`）。
///
/// # Errors
/// - 未知动作名 → `ToolInvalidArgs`
/// - 元素不支持该动作对应的 pattern → `CapabilityMissing`
/// - 后置条件不成立 → `VerifyFailed`
pub fn invoke_action(element: &ResolvedElement, action: &str) -> PlatformResult<()> {
    let normalized = action.trim().to_ascii_lowercase();
    handles::with_element(element.id(), |uia| match normalized.as_str() {
        "invoke" => invoke(uia),
        "toggle" => toggle(uia),
        "expand" => set_expanded(uia, true),
        "collapse" => set_expanded(uia, false),
        "select" => select_item(uia, "invoke_action(select)"),
        "focus" => focus(uia),
        "scroll_into_view" => scroll_into_view(uia),
        other => Err(error::invalid_args(format!(
            "unknown action `{other}`; supported actions: {SUPPORTED_ACTIONS}"
        ))),
    })
}

/// `InvokePattern::Invoke()` + **弱**后置条件（元素仍可达；理由见模块头）。
fn invoke(element: &IUIAutomationElement) -> PlatformResult<()> {
    let pattern: IUIAutomationInvokePattern = unsafe {
        element.GetCurrentPatternAs(UIA_InvokePatternId)
    }
    .map_err(|failure| pattern_missing(&failure, "invoke_action(invoke)", "InvokePattern"))?;
    // SAFETY: 调用一个标准 UIA pattern 方法；无指针跨界、无缓冲区。
    unsafe { pattern.Invoke() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "InvokePattern::Invoke"))?;
    // 后置条件（弱）：元素必须仍然可达。
    match unsafe { element.CurrentControlType() } {
        Ok(_) => Ok(()),
        Err(failure) => Err(error::verify_failed(format!(
            "invoke_action(invoke) postcondition failed: element is unreachable after Invoke \
             (HRESULT 0x{:08X}); its application-level effect cannot be verified from the UIA \
             layer (assertion engine = TASK-023)",
            failure.code().0
        ))),
    }
}

/// `TogglePattern::Toggle()` + **强**后置条件（`ToggleState` 必须改变）。
fn toggle(element: &IUIAutomationElement) -> PlatformResult<()> {
    let pattern: IUIAutomationTogglePattern = unsafe {
        element.GetCurrentPatternAs(UIA_TogglePatternId)
    }
    .map_err(|failure| pattern_missing(&failure, "invoke_action(toggle)", "TogglePattern"))?;
    // SAFETY: 只读属性。
    let before = unsafe { pattern.CurrentToggleState() }.map_err(|failure| {
        error::error_from_hresult(failure.code().0, "TogglePattern::CurrentToggleState")
    })?;
    // SAFETY: 标准 pattern 调用。
    unsafe { pattern.Toggle() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "TogglePattern::Toggle"))?;
    // SAFETY: 只读属性。
    let after = unsafe { pattern.CurrentToggleState() }.map_err(|failure| {
        error::error_from_hresult(failure.code().0, "TogglePattern::CurrentToggleState")
    })?;
    if before.0 != after.0 {
        return Ok(());
    }
    Err(error::verify_failed(format!(
        "invoke_action(toggle) postcondition failed: ToggleState stayed at {} after Toggle()",
        before.0
    )))
}

/// `ExpandCollapsePattern::Expand()` / `Collapse()` + **强**后置条件（状态确实到位）。
fn set_expanded(element: &IUIAutomationElement, expand: bool) -> PlatformResult<()> {
    let context = if expand {
        "invoke_action(expand)"
    } else {
        "invoke_action(collapse)"
    };
    let pattern: IUIAutomationExpandCollapsePattern =
        unsafe { element.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }
            .map_err(|failure| pattern_missing(&failure, context, "ExpandCollapsePattern"))?;
    // SAFETY: 标准 pattern 调用。
    let outcome = if expand {
        unsafe { pattern.Expand() }
    } else {
        unsafe { pattern.Collapse() }
    };
    outcome.map_err(|failure| error::error_from_hresult(failure.code().0, context))?;
    // SAFETY: 只读属性。
    let state = unsafe { pattern.CurrentExpandCollapseState() }.map_err(|failure| {
        error::error_from_hresult(
            failure.code().0,
            "ExpandCollapsePattern::CurrentExpandCollapseState",
        )
    })?;
    let satisfied = if expand {
        state == ExpandCollapseState_Expanded || state == ExpandCollapseState_PartiallyExpanded
    } else {
        state == ExpandCollapseState_Collapsed
    };
    if satisfied {
        return Ok(());
    }
    Err(error::verify_failed(format!(
        "{context} postcondition failed: ExpandCollapseState is {} after the call",
        state.0
    )))
}

/// `SelectionItemPattern::Select()` + **强**后置条件（`IsSelected` 必须为真）。
fn select_item(element: &IUIAutomationElement, context: &str) -> PlatformResult<()> {
    let pattern: IUIAutomationSelectionItemPattern =
        unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }
            .map_err(|failure| pattern_missing(&failure, context, "SelectionItemPattern"))?;
    // SAFETY: 标准 pattern 调用。
    unsafe { pattern.Select() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, context))?;
    // SAFETY: 只读属性。
    let selected = unsafe { pattern.CurrentIsSelected() }
        .map_err(|failure| {
            error::error_from_hresult(failure.code().0, "SelectionItemPattern::CurrentIsSelected")
        })?
        .as_bool();
    if selected {
        return Ok(());
    }
    Err(error::verify_failed(format!(
        "{context} postcondition failed: SelectionItemPattern::CurrentIsSelected is still false"
    )))
}

/// `SetFocus()` + **强**后置条件（`HasKeyboardFocus` 必须为真）。
fn focus(element: &IUIAutomationElement) -> PlatformResult<()> {
    // SAFETY: 标准 pattern 调用（UIA 的 `SetFocus` 不移动鼠标、不注入输入）。
    unsafe { element.SetFocus() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "Element::SetFocus"))?;
    // SAFETY: 只读属性。
    let focused = unsafe { element.CurrentHasKeyboardFocus() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentHasKeyboardFocus"))?
        .as_bool();
    if focused {
        return Ok(());
    }
    Err(error::verify_failed(
        "invoke_action(focus) postcondition failed: element does not report keyboard focus \
         (the window may not be foreground)",
    ))
}

/// `ScrollItemPattern::ScrollIntoView()` + **强**后置条件（元素不再离屏）。
fn scroll_into_view(element: &IUIAutomationElement) -> PlatformResult<()> {
    let pattern: IUIAutomationScrollItemPattern =
        unsafe { element.GetCurrentPatternAs(UIA_ScrollItemPatternId) }.map_err(|failure| {
            pattern_missing(
                &failure,
                "invoke_action(scroll_into_view)",
                "ScrollItemPattern",
            )
        })?;
    // SAFETY: 标准 pattern 调用。
    unsafe { pattern.ScrollIntoView() }.map_err(|failure| {
        error::error_from_hresult(failure.code().0, "ScrollItemPattern::ScrollIntoView")
    })?;
    // SAFETY: 只读属性。
    let offscreen = unsafe { element.CurrentIsOffscreen() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentIsOffscreen"))?
        .as_bool();
    if !offscreen {
        return Ok(());
    }
    Err(error::verify_failed(
        "invoke_action(scroll_into_view) postcondition failed: element still reports offscreen",
    ))
}

/// 选择项（列表 / 下拉框）。
///
/// # Errors
/// - `ByIndex` 越界 / `ByStableValue` 找不到 → `TargetNotFound`
/// - 目标不是可选项（无 `SelectionItemPattern`）→ `CapabilityMissing`
/// - 后置条件不成立 → `VerifyFailed`
pub fn select(element: &ResolvedElement, selection: &Selection) -> PlatformResult<()> {
    handles::with_element(element.id(), |uia| {
        let target = match selection {
            Selection::ByIndex(index) => child_at(uia, *index)?,
            Selection::ByStableValue(value) => descendant_with_automation_id(uia, value)?,
            _ => {
                return Err(error::invalid_args(
                    "unknown selection kind: refusing to guess (fail closed)",
                ));
            }
        };
        select_item(&target, "select")
    })
}

/// 取第 `index` 个 control view 子节点（0 起）。
fn child_at(element: &IUIAutomationElement, index: u32) -> PlatformResult<IUIAutomationElement> {
    com::with_automation(|automation| {
        // SAFETY: 只读地取 control view walker。
        let walker = unsafe { automation.ControlViewWalker() }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "ControlViewWalker"))?;
        // SAFETY: 只读遍历。
        let mut current = walk_step_for_actions(
            unsafe { walker.GetFirstChildElement(element) },
            "select: GetFirstChildElement",
        )?;
        let mut remaining = index;
        while let Some(child) = current {
            if remaining == 0 {
                return Ok(child);
            }
            remaining = remaining.saturating_sub(1);
            // SAFETY: 只读遍历。
            current = walk_step_for_actions(
                unsafe { walker.GetNextSiblingElement(&child) },
                "select: GetNextSiblingElement",
            )?;
        }
        Err(error::target_not_found(format!(
            "select: child index {index} is out of range for this element"
        )))
    })
}

/// 在子树里按 **`AutomationId`**（非本地化，ADR-0022 D4）找唯一后代。
fn descendant_with_automation_id(
    element: &IUIAutomationElement,
    automation_id: &str,
) -> PlatformResult<IUIAutomationElement> {
    if automation_id.is_empty() {
        return Err(error::invalid_args(
            "select(ByStableValue): stable value must not be empty",
        ));
    }
    com::with_automation(|automation| {
        let condition_value = VARIANT::from(automation_id);
        // SAFETY: `condition_value` 借用只在本次调用期间有效，UIA 会复制它。
        let condition = unsafe {
            automation.CreatePropertyCondition(UIA_AutomationIdPropertyId, &condition_value)
        }
        .map_err(|failure| {
            error::error_from_hresult(failure.code().0, "CreatePropertyCondition(AutomationId)")
        })?;
        // SAFETY: `condition` 在调用期间存活；作用域是元素子树（不是桌面根）。
        match unsafe { element.FindFirst(TreeScope_Descendants, &condition) } {
            Ok(found) => Ok(found),
            Err(failure) if failure.code() == HRESULT(0) => Err(error::target_not_found(format!(
                "select(ByStableValue): no descendant has AutomationId `{automation_id}`"
            ))),
            Err(failure) => Err(error::error_from_hresult(
                failure.code().0,
                "FindFirst(AutomationId)",
            )),
        }
    })
}

/// 滚动到指定位置（百分比）。
///
/// # Errors
/// - 百分比非有限或不在 `[0, 1]` → `ToolInvalidArgs`
/// - 两个方向都不可滚动 → `CapabilityMissing`
/// - 回读与请求相差超过容差 → `VerifyFailed`
pub fn scroll(element: &ResolvedElement, target: &ScrollTarget) -> PlatformResult<()> {
    let horizontal = target.horizontal_percent();
    let vertical = target.vertical_percent();
    if !horizontal.is_finite()
        || !vertical.is_finite()
        || !(0.0..=1.0).contains(&horizontal)
        || !(0.0..=1.0).contains(&vertical)
    {
        return Err(error::invalid_args(format!(
            "scroll percentages must be finite and within [0, 1], got ({horizontal}, {vertical})"
        )));
    }
    handles::with_element(element.id(), |uia| {
        let pattern: IUIAutomationScrollPattern =
            unsafe { uia.GetCurrentPatternAs(UIA_ScrollPatternId) }
                .map_err(|failure| pattern_missing(&failure, "scroll", "ScrollPattern"))?;
        // SAFETY: 只读属性。
        let horizontally_scrollable = unsafe { pattern.CurrentHorizontallyScrollable() }
            .map_err(|failure| {
                error::error_from_hresult(failure.code().0, "CurrentHorizontallyScrollable")
            })?
            .as_bool();
        // SAFETY: 只读属性。
        let vertically_scrollable = unsafe { pattern.CurrentVerticallyScrollable() }
            .map_err(|failure| {
                error::error_from_hresult(failure.code().0, "CurrentVerticallyScrollable")
            })?
            .as_bool();
        if !horizontally_scrollable && !vertically_scrollable {
            return Err(error::capability_missing(
                "scroll: element is not scrollable in either direction",
            ));
        }
        // UIA 约定：不可滚动的方向必须传 -1（`UIA_NoScroll`），传具体值会得到 E_INVALIDARG。
        let horizontal_argument = if horizontally_scrollable {
            horizontal * 100.0
        } else {
            -1.0
        };
        let vertical_argument = if vertically_scrollable {
            vertical * 100.0
        } else {
            -1.0
        };
        // SAFETY: 标准 pattern 调用。
        unsafe { pattern.SetScrollPercent(horizontal_argument, vertical_argument) }.map_err(
            |failure| {
                error::error_from_hresult(failure.code().0, "ScrollPattern::SetScrollPercent")
            },
        )?;
        if horizontally_scrollable {
            // SAFETY: 只读属性。
            let actual =
                unsafe { pattern.CurrentHorizontalScrollPercent() }.map_err(|failure| {
                    error::error_from_hresult(failure.code().0, "CurrentHorizontalScrollPercent")
                })?;
            if (actual - horizontal_argument).abs() > SCROLL_TOLERANCE_PERCENT {
                return Err(error::verify_failed(format!(
                    "scroll postcondition failed: requested horizontal {horizontal_argument}%, \
                     read back {actual}%"
                )));
            }
        }
        if vertically_scrollable {
            // SAFETY: 只读属性。
            let actual = unsafe { pattern.CurrentVerticalScrollPercent() }.map_err(|failure| {
                error::error_from_hresult(failure.code().0, "CurrentVerticalScrollPercent")
            })?;
            if (actual - vertical_argument).abs() > SCROLL_TOLERANCE_PERCENT {
                return Err(error::verify_failed(format!(
                    "scroll postcondition failed: requested vertical {vertical_argument}%, \
                     read back {actual}%"
                )));
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 支持动作集合必须与 `invoke_action` 的分支一致 —— 少写一个名字会让调用方
    /// 看到"支持"却得到 `ToolInvalidArgs`（文档与实现不一致）。
    #[test]
    fn test_supported_action_list_covers_every_branch() {
        for action in SUPPORTED_ACTIONS.split(", ") {
            assert!(
                matches!(
                    action,
                    "invoke"
                        | "toggle"
                        | "expand"
                        | "collapse"
                        | "select"
                        | "focus"
                        | "scroll_into_view"
                ),
                "`{action}` 在文档里被声明支持，但没有实现分支"
            );
        }
        assert_eq!(SUPPORTED_ACTIONS.split(", ").count(), 7);
    }

    /// 滚动容差必须是**有限的正数**，否则后置条件会永远成立（等于没有后置条件）。
    #[test]
    fn test_scroll_tolerance_is_a_finite_positive_number() {
        // 常量断言用 `const { .. }`：编译期就失败（clippy 的 `assertions_on_constants` 要求）。
        const { assert!(SCROLL_TOLERANCE_PERCENT.is_finite()) };
        const { assert!(SCROLL_TOLERANCE_PERCENT > 0.0) };
        const { assert!(SCROLL_TOLERANCE_PERCENT < 100.0) };
    }
}
