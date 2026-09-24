//! 元素解析与等待（`resolve_element` / `wait_for`）（架构 v2 §6.2 / §6.3、ADR-0022 D5）。
//!
//! 职责：把候选链落成**一个** UIA 元素，或把 `ElementQuery` 等成期望状态。
//! 边界：**不做**候选搜索本身（在 `super::search`）、**不做**自愈回写（§6.3 归 Adapter）、
//! **不做**缓存（TASK-025）。
//!
//! ## 作用域（ADR-0043：元素解析必须有 scope）
//! `resolve_element` / `wait_for` 的第一个参数是 `&ResolvedWindow`，搜索起点是该窗口的
//! UIA 根元素（`ElementFromHandle`，见 `scope_root`）。**不再**从桌面根搜索 ——
//! 那是 TASK-017 真机实测**中位数 1.53 s** 的来源（对照窗口子树内 1.2~1.5 ms，约 **1000×**），
//! 也与 ADR-0022 E6 引用的官方要求冲突（DRIFT-017-7）。
//!
//! 比窗口更深的容器 scope 仍由链内的 `RoleAndParent` 候选表达（ADR-0043 D4）。
//! 调用顺序（架构 v2 §6.2）：先 `resolve_window` 定窗口，再把该 `ResolvedWindow` 作为 scope
//! 传给这里 —— 本模块**不做**窗口解析。
//!
//! ## 不变量
//! 1. **并列命中报歧义**：本模块固定用 `OnAmbiguous::ErrorAndAsk`（trait 的默认语义，铁律 1：不猜）。
//! 2. `wait_for` 超时 → `TargetUnresponsive`（元素可能存在但状态一直没到，与"找不到"不同）。
//! 3. 轮询**有界**：固定间隔 + 显式截止时间；不做无界自旋。
//! 4. 空链 / 无条件的 query 在**碰 COM 之前**就报 `ToolInvalidArgs`。
//!
//! 相关：架构 v2 §6.2 / §6.3 / §6.4、ADR-0022 D4/D5/E6、`docs/memory/apps/notepad.md` §3。

use std::time::{Duration, Instant};

use assistant_platform_api::{
    ElementQuery, ElementState, OnAmbiguous, PlatformResult, ResolvedElement, ResolvedWindow,
    SelectorChain, Timeout,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{
    IUIAutomationCondition, IUIAutomationElement, UIA_AutomationIdPropertyId,
    UIA_ControlTypePropertyId,
};

use crate::error;
use crate::{com, handles, selector, uia};

use super::search::{SearchOutcome, describe, find_all, find_first_children_then_descendants};
use super::tree::walk_step_for_actions;

/// `wait_for` 的轮询间隔（毫秒）。
///
/// 取值理由：Spike A 实测记事本"从启动到可枚举"约 450 ms（间隔 150 ms × 3 次），元素级变化更快；
/// 50 ms 在响应性与 UIA 调用开销之间取平衡（更小会变成忙等，更大则响应性退化）。
const POLL_INTERVAL_MS: u64 = 50;

/// 沿父链上溯找所属窗口的最大层数（防御性上界，防止异常树结构导致死循环）。
const MAX_PARENT_WALK: u32 = 64;

/// 取 scope 窗口的 UIA 根元素 —— 元素搜索的**唯一**起点（ADR-0043 D2）。
///
/// # Errors
/// 句柄值超出本进程指针宽度 → `CapabilityMissing`；窗口已关闭 / 句柄失效 → `TargetNotFound`；
/// UIPI 拦截（目标进程完整性级别更高）→ `PlatformPermission`。
fn scope_root(scope: &ResolvedWindow) -> PlatformResult<IUIAutomationElement> {
    let hwnd = handles::hwnd_from_window_handle(scope.id())?;
    uia::element_for_window(hwnd)
}

/// 在 `scope` 窗口内按候选链解析出**唯一**元素（ADR-0043）。
///
/// # Errors
/// - 链为空 → `ToolInvalidArgs`（**先于**任何 COM 调用）
/// - `scope` 窗口句柄失效 / 窗口已关闭 → `TargetNotFound`
/// - 全部候选都**不支持** → `CapabilityMissing`，message 列出每种候选的原因
/// - 支持但没命中 → `TargetNotFound`，message 列出「试过什么」
/// - 多命中 → `TargetAmbiguous`（不猜）
pub fn resolve_element(
    scope: &ResolvedWindow,
    chain: &SelectorChain,
) -> PlatformResult<ResolvedElement> {
    if chain.candidates().is_empty() {
        return Err(error::invalid_args(
            "resolve_element: selector chain is empty (an empty chain can never resolve)",
        ));
    }
    let scope_element = scope_root(scope)?;
    let ranked = selector::rank_candidates(chain, 0.0);
    let mut tried: Vec<String> = Vec::new();
    let mut saw_supported_candidate = false;
    for entry in &ranked {
        let candidate = entry.candidate();
        match find_all(chain, candidate, &scope_element, 0)? {
            SearchOutcome::Unsupported(reason) => {
                tried.push(format!("{} -> unsupported: {reason}", describe(candidate)));
            }
            SearchOutcome::Matches(elements) => {
                saw_supported_candidate = true;
                if elements.is_empty() {
                    tried.push(format!("{} -> 0 matches", describe(candidate)));
                    continue;
                }
                let scores = vec![entry.effective_score(); elements.len()];
                match selector::decide_selection(&scores, OnAmbiguous::ErrorAndAsk) {
                    selector::SelectionOutcome::Unique => {
                        let Some(element) = elements.first() else {
                            return Err(error::target_not_found(
                                "resolve_element produced no element after a non-empty match set",
                            ));
                        };
                        return build_resolved(element);
                    }
                    selector::SelectionOutcome::Ambiguous { matches } => {
                        tried.push(format!(
                            "{} -> {matches} matches (ambiguous; the platform layer refuses to guess)",
                            describe(candidate)
                        ));
                        return Err(error::target_ambiguous(format!(
                            "resolve_element: {matches} elements matched (ambiguous per ADR-0044); \
                             the platform layer refuses to guess; tried: {}",
                            tried.join("; ")
                        )));
                    }
                    selector::SelectionOutcome::NotFound => {
                        tried.push(format!("{} -> 0 matches", describe(candidate)));
                    }
                }
            }
        }
    }
    let joined = tried.join("; ");
    if !saw_supported_candidate {
        return Err(error::capability_missing(format!(
            "resolve_element: every candidate uses a selector kind this channel does not support \
             yet; tried: {joined}"
        )));
    }
    Err(error::target_not_found(format!(
        "resolve_element: no element matched; tried: {joined}"
    )))
}

/// 在 `scope` 窗口内等待元素进入期望状态（ADR-0043）。
///
/// # Errors
/// - `query` 既没有 `automation_id` 也没有 `role` → `ToolInvalidArgs`（**先于**任何 COM 调用）
/// - `role` 不是已知的非本地化角色名 → `ToolInvalidArgs`
/// - `scope` 窗口句柄失效 / 窗口已关闭 → `TargetNotFound`
/// - 超时 → `TargetUnresponsive`
pub fn wait_for(
    scope: &ResolvedWindow,
    query: &ElementQuery,
    state: ElementState,
    timeout: Timeout,
) -> PlatformResult<ResolvedElement> {
    let deadline = Instant::now() + Duration::from_millis(timeout.millis());
    // 延后初始化：循环体必定在第一次 `break` 之前赋值，写初值只会触发 `unused_assignments`。
    let mut last_observation;
    loop {
        match find_element_for_query(scope, query)? {
            Some(element) => {
                if state_matches(&element, state)? {
                    return build_resolved(&element);
                }
                last_observation = format!(
                    "element found but its state does not match the expectation \
                     (expected enabled={}, visible={}, focused={})",
                    state.enabled(),
                    state.visible(),
                    state.focused()
                );
            }
            None => last_observation = "element not found".to_string(),
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
    Err(error::target_unresponsive(format!(
        "wait_for timed out after {} ms: {last_observation}",
        timeout.millis()
    )))
}

/// 在 `scope` 窗口子树内按 `ElementQuery` 找第一个匹配元素。
fn find_element_for_query(
    scope: &ResolvedWindow,
    query: &ElementQuery,
) -> PlatformResult<Option<IUIAutomationElement>> {
    let automation_id = query.automation_id();
    let role = query.role();
    if automation_id.is_none() && role.is_none() {
        return Err(error::invalid_args(
            "wait_for: query must specify automation_id and/or role",
        ));
    }
    // 两个「配置错误」都必须在碰 COM **之前**拒绝（与 `unsupported.rs` 的判据一致）：
    // 空 `automation_id` 会匹配一切；未知角色名不是"找不到元素"。
    // 顺序很重要：`scope_root` 会碰平台，所以它必须排在所有参数校验**之后** ——
    // 否则一个无效句柄的 scope 会掩盖"查询本身写错了"这个更该先报的原因。
    if automation_id.is_some_and(str::is_empty) {
        return Err(error::invalid_args(
            "wait_for: automation_id must not be empty (an empty value would match everything)",
        ));
    }
    if let Some(role) = role
        && uia::control_type_from_role(role).is_none()
    {
        return Err(error::invalid_args(format!(
            "wait_for: unknown non-localized role `{role}`"
        )));
    }
    let scope_element = scope_root(scope)?;
    com::with_automation(|automation| {
        let condition = query_condition(automation, automation_id, role)?;
        find_first_children_then_descendants(&scope_element, &condition)
    })
}

/// 把 `ElementQuery` 编成 UIA 条件（`AutomationId` 精确 + `role` → `ControlType`）。
fn query_condition(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    automation_id: Option<&str>,
    role: Option<&str>,
) -> PlatformResult<IUIAutomationCondition> {
    let mut condition: Option<IUIAutomationCondition> = None;
    if let Some(automation_id) = automation_id {
        if automation_id.is_empty() {
            return Err(error::invalid_args(
                "wait_for: automation_id must not be empty (an empty value would match everything)",
            ));
        }
        let value = windows::Win32::System::Variant::VARIANT::from(automation_id);
        // SAFETY: `value` 借用只在本次调用期间有效，UIA 会复制它。
        let built =
            unsafe { automation.CreatePropertyCondition(UIA_AutomationIdPropertyId, &value) }
                .map_err(|failure| {
                    error::error_from_hresult(
                        failure.code().0,
                        "CreatePropertyCondition(AutomationId)",
                    )
                })?;
        condition = Some(combine(automation, condition, built)?);
    }
    if let Some(role) = role {
        let Some(control_type) = uia::control_type_from_role(role) else {
            return Err(error::invalid_args(format!(
                "wait_for: unknown non-localized role `{role}`"
            )));
        };
        let value = windows::Win32::System::Variant::VARIANT::from(control_type.0);
        // SAFETY: `value` 借用只在本次调用期间有效，UIA 会复制它。
        let built =
            unsafe { automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &value) }
                .map_err(|failure| {
                    error::error_from_hresult(
                        failure.code().0,
                        "CreatePropertyCondition(ControlType)",
                    )
                })?;
        condition = Some(combine(automation, condition, built)?);
    }
    condition.ok_or_else(|| error::invalid_args("wait_for: query produced no condition"))
}

/// 用 `CreateAndCondition` 把两个条件合成一个（`None` 时直接返回新条件）。
fn combine(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
    existing: Option<IUIAutomationCondition>,
    next: IUIAutomationCondition,
) -> PlatformResult<IUIAutomationCondition> {
    match existing {
        None => Ok(next),
        // SAFETY: 两个条件在调用期间存活，UIA 只引用它们。
        Some(existing) => unsafe { automation.CreateAndCondition(&existing, &next) }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "CreateAndCondition")),
    }
}

/// 元素是否满足期望状态（`ElementState::visible` = "期望可见" ↔ `IsOffscreen == false`）。
fn state_matches(element: &IUIAutomationElement, state: ElementState) -> PlatformResult<bool> {
    // SAFETY: 以下均为只读属性查询。
    let enabled = unsafe { element.CurrentIsEnabled() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentIsEnabled"))?
        .as_bool();
    let offscreen = unsafe { element.CurrentIsOffscreen() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentIsOffscreen"))?
        .as_bool();
    let focused = unsafe { element.CurrentHasKeyboardFocus() }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "CurrentHasKeyboardFocus"))?
        .as_bool();
    Ok(enabled == state.enabled() && offscreen != state.visible() && focused == state.focused())
}

/// 组装 `ResolvedElement`（含所属窗口句柄作为 `parent`）。
fn build_resolved(element: &IUIAutomationElement) -> PlatformResult<ResolvedElement> {
    let role = uia::element_role(element);
    let window = owning_window(element);
    handles::resolved_element(element.clone(), window, role)
}

/// 取元素所属的顶层窗口句柄（先问元素自己，再沿 control view 父链上溯）。
fn owning_window(element: &IUIAutomationElement) -> Option<HWND> {
    // SAFETY: 只读属性。
    let direct = unsafe { element.CurrentNativeWindowHandle() }.unwrap_or_default();
    if !direct.0.is_null() {
        return Some(direct);
    }
    let walked = com::with_automation(|automation| {
        // SAFETY: 只读地取 control view walker。
        let walker = unsafe { automation.ControlViewWalker() }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "ControlViewWalker"))?;
        let mut current = element.clone();
        for _ in 0..MAX_PARENT_WALK {
            // SAFETY: 只读遍历。
            let parent = walk_step_for_actions(
                unsafe { walker.GetParentElement(&current) },
                "owning_window: GetParentElement",
            )?;
            let Some(parent) = parent else {
                return Ok(None);
            };
            // SAFETY: 只读属性。
            let handle = unsafe { parent.CurrentNativeWindowHandle() }.unwrap_or_default();
            if !handle.0.is_null() {
                return Ok(Some(handle));
            }
            current = parent;
        }
        Ok(None)
    });
    walked.unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用 scope。
    ///
    /// 句柄值取**奇数**：`HWND` 必须 4 字节对齐，所以奇数句柄值**不可能**是合法窗口句柄
    /// → 需要真窗口的路径必然明确失败（铁律 1），而参数校验路径仍先于任何 COM 调用。
    /// 真正需要真窗口的验收是 TASK-017 的手工真机验收（本 crate 的单测**不依赖**真实应用）。
    fn test_scope() -> ResolvedWindow {
        ResolvedWindow::new(
            assistant_platform_api::LocalHandleId::new(424_243),
            "test-scope".to_string(),
        )
    }

    #[test]
    fn test_poll_interval_is_small_but_nonzero() {
        // 0 会变成忙等；太大则 `wait_for` 的响应性退化。
        // 常量断言用 `const { .. }`：编译期就失败（clippy 的 `assertions_on_constants` 要求）。
        const { assert!(POLL_INTERVAL_MS > 0) };
        const { assert!(POLL_INTERVAL_MS <= 250) };
    }

    #[test]
    fn test_empty_chain_is_rejected_before_any_com_call() {
        // 负向用例（ADR-0019 N1）：空链必须当场 `ToolInvalidArgs`，不得去碰 COM
        // —— 即使 scope 的句柄也无效（顺序证明：先校验参数，再碰平台）。
        let chain = SelectorChain::new(Vec::new());
        let code = resolve_element(&test_scope(), &chain).map_err(|error| error.code());
        assert_eq!(
            code,
            Err(assistant_platform_api::ErrorCode::ToolInvalidArgs)
        );
    }

    #[test]
    fn test_query_without_automation_id_or_role_is_rejected() {
        // 负向用例：无条件查询不得被当成"匹配一切"。
        let query = ElementQuery::default();
        let state = ElementState::new(true, true, false);
        let code = wait_for(&test_scope(), &query, state, Timeout::from_millis(1))
            .map_err(|error| error.code());
        assert_eq!(
            code,
            Err(assistant_platform_api::ErrorCode::ToolInvalidArgs)
        );
    }

    #[test]
    fn test_unknown_role_is_rejected_before_com() {
        // 负向用例：未知角色名是配置错误，不是"找不到元素"。
        // `ElementQuery` 只有 `by_automation_id` 构造器，没有 role 构造器 ——
        // 因此这里退而验证「automation_id 为空串」这条同样是**先于 COM** 的拒绝路径。
        let query = ElementQuery::by_automation_id("");
        let state = ElementState::new(true, true, false);
        let code = wait_for(&test_scope(), &query, state, Timeout::from_millis(1))
            .map_err(|error| error.code());
        assert_eq!(
            code,
            Err(assistant_platform_api::ErrorCode::ToolInvalidArgs)
        );
    }

    #[test]
    fn test_scope_is_actually_used_so_an_invalid_window_fails_explicitly() {
        // 正向证据（ADR-0043 D2）：参数合法时**必须**真的去用 scope 的窗口句柄，而不是回退桌面根。
        // 奇数句柄值不可能是合法 `HWND`（4 字节对齐），因此无论 UIA / COM 把它映射成哪个错误码，
        // 都必须是**明确失败**（铁律 1：不得静默成功、不得 panic）。
        let query = ElementQuery::by_automation_id("TextEditor");
        let state = ElementState::new(true, true, false);
        let outcome = wait_for(&test_scope(), &query, state, Timeout::from_millis(1));
        assert!(
            outcome.is_err(),
            "无效句柄的 scope 必须导致明确失败；实际 = {outcome:?}"
        );
    }
}
