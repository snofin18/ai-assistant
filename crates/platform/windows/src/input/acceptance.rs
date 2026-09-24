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
    ErrorCode, FocusPolicy, KeyChord, KeyModifier, KeyTarget, NormalizedPoint, PlatformError,
    PlatformResult, PointerAction, ResolvedWindow, UiAutomationProvider, WindowFilter,
    WindowProvider,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW, WM_CLOSE};

use super::{is_ime_open, send_unicode_text};
use crate::WindowsPlatform;
use crate::coordinates::{MonitorRecord, coordinate_space_for_monitor, enumerate_monitors};

/// 坐标精度判据（架构 v2 §6.9 / 批次表 A2：≤ 2 px）。
const MAXIMUM_PIXEL_ERROR: i32 = 2;

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
    // SAFETY: 只改本进程的 DPI 感知模式，不改任何窗口；必须在查询坐标前调用。
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }.map_err(
        |failure| {
            PlatformError::new(
                ErrorCode::CapabilityMissing,
                format!("SetProcessDpiAwarenessContext(Per-Monitor V2) failed: {failure}"),
            )
        },
    )
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

/// 真机验收 1：`pointer_action(Move)` 必须把光标放到**请求的物理像素**（误差 ≤ 2 px）。
///
/// 这一条覆盖整条坐标链路：逻辑点 → `coordinate_space_for_logical_point`（显示器归属）
/// → `to_physical`（DPI 换算）→ `VirtualScreen::normalize`（0..=65535）→ `SendInput`
/// → 系统把光标放到该物理像素。
#[test]
#[ignore = "真机验收：需要真实桌面，默认不跑（见本文件头）"]
fn test_pointer_move_lands_on_the_requested_physical_point() {
    if let Err(failure) = declare_per_monitor_v2() {
        note(format_args!(
            "SKIP test_pointer_move_lands_on_the_requested_physical_point: \
             无法声明 Per-Monitor V2（{failure}）—— 坐标会被系统虚拟化，精度断言不成立"
        ));
        return;
    }
    let displays = ok(enumerate_monitors());
    let display = primary_display(&displays);
    let space = ok(coordinate_space_for_monitor(display));
    let scale = space.scale();
    // 取显示器内 1/3 处的点：避开边缘吸附与任务栏区域。
    let width = i64::from(display.right()) - i64::from(display.left());
    let height = i64::from(display.bottom()) - i64::from(display.top());
    let target_x = i32::try_from(i64::from(display.left()) + (width / 3))
        .unwrap_or_else(|_| unreachable!("显示器宽度必须落在 i32 内"));
    let target_y = i32::try_from(i64::from(display.top()) + (height / 3))
        .unwrap_or_else(|_| unreachable!("显示器高度必须落在 i32 内"));
    // 合成输入的入口是**逻辑**坐标：物理点 → 逻辑点（除以该显示器的缩放）。
    let logical = ok(NormalizedPoint::new(
        f64::from(target_x) / scale,
        f64::from(target_y) / scale,
    ));
    let platform = WindowsPlatform::new();
    ok(block_on(UiAutomationProvider::pointer_action(
        &platform,
        logical,
        &PointerAction::Move,
    )));
    let (cursor_x, cursor_y) = ok(cursor_position());
    let error_x = (cursor_x - target_x).abs();
    let error_y = (cursor_y - target_y).abs();
    note(format_args!(
        "pointer_move: display={} scale={scale} target=({target_x}, {target_y}) \
         cursor=({cursor_x}, {cursor_y}) error=({error_x}, {error_y}) px",
        display.device_name()
    ));
    assert!(
        error_x <= MAXIMUM_PIXEL_ERROR && error_y <= MAXIMUM_PIXEL_ERROR,
        "坐标精度超差：目标 ({target_x}, {target_y})，实际 ({cursor_x}, {cursor_y})，\
         判据 ≤ {MAXIMUM_PIXEL_ERROR} px"
    );
}

/// 真机验收 2：**真实记事本**里
/// ① `send_unicode_text` 写入中文（证明 `KEYEVENTF_UNICODE` 绕过 IME / 键盘布局）；
/// ② `key_action(Ctrl+S)` 保存（证明 VK 路径 + 前台校验 + 焦点链路）；
/// ③ 从**磁盘**读回内容 —— 后置条件由文件内容证明，不是"看起来成功"。
#[test]
#[ignore = "真机验收：需要真实记事本，默认不跑（见本文件头）"]
fn test_unicode_text_and_ctrl_s_round_trip_through_real_notepad() {
    let path = temp_file_path();
    if let Err(failure) = std::fs::write(&path, "start\n") {
        note(format_args!(
            "SKIP: 写临时文件失败 {}: {failure}",
            path.display()
        ));
        return;
    }

    let platform = WindowsPlatform::new();
    // 启动**前**的快照：清理时只动「快照里没有」的窗口 —— 这样即使用户自己也开着记事本，
    // 也不会碰到他的窗口（见 `close_notepad_windows_created_by_this_test` 的三条判据）。
    let preexisting = notepad_windows(platform);

    let Some(mut child) = launch_notepad(&path) else {
        remove_quietly(&path);
        return;
    };

    let Some(window) = wait_for_notepad_window(platform, &path, Duration::from_secs(15)) else {
        note(format_args!(
            "SKIP: 15 s 内没有等到记事本窗口（标题里应含 {}）",
            path.display()
        ));
        let cleaned = close_notepad_windows_created_by_this_test(
            platform,
            &path,
            preexisting.as_deref(),
            CLEANUP_TIMEOUT,
        );
        let _killed = child.kill();
        remove_quietly(&path);
        if !cleaned {
            note(format_args!("SKIP 之后仍有记事本窗口残留（见上面的 WARN）"));
        }
        return;
    };

    // 目标必须**真的**在前台：`bring_to_front` 内部已经回读过 `GetForegroundWindow`。
    ok(block_on(WindowProvider::bring_to_front(
        &platform,
        &window,
        FocusPolicy::AllowSteal,
    )));

    let raw = usize::try_from(window.id().value())
        .unwrap_or_else(|_| unreachable!("窗口句柄必须能放进 usize"));
    let hwnd = HWND(raw as *mut core::ffi::c_void);

    // `is_ime_open` 的契约（见 `input/win32.rs` 文档）：能查到就返回状态；目标没有 Win32 IME
    // 上下文（UWP / WinUI 走 TSF）就报 `CapabilityMissing`，**绝不猜一个 false**（铁律 1）。
    // 因此这里对两种结果都接受并打印实际走了哪条分支，只把**契约外**的错误当作真缺陷。
    match is_ime_open(hwnd) {
        Ok(ime_open) => note(format_args!("notepad: is_ime_open = {ime_open}")),
        Err(failure) if failure.code() == ErrorCode::CapabilityMissing => note(format_args!(
            "notepad: is_ime_open 不适用（CapabilityMissing: {}）—— WinUI 目标走 TSF，符合契约",
            failure.message()
        )),
        Err(failure) => unreachable!("is_ime_open 返回了契约外的错误: {failure}"),
    }

    let marker = "中文abc";
    ok(send_unicode_text(hwnd, marker));

    let save_chord = KeyChord::new("s".to_string(), vec![KeyModifier::Control]);
    ok(block_on(UiAutomationProvider::key_action(
        &platform,
        &save_chord,
        &KeyTarget::Window(window),
    )));

    let saved = wait_for_file_to_contain(&path, marker, Duration::from_secs(10));
    // 清理顺序 = **先关窗口、再杀存根**：`Child::kill()` 只杀得掉启动器存根，
    // 显示窗口的是另一个进程（见 `close_notepad_windows_created_by_this_test`）。
    let cleaned = close_notepad_windows_created_by_this_test(
        platform,
        &path,
        preexisting.as_deref(),
        CLEANUP_TIMEOUT,
    );
    let _killed = child.kill();
    let content = match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(failure) => unreachable!("读回临时文件失败: {failure}"),
    };
    remove_quietly(&path);
    note(format_args!("notepad: disk content = {content:?}"));
    note(format_args!("notepad: 窗口清理完成（无残留） = {cleaned}"));
    assert!(
        saved && content.contains(marker),
        "磁盘内容里没有 `{marker}`（Ctrl+S 或 Unicode 写入没有生效）: {content:?}"
    );
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
