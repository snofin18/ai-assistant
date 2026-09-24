//! 元素候选链的**搜索**实现（UIA 属性条件 + 递归 `RoleAndParent`）（ADR-0022 D5）。
//!
//! 职责：把 `SelectorCandidate` 变成一次 UIA 搜索，并如实区分「不支持」「没命中」「命中 n 个」。
//! 边界：**不做**歧义裁决（`resolve` 模块用 `selector::decide_selection` 统一裁决）、**不做**缓存。
//!
//! ## 作用域与代价（**实测**：桌面根的 `Descendants` 很贵）
//! 两条路径**不同**，勿混为一谈：
//! - `find_all`（`resolve_element` 用）：无父候选时**直接**对桌面根发
//!   `FindAll(TreeScope_Descendants)` —— 一次走遍整个桌面。2026-09-24 真机实测
//!   **中位数 1.53 s**（对照：Spike A 在**窗口子树内**搜索是 1.2~1.5 ms）。
//! - `find_first_children_then_descendants`（`wait_for` 用）：对根元素**先**
//!   `TreeScope_Children`（只扫顶层窗口，廉价），**再**退回 `Descendants`（ADR-0022 E6）。
//! - `RoleAndParent`：在**父元素子树**内搜索（有界）。
//!
//! ⚠ **已知偏差（DRIFT-017-7）**：ADR-0022 E6 引用的官方文档要求「在桌面上找顶层窗口必须用
//! `TreeScope_Children`，用 `Descendants` 可能让 provider 栈溢出」，且官方最佳实践是**从应用
//! 窗口 / 更低的容器开始搜索**。`find_all` 目前**没有**遵守 —— 根因是 `resolve_element` 的 trait
//! 形状没有 scope 参数（**PL-068**）。本卡**不**自行改搜索策略（那会改歧义判定语义），
//! 按漂移触发器 ⑧ 登记等裁决。
//!
//! ## 不支持的种类**必须**返回 `Unsupported`（铁律 1）
//! 把它当成"没命中"会让调用方看到 `TargetNotFound`，而真实原因是"这个候选种类本通道没实现"
//! —— 两者处置完全不同（换候选 vs 换通道）。
//!
//! 相关：架构 v2 §6.2 / §6.3、ADR-0022 D4/D5/E6、`docs/memory/apps/notepad.md` §3。

use assistant_platform_api::{PlatformResult, SelectorCandidate, SelectorChain, SelectorValue};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationCondition, IUIAutomationElement,
    PropertyConditionFlags_MatchSubstring, TreeScope, TreeScope_Children, TreeScope_Descendants,
    UIA_AutomationIdPropertyId, UIA_ClassNamePropertyId, UIA_ControlTypePropertyId,
    UIA_NamePropertyId, UIA_PROPERTY_ID,
};
use windows::core::HRESULT;

use crate::error;
use crate::{com, uia};

/// `RoleAndParent` 的最大递归深度（防止链内自引用 / 环形引用导致无限递归）。
const MAX_PARENT_DEPTH: u32 = 8;

/// 一次候选搜索的结果。
pub(super) enum SearchOutcome {
    /// 该候选种类本通道不支持（原因进 reasons，绝不静默当作"没命中"）。
    Unsupported(&'static str),
    /// 支持，命中 0..n 个元素（由调用方裁决歧义）。
    Matches(Vec<IUIAutomationElement>),
}

/// 搜索一个候选；`parent` 非空时在该父元素子树内搜索（`RoleAndParent` 递归用）。
pub(super) fn find_all(
    chain: &SelectorChain,
    candidate: &SelectorCandidate,
    parent: Option<&IUIAutomationElement>,
    depth: u32,
) -> PlatformResult<SearchOutcome> {
    if depth > MAX_PARENT_DEPTH {
        return Err(error::invalid_args(format!(
            "RoleAndParent nesting exceeded {MAX_PARENT_DEPTH} levels; refusing to recurse further"
        )));
    }
    match candidate.kind() {
        assistant_platform_api::SelectorKind::AutomationId => match candidate.value() {
            SelectorValue::Text(value) => {
                property_search(parent, UIA_AutomationIdPropertyId, value, false)
            }
            _ => Ok(SearchOutcome::Unsupported(
                "AutomationId 候选必须用 SelectorValue::Text 表达取值",
            )),
        },
        assistant_platform_api::SelectorKind::ClassAndRole => match candidate.value() {
            SelectorValue::ClassAndRole { class, role } => {
                class_and_role_search(parent, class, role)
            }
            _ => Ok(SearchOutcome::Unsupported(
                "ClassAndRole 候选必须用 SelectorValue::ClassAndRole 表达取值",
            )),
        },
        assistant_platform_api::SelectorKind::RoleAndParent => match candidate.value() {
            SelectorValue::RoleAndParent { role, parent_id } => {
                role_under_parent(chain, parent, role, parent_id, depth)
            }
            _ => Ok(SearchOutcome::Unsupported(
                "RoleAndParent 候选必须用 SelectorValue::RoleAndParent 表达取值",
            )),
        },
        // DRIFT-017-4：本卡没有 regex 引擎（加依赖 = 漂移触发器 ①），改用 UIA 原生的
        // **子串**匹配（`PropertyConditionFlags_MatchSubstring`）。只可能漏命中，不会假命中。
        assistant_platform_api::SelectorKind::NameRegex
        | assistant_platform_api::SelectorKind::TitleRegex => match candidate.value() {
            SelectorValue::Text(pattern) => {
                property_search(parent, UIA_NamePropertyId, pattern, true)
            }
            _ => Ok(SearchOutcome::Unsupported(
                "NameRegex / TitleRegex 候选必须用 SelectorValue::Text 表达取值",
            )),
        },
        assistant_platform_api::SelectorKind::RuntimeId => Ok(SearchOutcome::Unsupported(
            "UIA 客户端 API 没有按 RuntimeId 取回元素的方法；RuntimeId 候选只在同一会话内配合缓存才有意义（缓存归 TASK-025）",
        )),
        assistant_platform_api::SelectorKind::A11yPath => Ok(SearchOutcome::Unsupported(
            "A11yPath 由节点名（本地化）构成，本卡未实现路径遍历",
        )),
        assistant_platform_api::SelectorKind::AxIdentifier => Ok(SearchOutcome::Unsupported(
            "AxIdentifier 是 macOS 无障碍概念；Windows 通道的等价物是 AutomationId",
        )),
        assistant_platform_api::SelectorKind::VisualAnchor => Ok(SearchOutcome::Unsupported(
            "VisualAnchor 需要视觉 / OCR 通道（TASK-041 / 042）",
        )),
        _ => Ok(SearchOutcome::Unsupported(
            "未知的候选种类（本通道尚未支持）",
        )),
    }
}

/// 单属性条件搜索（`substring = true` 时用 UIA 原生子串匹配）。
fn property_search(
    parent: Option<&IUIAutomationElement>,
    property: UIA_PROPERTY_ID,
    value: &str,
    substring: bool,
) -> PlatformResult<SearchOutcome> {
    if value.is_empty() {
        // 空取值会匹配一切 —— 那是配置错误，不是"命中所有元素"。
        return Ok(SearchOutcome::Unsupported(
            "候选取值为空：空取值会匹配一切，拒绝执行（配置错误）",
        ));
    }
    let elements = com::with_automation(|automation| {
        let variant = VARIANT::from(value);
        let condition = if substring {
            // SAFETY: `variant` 借用只在本次调用期间有效，UIA 会复制它。
            unsafe {
                automation.CreatePropertyConditionEx(
                    property,
                    &variant,
                    PropertyConditionFlags_MatchSubstring,
                )
            }
        } else {
            // SAFETY: 同上。
            unsafe { automation.CreatePropertyCondition(property, &variant) }
        }
        .map_err(|failure| {
            error::error_from_hresult(failure.code().0, "CreatePropertyCondition")
        })?;
        let scope = search_scope(automation, parent)?;
        collect_matches(&scope, &condition)
    })?;
    Ok(SearchOutcome::Matches(elements))
}

/// `ClassName` + `ControlType` 组合搜索。
fn class_and_role_search(
    parent: Option<&IUIAutomationElement>,
    class: &str,
    role: &str,
) -> PlatformResult<SearchOutcome> {
    let Some(control_type) = uia::control_type_from_role(role) else {
        return Ok(SearchOutcome::Unsupported(
            "ClassAndRole 的 role 不是已知的非本地化角色名",
        ));
    };
    let elements = com::with_automation(|automation| {
        let class_condition = property_condition(automation, UIA_ClassNamePropertyId, class)?;
        let role_condition =
            property_condition(automation, UIA_ControlTypePropertyId, control_type.0)?;
        // SAFETY: 两个条件在调用期间存活，UIA 只引用它们。
        let condition = unsafe { automation.CreateAndCondition(&class_condition, &role_condition) }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "CreateAndCondition"))?;
        let scope = search_scope(automation, parent)?;
        collect_matches(&scope, &condition)
    })?;
    Ok(SearchOutcome::Matches(elements))
}

/// `RoleAndParent`：在**链内**解析父候选，再在父元素子树内按角色找。
fn role_under_parent(
    chain: &SelectorChain,
    parent: Option<&IUIAutomationElement>,
    role: &str,
    parent_id: &str,
    depth: u32,
) -> PlatformResult<SearchOutcome> {
    let Some(parent_candidate) = chain
        .candidates()
        .iter()
        .find(|candidate| candidate.id() == parent_id)
    else {
        return Ok(SearchOutcome::Unsupported(
            "RoleAndParent 的 parent_id 在本链内找不到对应候选",
        ));
    };
    let parent_element = match find_all(chain, parent_candidate, parent, depth.saturating_add(1))? {
        SearchOutcome::Unsupported(reason) => return Ok(SearchOutcome::Unsupported(reason)),
        SearchOutcome::Matches(elements) => match elements.len() {
            0 => return Ok(SearchOutcome::Matches(Vec::new())),
            1 => match elements.into_iter().next() {
                Some(element) => element,
                None => return Ok(SearchOutcome::Matches(Vec::new())),
            },
            // 父候选本身有歧义 → 拒绝继续（不猜父节点，铁律 1）。
            count => {
                return Err(error::target_ambiguous(format!(
                    "RoleAndParent: parent candidate `{parent_id}` matched {count} elements; \
                     refusing to guess which subtree to search"
                )));
            }
        },
    };
    let Some(control_type) = uia::control_type_from_role(role) else {
        return Ok(SearchOutcome::Unsupported(
            "RoleAndParent 的 role 不是已知的非本地化角色名",
        ));
    };
    let elements = com::with_automation(|automation| {
        let condition = property_condition(automation, UIA_ControlTypePropertyId, control_type.0)?;
        collect_matches(&parent_element, &condition)
    })?;
    Ok(SearchOutcome::Matches(elements))
}

/// 构造一个单属性条件。
fn property_condition<T: Into<VARIANT>>(
    automation: &IUIAutomation,
    property: UIA_PROPERTY_ID,
    value: T,
) -> PlatformResult<IUIAutomationCondition> {
    let variant = value.into();
    // SAFETY: `variant` 借用只在本次调用期间有效，UIA 会复制它。
    unsafe { automation.CreatePropertyCondition(property, &variant) }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CreatePropertyCondition"))
}

/// 搜索起点：有父候选用父元素，否则用桌面根。
fn search_scope(
    automation: &IUIAutomation,
    parent: Option<&IUIAutomationElement>,
) -> PlatformResult<IUIAutomationElement> {
    // `map_or_else` 而不是 `match`：clippy 的 `option_if_let_else`（pedantic）要求这样写。
    parent.map_or_else(
        || {
            // SAFETY: 只读地取桌面根元素。
            unsafe { automation.GetRootElement() }
                .map_err(|failure| error::error_from_hresult(failure.code().0, "GetRootElement"))
        },
        |parent| Ok(parent.clone()),
    )
}

/// 取某条件下的全部匹配（`FindAll` + `Length` + `GetElement`）。
///
/// 作用域用 `TreeScope_Descendants`：调用方要么传了父元素（子树有界），
/// 要么明确接受"从桌面根搜索"的代价（见模块头）。
fn collect_matches(
    scope: &IUIAutomationElement,
    condition: &IUIAutomationCondition,
) -> PlatformResult<Vec<IUIAutomationElement>> {
    // SAFETY: `condition` 在调用期间存活；返回的数组由本进程持有。
    let array = unsafe { scope.FindAll(TreeScope_Descendants, condition) }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "FindAll"))?;
    // SAFETY: 只读属性。
    let length = unsafe { array.Length() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "ElementArray::Length"))?;
    let mut elements = Vec::new();
    for index in 0..length {
        // SAFETY: `index` 由 `Length()` 界定，在合法区间内。
        let element = unsafe { array.GetElement(index) }.map_err(|failure| {
            error::error_from_hresult(failure.code().0, "ElementArray::GetElement")
        })?;
        elements.push(element);
    }
    Ok(elements)
}

/// `FindFirst`：把 UIA 的「没找到」（`S_OK` + NULL → `Err(HRESULT(0))`）翻成 `Ok(None)`。
pub(super) fn find_first(
    scope: &IUIAutomationElement,
    condition: &IUIAutomationCondition,
    tree_scope: TreeScope,
) -> PlatformResult<Option<IUIAutomationElement>> {
    // SAFETY: `condition` 在调用期间存活；返回元素由本进程持有。
    match unsafe { scope.FindFirst(tree_scope, condition) } {
        Ok(element) => Ok(Some(element)),
        Err(failure) if failure.code() == HRESULT(0) => Ok(None),
        Err(failure) => Err(error::error_from_hresult(failure.code().0, "FindFirst")),
    }
}

/// 先 `Children` 再 `Descendants` 的搜索（用于 `wait_for` 的 `ElementQuery`）。
pub(super) fn find_first_children_then_descendants(
    automation: &IUIAutomation,
    condition: &IUIAutomationCondition,
) -> PlatformResult<Option<IUIAutomationElement>> {
    let root = search_scope(automation, None)?;
    // ADR-0022 E6：对**根元素**发 FindFirst 必须先试 Children，避免一上来就走遍桌面。
    if let Some(found) = find_first(&root, condition, TreeScope_Children)? {
        return Ok(Some(found));
    }
    find_first(&root, condition, TreeScope_Descendants)
}

/// 候选的人类可读描述（进 reasons，使失败可从时间线定位）。
pub(super) fn describe(candidate: &SelectorCandidate) -> String {
    format!(
        "id={} kind={:?} score={:.3}",
        candidate.id(),
        candidate.kind(),
        candidate.score()
    )
}
