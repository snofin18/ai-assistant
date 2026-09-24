//! `input` 的 Windows 实现：真实 `SendInput` 调用 + IME 开关状态（`#[cfg(windows)]`）。
//!
//! 职责：把 `super`（三平台编译的**纯逻辑**）产出的事件序列交给 `SendInput`，并在发送前
//! **100% 校验前台窗口**；另外提供 `is_ime_open`。
//! 边界：**不做**键名 → 虚拟键码的映射（`super`）、**不做**坐标换算（`crate::coordinates`）、
//! **不做**权限判定（放行点是 `crates/policy`，TASK-021）、**不做**结果断言（TASK-023）。
//!
//! ## 为什么发送前必须校验前台窗口（本文件最关键的不变量）
//! `SendInput` 把事件插进**调用线程所在桌面**的输入队列，由**当前前台窗口**接收 —— 目标窗口
//! 不在前台时，按键会打到**用户正在用的应用**上（聊天窗口、编辑器、终端）。这是最危险的一类
//! 静默失败，所以本文件的顺序固定为：
//! `GetForegroundWindow() != target` → `bring_to_front`（`SW_RESTORE` + `SetForegroundWindow`
//! + **回读**）→ 仍不一致 → `TargetUnresponsive`，**绝不盲发**。
//!
//! ## 为什么文本走 `KEYEVENTF_UNICODE`
//! `KEYEVENTF_UNICODE` 直接投递 UTF-16 码元，**绕过键盘布局与 IME 组字**：IME 开着或布局是
//! 中文 / 日文时，`VK_*` 路径会把 `A` 变成候选字，而 Unicode 路径不会。BMP 之外的字符
//! （emoji）由 `super::utf16_units` 拆成**代理对**两个码元依次发送。
//!
//! ## 已知限制
//! `pointer_action` 的签名不带目标窗口（trait 形状冻结，TASK-016），所以它只能保证
//! **坐标正确**，不能保证「点到的就是预期窗口」—— 那需要带目标的签名，见
//! `docs/PARKING_LOT.md` PL-074。
//!
//! 相关：架构 v2 §13.2 / §6.9、`docs/memory/win32-input-research.md` §1 / §6 / §9、铁律 1 / 4 / 5。

use assistant_platform_api::{
    ErrorCode, KeyChord, KeyTarget, NormalizedPoint, PlatformError, PlatformResult, PointerAction,
    ResolvedElement,
};
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmGetDefaultIMEWnd, ImmGetOpenStatus, ImmReleaseContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, SendInput,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_IME_CONTROL};

use super::{KeyEvent, PointerStep, key_events_for_chord, pointer_steps, utf16_units};
use crate::coordinates::{
    VirtualScreen, coordinate_space_for_logical_point, enumerate_monitors, virtual_screen,
};
use crate::error;
use crate::handles::{hwnd_from_window_handle, with_element};
use crate::win32::{bring_to_front, foreground_hwnd};

/// `ERROR_ACCESS_DENIED`：UIPI 拒绝（目标进程完整性级别更高）。
const ERROR_ACCESS_DENIED: u32 = 5;

/// `ERROR_INVALID_PARAMETER`：`SendInput` 拒绝了一个或多个事件结构。
const ERROR_INVALID_PARAMETER: u32 = 87;

/// `IMC_GETOPENSTATUS`（`imm.h`）：`WM_IME_CONTROL` 的「取 IME 打开状态」子命令。
///
/// `windows` crate 0.62.2 **没有**导出这个常量，因此按头文件的值本地定义（`0x0005`）；
/// 它进 `WPARAM`，所以类型是 `usize`（避免一次 `as` 转换）。
const IMC_GETOPENSTATUS: usize = 0x0005;

/// 合成**指针**输入（`UiAutomationProvider::pointer_action` 的 Windows 实现）。
///
/// **调用方应先试 L1（`set_value` / `invoke_action`）~ L3（无障碍接口）** —— 本函数是铁律 5 的
/// **L4（最后手段）**：它抢用户的鼠标，且点击落在**屏幕坐标**上。
///
/// 流程：枚举显示器 → 按**逻辑点**反推它属于哪台显示器 → `to_physical` 换算 → 归一化到虚拟
/// 屏幕的 `0..=65535` → `SendInput`。
///
/// # Errors
/// - 点不在任何显示器上 / 换算越界 → `TargetNotFound`（**不**夹边界 —— 夹边界是静默点偏）
/// - 混合 DPI 下逻辑点无法唯一归属 → `CapabilityMissing`（`docs/PARKING_LOT.md` PL-074）
/// - 枚举不到显示器 → `CapabilityMissing`
/// - `SendInput` 被 UIPI 拒绝 → `PlatformPermission`；返回值与请求数不符 → 见
///   [`classify_send_input_failure`]
pub fn pointer_action(point: &NormalizedPoint, action: &PointerAction) -> PlatformResult<()> {
    // 换算（§6.9 规则 3）：`to_physical` 是**唯一**的换算入口，本函数不重写公式。
    let displays = enumerate_monitors()?;
    let space = coordinate_space_for_logical_point(&displays, point)?;
    let start = point.to_physical(&space)?;
    let drop = match action {
        PointerAction::DragTo { drop_at } => Some(drop_at.to_physical(&space)?),
        // `PointerAction` 是 `#[non_exhaustive]`：新动作若没在这里给出释放点，`pointer_steps`
        // 会明确报 `CapabilityMissing`，不会退化成一次单击。
        _ => None,
    };
    let steps = pointer_steps(action, start, drop)?;
    // 虚拟屏幕 = 全部显示器的最小包围盒（`MOUSEEVENTF_ABSOLUTE` 的归一化基准）。
    let screen = virtual_screen()?;
    send_pointer_steps(&screen, &steps)
}

/// 合成**键盘**输入（`UiAutomationProvider::key_action` 的 Windows 实现）。
///
/// **调用方应先试 L1（`edit_text`）~ L3（无障碍接口）** —— 本函数是铁律 5 的 **L4**。
///
/// 顺序固定（每一步失败都**不**继续）：
/// 1. **纯逻辑校验**键名 / 修饰键 —— 坏输入不该在碰过真实窗口之后才发现；
/// 2. 解析目标窗口（`KeyTarget::Element` 额外要求 UIA **回读确认**焦点）；
/// 3. **回读确认前台窗口**（[`ensure_foreground`]）—— 不一致就**不发送**；
/// 4. `SendInput` + 返回值校验。
///
/// # Errors
/// - 键名 / 修饰键非法 → `ToolInvalidArgs`
/// - 目标窗口拿不到 → `TargetNotFound`；前台化失败 / 元素焦点没确认 → `TargetUnresponsive`
/// - UIPI 拦截 → `PlatformPermission`
pub fn send_key_action(chord: &KeyChord, target: &KeyTarget) -> PlatformResult<()> {
    let events = key_events_for_chord(chord)?;
    let hwnd = resolve_key_target(target)?;
    ensure_foreground(hwnd)?;
    if let KeyTarget::Element(element) = target {
        focus_element(element)?;
    }
    let inputs: Vec<INPUT> = events.iter().copied().map(key_input).collect();
    send_inputs(&inputs, "SendInput(keyboard)")
}

/// 文本 → `KEYEVENTF_UNICODE` 事件序列（**绕过 IME 与键盘布局**）。
///
/// 这是「IME 开着也能正确写入」的根据：`KEYEVENTF_UNICODE` 把 UTF-16 码元直接投递进输入
/// 队列，既不经过键盘布局映射，也不触发 IME 组字（`docs/memory/win32-input-research.md` §1）。
///
/// **调用方应先试 L1 / L3 的文本通道**（`set_value` / `edit_text`）—— 本函数是 L4 兜底。
///
/// # Errors
/// - `text` 为空 → `ToolInvalidArgs`（空批次**不是**成功）
/// - 前台校验失败 → `TargetUnresponsive`（同 [`send_key_action`] 的第 3 步）
/// - UIPI / 参数错误 → 见 [`classify_send_input_failure`]
pub fn send_unicode_text(hwnd: HWND, text: &str) -> PlatformResult<()> {
    let units = utf16_units(text);
    if units.is_empty() {
        return Err(error::invalid_args(
            "send_unicode_text: text must not be empty (an empty batch is not a success)",
        ));
    }
    ensure_foreground(hwnd)?;
    // 每个码元一对 down/up：`KEYEVENTF_UNICODE` 要求 key-up 也带该标志，否则目标收到的是
    // 「按下一个键但永不松开」—— 输入队列会残留状态。
    let mut inputs = Vec::with_capacity(units.len() * 2);
    for unit in units {
        inputs.push(unicode_input(unit, false));
        inputs.push(unicode_input(unit, true));
    }
    send_inputs(&inputs, "SendInput(unicode)")
}

/// IME 是否处于「打开」状态。
///
/// 上层据此决定要不要提示用户。本层**不**自行关 IME —— 关 IME 会改用户环境，而文本路径
/// 本来就不经过 IME（见 [`send_unicode_text`]）。
///
/// 查询分两级（都是 Win32 文档化的做法）：
/// 1. `ImmGetContext(hwnd)` → `ImmGetOpenStatus`：窗口**自己**有输入上下文时最快；
/// 2. 否则 `ImmGetDefaultIMEWnd(hwnd)` → `SendMessageW(WM_IME_CONTROL, IMC_GETOPENSTATUS)`：
///    问该窗口所属线程的**默认 IME 窗口** —— 顶层目标窗口通常只有这一条路走得通
///    （IME 上下文属于**当前获得焦点的控件**，而顶层窗口一般没有自己的上下文）；
/// 3. 两条都不行 → `CapabilityMissing`：该窗口没有 Win32 IME 上下文。
///    实测（2026-09-25）：Win11 25H2 的记事本是 **`WinUI`** 应用，走 TSF 而不是 Win32 IME，
///    因此对它查 IME 开关状态**本来就不适用** —— 这时必须给出可解释的失败，
///    而不是猜一个 `false`（铁律 1；见 `docs/memory/pitfalls.md`）。
///
/// # Errors
/// - 窗口没有 Win32 IME 上下文（UWP / `WinUI` / 无 IME 的目标）→ `CapabilityMissing`
/// - `ImmReleaseContext` 失败 → 按**原始 Win32 码**分类（**不**静默忽略）
pub fn is_ime_open(hwnd: HWND) -> PlatformResult<bool> {
    // ① 窗口自己的输入上下文。
    // SAFETY: `ImmGetContext` 只读地取该窗口的输入上下文；它返回的 `HIMC` 必须由
    // `ImmReleaseContext` 释放，本函数保证在返回前释放。
    let context = unsafe { ImmGetContext(hwnd) };
    if !context.is_invalid() {
        // SAFETY: `context` 来自上面的 `ImmGetContext`，此刻仍未被释放。
        let is_open = unsafe { ImmGetOpenStatus(context) };
        // SAFETY: 释放本函数自己取得的输入上下文；`ImmReleaseContext` 是它唯一的释放方式。
        let released = unsafe { ImmReleaseContext(hwnd, context) };
        if !released.as_bool() {
            // SAFETY: 只读上一个 Win32 调用的错误码。
            let code = unsafe { GetLastError().0 };
            return Err(error::error_from_win32(code, "ImmReleaseContext"));
        }
        return Ok(is_open.as_bool());
    }
    // ② 该窗口所属线程的默认 IME 窗口。
    // SAFETY: 只读查询；返回的是 IME 类的窗口句柄（系统所有，不需要我们释放）。
    let ime_window = unsafe { ImmGetDefaultIMEWnd(hwnd) };
    if ime_window.0.is_null() {
        return Err(error::capability_missing(
            "is_ime_open: the window has no Win32 IME context (ImmGetContext and \
             ImmGetDefaultIMEWnd both returned nothing) — UWP / WinUI targets use TSF, so the \
             Win32 IME query does not apply to them",
        ));
    }
    // SAFETY: `SendMessageW` 同步把消息交给该窗口的过程；`WM_IME_CONTROL` +
    // `IMC_GETOPENSTATUS` 的返回值是「IME 是否打开」（非 0 = 打开）。四个参数都是立即数，
    // 不传指针，也不要求我们持有该窗口。
    let status = unsafe {
        SendMessageW(
            ime_window,
            WM_IME_CONTROL,
            Some(WPARAM(IMC_GETOPENSTATUS)),
            Some(LPARAM(0)),
        )
    };
    Ok(status.0 != 0)
}

/// 发送前把目标窗口带到前台并**回读确认**（本文件最关键的不变量）。
///
/// `SendInput` 由**当前前台窗口**接收，所以「以为发给了 A、实际发给了用户正在用的 B」是这里
/// 唯一不能容忍的失效模式：不一致就报错，**绝不盲发**。
fn ensure_foreground(hwnd: HWND) -> PlatformResult<()> {
    if foreground_hwnd() == Some(hwnd) {
        return Ok(());
    }
    // `bring_to_front` 内部已做过「`SetForegroundWindow` + **回读** `GetForegroundWindow`」：
    // 系统在"调用进程不是前台进程"时会静默拒绝置前，只看返回值会把拒绝当成成功。
    if bring_to_front(hwnd) {
        return Ok(());
    }
    Err(error::target_unresponsive(format!(
        "synthetic input: the target window ({:#x}) could not be brought to the foreground, \
         so the input was NOT sent (sending it would type into whatever window the user has in \
         front)",
        hwnd.0 as usize
    )))
}

/// `KeyTarget` → 目标窗口 `HWND`。
///
/// # Errors
/// - `KeyTarget::Window` / `Element` 的句柄不可用 → `TargetNotFound`
/// - 桌面没有前台窗口（`KeyTarget::Foreground`）→ `TargetNotFound`
/// - 未来新增的目标类型 → `CapabilityMissing`（**不**猜一个窗口）
fn resolve_key_target(target: &KeyTarget) -> PlatformResult<HWND> {
    match target {
        KeyTarget::Window(window) => hwnd_from_window_handle(window.id()),
        KeyTarget::Element(element) => element_window(element),
        KeyTarget::Foreground => foreground_hwnd().ok_or_else(|| {
            error::target_not_found("key_action(Foreground): this desktop has no foreground window")
        }),
        // `KeyTarget` 是 `#[non_exhaustive]`：新增目标类型时必须**显式**决定怎么定位 ——
        // 猜一个窗口 = 按键可能打到别的应用上。
        _ => Err(error::capability_missing(
            "key_action: this KeyTarget variant is not implemented by the Windows channel yet",
        )),
    }
}

/// 元素 → 它所在的原生窗口（UIA `CurrentNativeWindowHandle`；为空时退回解析时的父窗口句柄）。
///
/// # Errors
/// - 元素句柄不在本线程 / 元素已消失 → `TargetNotFound`（UIA 原始分类）
/// - 两个句柄都不可用 → `TargetNotFound`
fn element_window(element: &ResolvedElement) -> PlatformResult<HWND> {
    let native = with_element(element.id(), |element| {
        // SAFETY: 只读查询该元素的属性；元素由线程本地表独占持有（句柄不跨线程，铁律 8）。
        unsafe { element.CurrentNativeWindowHandle() }.map_err(|failure| {
            error::error_from_hresult(failure.code().0, "CurrentNativeWindowHandle")
        })
    })?;
    if !native.0.is_null() {
        return Ok(native);
    }
    let parent = hwnd_from_window_handle(element.parent())?;
    if parent.0.is_null() {
        return Err(error::target_not_found(
            "key_action(Element): neither the element nor its resolved window carries a usable \
             window handle, so a foreground target cannot be verified",
        ));
    }
    Ok(parent)
}

/// 给元素设焦点并**回读确认**（`KeyTarget::Element` 的契约：「平台实现负责先确认焦点」）。
///
/// 为什么必须回读：`SetForegroundWindow` 只改 z 序，**不**保证焦点落在目标控件上 ——
/// `docs/memory/win32-input-research.md` §3 / §6 实测「前台已命中但 `GetFocus` 仍是 0」时
/// `SendKeys` 完全没送到。所以这里要求 UIA 明确报出「该元素有键盘焦点」。
///
/// # Errors
/// `SetFocus` / `CurrentHasKeyboardFocus` 的 HRESULT 失败 → 按原始分类；回读为假 → `TargetUnresponsive`。
fn focus_element(element: &ResolvedElement) -> PlatformResult<()> {
    with_element(element.id(), |element| {
        // SAFETY: UIA 调用；`SetFocus` 会改变焦点 —— 这正是本函数的目的（不是只读查询）。
        unsafe { element.SetFocus() }
            .map_err(|failure| error::error_from_hresult(failure.code().0, "SetFocus"))?;
        // SAFETY: 只读回读该元素的属性。
        let has_focus = unsafe { element.CurrentHasKeyboardFocus() }.map_err(|failure| {
            error::error_from_hresult(failure.code().0, "CurrentHasKeyboardFocus")
        })?;
        if !has_focus.as_bool() {
            return Err(error::target_unresponsive(
                "key_action(Element): UIA SetFocus reported success but the element does not \
                 report keyboard focus; refusing to send input to an unverified focus target",
            ));
        }
        Ok(())
    })
}

/// 一次 `SendInput` 调用 + **返回值校验**（铁律 1：返回值必须判，禁止 `let _ =`）。
///
/// # Errors
/// - 批次为空 / 过大 → `ToolInvalidArgs` / `CapabilityMissing`
/// - 插入数 ≠ 请求数 → [`classify_send_input_failure`]
fn send_inputs(inputs: &[INPUT], context: &str) -> PlatformResult<()> {
    let requested = u32::try_from(inputs.len()).map_err(|_| {
        error::invalid_args(format!(
            "{context}: too many events for a single SendInput batch"
        ))
    })?;
    if requested == 0 {
        return Err(error::invalid_args(format!(
            "{context}: nothing to send (an empty batch is not a success)"
        )));
    }
    let input_size = i32::try_from(std::mem::size_of::<INPUT>())
        .map_err(|_| error::capability_missing("INPUT size does not fit in i32"))?;
    // SAFETY: `inputs` 是本栈帧上的合法 `INPUT` 数组，`input_size` 是它的元素大小
    // （Win32 契约要求 `cbSize == sizeof(INPUT)`）；`SendInput` **同步**把事件插入输入队列，
    // 不保留该指针，因此本函数返回后 `inputs` 立即失效是安全的。
    let sent = unsafe { SendInput(inputs, input_size) };
    if sent == requested {
        return Ok(());
    }
    // SAFETY: 只读上一个 Win32 调用的错误码（只在 `SendInput` 未全量插入时才有意义）。
    let code = unsafe { GetLastError().0 };
    Err(classify_send_input_failure(code, sent, requested, context))
}

/// `SendInput` 返回值不符时的失败分类（**纯函数**，单测不需要真机）。
///
/// `SendInput` 返回的是**成功插入输入队列的事件数**，不是「目标处理了几个」：
/// - `ERROR_ACCESS_DENIED`(5) = UIPI 拦截（目标进程完整性级别更高）→ `PlatformPermission`
/// - `ERROR_INVALID_PARAMETER`(87) = 事件结构被拒 → `ToolInvalidArgs`
/// - `0` = 输入队列被阻塞（`BlockInput` / 安全桌面 / 别的线程）—— **不是**成功 → `TargetUnresponsive`
/// - 其他 → 交给 `crate::error::error_from_win32`（未识别的码一律 `Fatal`，**不**降级成 `Transient`）
fn classify_send_input_failure(
    code: u32,
    sent: u32,
    requested: u32,
    context: &str,
) -> PlatformError {
    match code {
        ERROR_ACCESS_DENIED => PlatformError::new(
            ErrorCode::PlatformPermission,
            format!(
                "{context}: SendInput inserted {sent} of {requested} events; access denied \
                 (Win32 {ERROR_ACCESS_DENIED}) — UIPI blocks synthetic input to a window of \
                 higher integrity level"
            ),
        ),
        ERROR_INVALID_PARAMETER => error::invalid_args(format!(
            "{context}: SendInput rejected the event structure (Win32 {ERROR_INVALID_PARAMETER}); \
             inserted {sent} of {requested}"
        )),
        0 => error::target_unresponsive(format!(
            "{context}: SendInput inserted {sent} of {requested} events and reported no error — \
             the input queue is blocked (BlockInput / secure desktop / another thread)"
        )),
        other => error::error_from_win32(other, context),
    }
}

/// 一个键盘事件 → `INPUT`（VK 路径）。
const fn key_input(event: KeyEvent) -> INPUT {
    let flags = if event.is_key_up {
        KEYEVENTF_KEYUP
    } else {
        // 0 = 没有标志（key-down）；不用 `Default::default()`：`const fn` 里不能调 trait 方法。
        KEYBD_EVENT_FLAGS(0)
    };
    keyboard_input(VIRTUAL_KEY(event.key.raw()), 0, flags)
}

/// 一个 Unicode 码元 → `INPUT`（`KEYEVENTF_UNICODE` 路径）。
const fn unicode_input(scan: u16, is_key_up: bool) -> INPUT {
    let flags = if is_key_up {
        // `KEYEVENTF_UNICODE | KEYEVENTF_KEYUP` 的语义；这里用裸值构造，因为 `windows`
        // 给标志类型实现的 `BitOr` **不是** `const fn`（`KEYBD_EVENT_FLAGS` 是
        // `repr(transparent)` 的 `u32` 包装，`.0` 就是原始位模式）。
        KEYBD_EVENT_FLAGS(KEYEVENTF_UNICODE.0 | KEYEVENTF_KEYUP.0)
    } else {
        KEYEVENTF_UNICODE
    };
    // Unicode 路径必须把 `wVk` 留 0（只有 `wScan` 有效）：否则会**同时**按下一个真实键，
    // 而那个键在当前布局下可能是一个快捷键。
    keyboard_input(VIRTUAL_KEY(0), scan, flags)
}

/// 组装 `INPUT { type: INPUT_KEYBOARD, ki: … }`。
const fn keyboard_input(key: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// 组装 `INPUT { type: INPUT_MOUSE, mi: … }`（`dx` / `dy` 只在 `MOUSEEVENTF_MOVE` 时有意义）。
const fn mouse_input(dx: i32, dy: i32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// 指针步骤 → 一次 `SendInput`（移动用**归一化绝对坐标** + `VIRTUALDESK`；点击用左键 down/up）。
///
/// # Errors
/// 归一化越界 → `TargetNotFound`（`VirtualScreen::normalize` 判）；`SendInput` 返回值不符 →
/// [`classify_send_input_failure`]。
fn send_pointer_steps(screen: &VirtualScreen, steps: &[PointerStep]) -> PlatformResult<()> {
    let mut inputs = Vec::with_capacity(steps.len());
    for step in steps {
        inputs.push(match step {
            PointerStep::MoveTo(point) => {
                // 归一化到 `0..=65535`（`MOUSEEVENTF_ABSOLUTE` 的契约）。
                let (dx, dy) = screen.normalize(*point)?;
                mouse_input(
                    dx,
                    dy,
                    MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                )
            }
            PointerStep::LeftDown => mouse_input(0, 0, MOUSEEVENTF_LEFTDOWN),
            PointerStep::LeftUp => mouse_input(0, 0, MOUSEEVENTF_LEFTUP),
        });
    }
    send_inputs(&inputs, "SendInput(pointer)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::VirtualKey;

    /// 测试夹具：取 `INPUT` 的键盘联合体成员（由本文件的构造器决定它就是 `ki`）。
    fn keyboard_part(input: &INPUT) -> KEYBDINPUT {
        // SAFETY: `input` 由本文件的 `keyboard_input` / `unicode_input` 构造，联合体成员必然是
        // `ki`；`KEYBDINPUT` 是 `Copy`，复制出来不改变原值。
        unsafe { input.Anonymous.ki }
    }

    #[test]
    fn test_send_input_failure_distinguishes_uipi_from_bad_parameters() {
        // DoD：UIPI（`ERROR_ACCESS_DENIED`）与参数错误（87）必须**可区分**。
        let uipi = classify_send_input_failure(ERROR_ACCESS_DENIED, 0, 4, "SendInput(keyboard)");
        assert_eq!(uipi.code(), ErrorCode::PlatformPermission);
        assert!(uipi.message().contains("UIPI"));

        let bad = classify_send_input_failure(ERROR_INVALID_PARAMETER, 1, 4, "SendInput(keyboard)");
        assert_eq!(bad.code(), ErrorCode::ToolInvalidArgs);
        assert!(bad.message().contains("87"));
    }

    #[test]
    fn test_send_input_blocked_without_error_code_is_not_success() {
        // 负向用例（ADR-0019 N1）：`sent == 0` 且 `GetLastError() == 0` **不得**当成成功
        // （文档说明那是「输入队列被阻塞」：`BlockInput` / 安全桌面 / 别的线程）。
        let blocked = classify_send_input_failure(0, 0, 4, "SendInput(pointer)");
        assert_eq!(blocked.code(), ErrorCode::TargetUnresponsive);
    }

    #[test]
    fn test_unknown_send_input_error_stays_fatal() {
        // 负向用例：未识别的码**不得**降级成可重试的 `Transient`（否则诱导无意义重试）。
        let unknown = classify_send_input_failure(9999, 0, 4, "SendInput(keyboard)");
        assert_eq!(unknown.code(), ErrorCode::Fatal);
        assert!(unknown.message().contains("9999"));
    }

    #[test]
    fn test_unicode_input_uses_scan_code_and_leaves_vk_empty() {
        // `KEYEVENTF_UNICODE` 路径的**结构**断言：`wVk` 必须留 0（否则会同时按下真实键），
        // 抬起事件必须同时带 `KEYEVENTF_UNICODE | KEYEVENTF_KEYUP`。
        let down = keyboard_part(&unicode_input(0x4E2D, false));
        let up = keyboard_part(&unicode_input(0x4E2D, true));
        assert_eq!(down.wVk.0, 0);
        assert_eq!(down.wScan, 0x4E2D);
        assert_eq!(down.dwFlags, KEYEVENTF_UNICODE);
        assert_eq!(up.wVk.0, 0);
        assert_eq!(up.wScan, 0x4E2D);
        assert_eq!(up.dwFlags, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
    }

    #[test]
    fn test_key_input_marks_only_key_up_events() {
        let down = keyboard_part(&key_input(KeyEvent {
            key: VirtualKey::new(0x41),
            is_key_up: false,
        }));
        let up = keyboard_part(&key_input(KeyEvent {
            key: VirtualKey::new(0x41),
            is_key_up: true,
        }));
        assert_eq!(down.wVk.0, 0x41);
        assert_eq!(down.wScan, 0);
        assert_eq!(down.dwFlags, KEYBD_EVENT_FLAGS(0));
        assert_eq!(up.wVk.0, 0x41);
        assert_eq!(up.dwFlags, KEYEVENTF_KEYUP);
    }

    #[test]
    fn test_send_inputs_rejects_an_empty_batch() {
        // 负向用例：空批次不是成功（`SendInput(0, …)` 返回 0，若当成成功就是静默失败）。
        let failure = match send_inputs(&[], "SendInput(test)") {
            Ok(()) => unreachable!("空批次必须报错"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
    }
}
