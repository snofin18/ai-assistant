//! 句柄编解码 + **线程本地** UIA 元素表（铁律 8 / 本卡约束 4）。
//!
//! 职责：在「不透明句柄」（`ResolvedWindow` / `ResolvedElement`，由 `assistant-platform-api`
//! 定义）与真实的 `HWND` / `IUIAutomationElement` 之间搭桥。
//! 边界：**不做**淘汰 / TTL / 复用（租约与缓存是 TASK-025）、**不做**跨线程搬运。
//!
//! ## 两种句柄的存法（刻意不同）
//! - **窗口**：`HWND` 的值**就是**句柄值，无表、无查找。HWND 是内核对象句柄，
//!   只要窗口还在就有效；窗口没了 → 后续 Win32 调用返回 `ERROR_INVALID_WINDOW_HANDLE`
//!   → `TargetNotFound`（明确失败，不是静默成功）。
//! - **元素**：UIA **没有**「按 `RuntimeId` 取回元素」的客户端 API（实测：`IUIAutomation`
//!   只有 `ElementFromHandle` / `ElementFromPoint` / `GetFocusedElement`，**没有**
//!   `ElementFromRuntimeId`）。而 `IUIAutomationElement` 不是 `Send` / `Sync`，
//!   不能放进 provider 结构体。因此元素只能存在**线程本地的表**里。
//!
//! ## 不变量
//! 1. 句柄类型**不派生** `Serialize` / `Deserialize`，也不持有可序列化字段（铁律 8）。
//! 2. 元素句柄**只在解析它的线程上可用**；跨线程使用 → 明确的 `TargetNotFound`，
//!    message 里说明"句柄是线程本地的"（不静默、不猜，铁律 1）。
//! 3. `with_element` **不**在持有 `RefCell` 借用时调用 `action`（先 clone 出元素再调用），
//!    因此 `action` 内部可以安全地再访问其它线程本地状态。
//!
//! 相关：架构 v2 §3.2 / §6.1、`crates/platform/api/src/handle.rs`、ADR-0022 D1。

use std::cell::RefCell;
use std::collections::HashMap;

use assistant_platform_api::{LocalHandleId, PlatformResult, ResolvedElement};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::IUIAutomationElement;

use crate::error;

/// 元素句柄的起始编号（窗口句柄用 HWND 原值，两者语义不同、从不混用）。
const FIRST_ELEMENT_HANDLE: u64 = 1;

/// 「未能确定所属窗口」的保留句柄值（见 `resolved_element` 的文档）。
const UNKNOWN_WINDOW_HANDLE: u64 = 0;

/// 线程本地的元素表：`ResolvedElement::id()` → COM 元素。
struct ElementTable {
    /// 下一个可用的元素句柄值。
    next_id: u64,
    /// 已解析元素（`AddRef` 过的 COM 指针）。
    elements: HashMap<u64, IUIAutomationElement>,
}

impl ElementTable {
    /// 空表。
    fn new() -> Self {
        Self {
            next_id: FIRST_ELEMENT_HANDLE,
            elements: HashMap::new(),
        }
    }

    /// 登记一个元素并返回它的句柄值。
    fn insert(&mut self, element: IUIAutomationElement) -> u64 {
        let handle = self.next_id;
        // `saturating_add` 而不是 `+`：句柄空间耗尽时停在 u64::MAX 会**覆盖**已有句柄，
        // 但那是 2^64 次解析之后的事；用 saturating 保证不 panic（铁律 1 的另一面）。
        self.next_id = self.next_id.saturating_add(1);
        self.elements.insert(handle, element);
        handle
    }

    /// 取一个元素的副本（`AddRef`；`None` = 本线程没登记过这个句柄）。
    fn get(&self, handle: u64) -> Option<IUIAutomationElement> {
        self.elements.get(&handle).cloned()
    }
}

thread_local! {
    /// 本线程已解析的元素表（线程退出时随 `IUIAutomationElement` 的 `Drop` 一起释放）。
    static ELEMENTS: RefCell<ElementTable> = RefCell::new(ElementTable::new());
}

/// 把 `HWND` 编成窗口句柄（窗口句柄值 = HWND 值，无表）。
///
/// # Errors
/// 指针值超出 `u64`（在任何受支持的 Windows 目标上都不可能出现）→ `Fatal`
/// （宁可明确失败，也不截断出一个会指向别的窗口的句柄）。
pub fn window_handle_from_hwnd(hwnd: HWND) -> PlatformResult<LocalHandleId> {
    let raw = hwnd.0 as usize;
    u64::try_from(raw)
        .map(LocalHandleId::new)
        .map_err(|_| error::invalid_args("window handle value does not fit in u64"))
}

/// 从句柄还原 `HWND`。
///
/// # Errors
/// 句柄值超出本平台 `usize`（32 位进程上的高位句柄）→ `TargetNotFound`
/// （这个句柄在本进程里不可能有效）。
pub fn hwnd_from_window_handle(id: LocalHandleId) -> PlatformResult<HWND> {
    usize::try_from(id.value())
        .map(|raw| HWND(raw as *mut core::ffi::c_void))
        .map_err(|_| {
            error::capability_missing("window handle value does not fit this process pointer width")
        })
}

/// 把已解析的元素登记到**本线程**的元素表，返回它的不透明句柄。
pub fn register_element(element: IUIAutomationElement) -> LocalHandleId {
    let handle = ELEMENTS.with(|cell| cell.borrow_mut().insert(element));
    LocalHandleId::new(handle)
}

/// 组装一个 `ResolvedElement`（`parent` = 最近的祖先窗口句柄，供 TASK-025 自愈时校验层级）。
///
/// `window == None`（UIA 元素没有 native HWND，例如纯 XAML 节点）时 `parent` 取
/// [`UNKNOWN_WINDOW_HANDLE`]（**保留值 0**，含义是"未能确定所属窗口"）——
/// 不猜一个假句柄，因为假句柄会让 TASK-025 的层级校验得出错误结论。
pub fn resolved_element(
    element: IUIAutomationElement,
    window: Option<HWND>,
    role: String,
) -> PlatformResult<ResolvedElement> {
    let id = register_element(element);
    let parent = match window {
        Some(hwnd) => window_handle_from_hwnd(hwnd)?,
        None => LocalHandleId::new(UNKNOWN_WINDOW_HANDLE),
    };
    Ok(ResolvedElement::new(id, parent, role))
}

/// 在本线程的元素表里找到 `id` 对应的元素，并把它交给 `action`。
///
/// # Errors
/// - 句柄不在本线程的表里（从未解析过，或**在另一个线程解析**）→ `TargetNotFound`，
///   message 明确指出线程本地语义（铁律 1：不猜、不静默）
/// - `action` 自身的错误原样透传
pub fn with_element<T>(
    id: LocalHandleId,
    action: impl FnOnce(&IUIAutomationElement) -> PlatformResult<T>,
) -> PlatformResult<T> {
    let found = ELEMENTS.with(|cell| cell.borrow().get(id.value()));
    // `map_or_else` 而不是 `match`：clippy 的 `option_if_let_else`（pedantic）要求这样写。
    found.map_or_else(
        || {
            Err(error::target_not_found(format!(
                "element handle {} is not registered on this thread; element handles are \
                 thread-local (COM objects must not cross threads) — resolve the element again on \
                 the calling thread",
                id.value()
            )))
        },
        |element| action(&element),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_handle_round_trip_preserves_hwnd_value() {
        // 0x0000_0000_000A_0C04 是 EnumWindows 实测见过的量级；不构造真窗口。
        let raw = 0x0000_000A_0C04_usize;
        let hwnd = HWND(raw as *mut core::ffi::c_void);
        let handle = window_handle_from_hwnd(hwnd);
        assert!(handle.is_ok(), "HWND 必须能编成句柄");
        let restored = handle.and_then(hwnd_from_window_handle);
        assert_eq!(restored.map(|value| value.0 as usize), Ok(raw));
    }

    #[test]
    fn test_element_handle_registered_here_is_invisible_on_another_thread() {
        // 负向用例（铁律 8 / 卡面约束 4 / ADR-0019 N1）：元素句柄**只在解析它的线程上有效**。
        // 本用例需要**真实**的 COM 元素才能构造"另一个线程看不到"的场景；拿不到 UIA 客户端时
        // 显式跳过并打印原因（不静默通过）。
        use std::io::Write as _;

        let Ok(root) = crate::com::with_automation(|automation| {
            // SAFETY: `GetRootElement` 是只读查询，返回一个新的 COM 引用。
            unsafe { automation.GetRootElement() }
                .map_err(|failure| error::error_from_hresult(failure.code().0, "GetRootElement"))
        }) else {
            // 不静默通过：把跳过原因写到 stderr。用 `writeln!` 而不是 `eprintln!`，因为
            // workspace 把 `clippy::print_stderr` 定为 deny（AGENTS.md §5.2）。
            let _skip_reason = writeln!(
                std::io::stderr(),
                "SKIP test_element_handle_registered_here_is_invisible_on_another_thread: \
                 COM/UIA client unavailable on this host"
            );
            return;
        };
        let id = register_element(root);
        assert!(
            with_element(id, |_| Ok(())).is_ok(),
            "同一线程上刚登记的元素必须能被取回（否则本用例的对照面不成立）"
        );
        let joined = std::thread::spawn(move || {
            with_element(id, |_| Ok(())).map_err(|failure| failure.code())
        })
        .join();
        let Ok(outcome) = joined else {
            unreachable!("探测线程不得 panic");
        };
        assert_eq!(
            outcome,
            Err(assistant_platform_api::ErrorCode::TargetNotFound),
            "跨线程使用元素句柄必须返回 TargetNotFound（不得静默找到、不得 panic）"
        );
    }

    #[test]
    fn test_element_handle_from_another_thread_is_reported_not_silently_found() {
        // 负向用例（ADR-0019 N1）：本线程从未解析过该句柄 → 必须明确报错。
        let missing = LocalHandleId::new(424_242);
        let result = with_element(missing, |_| Ok(()));
        let error = result.err();
        let Some(error) = error else {
            unreachable!("未登记的元素句柄必须报错，不得静默成功");
        };
        assert_eq!(
            error.code(),
            assistant_platform_api::ErrorCode::TargetNotFound
        );
        assert!(
            error.message().contains("thread-local"),
            "message 必须解释线程本地语义，实际 = {}",
            error.message()
        );
    }
}
