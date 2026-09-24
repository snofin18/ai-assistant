//! 平台抽象 trait 与它们的参数/返回类型（架构 v2 §13.1.1）。
//!
//! 本模块按**消费面**分成三个子模块，避免单文件过长（gov §5.4 的 600 行硬上限）：
//! - [`session`]：会话级（显示器 / 会话状态 / `PlatformService`）
//! - [`window`]：窗口级（枚举 / 解析 / 状态 / 前台 / 截图 / `WindowProvider`）
//! - [`ui`]：元素级（树 / 动作 / 合成输入 / 指纹 / `UiAutomationProvider`）
//!
//! ## 为什么用 RPITIT 而不是 `async fn` in trait
//! `async fn` in trait 会触发 `async_fn_in_trait` lint（"public trait 的 auto trait bound 无法声明"），
//! 在 `-D warnings` 下是**错误**；要压住它只能用 `#[allow]`（漂移触发器 ⑥ 禁止）。
//! 因此这里写 `-> impl Future<Output = ...> + Send`（RPITIT，Rust 1.75 起稳定）：
//! **零 `#[allow]`、零第三方依赖**，并且显式要求 future 是 `Send`。
//!
//! **代价**：这些 trait **不是 `dyn`-compatible**。`core` 侧应写成泛型参数（`impl PlatformService`）
//! 或把具体类型当泛型实参传入。若 TASK-017 确需 `dyn`，届时按需裁决（仍不引依赖）。
//!
//! ## 不变量
//! 1. 每个方法都**可取消、都带超时**（§13.1.1）—— 超时参数在本层显式出现，不靠平台默认值。
//! 2. **`set_value` / `edit_text` / `invoke_action` 优先于 `pointer_action` / `key_action`**
//!    （前者不依赖焦点、不抢用户输入、跨平台语义一致）—— 这条是**调用方**的义务。
//! 3. `fingerprint` 是**一等接口**，不是可选装饰（§7.3）。
//! 4. 所有方法返回 `PlatformResult`（铁律 1：不得静默失败）。

mod session;
mod ui;
mod window;

pub use session::{DisplayInfo, PlatformService, SessionState};
pub use ui::{
    ElementQuery, ElementState, FingerprintScope, KeyChord, KeyModifier, KeyTarget, PointerAction,
    ScrollTarget, Selection, SelectorChain, TextEditOp, Timeout, TreeOptions, TreeSnapshot,
    UiAutomationProvider,
};
pub use window::{
    CaptureOptions, FocusPolicy, ImageRef, WindowFilter, WindowInfo, WindowProvider, WindowState,
};
