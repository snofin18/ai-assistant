//! COM 单元（apartment）与 `IUIAutomation` 的**线程本地**持有（本卡约束 4 / 铁律 8）。
//!
//! 职责：给每个调用线程提供**它自己的** `IUIAutomation` 客户端实例，并严格配对
//! `CoInitializeEx` / `CoUninitialize`。
//! 边界：**不做**线程池调度、**不做**跨线程传递、**不做**实例复用（跨线程复用正是本卡禁止的）。
//!
//! ## 为什么必须线程本地（实测事实，不是设计偏好）
//! `windows` 0.62.2 把 COM 接口投影成裸 `NonNull<c_void>` 持有者，**不**实现 `Send` / `Sync`：
//!
//! ```text
//! error[E0277]: `NonNull<c_void>` cannot be shared between threads safely
//!   within `IUIAutomationElement` ... required because it appears within the type `IUnknown`
//! ```
//!
//! 而 `assistant_platform_api::UiAutomationProvider` 要求实现类型是 `Send + Sync`。
//! 因此本 crate **不能**把 `IUIAutomation` 放进 provider 结构体（那会让整个 provider 变成 `!Send`）。
//! 正确解法与卡面约束 4 一致：**实例住线程本地**，provider 结构体只持有可跨线程的纯数据。
//!
//! ## 不变量
//! 1. `CoInitializeEx` 成功（含 `S_FALSE`）后**必定**配对一次 `CoUninitialize`
//!    （MSDN 明确要求 `S_FALSE` 也要配平）—— 由 `ApartmentGuard::drop` 保证。
//! 2. `CoCreateInstance` 失败时**不**留下半初始化的槽位（`guard` 会被 `?` 提前 drop）。
//! 3. `with_automation` 的 `action` **不得**重入 `with_automation`（否则 `RefCell` 双借用 panic）；
//!    本 crate 的所有调用点都满足（每个入口只取一次实例）。
//!
//! 相关：架构 v2 §13.2、ADR-0024 D1a、`docs/memory/win32-input-research.md`。

use std::cell::RefCell;

use assistant_platform_api::PlatformResult;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

use crate::error;

/// 本线程的 COM apartment：UIA 客户端实例 + 它的释放责任。
struct ThreadApartment {
    /// 线程本地 UIA 客户端（**绝不**离开本线程）。
    automation: IUIAutomation,
    /// `CoUninitialize` 的配对释放器（只负责 drop 时机）。
    _guard: ApartmentGuard,
}

/// `CoInitializeEx` 的配对释放器。
///
/// 只在 `CoInitializeEx` **成功**时构造（`S_OK` 与 `S_FALSE` 都算成功，MSDN 要求两者都配平）。
struct ApartmentGuard;

impl ApartmentGuard {
    /// 初始化本线程的 COM apartment（单元线程模型）。
    ///
    /// # Errors
    /// - `RPC_E_CHANGED_MODE`（线程已按**另一个**模型初始化过）→ `Fatal`
    ///   （这是真实失败：本线程上永远拿不到 UIA 客户端，重试无意义）
    /// - 其它 HRESULT → 按 `error_from_hresult` 分类
    fn initialize() -> PlatformResult<Self> {
        // SAFETY: `CoInitializeEx` 只写线程局部状态；`None` 是 `pvReserved` 的合法取值
        // （MSDN 要求它必须为 NULL），没有指针跨界。
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.is_err() {
            return Err(error::error_from_hresult(
                result.0,
                "CoInitializeEx(COINIT_APARTMENTTHREADED)",
            ));
        }
        Ok(Self)
    }
}

impl Drop for ApartmentGuard {
    fn drop(&mut self) {
        // SAFETY: 本 guard 只在 `CoInitializeEx` 成功后构造，且每线程至多一个
        // （它住在 `APARTMENT` 的 `Option` 槽位里），因此这次 `CoUninitialize`
        // 与那次成功的 `CoInitializeEx` 严格一一配对。
        unsafe { CoUninitialize() };
    }
}

thread_local! {
    /// 每个线程自己的 COM apartment 与 UIA 客户端实例（惰性创建，线程退出时释放）。
    static APARTMENT: RefCell<Option<ThreadApartment>> = const { RefCell::new(None) };
}

/// 在**本线程**的 COM apartment 内执行 `action`，必要时惰性初始化。
///
/// 所有需要 UIA 的入口都必须经此函数取实例：它是「COM 对象不跨线程」这条不变量的**唯一**守卫。
///
/// # Errors
/// - COM 初始化或 `CoCreateInstance(CLSID_CUIAutomation)` 失败 → `Fatal` / `PlatformPermission`
///   （见 `error::error_from_hresult`）
/// - `action` 自身的错误原样透传
pub fn with_automation<T>(
    action: impl FnOnce(&IUIAutomation) -> PlatformResult<T>,
) -> PlatformResult<T> {
    APARTMENT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            // `?` 提前返回时 `guard` 被 drop → 立刻 `CoUninitialize`，不留下半初始化的槽位。
            let guard = ApartmentGuard::initialize()?;
            // SAFETY: `CUIAutomation` 是已注册的进程内 COM 服务器（Both 模型），
            // `CLSCTX_INPROC_SERVER` 与 `punkOuter = None`（非聚合）都是它的合法用法；
            // 上一行的 `CoInitializeEx` 已保证本线程有可用 apartment。
            let automation =
                unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(
                    |failure| {
                        error::error_from_hresult(
                            failure.code().0,
                            "CoCreateInstance(CLSID_CUIAutomation)",
                        )
                    },
                )?;
            *slot = Some(ThreadApartment {
                automation,
                _guard: guard,
            });
        }
        // `map_or_else` 而不是 `match`：clippy 的 `option_if_let_else`（pedantic）要求这样写。
        slot.as_ref().map_or_else(
            // 不可达（上面刚保证 Some），但绝不 `unwrap`（workspace lint 禁止）：
            // 真出现就返回明确错误，而不是 panic 或静默成功。
            || {
                Err(error::capability_missing(
                    "COM apartment is unavailable on this thread",
                ))
            },
            |apartment| action(&apartment.automation),
        )
    })
}
