//! COM 单元（apartment）与 `IUIAutomation` 的**线程本地**持有（本卡约束 4 / 铁律 8）。
//!
//! 职责：提供线程内、确定释放的 STA apartment scope，并给每个调用线程提供
//! **它自己的** `IUIAutomation` 客户端实例，严格配对 `CoInitializeEx` /
//! `CoUninitialize`。
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
//!    （MSDN 明确要求 `S_FALSE` 也要配平）—— 由 `ComApartmentGuard::drop` 保证。
//! 2. `CoCreateInstance` 失败时**不**留下半初始化的槽位（`guard` 会被 `?` 提前 drop）。
//! 3. `with_automation` 的 `action` **不得**重入 `with_automation`（否则 `RefCell` 双借用 panic）；
//!    本 crate 的所有调用点都满足（每个入口只取一次实例）。
//!
//! 相关：架构 v2 §13.2、ADR-0024 D1a、`docs/memory/win32-input-research.md`。

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use assistant_platform_api::PlatformResult;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

use crate::error;

/// Owns one COM apartment guard together with a value created under that apartment.
///
/// The value is dropped before the guard because fields drop in declaration order.
pub struct ComApartment<T> {
    value: T,
    /// `CoUninitialize` 的配对释放器（只负责 drop 时机）。
    _guard: ComApartmentGuard,
}

impl<T> ComApartment<T> {
    const fn value(&self) -> &T {
        &self.value
    }
}

/// `CoInitializeEx` 的配对释放器。
///
/// 只在 `CoInitializeEx` **成功**时构造（`S_OK` 与 `S_FALSE` 都算成功，MSDN 要求两者都配平）。
pub struct ComApartmentGuard {
    /// `Rc` is neither `Send` nor `Sync`, so the compiler rejects cross-thread guard moves.
    _thread_bound: PhantomData<Rc<()>>,
}

impl ComApartmentGuard {
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
        Ok(Self {
            _thread_bound: PhantomData,
        })
    }
}

impl Drop for ComApartmentGuard {
    fn drop(&mut self) {
        // SAFETY: 本 guard 只在一次 `CoInitializeEx` 成功后构造；每个实例在 Drop 时
        // 恰好执行一次 `CoUninitialize`，与创建它的那次成功调用严格一一配对。
        unsafe { CoUninitialize() };
    }
}

/// Runs one single-threaded apartment scope and couples its lifetime to the action result.
///
/// The guard is borrowed while `action` runs, so it cannot escape by itself. On success, the
/// returned [`ComApartment`] owns both the value and its guard; dropping that value balances
/// `CoUninitialize`. Error and panic paths drop the guard before this function returns.
///
/// # Errors
/// - The thread is already initialized with another apartment model → fail closed.
/// - Other HRESULTs are mapped through [`error::error_from_hresult`].
/// - `action` errors pass through unchanged.
pub fn with_com_apartment<T>(
    action: impl FnOnce(&ComApartmentGuard) -> PlatformResult<T>,
) -> PlatformResult<ComApartment<T>> {
    let guard = ComApartmentGuard::initialize()?;
    let value = action(&guard)?;
    Ok(ComApartment {
        value,
        _guard: guard,
    })
}

thread_local! {
    /// 每个线程自己的 COM apartment 与 UIA 客户端实例（惰性创建，线程退出时释放）。
    static APARTMENT: RefCell<Option<ComApartment<IUIAutomation>>> = const { RefCell::new(None) };
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
            *slot = Some(with_com_apartment(|_guard| {
                // SAFETY: `CUIAutomation` is a registered in-process COM server (Both
                // threading model); `CLSCTX_INPROC_SERVER` and `punkOuter = None` are valid.
                // The guard has already initialized an STA on this thread.
                unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(
                    |failure| {
                        error::error_from_hresult(
                            failure.code().0,
                            "CoCreateInstance(CLSID_CUIAutomation)",
                        )
                    },
                )
            })?);
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
            |apartment| action(apartment.value()),
        )
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use std::panic::catch_unwind;

    use assistant_platform_api::PlatformResult;
    use windows::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize,
    };

    use super::with_com_apartment;

    fn assert_multithreaded_apartment_can_initialize() {
        // SAFETY: the helper runs on a dedicated test thread after the guarded STA scope ended.
        // It balances the successful initialization immediately.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        assert!(result.is_ok(), "MTA must be available: {result:?}");
        // SAFETY: paired with the successful initialization immediately above.
        unsafe { CoUninitialize() };
    }

    #[test]
    fn test_with_com_apartment_releases_after_success() {
        std::thread::spawn(|| {
            with_com_apartment(|_guard| Ok(())).expect("STA scope");
            assert_multithreaded_apartment_can_initialize();
        })
        .join()
        .expect("success-path thread");
    }

    #[test]
    fn test_with_com_apartment_balances_s_false_for_existing_sta() {
        std::thread::spawn(|| {
            // SAFETY: dedicated test thread; this outer initialization is balanced below.
            let outer = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            assert!(outer.is_ok(), "outer STA setup: {outer:?}");

            // The nested `CoInitializeEx` returns S_FALSE; the scope must balance that call.
            with_com_apartment(|_guard| Ok(())).expect("nested STA scope");

            // SAFETY: paired with the outer initialization above. If the nested S_FALSE was not
            // balanced, one apartment count remains and the MTA initialization below fails.
            unsafe { CoUninitialize() };
            assert_multithreaded_apartment_can_initialize();
        })
        .join()
        .expect("S_FALSE-path thread");
    }

    #[test]
    fn test_with_com_apartment_releases_after_error() {
        std::thread::spawn(|| {
            let error = with_com_apartment(|_guard| {
                Err::<(), _>(crate::error::capability_missing("expected closure error"))
            })
            .err()
            .expect("closure error must pass through");
            assert!(error.to_string().contains("expected closure error"));
            assert_multithreaded_apartment_can_initialize();
        })
        .join()
        .expect("error-path thread");
    }

    #[test]
    fn test_with_com_apartment_releases_after_panic() {
        std::thread::spawn(|| {
            let panic = catch_unwind(|| {
                let _ = with_com_apartment(|_guard| -> PlatformResult<()> {
                    panic!("expected apartment panic");
                });
            });
            assert!(panic.is_err(), "closure must panic");
            assert_multithreaded_apartment_can_initialize();
        })
        .join()
        .expect("panic-path thread");
    }

    #[test]
    fn test_with_com_apartment_rejects_thread_initialized_as_mta() {
        std::thread::spawn(|| {
            // SAFETY: dedicated test thread; paired immediately after the assertion.
            let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            assert!(initialized.is_ok(), "MTA setup: {initialized:?}");
            let error = with_com_apartment(|_guard| Ok(()))
                .err()
                .expect("STA scope must reject an MTA-initialized thread");
            assert!(
                error.to_string().contains("different COM apartment model"),
                "unexpected error: {error}"
            );
            // SAFETY: paired with this test's successful MTA initialization.
            unsafe { CoUninitialize() };
        })
        .join()
        .expect("wrong-apartment thread");
    }
}
