//! # assistant-platform-windows —— Windows 平台实现层（Win32 窗口 + UI Automation）
//!
//! 职责：把 `assistant-platform-api` 定义的两个 trait（`WindowProvider` 5 方法 +
//! `UiAutomationProvider` 13 方法）落到**真实的 Win32 / UIA3 客户端 COM** 上，
//! 使 `core` / Host 永远不需要 `use windows::…`（铁律 7）。
//! 本 crate **只做** `src/window/**`（窗口域）与 `src/uia/**`（元素域）。
//!
//! ## 边界（不做什么）
//! - **不做**合成输入（`SendInput` / `keybd_event` / `SendKeys`）与 IME → TASK-018
//! - **不做**坐标归一化 / DPI / 多屏换算 → TASK-018（`src/coordinates/**`）
//! - **不做**截图 / 脱敏 / 视觉兜底 → TASK-041 / 042（`capture` 本卡显式报错）
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
mod com;
#[cfg(windows)]
mod handles;
#[cfg(windows)]
mod uia;
#[cfg(windows)]
mod win32;
#[cfg(windows)]
mod window;

#[cfg(not(windows))]
mod unsupported;

#[cfg(windows)]
pub use window::WindowsPlatform;

#[cfg(not(windows))]
pub use unsupported::WindowsPlatform;
