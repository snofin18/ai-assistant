//! 窗口域：`WindowProvider` 的 5 个方法（架构 v2 §13.2、ADR-0022 D1/D2）。
//!
//! 职责：窗口枚举 / 按候选链解析 / 状态查询 / 前台化 / 截图占位。
//! 边界：**不做**元素定位（`uia/**`）、**不做**截图（TASK-041，本卡显式报错）、
//! **不做**策略判定（`NeverSteal` / `RequireUserConsent` 一律拒绝，放行点是 TASK-021）。
//!
//! ## 不变量
//! 1. **窗口句柄就是 HWND 的值**（`handles::window_handle_from_hwnd`）：无表、无缓存、无淘汰。
//! 2. **属主 PID 只从 `GetWindowThreadProcessId` 取**，`app_id` 口径 = 映像名叶子（ADR-0022 D1）。
//! 3. 解析链按**有效分降序**逐个尝试；并列第一**报歧义**而不是取第一个（铁律 1，`selector` 模块）。
//! 4. 抢焦点受策略约束：`NeverSteal` / `RequireUserConsent` → `PolicyDenied`
//!    （本层**没有**征求用户同意的能力，那属于策略引擎 TASK-021 / Host TASK-019）。
//!
//! 相关：架构 v2 §13.1.1 / §13.2 / §6.2 / §6.3、ADR-0022 D1/D2/D4、`docs/memory/apps/notepad.md` §2。

mod candidates;

use std::future::{Future, poll_fn};
use std::task::Poll;

use assistant_platform_api::{
    CaptureOptions, ErrorCode, FocusPolicy, ImageRef, PlatformError, PlatformResult,
    ResolvedWindow, SelectorCandidate, SelectorChain, TargetDescriptor, WindowFilter, WindowInfo,
    WindowProvider, WindowState,
};

use crate::error;
use crate::{handles, selector, uia, win32};

use candidates::{CandidateMatch, WindowFacts, contains_ignore_ascii_case, match_window};
use selector::SelectionOutcome;
use win32::WindowRecord;

/// Windows 平台实现的**唯一入口类型**（架构 v2 §13.1.1 的「平台服务」）。
///
/// 为什么是零字段的单元结构体：`IUIAutomation` / `IUIAutomationElement` **不是** `Send` / `Sync`
/// （`windows` 0.62.2 实测），而本类型必须实现 `Send + Sync` 的 trait。因此 COM 对象全部住在
/// **线程本地**（`crate::com` / `crate::handles`），本类型只作为 trait 的载体。
/// 它同时实现 `WindowProvider` 与 `UiAutomationProvider`。
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    /// 构造平台实现（零状态，无副作用，可在任意线程调用）。
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

// 为什么不用 `async fn`：这些函数体没有 `.await`，clippy 的 `unused_async_trait_impl`（nursery，
// 本 workspace 为 warn → `-D warnings` 下即错误）要求去掉 `async`；而本 crate 禁止 `#[allow]`。
// 用 `poll_fn` 而不是 `std::future::ready(..)`：前者把工作留在**第一次 poll**，与 `async fn` 的
// 语义一致 —— 构造 future 不产生副作用，drop 掉未 poll 的 future 不会触发任何 Win32 调用。
impl WindowProvider for WindowsPlatform {
    fn list_windows(
        &self,
        filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        poll_fn(move |_context| Poll::Ready(list_windows(filter)))
    }

    fn resolve_window(
        &self,
        descriptor: &TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        poll_fn(move |_context| Poll::Ready(resolve_window(descriptor)))
    }

    fn window_state(
        &self,
        window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send {
        poll_fn(move |_context| Poll::Ready(window_state(window)))
    }

    fn bring_to_front(
        &self,
        window: &ResolvedWindow,
        policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        poll_fn(move |_context| Poll::Ready(bring_to_front(window, policy)))
    }

    fn capture(
        &self,
        window: &ResolvedWindow,
        options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send {
        poll_fn(move |_context| Poll::Ready(capture(window, *options)))
    }
}

/// 枚举可见顶层窗口并按 `filter` 过滤（架构 v2 §13.1.1 的 `list_windows`）。
///
/// `filter.title_regex` 当前按**大小写不敏感的字面量子串**处理（DRIFT-017-4：本卡无 regex 引擎，
/// 加依赖属漂移触发器 ①）；详见 `crates/platform/windows/README.md`「已知限制」。
fn list_windows(filter: &WindowFilter) -> PlatformResult<Vec<WindowInfo>> {
    let mut listed = Vec::new();
    for hwnd in win32::enumerate_visible_top_level_windows()? {
        // 竞态：窗口在枚举与查询之间关闭 → 它已经不是候选了，跳过不是失败（铁律 1 只禁"静默成功"）。
        let Ok(record) = win32::window_record(hwnd) else {
            continue;
        };
        if let Some(app_id) = filter.app_id()
            && !record.app_id.eq_ignore_ascii_case(app_id)
        {
            continue;
        }
        if let Some(pattern) = filter.title_regex()
            && !contains_ignore_ascii_case(&record.title, pattern)
        {
            continue;
        }
        let id = handles::window_handle_from_hwnd(hwnd)?;
        let label = format!("{} [{}] {}", record.app_id, record.class_name, record.title);
        listed.push(WindowInfo::new(
            ResolvedWindow::new(id, label),
            record.app_id.clone(),
            record.title.clone(),
        ));
    }
    Ok(listed)
}

/// 按 `TargetDescriptor` 的窗口候选链解析出**唯一**窗口（架构 v2 §6.2）。
///
/// 语义要点：
/// - 先 `validate()`（描述自身不合法 → `ToolInvalidArgs`，绝不带着坏输入去枚举）
/// - 候选按有效分降序尝试；**第一个有命中的候选**决定结果
/// - 该候选有多个命中且无法安全择一 → `TargetAmbiguous`（不猜，铁律 1）
/// - 全部候选都没命中 → `TargetNotFound`，message 里带「试过什么」
fn resolve_window(descriptor: &TargetDescriptor) -> PlatformResult<ResolvedWindow> {
    descriptor.validate()?;
    let policy = descriptor.resolution_policy();
    let chain = SelectorChain::new(descriptor.window_candidates().to_vec());
    let ranked = selector::rank_candidates(&chain, policy.min_score_to_try());

    let records = collect_window_records()?;
    let needs_uia = ranked
        .iter()
        .any(|entry| needs_uia_facts(entry.candidate()));
    let facts = collect_window_facts(&records, needs_uia);

    let mut tried: Vec<String> = Vec::new();
    for entry in &ranked {
        let candidate = entry.candidate();
        let mut matches: Vec<&WindowRecord> = Vec::new();
        let mut unsupported: Option<&'static str> = None;
        for (record, facts) in records.iter().zip(facts.iter()) {
            match match_window(facts, candidate) {
                CandidateMatch::Matched => matches.push(record),
                CandidateMatch::NotMatched => {}
                CandidateMatch::Unsupported(reason) => unsupported = Some(reason),
            }
        }
        if let Some(reason) = unsupported {
            tried.push(format!("{} -> unsupported: {reason}", describe(candidate)));
            continue;
        }
        if matches.is_empty() {
            tried.push(format!("{} -> 0 matches", describe(candidate)));
            continue;
        }
        let scores = vec![entry.effective_score(); matches.len()];
        match selector::decide_selection(&scores, policy.on_ambiguous()) {
            SelectionOutcome::Unique => {
                let Some(record) = matches.first() else {
                    // 不可达（matches 非空），但绝不 unwrap（workspace lint 禁止）。
                    return Err(error::target_not_found(
                        "window resolution produced no candidate after a non-empty match set",
                    ));
                };
                let id = handles::window_handle_from_hwnd(record.hwnd)?;
                let label = format!("{} [{}] {}", record.app_id, record.class_name, record.title);
                return Ok(ResolvedWindow::new(id, label));
            }
            SelectionOutcome::Ambiguous { matches: count } => {
                tried.push(format!(
                    "{} -> {count} matches with tied top score",
                    describe(candidate)
                ));
                return Err(selector::resolution_error(
                    descriptor.app_id(),
                    SelectionOutcome::Ambiguous { matches: count },
                    &tried,
                ));
            }
            SelectionOutcome::NotFound => {
                tried.push(format!("{} -> 0 matches", describe(candidate)));
            }
        }
    }
    Err(selector::resolution_error(
        descriptor.app_id(),
        SelectionOutcome::NotFound,
        &tried,
    ))
}

/// 枚举当前可见顶层窗口并采集只读快照（竞态失败的窗口被跳过）。
fn collect_window_records() -> PlatformResult<Vec<WindowRecord>> {
    let handles = win32::enumerate_visible_top_level_windows()?;
    Ok(handles
        .into_iter()
        .filter_map(|hwnd| win32::window_record(hwnd).ok())
        .collect())
}

/// 采集匹配用事实；`with_uia` 为假时不碰 COM（绝大多数候选只用 Win32 事实）。
fn collect_window_facts(records: &[WindowRecord], with_uia: bool) -> Vec<WindowFacts> {
    records
        .iter()
        .map(|record| {
            let mut facts = WindowFacts {
                class_name: record.class_name.clone(),
                title: record.title.clone(),
                // 不需要 UIA 事实时把 `uia_available` 置真：该字段只在 UIA 类候选里被读，
                // 而那种情况下 `with_uia` 必为真（见 `needs_uia_facts`）。
                uia_available: true,
                ..WindowFacts::default()
            };
            if with_uia {
                let (available, automation_id, runtime_id) = uia::window_uia_facts(record.hwnd);
                facts.uia_available = available;
                facts.automation_id = automation_id;
                facts.runtime_id = runtime_id;
            }
            facts
        })
        .collect()
}

/// 该候选是否需要 UIA 事实（只有机器标识类候选需要，标题/类名类不需要）。
const fn needs_uia_facts(candidate: &SelectorCandidate) -> bool {
    matches!(
        candidate.kind(),
        assistant_platform_api::SelectorKind::RuntimeId
            | assistant_platform_api::SelectorKind::AutomationId
    )
}

/// 候选的人类可读描述（进 `TargetNotFound` 的 reasons，使失败可从时间线定位）。
fn describe(candidate: &SelectorCandidate) -> String {
    format!(
        "id={} kind={:?} score={:.3}",
        candidate.id(),
        candidate.kind(),
        candidate.score()
    )
}

/// 查询窗口状态（最小化 / 前台 / 遮挡）。
///
/// # Errors
/// 句柄已失效（窗口已关闭）→ `TargetNotFound`（trait 文档明确要求）。
fn window_state(window: &ResolvedWindow) -> PlatformResult<WindowState> {
    let hwnd = handles::hwnd_from_window_handle(window.id())?;
    if !win32::is_window(hwnd) {
        return Err(error::target_not_found(format!(
            "window `{}` no longer exists",
            window.display_label()
        )));
    }
    Ok(WindowState::new(
        win32::is_minimized(hwnd),
        win32::foreground_hwnd() == Some(hwnd),
        win32::is_occluded(hwnd),
    ))
}

/// 把窗口带到前台（**受策略约束**）。
///
/// # Errors
/// - `NeverSteal` / `RequireUserConsent` → `PolicyDenied`（本层不征求用户同意）
/// - 句柄失效 → `TargetNotFound`
/// - 系统拒绝前台请求（`SetForegroundWindow` 被静默忽略）→ `TargetUnresponsive`
fn bring_to_front(window: &ResolvedWindow, policy: FocusPolicy) -> PlatformResult<()> {
    match policy {
        FocusPolicy::AllowSteal => {}
        FocusPolicy::NeverSteal => {
            return Err(PlatformError::new(
                ErrorCode::PolicyDenied,
                "focus policy is NeverSteal: this layer must not steal focus",
            ));
        }
        FocusPolicy::RequireUserConsent => {
            return Err(PlatformError::new(
                ErrorCode::PolicyDenied,
                "focus policy is RequireUserConsent: consent acquisition belongs to the policy \
                 engine (TASK-021), which this provider layer cannot perform",
            ));
        }
        _ => {
            return Err(PlatformError::new(
                ErrorCode::PolicyDenied,
                "unknown focus policy: refusing to steal focus (fail closed)",
            ));
        }
    }
    let hwnd = handles::hwnd_from_window_handle(window.id())?;
    if !win32::is_window(hwnd) {
        return Err(error::target_not_found(format!(
            "window `{}` no longer exists",
            window.display_label()
        )));
    }
    if win32::bring_to_front(hwnd) {
        return Ok(());
    }
    Err(error::target_unresponsive(format!(
        "SetForegroundWindow did not make `{}` the foreground window (Windows foreground lock)",
        window.display_label()
    )))
}

/// 截取窗口 —— **本卡不实现**。
///
/// 截图 / 脱敏 / 视觉验证按批次表归 TASK-041 / 042；本卡**不**引入截图依赖，
/// 因此这里返回明确的未实现错误（`CapabilityMissing`），而不是 `todo!()` / `unimplemented!()`。
fn capture(_window: &ResolvedWindow, _options: CaptureOptions) -> PlatformResult<ImageRef> {
    // STUB(TASK-041): 截图通道落地后替换本函数（脱敏由 CaptureOptions 决定）。
    Err(error::stub_not_implemented(
        "TASK-041",
        "WindowProvider::capture (screen capture / redaction)",
    ))
}

/// 编译期断言：本类型必须满足两个 trait 的 `Send + Sync` 约束（否则 async 方法无法编译）。
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WindowsPlatform>();
};
