//! 窗口域：`WindowProvider` 的 5 个方法（架构 v2 §13.2、ADR-0022 D1/D2）。
//!
//! 职责：窗口枚举 / 按候选链解析 / 状态查询 / 前台化 / 单窗口截图。
//! 边界：**不做**元素定位（`uia/**`）、**不做**全屏截图、**不做**视觉验证（TASK-042）、
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
mod capture;

use std::future::{Future, poll_fn};
use std::task::Poll;

use assistant_platform_api::{
    CaptureOptions, ErrorCode, FocusPolicy, ImageBlobSink, ImageRef, PlatformError, PlatformResult,
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
/// 为什么不持 COM 状态：`IUIAutomation` / `IUIAutomationElement` **不是** `Send` / `Sync`
/// （`windows` 0.62.2 实测），而本类型必须实现 `Send + Sync` 的 trait。因此 COM 对象全部住在
/// **线程本地**（`crate::com` / `crate::handles`），本类型只作为 trait 的载体 + 一个可选注入点。
///
/// 唯一持有的状态是**截图 blob 写入端**（ADR-0076）：平台层产生 BGRA 与内容地址后，把落盘
/// 委托给它，从而既不依赖 `crates/storage`、也不私开写库连接（铁律 7）。未注入时 `capture`
/// 显式失败，而不是返回一个谎称已持久化的 `ImageRef`。
/// 它同时实现 `WindowProvider` 与 `UiAutomationProvider`。
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsPlatform {
    /// 进程级写入端（`&'static`，见 [`Self::with_blob_sink`]）。用静态引用而非 `Arc`，
    /// 是为了让本类型继续 `Copy` —— 它被大量按值传递的测试辅助函数依赖。
    blob_sink: Option<&'static dyn ImageBlobSink>,
}

impl WindowsPlatform {
    /// 构造平台实现（无 COM 状态、无副作用，可在任意线程调用）。
    ///
    /// 未注入 blob 写入端时 `capture` 会显式失败（见 [`Self::with_blob_sink`]）。
    #[must_use]
    pub const fn new() -> Self {
        Self { blob_sink: None }
    }

    /// 注入截图 blob 写入端（ADR-0076）。
    ///
    /// binary 装配层在启动时构造**进程级单例**写入端并把它的 `&'static` 引用注入进来
    /// （单例在进程生命周期内有界，符合 ADR-0063）；平台层只调用 trait，不知道 storage 的存在。
    #[must_use]
    pub const fn with_blob_sink(mut self, sink: &'static dyn ImageBlobSink) -> Self {
        self.blob_sink = Some(sink);
        self
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

    fn window_geometry(
        &self,
        window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<assistant_platform_api::WindowGeometry>> + Send {
        poll_fn(move |_context| Poll::Ready(window_geometry(window)))
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
        let blob_sink = self.blob_sink;
        poll_fn(move |_context| Poll::Ready(capture(window, *options, blob_sink)))
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
    let chain = SelectorChain::new(descriptor.window_candidates().to_vec())
        .with_allow_fallback_entry(descriptor.allow_fallback_entry());
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

/// 查询窗口状态（最小化 / 前台 / 遮挡或阻塞弹层）。
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
        // 遮挡判定除中心点采样外，还要纳入目标线程上的 WinUI / 经典弹层：
        // Paint 的未保存三态框是 `Microsoft.UI.Content.PopupWindowSiteBridge`，
        // 它不对 UIA 暴露，也不满足 `IsWindowVisible`，但会挡住后续交互。
        win32::is_occluded(hwnd) || win32::has_blocking_popup(hwnd),
    ))
}

/// 窗口几何视图：设备像素矩形 + 它所属的坐标空间（ADR-0092 细则 3 / ADR-0093 细则 3）。
///
/// 归属查询走 `MonitorFromWindow`（**不枚举显示器**）—— 这是 ADR-0092 细则 5 的开销纪律在
/// Windows 上的落地方式：单次 O(1) 归属 + `GetDpiForMonitor`，与「枚举全部显示器再逐个比对」
/// 相比省掉了整张显示器列表的构造与分配。
///
/// # Errors
/// 句柄已失效或窗口已关闭 → `TargetNotFound`；最小化 / 矩形不可读 / 零尺寸 → `TargetUnresponsive`；
/// `GetMonitorInfoW` / `GetDpiForMonitor` 失败 → 由 `coordinates` 侧映射（`Fatal`，不用 96 兜底）。
fn window_geometry(
    window: &ResolvedWindow,
) -> PlatformResult<assistant_platform_api::WindowGeometry> {
    let hwnd = handles::hwnd_from_window_handle(window.id())?;
    if !win32::is_window(hwnd) {
        return Err(error::target_not_found(format!(
            "window `{}` no longer exists",
            window.display_label()
        )));
    }
    // 判定规则**只在 seam 里写一次**；这里只负责把 Win32 的事实取出来。
    let minimized = win32::is_minimized(hwnd);
    let rect = win32::window_rect(hwnd).map(|rect| (rect.left, rect.top, rect.right, rect.bottom));
    let monitor = crate::coordinates::monitor_for_window(hwnd)?;
    window_geometry_from_parts(window.display_label(), minimized, rect, &monitor)
}

/// **纯函数 seam**：把「窗口事实 + 显示器记录」变成几何视图（不碰 Win32，因此可单测）。
///
/// 为什么单独抽出来：三个负向分支（最小化 / 矩形不可读 / 零尺寸）若必须靠真实 HWND 才能验，
/// 就只能写成真机 `#[ignore]` 用例 —— 也就是**平时 CI 跑不到**。抽成纯函数后它们进了
/// `cargo test` 的常规路径，坏掉会真的红。
///
/// # Errors
/// - `minimized` → `TargetUnresponsive`（最小化窗口的矩形在 Win32 里"有效但不可用"，常在 -32000 附近）；
/// - `rect` 为 `None` → `TargetUnresponsive`（矩形不可读）；
/// - 矩形非正面积 → `TargetUnresponsive`（零尺寸 / 退化矩形）；
/// - 显示器记录不可用（设备名为空、DPI 非法）→ 由 `coordinate_space_for_monitor` 显式失败，
///   **不**退回主屏、**不**用 96 兜底。
fn window_geometry_from_parts(
    display_label: &str,
    minimized: bool,
    rect: Option<(i32, i32, i32, i32)>,
    monitor: &crate::coordinates::MonitorRecord,
) -> PlatformResult<assistant_platform_api::WindowGeometry> {
    if minimized {
        return Err(error::target_unresponsive(format!(
            "window `{display_label}` is minimized; its rectangle is not usable for synthetic input"
        )));
    }
    let Some((left, top, right, bottom)) = rect else {
        return Err(error::target_unresponsive(format!(
            "window `{display_label}` has no readable rectangle"
        )));
    };
    if right <= left || bottom <= top {
        return Err(error::target_unresponsive(format!(
            "window `{display_label}` has a degenerate rectangle ({left}, {top})-({right}, {bottom})"
        )));
    }
    let bounds = assistant_platform_api::ElementBounds::new(left, top, right, bottom)?;
    let space = crate::coordinates::coordinate_space_for_monitor(monitor)?;
    Ok(assistant_platform_api::WindowGeometry::new(bounds, space))
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

/// 截取一个已解析窗口（真实平台路径在 `capture` 子模块）。
///
/// 不接收屏幕坐标 / 全屏参数；只接受调用方已经解析出的 `ResolvedWindow`。
fn capture(
    window: &ResolvedWindow,
    options: CaptureOptions,
    blob_sink: Option<&dyn ImageBlobSink>,
) -> PlatformResult<ImageRef> {
    capture::capture_window(window, options, blob_sink)
}

/// 编译期断言：本类型必须满足两个 trait 的 `Send + Sync` 约束（否则 async 方法无法编译）。
const _: () = {
    // STUB(TASK-282): compile-time assertion function intentionally has no runtime body.
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WindowsPlatform>();
};

/// 几何视图的判定规则单测。
///
/// 刻意放在文件**末尾**：`clippy::items_after_test_module` 要求测试模块之后不能再有 item，
/// 所以测试模块必须是本文件的最后一个 item。
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod window_geometry_tests {
    use super::*;
    use crate::coordinates::MonitorRecord;
    use assistant_platform_api::CoordinateSpaceKind;

    /// 一台**合法**显示器（设备名非空、DPI 96 = 100%）。
    fn primary_monitor() -> MonitorRecord {
        MonitorRecord::new(r"\\.\DISPLAY1".to_string(), 0, 0, 1920, 1080, 96)
    }

    #[test]
    fn test_window_geometry_rejects_minimized_window() {
        let error =
            window_geometry_from_parts("w", true, Some((0, 0, 100, 100)), &primary_monitor())
                .expect_err("minimized window must fail closed");
        assert!(error.message().contains("minimized"), "{}", error.message());
    }

    #[test]
    fn test_window_geometry_rejects_unreadable_rectangle() {
        let error = window_geometry_from_parts("w", false, None, &primary_monitor())
            .expect_err("missing rectangle must fail closed");
        assert!(
            error.message().contains("no readable rectangle"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn test_window_geometry_rejects_degenerate_rectangle() {
        // 零宽度与零高度各一次：右 / 下边界都是**开区间**，相等即退化。
        for rect in [(10, 10, 10, 40), (10, 10, 40, 10)] {
            let error = window_geometry_from_parts("w", false, Some(rect), &primary_monitor())
                .expect_err("degenerate rectangle must fail closed");
            assert!(
                error.message().contains("degenerate rectangle"),
                "{}",
                error.message()
            );
        }
    }

    #[test]
    fn test_window_geometry_rejects_empty_display_name() {
        // 「未知显示器」的纯函数等价物：设备名空 → CoordinateSpace::new 拒绝，
        // 绝不允许退化成"没有归属也能点"。
        let unknown = MonitorRecord::new(String::new(), 0, 0, 1920, 1080, 96);
        assert!(
            window_geometry_from_parts("w", false, Some((10, 10, 40, 40)), &unknown).is_err(),
            "an empty display name must not produce a usable coordinate space"
        );
    }

    #[test]
    fn test_window_geometry_reports_device_pixel_bounds_and_physical_space() {
        let geometry =
            window_geometry_from_parts("w", false, Some((10, 20, 110, 220)), &primary_monitor())
                .expect("a readable rectangle on a real display must succeed");
        assert_eq!(geometry.bounds().left(), 10);
        assert_eq!(geometry.bounds().top(), 20);
        assert_eq!(geometry.bounds().right(), 110);
        assert_eq!(geometry.bounds().bottom(), 220);
        assert_eq!(geometry.space().kind(), CoordinateSpaceKind::PhysicalPixels);
        assert_eq!(geometry.space().origin_display(), r"\\.\DISPLAY1");
        assert!((geometry.space().scale() - 1.0).abs() < f64::EPSILON);
    }
}
