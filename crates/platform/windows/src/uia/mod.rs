//! 元素域：`UiAutomationProvider` 的 13 个方法（架构 v2 §13.1.1 / §13.2、ADR-0022）。
//!
//! 职责：树快照 / 候选链解析 / 等待 / 读文本 / 五类写操作（带回读后置条件）/ 指纹；
//! 以及两个**合成输入**方法与一个**截图**方法的显式未实现出口（见下）。
//! 边界：**不做**合成输入（TASK-018）、**不做**截图（TASK-041）、**不做**策略判定（TASK-021）、
//! **不做**后置断言引擎（TASK-023，本层只做"这一次写操作是否生效"的回读）。
//!
//! ## 为什么 `pointer_action` / `key_action` / `capture` 在这里"报错"而不是"实现"
//! 批次表把 `src/input/**`（`SendInput` / `keybd_event` / `SendKeys`）与 `src/coordinates/**`
//! （DPI / 多屏归一化）分给 **TASK-018**，把截图分给 **TASK-041 / 042**。
//! 但 `UiAutomationProvider` / `WindowProvider` 的 trait 形状**已经**含这三个方法（TASK-016 定死）。
//! 因此本卡按 Q2 / Q3 裁决：**实现到"明确报错"** 的程度，源码标**占位实现标记**
//! （格式见 `docs/spec/naming.md` §8），
//! 返回 `CapabilityMissing` —— 不是 `todo!()` / `unimplemented!()`（那会让非 Windows 平台与
//! 未实现路径以 panic 形式失败，违反铁律 1）。
//!
//! ## 不变量
//! 1. **写操作必回读**（铁律 4）：`set_value` / `edit_text` / `invoke_action` / `select` / `scroll`
//!    都验证后置条件；不一致 → `VerifyFailed`，**绝不**返回 `Ok`。
//! 2. **COM 对象不跨线程**：一切 UIA 调用都经 `crate::com::with_automation`（线程本地实例）
//!    与 `crate::handles::with_element`（线程本地元素表）。
//! 3. **角色名非本地化**（ADR-0022 D4）：`ResolvedElement::role` 用 UIA control type 的
//!    **规范英文名**（`Document` / `Edit` / `Button` …），**不用** `LocalizedControlType`。
//! 4. **`FindFirst` / walker 的"没有更多"约定**：UIA 用 `S_OK` + `NULL` 表示"没有"，
//!    `windows` crate 把它翻成 `Err(HRESULT(0))` —— 必须显式映射成 `Ok(None)`，
//!    否则会把"没找到"当成真失败（该坑来自 `spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`）。
//!
//! 相关：架构 v2 §13.1.1 / §13.2 / §6.2 / §7.3、ADR-0022 D4/D5、`docs/memory/apps/notepad.md` §3。

mod actions;
mod patterns;
mod resolve;
mod search;
mod tree;

use std::future::{Future, poll_fn};
use std::task::Poll;

use assistant_platform_api::{
    ElementQuery, ElementState, Fingerprint, FingerprintScope, KeyChord, KeyTarget,
    NormalizedPoint, PlatformResult, PointerAction, ResolvedElement, ResolvedWindow, ScrollTarget,
    Selection, SelectorChain, TextEditOp, Timeout, TreeOptions, TreeSnapshot, UiAutomationProvider,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, UIA_ButtonControlTypeId, UIA_CONTROLTYPE_ID, UIA_CalendarControlTypeId,
    UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId, UIA_CustomControlTypeId,
    UIA_DataGridControlTypeId, UIA_DataItemControlTypeId, UIA_DocumentControlTypeId,
    UIA_EditControlTypeId, UIA_GroupControlTypeId, UIA_HeaderControlTypeId,
    UIA_HeaderItemControlTypeId, UIA_HyperlinkControlTypeId, UIA_ImageControlTypeId,
    UIA_ListControlTypeId, UIA_ListItemControlTypeId, UIA_MenuBarControlTypeId,
    UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_PaneControlTypeId,
    UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId, UIA_ScrollBarControlTypeId,
    UIA_SeparatorControlTypeId, UIA_SliderControlTypeId, UIA_SpinnerControlTypeId,
    UIA_SplitButtonControlTypeId, UIA_StatusBarControlTypeId, UIA_TabControlTypeId,
    UIA_TabItemControlTypeId, UIA_TableControlTypeId, UIA_TextControlTypeId,
    UIA_ThumbControlTypeId, UIA_TitleBarControlTypeId, UIA_ToolBarControlTypeId,
    UIA_ToolTipControlTypeId, UIA_TreeControlTypeId, UIA_TreeItemControlTypeId,
    UIA_WindowControlTypeId,
};
use windows::core::BSTR;

use crate::com;
use crate::error;
use crate::window::WindowsPlatform;

// 为什么不用 `async fn`：这些函数体没有 `.await`，clippy 的 `unused_async_trait_impl`（nursery，
// 本 workspace 为 warn → `-D warnings` 下即错误）要求去掉 `async`；而本 crate 禁止 `#[allow]`。
// 用 `poll_fn` 而不是 `std::future::ready(..)`：前者把工作留在**第一次 poll**，与 `async fn` 的
// 语义一致 —— 构造 future 不产生副作用，drop 掉未 poll 的 future 不会触发任何 UIA 调用。
impl UiAutomationProvider for WindowsPlatform {
    fn snapshot_tree(
        &self,
        root: &ResolvedWindow,
        options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send {
        poll_fn(move |_context| Poll::Ready(tree::snapshot_tree(root, options)))
    }

    fn resolve_element(
        &self,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        poll_fn(move |_context| Poll::Ready(resolve::resolve_element(chain)))
    }

    fn wait_for(
        &self,
        query: &ElementQuery,
        state: &ElementState,
        timeout: Timeout,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        poll_fn(move |_context| Poll::Ready(resolve::wait_for(query, *state, timeout)))
    }

    fn read_text(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        poll_fn(move |_context| Poll::Ready(patterns::read_text(element)))
    }

    fn set_value(
        &self,
        element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        poll_fn(move |_context| Poll::Ready(patterns::set_value(element, value)))
    }

    fn edit_text(
        &self,
        element: &ResolvedElement,
        operation: &TextEditOp,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        poll_fn(move |_context| Poll::Ready(patterns::edit_text(element, operation)))
    }

    fn invoke_action(
        &self,
        element: &ResolvedElement,
        action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        poll_fn(move |_context| Poll::Ready(actions::invoke_action(element, action)))
    }

    fn select(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        poll_fn(move |_context| Poll::Ready(actions::select(element, selection)))
    }

    fn scroll(
        &self,
        element: &ResolvedElement,
        target: &ScrollTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        poll_fn(move |_context| Poll::Ready(actions::scroll(element, target)))
    }

    fn pointer_action(
        &self,
        _point: NormalizedPoint,
        _action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        // STUB(TASK-018): 合成指针输入（SendInput）+ 坐标归一化归 TASK-018（src/input/**、src/coordinates/**）。
        poll_fn(move |_context| {
            Poll::Ready(Err(error::stub_not_implemented(
                "TASK-018",
                "UiAutomationProvider::pointer_action (synthetic pointer input)",
            )))
        })
    }

    fn key_action(
        &self,
        _chord: &KeyChord,
        _target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        // STUB(TASK-018): 合成键盘输入（SendInput）归 TASK-018。
        poll_fn(move |_context| {
            Poll::Ready(Err(error::stub_not_implemented(
                "TASK-018",
                "UiAutomationProvider::key_action (synthetic keyboard input)",
            )))
        })
    }

    fn fingerprint(
        &self,
        window: &ResolvedWindow,
        scope: &FingerprintScope,
    ) -> impl Future<Output = PlatformResult<Fingerprint>> + Send {
        poll_fn(move |_context| Poll::Ready(tree::fingerprint(window, scope)))
    }
}

/// `BSTR` → `String`（`windows` 的 `BSTR::to_string` 对 NULL 返回空串，不 panic）。
pub fn bstr_to_string(value: &BSTR) -> String {
    value.to_string()
}

/// 取窗口的 UIA 根元素（`ElementFromHandle`）。
///
/// # Errors
/// 句柄失效 → `TargetNotFound`；UIPI 拦截（目标进程完整性级别更高）→ `PlatformPermission`。
pub fn element_for_window(hwnd: HWND) -> PlatformResult<IUIAutomationElement> {
    com::with_automation(|automation| {
        // SAFETY: `hwnd` 由调用方保证来自本进程枚举到的真实窗口；UIA 只读它，不接管所有权。
        unsafe { automation.ElementFromHandle(hwnd) }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "ElementFromHandle"))
    })
}

/// 采集一个窗口的 UIA 事实（`AutomationId` / `RuntimeId`），供窗口候选匹配使用。
///
/// 返回 `(uia_available, automation_id, runtime_id)`：
/// `uia_available == false` 表示**事实不完整**（UIPI 拦截、窗口已消失、COM 不可用），
/// 调用方必须把它转成显式的 `Unsupported` 理由，**不得**当成"这个窗口没有该属性"。
pub fn window_uia_facts(hwnd: HWND) -> (bool, String, String) {
    let collected = com::with_automation(|automation| {
        // SAFETY: 与 `element_for_window` 同（只读句柄）。
        let element = unsafe { automation.ElementFromHandle(hwnd) }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "ElementFromHandle"))?;
        // SAFETY: 只读属性查询；返回的 `BSTR` 由本进程持有并在 drop 时释放。
        let automation_id = unsafe { element.CurrentAutomationId() }
            .map(|value| bstr_to_string(&value))
            .unwrap_or_default();
        Ok((automation_id, runtime_id_string(&element)))
    });
    // 这里**故意**把错误压成 `false`：错误详情由调用方的 `Unsupported` 理由表达
    // （候选匹配的 reasons 只接受 `&'static str`，见 `window::candidates`）。
    // 关键点是**不**把它当成"属性为空"，因此不会产生假命中。
    match collected {
        Ok((automation_id, runtime_id)) => (true, automation_id, runtime_id),
        Err(_) => (false, String::new(), String::new()),
    }
}

/// 读取元素的 `RuntimeId` 并规范成「十进制、逗号分隔」（空串 = 不可用）。
pub fn runtime_id_string(element: &IUIAutomationElement) -> String {
    // SAFETY: 只读属性；返回的 SAFEARRAY 所有权归本函数，下面显式销毁。
    let Ok(array) = (unsafe { element.GetRuntimeId() }) else {
        return String::new();
    };
    if array.is_null() {
        return String::new();
    }
    let rendered = read_i32_safearray(array).unwrap_or_default();
    // SAFETY: `array` 由上面的 `GetRuntimeId` 产出且未被其它人持有；与它配对的销毁调用只有这一次。
    let _destroy_outcome = unsafe { SafeArrayDestroy(array) };
    rendered
}

/// 把一维 `i32` SAFEARRAY 读成「十进制、逗号分隔」字符串。
fn read_i32_safearray(array: *const SAFEARRAY) -> Option<String> {
    // SAFETY: `array` 由调用方保证非空且是一维 `VT_I4` SAFEARRAY（`GetRuntimeId` 的约定）。
    let lower = unsafe { SafeArrayGetLBound(array, 1) }.ok()?;
    // SAFETY: 同上。
    let upper = unsafe { SafeArrayGetUBound(array, 1) }.ok()?;
    let span = i64::from(upper) - i64::from(lower) + 1;
    if span <= 0 {
        return Some(String::new());
    }
    let count = usize::try_from(span).ok()?;
    let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: 取出数据区指针；必须与下面的 `SafeArrayUnaccessData` 严格配对。
    unsafe { SafeArrayAccessData(array, &raw mut data) }.ok()?;
    let mut parts: Vec<String> = Vec::with_capacity(count);
    if !data.is_null() {
        let values = data.cast::<i32>();
        for index in 0..count {
            // SAFETY: `values` 指向至少 `count` 个连续 `i32`（SAFEARRAY 的 lbound..=ubound）。
            let value = unsafe { *values.add(index) };
            parts.push(value.to_string());
        }
    }
    // SAFETY: 与上面的 `SafeArrayAccessData` 配对（释放数据区访问锁）。
    let _unaccess_outcome = unsafe { SafeArrayUnaccessData(array) };
    Some(parts.join(","))
}

/// UIA control type ↔ **非本地化**角色名（ADR-0022 D4：绝不用 `LocalizedControlType`）。
///
/// **单一事实源**：`role_name`（正向）与 `control_type_from_role`（反向）共用本表。
/// 此前这里正反两份手写表**已经漂移过**：`match` 有 39 支而反查数组只有 38 项、漏了 `TreeItem`，
/// 于是 `control_type_from_role("TreeItem")` 返回 `None`（`wait_for(role = "TreeItem")` 会报
/// `ToolInvalidArgs`，而 `TreeItem` 是合法角色）。合成一份表 + 逐项遍历的回归用例才能防住这类漏项。
///
/// 为什么是表而不是 `match`：`windows` crate 的常量名是 camelCase（`UIA_ButtonControlTypeId`），
/// 出现在 `match` 的**模式**位置会触发 `non_upper_case_globals`；表达式位置不触发，
/// 而本 crate 禁止 `#[allow]` 放宽（卡面 `DoD`）。
const CONTROL_TYPE_NAMES: [(UIA_CONTROLTYPE_ID, &str); 39] = [
    (UIA_ButtonControlTypeId, "Button"),
    (UIA_CalendarControlTypeId, "Calendar"),
    (UIA_CheckBoxControlTypeId, "CheckBox"),
    (UIA_ComboBoxControlTypeId, "ComboBox"),
    (UIA_CustomControlTypeId, "Custom"),
    (UIA_DataGridControlTypeId, "DataGrid"),
    (UIA_DataItemControlTypeId, "DataItem"),
    (UIA_DocumentControlTypeId, "Document"),
    (UIA_EditControlTypeId, "Edit"),
    (UIA_GroupControlTypeId, "Group"),
    (UIA_HeaderControlTypeId, "Header"),
    (UIA_HeaderItemControlTypeId, "HeaderItem"),
    (UIA_HyperlinkControlTypeId, "Hyperlink"),
    (UIA_ImageControlTypeId, "Image"),
    (UIA_ListControlTypeId, "List"),
    (UIA_ListItemControlTypeId, "ListItem"),
    (UIA_MenuBarControlTypeId, "MenuBar"),
    (UIA_MenuControlTypeId, "Menu"),
    (UIA_MenuItemControlTypeId, "MenuItem"),
    (UIA_PaneControlTypeId, "Pane"),
    (UIA_ProgressBarControlTypeId, "ProgressBar"),
    (UIA_RadioButtonControlTypeId, "RadioButton"),
    (UIA_ScrollBarControlTypeId, "ScrollBar"),
    (UIA_SeparatorControlTypeId, "Separator"),
    (UIA_SliderControlTypeId, "Slider"),
    (UIA_SpinnerControlTypeId, "Spinner"),
    (UIA_SplitButtonControlTypeId, "SplitButton"),
    (UIA_StatusBarControlTypeId, "StatusBar"),
    (UIA_TabControlTypeId, "Tab"),
    (UIA_TabItemControlTypeId, "TabItem"),
    (UIA_TableControlTypeId, "Table"),
    (UIA_TextControlTypeId, "Text"),
    (UIA_ThumbControlTypeId, "Thumb"),
    (UIA_TitleBarControlTypeId, "TitleBar"),
    (UIA_ToolBarControlTypeId, "ToolBar"),
    (UIA_ToolTipControlTypeId, "ToolTip"),
    (UIA_TreeControlTypeId, "Tree"),
    (UIA_TreeItemControlTypeId, "TreeItem"),
    (UIA_WindowControlTypeId, "Window"),
];

/// control type → **非本地化**角色名；未知的 control type 返回 `ControlType(<数字>)`。
///
/// 不硬套一个"看起来合理"的名字：角色会被 `ClassAndRole` / `ElementQuery.role()` 匹配，
/// 猜错会命中别的控件（铁律 1）。
#[must_use]
pub fn role_name(control_type: UIA_CONTROLTYPE_ID) -> String {
    CONTROL_TYPE_NAMES
        .iter()
        .find(|(id, _)| *id == control_type)
        .map_or_else(
            || format!("ControlType({})", control_type.0),
            |(_, name)| (*name).to_string(),
        )
}

/// **非本地化**角色名 → control type id（`role_name` 的逆；未知名字 → `None`，不猜）。
#[must_use]
pub fn control_type_from_role(role: &str) -> Option<UIA_CONTROLTYPE_ID> {
    CONTROL_TYPE_NAMES
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(role))
        .map(|(id, _)| *id)
}

/// 读取元素的角色名（`CurrentControlType` → `role_name`）。
pub fn element_role(element: &IUIAutomationElement) -> String {
    // SAFETY: 只读属性查询。
    unsafe { element.CurrentControlType() }.map_or_else(|_| String::new(), role_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_control_type_round_trips_through_role_name() {
        // 回归（ADR-0019 N1）：正反两份手写表曾经漂移 —— `match` 有 39 支而反查数组只有 38 项，
        // 漏了 `TreeItem`，于是 `control_type_from_role("TreeItem")` 返回 `None`。
        // 逐项遍历整张表，任何漏项 / 错配都会立刻红。
        for (id, name) in CONTROL_TYPE_NAMES {
            assert_eq!(
                control_type_from_role(name),
                Some(id),
                "角色 `{name}` 必须能反查到 control type"
            );
            assert_eq!(role_name(id), name, "control type 必须正查回 `{name}`");
        }
    }

    #[test]
    fn test_control_type_table_has_no_duplicate_ids_or_names() {
        // 重复项会让正查 / 反查的结果取决于表序（不确定），必须拦住。
        let mut ids: Vec<i32> = CONTROL_TYPE_NAMES.iter().map(|(id, _)| id.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            CONTROL_TYPE_NAMES.len(),
            "control type id 不得重复"
        );

        let mut names: Vec<&str> = CONTROL_TYPE_NAMES.iter().map(|(_, name)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CONTROL_TYPE_NAMES.len(), "角色名不得重复");
    }

    #[test]
    fn test_role_name_is_case_insensitive_on_lookup() {
        assert_eq!(
            control_type_from_role("document").map(role_name),
            Some("Document".to_string())
        );
    }

    #[test]
    fn test_unknown_role_is_not_guessed() {
        // 负向用例（铁律 1）：未知角色名**不得**猜成一个"看起来合理"的 control type。
        assert_eq!(control_type_from_role("DefinitelyNotAControlType"), None);
    }

    #[test]
    fn test_unknown_control_type_is_reported_with_its_id() {
        // 负向用例：未知 control type 不得被硬套成 Custom（那会让角色匹配命中别的控件）。
        let unknown = UIA_CONTROLTYPE_ID(60_999);
        assert_eq!(role_name(unknown), "ControlType(60999)");
    }
}
