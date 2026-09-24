//! 非 Windows 平台的后端：**每个方法都返回明确的 `CapabilityMissing`**（铁律 1 + 本卡 `DoD`）。
//!
//! 职责：让 `crates/platform/windows` 在 macOS / Linux 的 CI runner 上**能编译、能跑测试**，
//! 并让误在非 Windows 上调用它的代码得到**可解释的失败**，而不是链接错误或 panic。
//! 边界：**不做**任何平台模拟、**不返回**任何"看起来成功"的默认值（铁律 1：禁止用默认值冒充结果）。
//!
//! ## 为什么不用 `#[cfg]` 把整个 crate 排除掉
//! ① CI 的三平台矩阵会跑 `cargo test --workspace`，crate 被 workspace `members` 引用，
//!    没有 lib target 会直接失败；② 铁律 7 要求平台实现住在 `crates/platform/*`，
//!    上层若在非 Windows 上引用它，应当在**运行期**得到 `CapabilityMissing`（可被策略引擎
//!    转成"本机不支持该能力"），而不是编译期炸掉整个 workspace。
//!
//! ## 为什么**不**用 `todo!()` / `unimplemented!()`
//! 两者都会 panic；本 crate 的 workspace lint 把 `todo` / `unimplemented` 设为 **deny**，
//! 而且 panic 是"没有 `ErrorCode` 的失败"，违反铁律 1。
//!
//! ## 为什么 impl 写 `fn -> impl Future + Send` 而不是 `async fn`
//! `crates/platform/api` 的 trait 用 RPITIT 声明（`-> impl Future<Output = …> + Send`，
//! 理由见 `traits/mod.rs` 的「为什么用 RPITIT」）。impl 侧若写 `async fn` 而函数体里
//! **没有 `.await`**（本文件全部如此 —— 每个方法只是立刻返回 `Err`），会触发
//! `clippy::unused_async_trait_impl`（pedantic + CI 的 `-D warnings` = 错误），
//! 压住它只能加 `#[allow]`（漂移触发器 ⑥ 禁止）。
//! 因此这里与同 crate 的 `window/mod.rs` / `uia/mod.rs` **保持同一形状**：显式返回
//! `std::future::ready(…)`（与那两处的 `poll_fn(|_| Poll::Ready(…))` 语义等价，只是更短），
//! 既如实表达"立即就绪"，也不需要任何 `#[allow]`。
//!
//! 相关：`crates/platform/api/src/traits/**`、AGENTS.md 铁律 1 / 7、`tasks/TASK-017-*.md` Q2/Q3。

use std::future::Future;

use assistant_platform_api::{
    CapabilityMatrix, CaptureOptions, ElementQuery, ElementState, ErrorCode, Fingerprint,
    FingerprintScope, FocusPolicy, ImageRef, KeyChord, KeyTarget, NormalizedPoint, PlatformError,
    PlatformResult, PointerAction, ResolvedElement, ResolvedWindow, ScrollTarget, Selection,
    SelectorChain, SessionState, TextEditOp, Timeout, TreeOptions, TreeSnapshot,
    UiAutomationProvider, WindowFilter, WindowInfo, WindowProvider, WindowState,
};

/// 非 Windows 平台上的 `WindowsPlatform`。
///
/// 它的**每一个**方法都返回 `CapabilityMissing`（语义 = 本通道在当前平台不可用），
/// 因此上层可以按错误码统一降级，而不需要 `#[cfg]` 分叉。
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    /// 构造平台实现（零状态、无副作用）。
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// 「本通道只在 Windows 上可用」的标准错误。
fn requires_windows(operation: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::CapabilityMissing,
        format!(
            "{operation} requires the Windows platform channel (Win32 / UI Automation); \
             this build targets a non-Windows OS"
        ),
    )
}

/// `resolve_window` 的**平台无关前半段**。
///
/// 描述自身的校验是**平台无关**的纯逻辑：坏输入在哪个平台都该得到 `ToolInvalidArgs`，
/// 而不是被"平台不可用"掩盖（否则调用方会去排查平台而不是修描述）。
fn resolve_window_outcome(
    descriptor: &assistant_platform_api::TargetDescriptor,
) -> PlatformResult<ResolvedWindow> {
    descriptor.validate()?;
    Err(requires_windows("WindowProvider::resolve_window"))
}

/// `resolve_element` 的**平台无关前半段**：空链在任何平台都是 `ToolInvalidArgs`
/// （与 Windows 后端同一判据）。
fn resolve_element_outcome(chain: &SelectorChain) -> PlatformResult<ResolvedElement> {
    if chain.candidates().is_empty() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "resolve_element: selector chain is empty (an empty chain can never resolve)",
        ));
    }
    Err(requires_windows("UiAutomationProvider::resolve_element"))
}

/// `wait_for` 的**平台无关前半段**：既无 `automation_id` 也无 `role` 的查询在任何平台都是
/// `ToolInvalidArgs`（与 Windows 后端同一判据）。
fn wait_for_outcome(query: &ElementQuery) -> PlatformResult<ResolvedElement> {
    if query.automation_id().is_none() && query.role().is_none() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "wait_for: query must specify automation_id and/or role",
        ));
    }
    Err(requires_windows("UiAutomationProvider::wait_for"))
}

impl WindowProvider for WindowsPlatform {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        std::future::ready(Err(requires_windows("WindowProvider::list_windows")))
    }

    fn resolve_window(
        &self,
        descriptor: &assistant_platform_api::TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        std::future::ready(resolve_window_outcome(descriptor))
    }

    fn window_state(
        &self,
        _window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send {
        std::future::ready(Err(requires_windows("WindowProvider::window_state")))
    }

    fn bring_to_front(
        &self,
        _window: &ResolvedWindow,
        _policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("WindowProvider::bring_to_front")))
    }

    fn capture(
        &self,
        _window: &ResolvedWindow,
        _options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send {
        std::future::ready(Err(requires_windows("WindowProvider::capture")))
    }
}

impl UiAutomationProvider for WindowsPlatform {
    fn snapshot_tree(
        &self,
        _root: &ResolvedWindow,
        _options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::snapshot_tree")))
    }

    fn resolve_element(
        &self,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        std::future::ready(resolve_element_outcome(chain))
    }

    fn wait_for(
        &self,
        query: &ElementQuery,
        _state: &ElementState,
        _timeout: Timeout,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        std::future::ready(wait_for_outcome(query))
    }

    fn read_text(
        &self,
        _element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::read_text")))
    }

    fn set_value(
        &self,
        _element: &ResolvedElement,
        _value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::set_value")))
    }

    fn edit_text(
        &self,
        _element: &ResolvedElement,
        _operation: &TextEditOp,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::edit_text")))
    }

    fn invoke_action(
        &self,
        _element: &ResolvedElement,
        _action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::invoke_action")))
    }

    fn select(
        &self,
        _element: &ResolvedElement,
        _selection: &Selection,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::select")))
    }

    fn scroll(
        &self,
        _element: &ResolvedElement,
        _target: &ScrollTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::scroll")))
    }

    fn pointer_action(
        &self,
        _point: NormalizedPoint,
        _action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows(
            "UiAutomationProvider::pointer_action",
        )))
    }

    fn key_action(
        &self,
        _chord: &KeyChord,
        _target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::key_action")))
    }

    fn fingerprint(
        &self,
        _window: &ResolvedWindow,
        _scope: &FingerprintScope,
    ) -> impl Future<Output = PlatformResult<Fingerprint>> + Send {
        std::future::ready(Err(requires_windows("UiAutomationProvider::fingerprint")))
    }
}

/// `PlatformService` 在非 Windows 上同样只报"通道不可用"（不返回空矩阵 —— 空矩阵会被
/// 下游误读成"什么都不能做"而不是"没探测"）。
impl assistant_platform_api::PlatformService for WindowsPlatform {
    fn probe_capabilities(&self) -> impl Future<Output = PlatformResult<CapabilityMatrix>> + Send {
        std::future::ready(Err(requires_windows("PlatformService::probe_capabilities")))
    }

    fn session_state(&self) -> impl Future<Output = PlatformResult<SessionState>> + Send {
        std::future::ready(Err(requires_windows("PlatformService::session_state")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 负向用例（ADR-0019 N1）：非 Windows 后端**必须**报 `CapabilityMissing`，
    /// 而不是 panic、也不是返回一个"看起来成功"的空值。
    #[test]
    fn test_every_method_reports_capability_missing_instead_of_panicking() {
        let platform = WindowsPlatform::new();
        let window = ResolvedWindow::new(assistant_platform_api::LocalHandleId::new(1), "x".into());
        let element = ResolvedElement::new(
            assistant_platform_api::LocalHandleId::new(2),
            assistant_platform_api::LocalHandleId::new(1),
            "Document".into(),
        );
        let checks: [PlatformResult<()>; 6] = [
            futures_block_on(WindowProvider::window_state(&platform, &window)).map(|_| ()),
            futures_block_on(WindowProvider::bring_to_front(
                &platform,
                &window,
                FocusPolicy::AllowSteal,
            )),
            futures_block_on(UiAutomationProvider::read_text(&platform, &element)).map(|_| ()),
            futures_block_on(UiAutomationProvider::set_value(&platform, &element, "x")),
            futures_block_on(UiAutomationProvider::invoke_action(
                &platform, &element, "invoke",
            )),
            futures_block_on(UiAutomationProvider::fingerprint(
                &platform,
                &window,
                &FingerprintScope::WholeWindow,
            ))
            .map(|_| ()),
        ];
        for outcome in checks {
            assert_eq!(
                outcome.map_err(|error| error.code()),
                Err(ErrorCode::CapabilityMissing)
            );
        }
    }

    /// 坏描述在非 Windows 上仍然得到 `ToolInvalidArgs`（输入校验与平台无关）。
    #[test]
    fn test_invalid_descriptor_still_reports_invalid_args() {
        use assistant_platform_api::{OnAmbiguous, OnNotFound, ResolutionPolicy, TargetDescriptor};
        let Ok(policy) = ResolutionPolicy::new(
            OnAmbiguous::ErrorAndAsk,
            OnNotFound::new(Vec::new(), false),
            1_000,
            0.0,
        ) else {
            unreachable!("测试夹具必须合法");
        };
        // 空窗口候选链 → `validate()` 必报 `ToolInvalidArgs`。
        let descriptor = TargetDescriptor::new(
            "2.0".into(),
            "com.example.app".into(),
            Vec::new(),
            Vec::new(),
            policy,
        );
        let code = futures_block_on(WindowProvider::resolve_window(
            &WindowsPlatform::new(),
            &descriptor,
        ))
        .map_err(|error| error.code());
        assert_eq!(code, Err(ErrorCode::ToolInvalidArgs));
    }

    /// 极简 executor：本 crate 不依赖 async runtime，测试只需要把"同步返回的 future"跑完。
    fn futures_block_on<T>(future: impl std::future::Future<Output = T>) -> T {
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
}
