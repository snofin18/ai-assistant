//! # assistant-platform-windows —— Windows 平台实现层（Win32 窗口 + UI Automation）
//!
//! 职责：把 `assistant-platform-api` 定义的两个 trait（`WindowProvider` 5 方法 +
//! `UiAutomationProvider` 13 方法）落到**真实的 Win32 / UIA3 客户端 COM** 上，
//! 使 `core` / Host 永远不需要 `use windows::…`（铁律 7）。
//! 本 crate 负责 `src/window/**`（窗口域）、`src/uia/**`（元素域）、
//! `src/cdp/**`（浏览器 CDP transport）与 `src/browser_launcher.rs`
//! （专用 profile 进程生命周期）。
//!
//! ## 边界（不做什么）
//! - **不做**合成输入（`SendInput` / `keybd_event` / `SendKeys`）与 IME → TASK-018
//! - **不做**坐标归一化 / DPI / 多屏换算 → TASK-018（`src/coordinates/**`）
//! - **不做**视觉验证 / OCR / 感知哈希兜底 → TASK-042；平台截图与像素遮挡见 ADR-0074
//! - **不启动生产浏览器**：launcher 只负责专用 profile 与进程树回收，真实 Edge
//!   验收由 TASK-056 的人类在场会话执行
//! - **不做**策略判定（放行点是 `crates/policy`，TASK-021）、**不做**租约与淘汰（TASK-025）、
//!   **不做**撤销（TASK-024）、**不做**后置断言引擎（TASK-023）
//! - **不缓存**：本 crate 没有 TTL / 淘汰 / 复用逻辑；句柄表只在**线程内**存活
//!
//! ## 不变量
//! 1. **句柄不跨进程**（铁律 8）：`ResolvedWindow` / `ResolvedElement` 由
//!    `assistant_platform_api::handle` 定义，**不派生** `Serialize` / `Deserialize`；
//!    本 crate 也**不新增**可序列化的句柄类型（`tests/handle_discipline.rs` 机器校验）。
//! 2. **COM 对象不跨线程**（本卡约束 4）：`IUIAutomation` 与 `IUIAutomationElement`
//!    都不是 `Send` / `Sync`（`windows` 0.62.2 的实测事实，见 `src/com.rs`），
//!    因此它们只存在于**线程本地**（`src/com.rs` 的 `APARTMENT`、`src/handles.rs` 的 `ELEMENTS`）。
//!    在另一个线程使用元素句柄会得到**明确错误**（不是静默失败，铁律 1）。
//! 3. **写操作必有 postcondition**（铁律 4）：`set_value` / `edit_text` / `invoke_action` /
//!    `select` / `scroll` 都回读验证；验证不通过返回 `VerifyFailed`，绝不返回 `Ok`。
//! 4. **未识别失败不得降级**（铁律 1）：未知 HRESULT / Win32 码 → `Fatal`，不伪装成 `Transient`。
//! 5. **纯函数优先**：selector 排序 / 歧义判定 / SHA-256 / 句柄编解码都是纯函数，
//!    三平台 CI 都能单测（不依赖真实应用，见 ADR-0019 N1）。
//!
//! ## 典型用法
//!
//! ```no_run
//! use assistant_platform_api::{UiAutomationProvider, WindowFilter, WindowProvider};
//! use assistant_platform_windows::WindowsPlatform;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let platform = WindowsPlatform::new();
//! let windows = platform.list_windows(&WindowFilter::for_app("notepad.exe")).await?;
//! # let _ = windows;
//! # Ok(())
//! # }
//! ```
//!
//! ## 关于 `unsafe_code`
//! 根 `Cargo.toml` 的 `[workspace.lints.rust] unsafe_code = "deny"` 是给 core / policy /
//! task-engine 的默认值；本 crate 是**铁律 7 指定的唯一 FFI 层**，调用 `windows` crate 的
//! 函数必须 `unsafe`。因此这里**crate 级**放开一次（而不是散落几十处 `#[allow]`），
//! 并且每个 `unsafe` 块都带 `// SAFETY:` 说明前置条件。取舍登记在
//! `tasks/TASK-017-platform-windows-uia-provider.md` §5（DRIFT-017-1）。
//!
//! 相关：架构 v2 §13.1.1 / §13.2 / §6.2 / §6.3 / §7.3 / §3.2、ADR-0022（Windows 目标身份）、
//! ADR-0024（`windows` crate 与 feature 名）、ADR-0019 N1（负向验证）、`docs/spec/naming.md` §7。
#![allow(unsafe_code)]

// `digest` 是纯函数（SHA-256）：三平台都编译并跑 FIPS 向量。
// `error` / `selector` 是 Windows 通道专属（HRESULT 映射 / 候选链裁决），非 Windows 不编译
// —— 否则它们的 `pub(crate)` 项在非 Windows 上不可达，会触发 `dead_code`（`-D warnings` 下是错误）。
#[cfg(any(windows, test))]
mod digest;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod selector;

#[cfg(windows)]
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod handles;
#[cfg(windows)]
mod uia;
#[cfg(windows)]
mod win32;
#[cfg(windows)]
mod window;

// 合成输入（`SendInput`）与坐标归一化（DPI / 多屏）—— TASK-018。
// **纯逻辑部分在所有平台编译**（ADR-0045 的非宿主 clippy 门禁要覆盖它们），真实的
// `SendInput` / 显示器枚举在各自的 `#[cfg(windows)] mod win32` 子模块里。
// 公开这两个模块是**有意的**：`send_unicode_text` / `is_ime_open` 是 TASK-018 的交付物，
// 而本 crate 的零 `#[allow]` 策略不允许用 `#[allow(dead_code)]` 掩盖「暂时没人调用」。
#[cfg(windows)]
pub mod browser_launcher;
pub mod cdp;
pub mod coordinates;
pub mod input;

#[cfg(not(windows))]
mod unsupported;

#[cfg(windows)]
pub use handles::clear_thread_elements;
#[cfg(windows)]
pub use window::WindowsPlatform;

#[cfg(not(windows))]
pub use unsupported::WindowsPlatform;

/// 声明本进程为 **Per-Monitor-V2 DPI 感知**（ADR-0092 细则 3）。
///
/// 为什么必须由**宿主**显式调用：DPI 感知是**进程级**状态，且必须在任何窗口创建 / DPI 查询
/// **之前**设置；库无法替宿主决定，也无法事后补上。
///
/// 不声明的后果有两条（一手证据见 `docs/memory/apps/paint.md` §12.5）：
/// 1. **坐标口径**：`GetDpiForMonitor` / UIA 都按 96 DPI 上报，`CoordinateSpace.scale` 恒为 1.0；
/// 2. **截图被截掉 3/4**：`GetWindowRect` 返回**虚拟化**尺寸，而 `PrintWindow` 按**物理**尺寸渲染
///    → `capture` 只拿到窗口左上角一块（缩放显示器上尤其明显）。
///
/// # Returns
/// - `Ok(true)`：本次成功设为 Per-Monitor-V2；
/// - `Ok(false)`：进程**已经**处于某个 DPI 感知模式（宿主 manifest 已声明 / 重复调用）——
///   保持现状即正确，**不是**失败；
///
/// # Errors
/// `SetProcessDpiAwarenessContext` 返回非 `ERROR_ACCESS_DENIED` 的系统失败（含 API 不可用）→
/// `CapabilityMissing`（调用方应显式处理，不得静默降级）。
#[cfg(windows)]
pub fn enable_per_monitor_v2_dpi_awareness() -> assistant_platform_api::PlatformResult<bool> {
    use windows::Win32::Foundation::E_ACCESSDENIED;
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };

    // SAFETY: 只设置进程级 DPI 感知状态；不转移所有权、不读写缓冲区。
    match unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) } {
        Ok(()) => Ok(true),
        // 已经设置过（含 manifest 声明）→ 保持现状。
        Err(failure) if failure.code() == E_ACCESSDENIED => Ok(false),
        Err(failure) => Err(assistant_platform_api::PlatformError::new(
            assistant_platform_api::ErrorCode::CapabilityMissing,
            format!("SetProcessDpiAwarenessContext(PerMonitorV2) failed: {failure}"),
        )),
    }
}

/// 非 Windows：没有 DPI 感知需要声明（保持调用方代码平台中立）。
///
/// # Errors
/// 恒不失败（返回 `Ok(false)`）；保留 `Result` 只为与 Windows 侧同签名。
#[cfg(not(windows))]
pub const fn enable_per_monitor_v2_dpi_awareness() -> assistant_platform_api::PlatformResult<bool> {
    Ok(false)
}
