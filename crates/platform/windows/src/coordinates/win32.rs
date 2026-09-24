//! `coordinates` 的 Windows 实现：真实 DPI 与显示器枚举（`#[cfg(windows)]`）。
//!
//! 职责：把 `EnumDisplayMonitors` / `GetMonitorInfoW` / `GetDpiForMonitor` / `GetDpiForWindow` /
//! `GetSystemMetrics(SM_*VIRTUALSCREEN)` 包装成 `MonitorRecord` / `CoordinateSpace` / `VirtualScreen`。
//! 边界：**不**做换算（`super` 的纯函数 + `crates/platform/api` 的 `to_physical` 负责）。
//!
//! ## 不变量
//! 1. **DPI 必须来自真实显示器**：`GetDpiForWindow` 返回 0 → 明确报错，**不**退回 96。
//! 2. 所有 Win32 失败经 `error_from_win32_failure` / `error_from_win32` 分类（铁律 1）。
//!
//! 相关：架构 v2 §6.9、`docs/memory/win32-input-research.md` §9。

use assistant_platform_api::{CoordinateSpace, PlatformResult};
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW,
    MonitorFromPoint, MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use windows::core::BOOL;

use super::{MonitorRecord, VirtualScreen, coordinate_space_for_monitor};
use crate::error;

/// `MONITORINFOF_PRIMARY`（`wingdi.h`）。`windows` crate 0.62.2 没有导出这个常量，
/// 因此这里按头文件的值本地定义（而不是靠"是不是第一台"来猜主显示器）。
const MONITORINFOF_PRIMARY_FLAG: u32 = 0x0000_0001;

/// 枚举当前桌面的全部显示器（物理像素矩形 + 有效 DPI + 设备名）。
///
/// # Errors
/// `EnumDisplayMonitors` 失败 → 按 `error_from_win32_failure` 分类；
/// 某台显示器查询失败 → `Fatal`（枚举到却查不到属性说明状态不一致，不该猜）。
pub fn enumerate_monitors() -> PlatformResult<Vec<MonitorRecord>> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    let sink = LPARAM(std::ptr::addr_of_mut!(handles) as isize);
    // SAFETY: `sink` 指向本栈帧上的 `handles`，`EnumDisplayMonitors` **同步**调用回调，
    // 因此指针在整个枚举期间有效；回调只做 `push`。
    // 注意：它返回 `BOOL`（**不是** `Result`）—— 失败时必须自己取 `GetLastError` 分类。
    let ok = unsafe { EnumDisplayMonitors(None, None, Some(collect_monitor), sink) };
    if !ok.as_bool() {
        // SAFETY: 只读上一个 Win32 调用的错误码。
        let code = unsafe { GetLastError().0 };
        return Err(error::error_from_win32(code, "EnumDisplayMonitors"));
    }
    handles
        .iter()
        .map(|handle| record_for_monitor(*handle))
        .collect()
}

/// `EnumDisplayMonitors` 回调：把每台显示器追加到调用方的 `Vec<HMONITOR>`。
///
/// SAFETY: `lparam` 必须指向一个在整个枚举期间存活的 `Vec<HMONITOR>`；
/// `enumerate_monitors` 保证了这一点。返回 `TRUE` 让枚举继续。
unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let sink = lparam.0 as *mut Vec<HMONITOR>;
    if !sink.is_null() {
        // SAFETY: 见函数级 SAFETY 说明（调用方保证指针有效且独占）。
        unsafe { (*sink).push(monitor) };
    }
    BOOL(1)
}

/// 读取一台显示器的属性（设备名 / 矩形 / 有效 DPI / 是否主显示器）。
///
/// # Errors
/// `GetMonitorInfoW` 或 `GetDpiForMonitor` 失败 → `Fatal`（**不**用 96 兜底）。
fn record_for_monitor(monitor: HMONITOR) -> PlatformResult<MonitorRecord> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = u32::try_from(std::mem::size_of::<MONITORINFOEXW>())
        .map_err(|_| error::capability_missing("MONITORINFOEXW size does not fit in u32"))?;
    // SAFETY: `info` 是本栈帧上的合法 out-参数，`cbSize` 已按头文件要求填好；
    // `MONITORINFOEXW` 以 `MONITORINFO` 开头（`repr(C)`），因此把内部字段的地址当
    // `*mut MONITORINFO` 传入是布局兼容的（这是 Win32 规定的用法）。
    let ok = unsafe { GetMonitorInfoW(monitor, &raw mut info.monitorInfo) };
    if !ok.as_bool() {
        // SAFETY: 只读上一个 Win32 调用的错误码。
        let code = unsafe { GetLastError().0 };
        return Err(error::error_from_win32(code, "GetMonitorInfoW"));
    }
    let mut dpi_x: u32 = 0;
    let mut dpi_y: u32 = 0;
    // SAFETY: 两个 out-参数都在本栈帧上；`monitor` 来自刚枚举成功的句柄。
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) }
        .map_err(|failure| error::error_from_hresult(failure.code().0, "GetDpiForMonitor"))?;
    let rect = info.monitorInfo.rcMonitor;
    Ok(MonitorRecord::new(
        device_name(&info),
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        dpi_x,
    )
    // 主显示器只认 `MONITORINFOF_PRIMARY`（枚举顺序不保证第一台就是主屏）。
    .with_primary(info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY_FLAG != 0))
}

/// 从 `MONITORINFOEXW.szDevice`（以 NUL 结尾的 UTF-16）取出设备名。
fn device_name(info: &MONITORINFOEXW) -> String {
    let end = info
        .szDevice
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(info.szDevice.len());
    let slice = info.szDevice.get(..end).unwrap_or(&[]);
    String::from_utf16_lossy(slice)
}

/// 窗口所在的显示器（`MONITOR_DEFAULTTONEAREST`：窗口跨屏时取最近的一台）。
///
/// # Errors
/// 显示器属性查询失败 → `Fatal`；`MonitorFromWindow` 返回空句柄 → `TargetNotFound`。
pub fn monitor_for_window(hwnd: HWND) -> PlatformResult<MonitorRecord> {
    // SAFETY: 只读查询一个由调用方保证存在的窗口句柄。
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.0.is_null() {
        return Err(error::target_not_found(
            "MonitorFromWindow returned no monitor for the window",
        ));
    }
    record_for_monitor(monitor)
}

/// 物理点所在的显示器（`MONITOR_DEFAULTTONEAREST`）。
///
/// # Errors
/// 同 `monitor_for_window`。
pub fn monitor_for_physical_point(x: i32, y: i32) -> PlatformResult<MonitorRecord> {
    // SAFETY: 只读查询。
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    if monitor.0.is_null() {
        return Err(error::target_not_found(format!(
            "MonitorFromPoint returned no monitor for ({x}, {y})"
        )));
    }
    record_for_monitor(monitor)
}

/// 窗口的有效 DPI（`GetDpiForWindow`；Per-Monitor V2 下就是该窗口所在显示器的 DPI）。
///
/// # Errors
/// 返回 0（无效窗口 / 系统不支持）→ `TargetNotFound` —— **不**退回 96，
/// 因为用错缩放会让点击落到错误的位置（静默失败）。
pub fn dpi_for_window(hwnd: HWND) -> PlatformResult<u32> {
    // SAFETY: 只读查询。
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        return Err(error::target_not_found(
            "GetDpiForWindow returned 0 (invalid window or unsupported DPI awareness)",
        ));
    }
    Ok(dpi)
}

/// 目标窗口的坐标空间（`SendInput` 前换算用的那一个）。
///
/// 设备名取窗口所在显示器的设备名 —— 与架构 v2 §6.2 的
/// `resolved.coordinate_space.origin_monitor` 对应。
///
/// # Errors
/// 窗口 / 显示器查询失败 → 见上；DPI 为 0 → `TargetNotFound`。
pub fn coordinate_space_for_window(hwnd: HWND) -> PlatformResult<CoordinateSpace> {
    let monitor = monitor_for_window(hwnd)?;
    let dpi = dpi_for_window(hwnd)?;
    let record = MonitorRecord::new(
        monitor.device_name().to_string(),
        monitor.left(),
        monitor.top(),
        monitor.right(),
        monitor.bottom(),
        dpi,
    )
    .with_primary(monitor.is_primary());
    coordinate_space_for_monitor(&record)
}

/// 虚拟屏幕（全部显示器的最小包围盒，`SendInput` 绝对坐标的归一化基准）。
///
/// # Errors
/// 任一边长 ≤ 1 → `ToolInvalidArgs`（归一化分母会是 0）。
pub fn virtual_screen() -> PlatformResult<VirtualScreen> {
    // SAFETY: 四个 `GetSystemMetrics` 都是无参数只读查询。
    let (left, top, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if width <= 1 || height <= 1 {
        return Err(error::invalid_args(format!(
            "virtual screen metrics are degenerate: {width}x{height} at ({left}, {top})"
        )));
    }
    Ok(VirtualScreen::new(left, top, width, height))
}
