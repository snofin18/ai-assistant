//! TASK-018 的**真机验收**（真实显示器 + 真实记事本）—— 默认 `#[ignore]`，CI 不跑。
//!
//! ## 为什么住在 `src/input/` 而不是 `tests/`
//! 真机验收必须调 Win32（`GetCursorPos` / `SetProcessDpiAwarenessContext`），而本 workspace 把
//! `unsafe_code` 定为 `deny`：**只有本 crate** 在 `src/lib.rs` 做了 crate 级放开。`tests/**` 是
//! **独立 crate**，拿不到那条授权，也不允许用 `#[allow]` 放宽（TASK-018 的 `DoD`）。
//! 因此真机验收住在 lib 的 `#[cfg(all(test, windows))]` 里，并用 `#[ignore]` 保证 CI 不跑。
//!
//! ## 怎么跑（会移动光标、会短暂抢前台焦点，请先保存手头工作）
//!
//! ```text
//! cargo test -p assistant-platform-windows --lib -- --ignored --nocapture
//! ```
//!
//! 按 TASK-018 的 Q3 裁决，真机验收是**人类在本机跑的手工验收**，结果贴进
//! `tasks/TASK-018-platform-windows-synthetic-input-ime.md` §3，**不作为 CI 门禁**。
//!
//! ## 它开出来的记事本会自己收掉
//! 记事本那一条会**开一个真窗口**。Win11 的 `notepad.exe` 只是启动器存根（真正的应用是打包的
//! MSIX `Microsoft.WindowsNotepad`），所以 `Child::kill()` **关不掉窗口**。测试末尾按「启动前快照」
//! 与「标题含本次临时文件名」两条判据，**只关它自己创建的那个窗口**（详见
//! [`close_notepad_windows_created_by_this_test`]；关不掉时会**打印**残留，不静默）。
//!
//! ## 前置条件（架构 v2 §6.9）
//! 坐标通道要求进程声明 **Per-Monitor V2** DPI 感知，否则 Win32 会把坐标**虚拟化**，
//! 「物理像素」这个前提就不成立。测试二进制没有 manifest，因此本模块在运行时显式声明
//! （这正是未来 Host（TASK-019）必须做的事）；声明不了就**显式跳过**并打印原因，**不静默通过**。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assistant_platform_api::{
    ErrorCode, OnAmbiguous, OnNotFound, PlatformError, PlatformResult, ResolutionPolicy,
    ResolvedElement, ResolvedWindow, SelectorCandidate, SelectorChain, SelectorKind, SelectorValue,
    TargetDescriptor, UiAutomationProvider, WindowFilter, WindowProvider,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, DPI_AWARENESS_PER_MONITOR_AWARE,
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW, WM_CLOSE};

use crate::WindowsPlatform;
use crate::coordinates::{MonitorRecord, coordinate_space_for_monitor, enumerate_monitors};
use crate::handles::with_element;

#[path = "acceptance_record.rs"]
mod acceptance_record;
mod cases;

/// 坐标精度判据（架构 v2 §6.9 / 批次表 A2：≤ 2 px）。
const MAXIMUM_PIXEL_ERROR: i32 = 2;

/// 首次校准每台显示器取几个内点（整数分数，避免浮点取整）。
const SAMPLES_PER_DISPLAY: usize = 3;

/// 内点位置（分子 / 分母，相对显示器宽高）：1/4、1/2、3/4 宽 × 1/3 高。
const CALIBRATION_POINT_STEPS: [(i64, i64); SAMPLES_PER_DISPLAY] = [(1, 4), (1, 2), (3, 4)];

/// 垂直方向取显示器高度的 1/3 处：避开顶栏与任务栏。
const CALIBRATION_VERTICAL_STEP: (i64, i64) = (1, 3);

/// 点击后等待焦点切换的上限。
const FOCUS_TIMEOUT: Duration = Duration::from_secs(2);

/// 等待本次启动的记事本窗口出现（含轮询）的上限。
const NOTEPAD_WINDOW_TIMEOUT: Duration = Duration::from_secs(15);

/// 关窗超时：`WM_CLOSE` 之后等窗口消失的上限（超时**打印残留**，不静默通过）。
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

/// 把一条说明写到 stderr。用 `writeln!` 而不是 `eprintln!`：workspace 把
/// `clippy::print_stderr` 定为 deny（AGENTS.md §5.2），而**跳过原因必须可见**（不静默通过）。
fn note(message: std::fmt::Arguments<'_>) {
    let _written = writeln!(std::io::stderr(), "{message}");
}

/// 极简 executor：本 crate 不依赖 async runtime，验收只需要把"同步返回的 future"跑完。
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    use std::task::{Context, Poll, Waker};
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// 把 `PlatformResult` 展开（失败即判该条验收失败，并打印原始原因）。
fn ok<T>(outcome: PlatformResult<T>) -> T {
    match outcome {
        Ok(value) => value,
        Err(failure) => unreachable!("真机验收步骤失败: {failure}"),
    }
}

/// 声明 Per-Monitor V2 DPI 感知（**必须先于任何窗口 / 显示器查询**）。
fn declare_per_monitor_v2() -> PlatformResult<()> {
    if PER_MONITOR_V2_CONFIRMED.get().copied().unwrap_or(false) {
        return Ok(());
    }
    // SAFETY: 只改本进程的 DPI 感知模式，不改任何窗口；必须在查询坐标前调用。
    let attempt =
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if attempt.is_ok() {
        let _already = PER_MONITOR_V2_CONFIRMED.set(true);
        return Ok(());
    }
    // 一个进程只能设置一次 DPI 感知：本模块有多个真机用例，先跑的会占掉这次机会，
    // 之后都会拿到 `ERROR_ACCESS_DENIED`。那**不是**失败 —— 回读当前线程的感知档位即可
    // 确认进程确实已经是 Per-Monitor Aware；档位不对才显式失败（绝不静默通过）。
    match current_dpi_awareness() {
        DPI_AWARENESS_PER_MONITOR_AWARE => {
            let _already = PER_MONITOR_V2_CONFIRMED.set(true);
            Ok(())
        }
        other => Err(PlatformError::new(
            ErrorCode::CapabilityMissing,
            format!(
                "SetProcessDpiAwarenessContext(Per-Monitor V2) failed: {} ; 当前进程的 DPI 感知档位是 \
                 {other:?}（不是 Per-Monitor Aware）—— 物理像素前提不成立",
                attempt
                    .err()
                    .map_or_else(|| "unknown".to_owned(), |failure| failure.to_string())
            ),
        )),
    }
}

/// 本进程是否已确认过 Per-Monitor V2 感知（一个进程只能设置一次 DPI 感知）。
static PER_MONITOR_V2_CONFIRMED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// 回读当前线程的 DPI 感知档位。
fn current_dpi_awareness() -> DPI_AWARENESS {
    // SAFETY: 只读查询；`GetThreadDpiAwarenessContext` 返回当前线程的上下文，不转移所有权。
    let context = unsafe { GetThreadDpiAwarenessContext() };
    // SAFETY: 同上，纯查询。
    unsafe { GetAwarenessFromDpiAwarenessContext(context) }
}

/// 读当前光标位置（物理像素；仅在 Per-Monitor V2 进程里与 `rcMonitor` 同一坐标系）。
fn cursor_position() -> PlatformResult<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: `point` 是本栈帧上的合法 out-参数。
    unsafe { GetCursorPos(&raw mut point) }.map_err(|failure| {
        PlatformError::new(ErrorCode::Fatal, format!("GetCursorPos failed: {failure}"))
    })?;
    Ok((point.x, point.y))
}

/// 取主显示器（拿不到就退回第一台；列表为空则判夹具坏掉）。
fn primary_display(displays: &[MonitorRecord]) -> &MonitorRecord {
    displays
        .iter()
        .find(|display| display.is_primary())
        .or_else(|| displays.first())
        .unwrap_or_else(|| unreachable!("真机验收要求至少 1 台显示器"))
}

/// 启动记事本打开 `path`；失败返回 `None` 并**打印原因**（不静默跳过）。
///
/// **注意**：返回的 `Child` 只是**启动器存根**，不是显示窗口的那个进程（见
/// [`close_notepad_windows_created_by_this_test`]）。`Child::kill()` 因此**不足以**关掉窗口。
fn launch_notepad(path: &Path) -> Option<std::process::Child> {
    match std::process::Command::new("notepad.exe").arg(path).spawn() {
        Ok(child) => Some(child),
        Err(failure) => {
            note(format_args!("SKIP: 启动 notepad.exe 失败（{failure}）"));
            None
        }
    }
}

/// 列举当前桌面上 `notepad.exe` 的窗口（窗口句柄 + 标题）。枚举失败 → `None` 并**打印原因**。
///
/// 用途有两个：① 启动**前**取快照；② 清理时做差集。`None` 一律意味着「本轮不做自动清理」——
/// 宁可在桌面上留下一个记事本并**说明**，也不要在信息不全时去关别人的窗口（铁律 1：不猜）。
fn notepad_windows(platform: WindowsPlatform) -> Option<Vec<(u64, String)>> {
    match block_on(WindowProvider::list_windows(
        &platform,
        &WindowFilter::for_app("notepad.exe"),
    )) {
        Ok(windows) => Some(
            windows
                .into_iter()
                .map(|window| (window.window().id().value(), window.title().to_string()))
                .collect(),
        ),
        Err(failure) => {
            note(format_args!(
                "WARN: 枚举记事本窗口失败（{failure}）—— 本轮不做自动清理"
            ));
            None
        }
    }
}

/// 本测试创建、且**当前仍存在**的记事本窗口句柄 —— 判据：不在 `preexisting` 快照里 **且**
/// 标题含本次独一无二的临时文件名。枚举失败 → `None`（调用方**必须**显式处理，
/// 不得把「查不到」当成「没有残留」）。
fn notepad_windows_created_by_this_test(
    platform: WindowsPlatform,
    expected: &str,
    preexisting: &[(u64, String)],
) -> Option<Vec<u64>> {
    let current = notepad_windows(platform)?;
    Some(
        current
            .into_iter()
            .filter(|(id, title)| {
                !preexisting.iter().any(|(known, _)| known == id) && title.contains(expected)
            })
            .map(|(id, _)| id)
            .collect(),
    )
}

/// `u64` 句柄值 → `HWND`（放不进 `usize` → `None`，**不 panic**）。
fn hwnd_from_handle_value(value: u64) -> Option<HWND> {
    let raw = usize::try_from(value).ok()?;
    Some(HWND(raw as *mut core::ffi::c_void))
}

/// 关掉**本次测试自己打开**的记事本窗口，并轮询确认它真的消失。
///
/// ## 为什么不能只 `Child::kill()`
/// Windows 11 25H2 的记事本是**打包 MSIX 应用**（本机实测 `Microsoft.WindowsNotepad` 11.2607.14.0），
/// `C:\Windows\System32\notepad.exe` 只是启动器存根。2026-09-25 本机实测：
/// 启动一次会创建**两个**进程（存根 + 真正的打包进程），`Child::kill()` **只杀得掉存根** ——
/// 窗口不会关，于是每跑一次真机验收都会在桌面上留下一个记事本（第一轮验收后的现场残留就是这么来的）。
///
/// ## 判据（三条**全部**满足才动那个窗口）
/// ① 属于 `notepad.exe`；② **不在**启动前的快照里（= 本次新建）；③ 标题含本次独一无二的临时文件名。
/// 任何一条不满足都**不碰** —— 用户自己也开着记事本时，他的窗口至少缺两条。
///
/// ## 返回值
/// `true` = 已确认没有残留（含「本来就没有」）；`false` = 有残留或信息不足。
/// 两种失败都**打印原因**，绝不静默通过（铁律 1）。
fn close_notepad_windows_created_by_this_test(
    platform: WindowsPlatform,
    path: &Path,
    preexisting: Option<&[(u64, String)]>,
    timeout: Duration,
) -> bool {
    let Some(preexisting) = preexisting else {
        note(format_args!(
            "WARN: 启动前的记事本窗口快照不可用 —— 跳过自动清理；请手动关掉含 {} 的记事本",
            path.display()
        ));
        return false;
    };
    let expected = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let Some(targets) = notepad_windows_created_by_this_test(platform, &expected, preexisting)
    else {
        note(format_args!(
            "WARN: 清理时无法枚举记事本窗口 —— 请手动关掉含 {expected} 的记事本"
        ));
        return false;
    };
    if targets.is_empty() {
        return true;
    }
    for id in &targets {
        let Some(hwnd) = hwnd_from_handle_value(*id) else {
            continue;
        };
        // SAFETY: `PostMessageW` 只把 `WM_CLOSE` **投递**进目标窗口的消息队列（异步、不阻塞、
        // 不读也不写任何指针）；句柄来自上面刚枚举到的记事本窗口，即本测试自己打开的那一个。
        // 用 `WM_CLOSE`（正常关闭请求）而不是强杀：给目标应用走自己的收尾路径的机会。
        if let Err(failure) = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) } {
            note(format_args!(
                "WARN: 给记事本窗口 {id:#x} 发 WM_CLOSE 失败（{failure}）"
            ));
        }
    }
    let deadline = Instant::now() + timeout;
    loop {
        match notepad_windows_created_by_this_test(platform, &expected, preexisting) {
            Some(remaining) if remaining.is_empty() => return true,
            Some(remaining) => {
                if Instant::now() >= deadline {
                    note(format_args!(
                        "WARN: {} s 内记事本窗口仍未关闭（可能弹了「是否保存」对话框）：{remaining:x?}                          —— 请手动关掉",
                        timeout.as_secs()
                    ));
                    return false;
                }
            }
            None => {
                note(format_args!(
                    "WARN: 清理时无法枚举记事本窗口 —— 请手动关掉含 {expected} 的记事本"
                ));
                return false;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 轮询解析「本次启动的那个」记事本窗口（标题含唯一文件名）。
fn resolve_notepad_window(platform: WindowsPlatform, path: &Path) -> Option<ResolvedWindow> {
    let expected_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())?;
    let candidate = match SelectorCandidate::new(
        "notepad-window-title",
        SelectorKind::TitleRegex,
        SelectorValue::Text(expected_name),
        1.0,
        false,
    ) {
        Ok(candidate) => candidate,
        Err(failure) => {
            note(format_args!("SKIP: 构造窗口候选失败（{failure}）"));
            return None;
        }
    };
    let policy = match ResolutionPolicy::new(
        OnAmbiguous::ErrorAndAsk,
        OnNotFound::new(vec![200, 500, 1_000], false),
        5_000,
        0.1,
    ) {
        Ok(policy) => policy,
        Err(failure) => {
            note(format_args!("SKIP: 构造解析策略失败（{failure}）"));
            return None;
        }
    };
    let descriptor = TargetDescriptor::new(
        "2.0".to_owned(),
        "com.microsoft.notepad".to_owned(),
        vec![candidate],
        Vec::new(),
        policy,
    );
    let window = resolve_window_with_timeout(platform, &descriptor, NOTEPAD_WINDOW_TIMEOUT);
    if window.is_none() {
        note(format_args!(
            "SKIP: {NOTEPAD_WINDOW_TIMEOUT:?} 内没有解析到本次记事本窗口（标题需含唯一文件名）"
        ));
    }
    window
}

/// 按适配包 `com.microsoft.notepad` 的主选择器解析编辑器元素（class + role，非本地化）。
fn resolve_notepad_editor(
    platform: WindowsPlatform,
    window: &ResolvedWindow,
) -> Option<ResolvedElement> {
    let candidate = match SelectorCandidate::new(
        "editor-modern",
        SelectorKind::ClassAndRole,
        SelectorValue::ClassAndRole {
            class: "RichEditD2DPT".to_owned(),
            role: "Document".to_owned(),
        },
        1.0,
        false,
    ) {
        Ok(candidate) => candidate,
        Err(failure) => {
            note(format_args!("SKIP: 构造编辑器候选失败（{failure}）"));
            return None;
        }
    };
    let chain = SelectorChain::new(vec![candidate]);
    match block_on(UiAutomationProvider::resolve_element(
        &platform, window, &chain,
    )) {
        Ok(element) => Some(element),
        Err(failure) => {
            note(format_args!(
                "SKIP: 解析记事本编辑器元素失败（{failure}）—— 本机 Notepad 版本可能换了 class"
            ));
            None
        }
    }
}

/// 读元素屏幕矩形的中心（物理像素）。
fn editor_center(editor: &ResolvedElement) -> Option<(i32, i32)> {
    let rectangle = match element_bounding_rectangle(editor) {
        Ok(rectangle) => rectangle,
        Err(failure) => {
            note(format_args!("SKIP: 读元素矩形失败（{failure}）"));
            return None;
        }
    };
    Some((
        rectangle.left + (rectangle.right - rectangle.left) / 2,
        rectangle.top + (rectangle.bottom - rectangle.top) / 2,
    ))
}

/// 关窗 + reap + 删临时文件（本测试所有跳过/结束路径共用），返回「确认无残留」。
fn cleanup_notepad_probe(
    platform: WindowsPlatform,
    path: &Path,
    preexisting: Option<&[(u64, String)]>,
    child: &mut std::process::Child,
) -> bool {
    let cleaned =
        close_notepad_windows_created_by_this_test(platform, path, preexisting, CLEANUP_TIMEOUT);
    let launcher_reaped = reap_launcher_stub(child);
    remove_quietly(path);
    note(format_args!(
        "notepad_probe: 窗口清理完成（无残留） = {cleaned}; launcher_reaped = {launcher_reaped}"
    ));
    cleaned
}

fn reap_launcher_stub(child: &mut std::process::Child) -> bool {
    match child.try_wait() {
        Ok(Some(_)) => return true,
        Ok(None) => {}
        Err(failure) => {
            note(format_args!(
                "WARN: try_wait 查询 notepad.exe 启动器失败（{failure}），继续尝试终止"
            ));
        }
    }
    if let Err(failure) = child.kill() {
        note(format_args!(
            "WARN: 终止 notepad.exe 启动器失败（{failure}），继续 wait reap"
        ));
    }
    match child.wait() {
        Ok(status) => {
            note(format_args!("notepad: 启动器已 reap（status={status}）"));
            true
        }
        Err(failure) => {
            note(format_args!(
                "WARN: reap notepad.exe 启动器失败（{failure}）"
            ));
            false
        }
    }
}

/// 轮询确认元素已取得键盘焦点。
fn wait_for_keyboard_focus(element: &ResolvedElement, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ok(element_has_keyboard_focus(element)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}
/// 读元素的屏幕矩形（UIA 只读属性，物理像素）。
fn element_bounding_rectangle(element: &ResolvedElement) -> PlatformResult<RECT> {
    with_element(element.id(), |automation_element| {
        // SAFETY: `automation_element` 是本线程元素表里持有的 COM 元素；这里只做只读属性
        // 查询，不转移所有权、不释放引用。
        unsafe { automation_element.CurrentBoundingRectangle() }.map_err(|failure| {
            crate::error::error_from_hresult(failure.code().0, "CurrentBoundingRectangle")
        })
    })
}

/// 读元素当前的键盘焦点状态（UIA 只读属性）。
fn element_has_keyboard_focus(element: &ResolvedElement) -> PlatformResult<bool> {
    with_element(element.id(), |automation_element| {
        // SAFETY: 同上，只读属性查询。
        unsafe { automation_element.CurrentHasKeyboardFocus() }
            .map(bool::from)
            .map_err(|failure| {
                crate::error::error_from_hresult(failure.code().0, "CurrentHasKeyboardFocus")
            })
    })
}

/// 找出包含该物理点的显示器（用于把物理中心换成该显示器缩放下的逻辑点）。
fn display_containing(displays: &[MonitorRecord], x: i32, y: i32) -> Option<&MonitorRecord> {
    displays.iter().find(|display| {
        x >= display.left() && x < display.right() && y >= display.top() && y < display.bottom()
    })
}

/// 轮询解析目标窗口：记事本从启动到窗口出现之间有延迟，单次调用会假阴性。
///
/// 超时返回 `None`（调用方**必须**打印原因并清理自己启动的进程，绝不静默通过）。
fn resolve_window_with_timeout(
    platform: WindowsPlatform,
    descriptor: &TargetDescriptor,
    timeout: Duration,
) -> Option<ResolvedWindow> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(window) = block_on(WindowProvider::resolve_window(&platform, descriptor)) {
            return Some(window);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// 临时文件路径（在系统临时目录下，名字带进程号避免并发冲突）。
fn temp_file_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("aia-task018-{}.txt", std::process::id()));
    path
}

/// 删除临时文件（删不掉也要说明，**不静默**）。
fn remove_quietly(path: &Path) {
    if let Err(failure) = std::fs::remove_file(path) {
        note(format_args!(
            "WARN: 清理 {} 失败: {failure}",
            path.display()
        ));
    }
}

/// 轮询等记事本窗口出现（标题里含文件名）。
fn wait_for_notepad_window(
    platform: WindowsPlatform,
    path: &Path,
    timeout: Duration,
) -> Option<ResolvedWindow> {
    let expected = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(windows) = block_on(WindowProvider::list_windows(
            &platform,
            &WindowFilter::for_app("notepad.exe"),
        )) {
            let found = windows
                .into_iter()
                .find(|window| window.title().contains(&expected));
            if let Some(window) = found {
                return Some(window.window().clone());
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    None
}

/// 轮询等磁盘内容里出现标记（Ctrl+S 是异步落盘，不能立刻断言）。
fn wait_for_file_to_contain(path: &Path, marker: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(bytes) = std::fs::read(path)
            && String::from_utf8_lossy(&bytes).contains(marker)
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}
