//! TASK-228 真机验收用例与用例级 helper。
//!
//! 职责：四个 `#[ignore]` 真机用例调用 `acceptance_record::run_case` 固化最终状态。
//! 边界：只放用例及其专用 helper；通用窗口/DPI/清理夹具仍在父模块 `acceptance.rs`。

use std::path::Path;
use std::time::Duration;

use assistant_platform_api::{
    ErrorCode, FocusPolicy, KeyChord, KeyModifier, KeyTarget, NormalizedPoint, PointerAction,
    UiAutomationProvider,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use super::super::calibration::{
    CalibrationSample, MAXIMUM_CALIBRATION_SAMPLES, MINIMUM_CALIBRATION_SAMPLES,
    calibrate_pointer_samples,
};
use super::super::{is_ime_open, send_unicode_text};
use super::acceptance_record::{
    CaseOutcome, MeasurementValue, POINTER_CALIBRATION_CASE, POINTER_CLICK_CASE, POINTER_MOVE_CASE,
    UNICODE_NOTEPAD_CASE, run_case,
};
use super::*;

/// 真机验收 1：`pointer_action(Move)` 必须把光标放到**请求的物理像素**（误差 ≤ 2 px）。
///
/// 这一条覆盖整条坐标链路：逻辑点 + 显式 `CoordinateSpace`（ADR-0067）
/// → `to_physical`（DPI 换算）→ `VirtualScreen::normalize`（0..=65535）→ `SendInput`
/// → 系统把光标放到该物理像素。
#[test]
#[ignore = "真机验收：需要真实桌面，默认不跑（见本文件头；TASK-228 记录结构化结果）"]
fn test_pointer_move_lands_on_the_requested_physical_point() {
    run_case(POINTER_MOVE_CASE, || {
        if let Err(failure) = declare_per_monitor_v2() {
            let reason = format!(
                "无法声明 Per-Monitor V2（{failure}）—— 坐标会被系统虚拟化，精度断言不成立"
            );
            note(format_args!("SKIP {POINTER_MOVE_CASE}: {reason}"));
            return CaseOutcome::skip(reason, Vec::new());
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
            &space,
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
        CaseOutcome::pass(vec![
            (
                "display",
                MeasurementValue::text(display.device_name().to_owned()),
            ),
            ("scale_factor", MeasurementValue::number(scale)),
            ("target_x", MeasurementValue::number(f64::from(target_x))),
            ("target_y", MeasurementValue::number(f64::from(target_y))),
            ("cursor_x", MeasurementValue::number(f64::from(cursor_x))),
            ("cursor_y", MeasurementValue::number(f64::from(cursor_y))),
            ("error_x", MeasurementValue::number(f64::from(error_x))),
            ("error_y", MeasurementValue::number(f64::from(error_y))),
        ])
    });
}

/// 真机验收 2：**真实记事本**里
/// ① `send_unicode_text` 写入中文（证明 `KEYEVENTF_UNICODE` 绕过 IME / 键盘布局）；
/// ② `key_action(Ctrl+S)` 保存（证明 VK 路径 + 前台校验 + 焦点链路）；
/// ③ 从**磁盘**读回内容 —— 后置条件由文件内容证明，不是"看起来成功"。
#[test]
#[ignore = "真机验收：需要真实记事本，默认不跑（见本文件头；TASK-228 记录结构化结果）"]
fn test_unicode_text_and_ctrl_s_round_trip_through_real_notepad() {
    run_case(UNICODE_NOTEPAD_CASE, run_unicode_notepad_case);
}

fn run_unicode_notepad_case() -> CaseOutcome {
    let path = temp_file_path();
    if let Err(failure) = std::fs::write(&path, "start\n") {
        let reason = format!("写临时文件失败 {}: {failure}", path.display());
        note(format_args!("SKIP {UNICODE_NOTEPAD_CASE}: {reason}"));
        return CaseOutcome::skip(reason, Vec::new());
    }

    let platform = WindowsPlatform::new();
    // 启动**前**的快照：清理时只动「快照里没有」的窗口 —— 这样即使用户自己也开着记事本，
    // 也不会碰到他的窗口（见 `close_notepad_windows_created_by_this_test` 的三条判据）。
    let preexisting = notepad_windows(platform);
    let Some(mut child) = launch_notepad(&path) else {
        remove_quietly(&path);
        return CaseOutcome::skip("启动 notepad.exe 失败", Vec::new());
    };
    let Some(window) = wait_for_notepad_window(platform, &path, Duration::from_secs(15)) else {
        return skip_after_missing_notepad_window(
            platform,
            &path,
            preexisting.as_deref(),
            &mut child,
        );
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
    let ime_is_applicable = ime_is_applicable(hwnd);

    let marker = "中文abc";
    ok(send_unicode_text(hwnd, marker));
    let save_chord = KeyChord::new("s".to_string(), vec![KeyModifier::Control]);
    ok(block_on(UiAutomationProvider::key_action(
        &platform,
        &save_chord,
        &KeyTarget::Window(window),
    )));

    let saved = wait_for_file_to_contain(&path, marker, Duration::from_secs(10));
    finish_unicode_notepad_case(
        platform,
        &path,
        preexisting.as_deref(),
        &mut child,
        saved,
        ime_is_applicable,
    )
}

fn finish_unicode_notepad_case(
    platform: WindowsPlatform,
    path: &Path,
    preexisting: Option<&[(u64, String)]>,
    child: &mut std::process::Child,
    saved: bool,
    ime_is_applicable: bool,
) -> CaseOutcome {
    let marker = "中文abc";
    // 清理顺序 = **先关窗口、再杀存根**：`Child::kill()` 只杀得掉启动器存根，
    // 显示窗口的是另一个进程（见 `close_notepad_windows_created_by_this_test`）。
    let cleaned =
        close_notepad_windows_created_by_this_test(platform, path, preexisting, CLEANUP_TIMEOUT);
    let launcher_reaped = reap_launcher_stub(child);
    let content = match std::fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(failure) => unreachable!("读回临时文件失败: {failure}"),
    };
    remove_quietly(path);
    note(format_args!("notepad: disk content = {content:?}"));
    note(format_args!("notepad: 窗口清理完成（无残留） = {cleaned}"));
    assert!(
        saved && content.contains(marker),
        "磁盘内容里没有 `{marker}`（Ctrl+S 或 Unicode 写入没有生效）: {content:?}"
    );
    CaseOutcome::pass(vec![
        ("saved", MeasurementValue::boolean(saved)),
        (
            "disk_content_contains_marker",
            MeasurementValue::boolean(content.contains(marker)),
        ),
        ("cleanup_ok", MeasurementValue::boolean(cleaned)),
        (
            "launcher_reaped",
            MeasurementValue::boolean(launcher_reaped),
        ),
        (
            "ime_is_applicable",
            MeasurementValue::boolean(ime_is_applicable),
        ),
    ])
}

fn skip_after_missing_notepad_window(
    platform: WindowsPlatform,
    path: &Path,
    preexisting: Option<&[(u64, String)]>,
    child: &mut std::process::Child,
) -> CaseOutcome {
    let reason = format!("15 s 内没有等到记事本窗口（标题里应含 {}）", path.display());
    note(format_args!("SKIP {UNICODE_NOTEPAD_CASE}: {reason}"));
    let cleaned =
        close_notepad_windows_created_by_this_test(platform, path, preexisting, CLEANUP_TIMEOUT);
    let launcher_reaped = reap_launcher_stub(child);
    remove_quietly(path);
    if !cleaned {
        note(format_args!("SKIP 之后仍有记事本窗口残留（见上面的 WARN）"));
    }
    CaseOutcome::skip(
        reason,
        vec![
            ("cleanup_ok", MeasurementValue::boolean(cleaned)),
            (
                "launcher_reaped",
                MeasurementValue::boolean(launcher_reaped),
            ),
        ],
    )
}

fn ime_is_applicable(hwnd: HWND) -> bool {
    // `is_ime_open` 的契约（见 `input/win32.rs` 文档）：能查到就返回状态；目标没有 Win32 IME
    // 上下文（UWP / WinUI 走 TSF）就报 `CapabilityMissing`，**绝不猜一个 false**（铁律 1）。
    // 因此这里对两种结果都接受并打印实际走了哪条分支，只把**契约外**的错误当作真缺陷。
    match is_ime_open(hwnd) {
        Ok(ime_open) => {
            note(format_args!("notepad: is_ime_open = {ime_open}"));
            true
        }
        Err(failure) if failure.code() == ErrorCode::CapabilityMissing => {
            note(format_args!(
                "notepad: is_ime_open 不适用（CapabilityMissing: {}）—— WinUI 目标走 TSF，符合契约",
                failure.message()
            ));
            false
        }
        Err(failure) => unreachable!("is_ime_open 返回了契约外的错误: {failure}"),
    }
}

/// 真机验收 3（TASK-040）：首次校准必须覆盖**真实显示器组合**。
///
/// 判据来自 `input::calibration`：每台显示器取 3 个内点（1/4、1/2、3/4 宽 × 1/3 高），
/// 用 `pointer_action(Move)` 真的移动光标，再用 `GetCursorPos` 读回，形成 expected /
/// observed 样本对；全部样本交给 `calibrate_pointer_samples`（容差 = 2 px）。
///
/// 样本上限 `MAXIMUM_CALIBRATION_SAMPLES`（64）→ 每台 3 个点意味着最多覆盖 21 台显示器；
/// 超出部分**不静默丢弃**，会打印被跳过的台数。
///
/// **只移动、不点击**：`pointer_action` 没有前台校验（只有键盘通道有），盲点会落到坐标
/// 下方的任意窗口上。点击只允许在「已确认目标窗口在前台」的用例里做 —— 见
/// `test_pointer_click_focuses_a_known_notepad_element`。
#[test]
#[ignore = "真机验收：需要真实桌面（会移动光标），默认不跑（见本文件头；TASK-228 记录结构化结果）"]
fn test_pointer_calibration_covers_the_real_display_set() {
    run_case(POINTER_CALIBRATION_CASE, run_pointer_calibration_case);
}

fn run_pointer_calibration_case() -> CaseOutcome {
    if let Err(failure) = declare_per_monitor_v2() {
        let reason = format!("无法声明 Per-Monitor V2（{failure}）—— 物理像素前提不成立");
        note(format_args!("SKIP {POINTER_CALIBRATION_CASE}: {reason}"));
        return CaseOutcome::skip(reason, Vec::new());
    }
    let displays = ok(enumerate_monitors());
    let platform = WindowsPlatform::new();
    let capacity = MAXIMUM_CALIBRATION_SAMPLES / SAMPLES_PER_DISPLAY;
    let samples = collect_calibration_samples(&displays, platform, capacity);
    if displays.len() > capacity {
        note(format_args!(
            "pointer_calibration: 显示器 {} 台超出样本上限，本轮只覆盖前 {capacity} 台",
            displays.len()
        ));
    }
    let tolerance = u32::try_from(MAXIMUM_PIXEL_ERROR).unwrap_or(u32::MAX);
    let report = ok(calibrate_pointer_samples(&samples, tolerance));
    note(format_args!(
        "pointer_calibration: samples={} displays={} max_error={} px total_error={} px tolerance={} px",
        report.sample_count(),
        report.display_count(),
        report.maximum_error_pixels(),
        report.total_error_pixels(),
        report.tolerance_pixels()
    ));
    assert!(
        report.sample_count() >= MINIMUM_CALIBRATION_SAMPLES,
        "真实显示器组合至少要采到 {MINIMUM_CALIBRATION_SAMPLES} 个样本"
    );
    assert!(report.display_count() >= 1, "至少要覆盖 1 台真实显示器");
    CaseOutcome::pass(vec![
        (
            "sample_count",
            MeasurementValue::integer(u64::try_from(report.sample_count()).unwrap_or(u64::MAX)),
        ),
        (
            "display_count",
            MeasurementValue::integer(u64::try_from(report.display_count()).unwrap_or(u64::MAX)),
        ),
        (
            "maximum_error_pixels",
            MeasurementValue::integer(u64::from(report.maximum_error_pixels())),
        ),
        (
            "total_error_pixels",
            MeasurementValue::integer(report.total_error_pixels()),
        ),
        (
            "tolerance_pixels",
            MeasurementValue::integer(u64::from(report.tolerance_pixels())),
        ),
    ])
}

fn collect_calibration_samples(
    displays: &[MonitorRecord],
    platform: WindowsPlatform,
    capacity: usize,
) -> Vec<CalibrationSample> {
    let mut samples = Vec::new();
    for (ordinal, display) in displays.iter().take(capacity).enumerate() {
        let space = ok(coordinate_space_for_monitor(display));
        let scale = space.scale();
        let left = i64::from(display.left());
        let top = i64::from(display.top());
        let width = i64::from(display.right()) - left;
        let height = i64::from(display.bottom()) - top;
        let offset_y = height * CALIBRATION_VERTICAL_STEP.0 / CALIBRATION_VERTICAL_STEP.1;
        for (numerator, denominator) in CALIBRATION_POINT_STEPS {
            let offset_x = width * numerator / denominator;
            let physical_x = left + offset_x;
            let physical_y = top + offset_y;
            let (physical_x, physical_y) = (
                i32::try_from(physical_x).unwrap_or(i32::MAX),
                i32::try_from(physical_y).unwrap_or(i32::MAX),
            );
            // 合成输入的入口是**逻辑**坐标：物理点 → 逻辑点（除以该台显示器的缩放）。
            let logical = ok(NormalizedPoint::new(
                f64::from(physical_x) / scale,
                f64::from(physical_y) / scale,
            ));
            // expected 由同一条换算链路产生（而不是手写公式），observed 是系统回读。
            let expected = ok(logical.to_physical(&space));
            ok(block_on(UiAutomationProvider::pointer_action(
                &platform,
                &space,
                logical,
                &PointerAction::Move,
            )));
            let (cursor_x, cursor_y) = ok(cursor_position());
            let observed = ok(ok(NormalizedPoint::new(
                f64::from(cursor_x) / scale,
                f64::from(cursor_y) / scale,
            ))
            .to_physical(&space));
            samples.push(CalibrationSample::new(
                u16::try_from(ordinal).unwrap_or(u16::MAX),
                expected,
                observed,
            ));
        }
    }
    samples
}

/// 真机验收 4（TASK-040）：点击**已知元素**必须真的命中。
///
/// 链路：唯一临时文件名打开**真实记事本** → 按标题（唯一文件名）解析窗口 → 按适配包的主
/// 选择器（class `RichEditD2DPT` + role `Document`）解析编辑器元素 → 读它的屏幕矩形 →
/// 把光标移到矩形中心并**单击** → 轮询回读该元素的 `HasKeyboardFocus`，必须为 true。
///
/// 安全前置：点击前必须确认「本次启动的记事本窗口就是前台」（`GetForegroundWindow`）；
/// 不是 → **打印原因并跳过**，绝不盲点（`pointer_action` 没有前台校验）。
#[test]
#[ignore = "真机验收：需要真实桌面 + 真实记事本，默认不跑（见本文件头；TASK-228 记录结构化结果）"]
fn test_pointer_click_focuses_a_known_notepad_element() {
    run_case(POINTER_CLICK_CASE, run_pointer_click_case);
}

fn run_pointer_click_case() -> CaseOutcome {
    if let Err(failure) = declare_per_monitor_v2() {
        let reason = format!("无法声明 Per-Monitor V2（{failure}）");
        note(format_args!("SKIP {POINTER_CLICK_CASE}: {reason}"));
        return CaseOutcome::skip(reason, Vec::new());
    }
    let path = temp_file_path();
    if let Err(failure) = std::fs::write(&path, "calibration\n") {
        let reason = format!("写临时文件失败 {}: {failure}", path.display());
        note(format_args!("SKIP {POINTER_CLICK_CASE}: {reason}"));
        return CaseOutcome::skip(reason, Vec::new());
    }
    let platform = WindowsPlatform::new();
    let preexisting = notepad_windows(platform);
    let Some(mut child) = launch_notepad(&path) else {
        remove_quietly(&path);
        return CaseOutcome::skip("启动 notepad.exe 失败", Vec::new());
    };
    let (editor, center_x, center_y) =
        match resolve_click_target(platform, &path, preexisting.as_deref(), &mut child) {
            Ok(target) => target,
            Err(skip) => return skip,
        };
    let displays = ok(enumerate_monitors());
    let Some(display) = display_containing(&displays, center_x, center_y) else {
        let reason = format!("元素中心 ({center_x}, {center_y}) 不在任何已枚举显示器内");
        note(format_args!("SKIP {POINTER_CLICK_CASE}: {reason}"));
        return skip_with_notepad_cleanup(
            platform,
            &path,
            preexisting.as_deref(),
            &mut child,
            &reason,
        );
    };
    let space = ok(coordinate_space_for_monitor(display));
    let scale = space.scale();
    let logical = ok(NormalizedPoint::new(
        f64::from(center_x) / scale,
        f64::from(center_y) / scale,
    ));
    ok(block_on(UiAutomationProvider::pointer_action(
        &platform,
        &space,
        logical,
        &PointerAction::Click,
    )));
    let focused = wait_for_keyboard_focus(&editor, FOCUS_TIMEOUT);
    let (cursor_x, cursor_y) = ok(cursor_position());
    note(format_args!(
        "pointer_click: element=editor center=({center_x}, {center_y}) \
         cursor=({cursor_x}, {cursor_y}) focused={focused}"
    ));
    let cleaned = cleanup_notepad_probe(platform, &path, preexisting.as_deref(), &mut child);
    assert!(
        focused,
        "点击元素矩形中心后该元素必须取得键盘焦点（命中判据）；本轮窗口无残留 = {cleaned}"
    );
    CaseOutcome::pass(vec![
        ("center_x", MeasurementValue::number(f64::from(center_x))),
        ("center_y", MeasurementValue::number(f64::from(center_y))),
        ("cursor_x", MeasurementValue::number(f64::from(cursor_x))),
        ("cursor_y", MeasurementValue::number(f64::from(cursor_y))),
        ("focused", MeasurementValue::boolean(focused)),
        ("cleanup_ok", MeasurementValue::boolean(cleaned)),
    ])
}

type ClickTarget = (ResolvedElement, i32, i32);

fn resolve_click_target(
    platform: WindowsPlatform,
    path: &Path,
    preexisting: Option<&[(u64, String)]>,
    child: &mut std::process::Child,
) -> Result<ClickTarget, CaseOutcome> {
    let Some(window) = resolve_notepad_window(platform, path) else {
        return Err(skip_with_notepad_cleanup(
            platform,
            path,
            preexisting,
            child,
            "解析本次记事本窗口失败",
        ));
    };
    let Some(editor) = resolve_notepad_editor(platform, &window) else {
        return Err(skip_with_notepad_cleanup(
            platform,
            path,
            preexisting,
            child,
            "解析记事本编辑器元素失败",
        ));
    };
    let Some((center_x, center_y)) = editor_center(&editor) else {
        return Err(skip_with_notepad_cleanup(
            platform,
            path,
            preexisting,
            child,
            "读元素屏幕矩形失败",
        ));
    };
    // 安全前置：`pointer_action` 无前台校验 —— 不是我们的窗口在前台就**不点**。
    // SAFETY: 只读查询当前前台窗口，不转移所有权。
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0 as u64 != window.id().value() {
        let reason = format!(
            "本次记事本窗口不在前台（foreground={:?}，target={}）—— 绝不盲点",
            foreground.0,
            window.id().value()
        );
        note(format_args!("SKIP {POINTER_CLICK_CASE}: {reason}"));
        return Err(skip_with_notepad_cleanup(
            platform,
            path,
            preexisting,
            child,
            &reason,
        ));
    }
    Ok((editor, center_x, center_y))
}

fn skip_with_notepad_cleanup(
    platform: WindowsPlatform,
    path: &Path,
    preexisting: Option<&[(u64, String)]>,
    child: &mut std::process::Child,
    reason: &str,
) -> CaseOutcome {
    let cleaned = cleanup_notepad_probe(platform, path, preexisting, child);
    CaseOutcome::skip(
        reason.to_owned(),
        vec![("cleanup_ok", MeasurementValue::boolean(cleaned))],
    )
}
