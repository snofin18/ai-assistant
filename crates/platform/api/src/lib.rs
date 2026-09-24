//! # assistant-platform-api —— 平台抽象层（**铁律 7 指定的唯一平台入口**）
//!
//! 职责：给 `core` / `Host` 提供**与平台无关**的类型与 trait，使上层代码永远不需要
//! `use windows::…` / `use objc2::…`。平台实现住在 `crates/platform/<os>`（TASK-017 起）。
//!
//! ## 边界（不做什么）
//! - 不实现任何平台（Win32 / UIA / AT-SPI / CDP 都不在这里）
//! - 不做策略判定（默认拒绝的放行点是 `crates/policy`，TASK-021）
//! - 不新增 `ErrorCode`（复用 `assistant_protocol::ErrorCode`；新增 = ADR）
//! - 不碰 IO：本 crate 是**纯类型 + trait 形状**，因此单测不需要真机
//!
//! ## 不变量
//! 1. **句柄不得跨进程**（铁律 8）：`ResolvedWindow` / `ResolvedElement` 不派生
//!    `Serialize` / `Deserialize`，也不持有平台对象（机器校验：`tests/handles_not_serializable.rs`）
//! 2. **错误必带 `ErrorCode`**（铁律 1）：`PlatformError` 三字段皆非空
//! 3. **能力矩阵单调升级**：`L3+ ⇒ required`、`L5 ⇒ forbidden`（`CapabilityMatrix::validate`）
//! 4. **坐标只有一个规范空间**：全局逻辑坐标、原点主显示器左上（架构 v2 §6.9 规则 1）
//! 5. **判定逻辑是纯函数**：`validate()` / 坐标换算 / 指纹比较都不碰 IO
//!
//! ## 典型用法
//!
//! trait 用 RPITIT，因此调用方写**泛型参数**而不是 `&dyn`（见 README「已知限制」）：
//!
//! ```no_run
//! use assistant_platform_api::{CapabilityMatrix, PlatformError, PlatformResult, PlatformService};
//!
//! async fn probe_and_check<S: PlatformService>(service: &S) -> PlatformResult<CapabilityMatrix> {
//!     let matrix = service.probe_capabilities().await?;
//!     // 写 `PlatformError::from` 而不是 `Into::into`：后者在 `?` 的目标类型已知前无法推断。
//!     matrix.validate().map_err(PlatformError::from)?;
//!     Ok(matrix)
//! }
//! ```
//!
//! 相关：架构 v2 §3.1 / §6.2 / §6.9 / §7.3 / §13.1.1 / §13.1.2、`docs/spec/capability-matrix.md`、
//! `docs/spec/naming.md` §7、`tasks/TASK-016-platform-api-trait-capability-matrix.md`。

#![deny(unsafe_code)]

mod capability;
mod error;
mod fingerprint;
mod geometry;
mod handle;
mod matrix;
mod target;
mod traits;

pub use assistant_protocol::ErrorCode;
pub use capability::{ApprovalRequirement, CapabilityEntry, ResourceAccess, RiskLevel, SideEffect};
pub use error::{PlatformError, PlatformResult};
pub use fingerprint::Fingerprint;
pub use geometry::{CoordinateSpace, CoordinateSpaceKind, NormalizedPoint, PhysicalPoint};
pub use handle::{LocalHandleId, ResolvedElement, ResolvedWindow};
pub use matrix::{
    CapabilityMatrix, CapabilityMatrixError, ChannelAvailability, ChannelState, Degradation,
    ProbeContext,
};
pub use target::{
    OnAmbiguous, OnNotFound, ResolutionPolicy, SelectorCandidate, SelectorKind, SelectorValue,
    TargetDescriptor,
};
pub use traits::{
    CaptureOptions, DisplayInfo, ElementQuery, ElementState, FingerprintScope, FocusPolicy,
    ImageRef, KeyChord, KeyModifier, KeyTarget, PlatformService, PointerAction, ScrollTarget,
    Selection, SessionState, TextEditOp, Timeout, TreeOptions, TreeSnapshot, UiAutomationProvider,
    WindowFilter, WindowInfo, WindowProvider, WindowState,
};
