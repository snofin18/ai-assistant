//! Win32 薄封装（`EnumWindows` / 窗口属性 / 进程身份 / 前台与遮挡判定）。
//!
//! 职责：把本 crate 需要的 **Win32 调用集中在一处**，使 `window/**` 与 `uia/**` 里几乎没有裸
//! `unsafe`；同时把每个失败翻译成带 `ErrorCode` 的 `PlatformError`（铁律 1）。
//! 边界：**不做** UIA、**不做**合成输入、**不做**窗口管理策略（抢不抢焦点由调用方给策略）。
//!
//! ## 不变量
//! 1. **属主 PID 只从 `GetWindowThreadProcessId` 取**（ADR-0022 D1）：`.NET Process.MainWindowHandle`
//!    是启发式结果、启动返回的 PID ≠ 窗口属主 —— 两条都已被实测否决（`docs/memory/rejected.md`）。
//! 2. 进程身份比较用**映像名叶子**且大小写不敏感：打包应用（MSIX）的完整路径在
//!    `C:\Program Files\WindowsApps\…` 下，路径会随版本变化，叶子不会（`apps/notepad.md` §1）。
//! 3. 缓冲区长度一律来自 `GetWindowTextLengthW` / `GetClassNameW` 的返回值并做**上界**检查，
//!    绝不假定固定长度足够（长标题会被截断成"看起来匹配"的静默失败）。
//! 4. 整数运算不依赖 `as` 截断（workspace 把 `clippy::cast_possible_truncation` 当 deny）；
//!    越界一律走 `i32::try_from` / `i64` 中间量。
//!
//! 相关：架构 v2 §13.2、ADR-0022 D1/D2、`docs/memory/apps/notepad.md` §2。

use assistant_platform_api::PlatformResult;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindow, WindowFromPoint,
};
use windows::core::{BOOL, PWSTR};

use crate::error;

/// 一个顶层窗口的只读快照（`list_windows` 与 `resolve_window` 的公共中间物）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRecord {
    /// 窗口句柄。
    pub(crate) hwnd: HWND,
    /// 窗口标题（**仅供展示 / 兜底匹配**，ADR-0022 D4 禁止当主 selector）。
    pub(crate) title: String,
    /// Win32 窗口类名（`Notepad` / `#32770` …）—— **非本地化**。
    pub(crate) class_name: String,
    /// 属主进程映像名叶子（`notepad.exe` …）—— 本 crate 的 `app_id` 口径。
    pub(crate) app_id: String,
    /// 属主进程 id（来自 `GetWindowThreadProcessId`）。
    pub(crate) owner_pid: u32,
}

/// 枚举全部**可见**的顶层窗口，按 Win32 枚举顺序返回。
///
/// 为什么过滤不可见窗口：不可见窗口没有可操作表面，让它们进入候选池只会制造歧义
/// （`AmbiguousTarget`）而不会带来任何可执行目标。
///
/// # Errors
/// `EnumWindows` 自身失败 → 按 `error_from_win32` 分类。
pub fn enumerate_visible_top_level_windows() -> PlatformResult<Vec<HWND>> {
    let mut collected: Vec<HWND> = Vec::new();
    let sink = LPARAM(std::ptr::addr_of_mut!(collected) as isize);
    // SAFETY: `sink` 指向本栈帧上的 `collected`，`EnumWindows` **同步**调用回调，
    // 因此指针在整个枚举期间有效；回调只做 `push`。
    unsafe { EnumWindows(Some(collect_window), sink) }
        .map_err(|failure| error::error_from_win32_failure(&failure, "EnumWindows"))?;
    collected.retain(|hwnd| {
        // SAFETY: 只读查询一个刚枚举到的窗口句柄。
        unsafe { IsWindowVisible(*hwnd).as_bool() }
    });
    Ok(collected)
}

/// `EnumWindows` 回调：把每个句柄追加到调用方的 `Vec<HWND>`。
///
/// SAFETY: `lparam` 必须指向一个在整个枚举期间存活的 `Vec<HWND>`；`enumerate_visible_top_level_windows`
/// 保证了这一点。返回 `TRUE` 让枚举继续。
unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let sink = lparam.0 as *mut Vec<HWND>;
    if !sink.is_null() {
        // SAFETY: 见函数级 SAFETY 说明（调用方保证指针有效且独占）。
        unsafe { (*sink).push(hwnd) };
    }
    BOOL(1)
}

/// 读取窗口的只读快照（标题 / 类名 / 属主 PID / 映像名叶子）。
///
/// # Errors
/// 句柄已失效（窗口在枚举与查询之间关闭）→ `TargetNotFound`
/// （**不是** `Fatal`：这是正常的竞态，调用方可以重新枚举）。
pub fn window_record(hwnd: HWND) -> PlatformResult<WindowRecord> {
    let mut owner_pid: u32 = 0;
    // SAFETY: `owner_pid` 是本栈帧上的合法 out-参数。
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut owner_pid)) };
    if owner_pid == 0 {
        return Err(error::target_not_found(
            "window handle became invalid while being inspected",
        ));
    }
    let title = window_title(hwnd);
    let class_name = window_class_name(hwnd);
    let app_id = process_image_leaf(owner_pid).unwrap_or_else(|| format!("pid:{owner_pid}"));
    Ok(WindowRecord {
        hwnd,
        title,
        class_name,
        app_id,
        owner_pid,
    })
}

/// 读取窗口标题（失败或超长时返回已读到的部分；空标题是合法结果）。
fn window_title(hwnd: HWND) -> String {
    // SAFETY: 只读查询；返回长度用于分配缓冲。
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }
    // +1 给结尾 NUL；`length` 是 i32 且已 > 0，转换不会失败。
    let Ok(capacity) = usize::try_from(length) else {
        return String::new();
    };
    let mut buffer = vec![0_u16; capacity.saturating_add(1)];
    // SAFETY: `buffer` 是 `capacity + 1` 个可写 u16，`GetWindowTextW` 最多写这么多单元。
    let written = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    utf16_prefix_to_string(&buffer, written)
}

/// 读取 Win32 窗口类名（非本地化；失败时返回空串）。
fn window_class_name(hwnd: HWND) -> String {
    let mut buffer = vec![0_u16; 256];
    // SAFETY: `buffer` 是 256 个可写 u16。
    let written = unsafe { GetClassNameW(hwnd, &mut buffer) };
    utf16_prefix_to_string(&buffer, written)
}

/// 把 `GetWindowTextW` / `GetClassNameW` 的写入结果转成 `String`。
///
/// `written <= 0` = 失败或空串；`written` 超出缓冲长度（理论上不可能）时按缓冲长度截断，
/// 但**不**改变"已写入部分就是结果"的语义（调用方拿到的永远是真实的已写入内容）。
fn utf16_prefix_to_string(buffer: &[u16], written: i32) -> String {
    let Ok(count) = usize::try_from(written) else {
        return String::new();
    };
    let bounded = count.min(buffer.len());
    let slice = buffer.get(..bounded).unwrap_or_default();
    String::from_utf16_lossy(slice)
}

/// 取属主进程的映像名**叶子**（`notepad.exe`），大小写保持原样。
///
/// 为什么是叶子而不是全路径：MSIX 打包应用的安装路径含版本号（`…_11.2607.14.0_x64__…`），
/// 版本一升路径就变；叶子名是稳定的（`apps/notepad.md` §1）。
pub fn process_image_leaf(pid: u32) -> Option<String> {
    // SAFETY: 申请**最小**权限的查询权；句柄在下面显式关闭，且之后不再使用。
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = vec![0_u16; 1024];
    let mut size = u32::try_from(buffer.len()).unwrap_or(0);
    // SAFETY: `buffer` / `size` 描述一块 `size` 个 u16 的可写区域，`size` 会被原地更新为
    // 实际写入的单元数；`handle` 由本函数独占持有。
    let queried = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &raw mut size,
        )
    };
    // SAFETY: 关闭本函数自己打开的句柄。
    // 关闭失败的唯一原因是"句柄已无效"（本函数不会重复关闭），没有任何可执行分支；
    // 这里显式忽略并说明理由，而不是用 `let _ =` 静默吞掉（铁律 1）。
    let _close_outcome = unsafe { CloseHandle(handle) };
    if queried.is_err() {
        return None;
    }
    let Ok(count) = usize::try_from(size) else {
        return None;
    };
    let slice = buffer.get(..count.min(buffer.len()))?;
    let image = String::from_utf16_lossy(slice);
    image
        .rsplit(['\\', '/'])
        .next()
        .filter(|leaf| !leaf.is_empty())
        .map(ToString::to_string)
}

/// 句柄是否仍指向一个存在的窗口（用于把"已关闭"和"权限不足"分开）。
pub fn is_window(hwnd: HWND) -> bool {
    // SAFETY: 只读查询，不访问任何窗口内容。
    unsafe { IsWindow(Some(hwnd)).as_bool() }
}

/// 当前前台窗口句柄（没有前台窗口时 `None`）。
pub fn foreground_hwnd() -> Option<HWND> {
    // SAFETY: 无参数只读查询。
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() { None } else { Some(hwnd) }
}

/// 窗口是否最小化。
pub fn is_minimized(hwnd: HWND) -> bool {
    // SAFETY: 只读查询。
    unsafe { IsIconic(hwnd).as_bool() }
}

/// 窗口是否被遮挡（**近似判定**，见下）。
///
/// 判定方式：取窗口矩形的中心点，看该点最上层的窗口的根祖先是不是本窗口。
/// 这是**近似**：部分遮挡、非矩形窗口、以及被透明覆盖层压住的窗口都会误判。
/// 真正的遮挡检测需要逐像素或 `DwmGetWindowAttribute`，属后续卡；
/// 这里宁可**报"被遮挡"**（保守方向，调用方会先 `bring_to_front`）也不报"没被遮挡"。
pub fn is_occluded(hwnd: HWND) -> bool {
    let Some(rect) = window_rect(hwnd) else {
        return true;
    };
    let (Some(center_x), Some(center_y)) = (
        rect_center(rect.left, rect.right),
        rect_center(rect.top, rect.bottom),
    ) else {
        return true;
    };
    // SAFETY: 只读查询。
    let topmost = unsafe {
        WindowFromPoint(POINT {
            x: center_x,
            y: center_y,
        })
    };
    if topmost.0.is_null() {
        return true;
    }
    // SAFETY: 只读查询（`GA_ROOT` 走父链到根，不修改窗口状态）。
    let root = unsafe { GetAncestor(topmost, GA_ROOT) };
    root != hwnd
}

/// 窗口矩形的中心坐标（用 `i64` 中间量避免 `left + right` 溢出）。
fn rect_center(low: i32, high: i32) -> Option<i32> {
    let sum = i64::from(low) + i64::from(high);
    i32::try_from(sum / 2).ok()
}

/// 取窗口矩形（失败时 `None`）。
pub fn window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    // SAFETY: `rect` 是本栈帧上的合法 out-参数。
    unsafe { GetWindowRect(hwnd, &raw mut rect) }.ok()?;
    Some(rect)
}

/// 把窗口带到前台：必要时先 `SW_RESTORE`，再 `SetForegroundWindow`。
///
/// 返回**是否真的成为前台窗口**（调用方据此决定 `Ok` 还是 `TargetUnresponsive`）——
/// `SetForegroundWindow` 在"当前进程不是前台进程"时会被系统静默拒绝并返回成功语义，
/// 所以必须回读 `GetForegroundWindow`，不能只看返回值（铁律 4 的同一条精神）。
pub fn bring_to_front(hwnd: HWND) -> bool {
    if is_minimized(hwnd) {
        // SAFETY: `SW_RESTORE` 对已最小化窗口是标准用法。它的返回值是"窗口之前是否可见"，
        // 与本函数的结论无关（结论只由下面的前台回读决定），因此不参与判定；
        // 这里显式绑定并说明理由，而不是用 `let _ =` 静默丢弃（铁律 1）。
        let _was_visible_before_restore = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    }
    // SAFETY: 只改前台归属，不改窗口内容。
    let accepted = unsafe { SetForegroundWindow(hwnd) }.as_bool();
    // **必须回读**：`SetForegroundWindow` 在"调用进程不是前台进程"时会被系统拒绝，
    // 单看返回值会把拒绝当成成功（静默失败）。因此要求"系统接受 **且** 回读命中"。
    accepted && foreground_hwnd() == Some(hwnd)
}
