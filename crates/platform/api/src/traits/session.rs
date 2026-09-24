//! 平台抽象 trait 与它们的参数/返回类型（架构 v2 §13.1.1）。
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

use std::future::Future;

use crate::error::PlatformResult;
use crate::matrix::CapabilityMatrix;

/// 显示器信息（架构 v2 §13.1.2 的 `session.displays`）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DisplayInfo {
    /// 显示器标识（如 `DP-1`）。
    id: String,
    /// 缩放（以**整数比值**表达，避免浮点比较；`2/1` = 2.0）。
    scale_numerator: u32,
    /// 缩放分母。
    scale_denominator: u32,
    /// 是否主显示器（坐标原点所在）。
    primary: bool,
}

impl DisplayInfo {
    /// 构造显示器信息。
    #[must_use]
    pub const fn new(
        id: String,
        scale_numerator: u32,
        scale_denominator: u32,
        primary: bool,
    ) -> Self {
        Self {
            id,
            scale_numerator,
            scale_denominator,
            primary,
        }
    }

    /// 显示器标识。
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 缩放（浮点形式；分母为 0 时按 1.0 处理，绝不 panic）。
    #[must_use]
    pub fn scale(&self) -> f64 {
        if self.scale_denominator == 0 {
            1.0
        } else {
            f64::from(self.scale_numerator) / f64::from(self.scale_denominator)
        }
    }

    /// 是否主显示器。
    #[must_use]
    pub const fn is_primary(&self) -> bool {
        self.primary
    }
}

/// 平台/会话健康状态（架构 v2 §13.1.1 的 `session_state`）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SessionState {
    /// 是否锁屏。
    locked: bool,
    /// 是否远程会话。
    remote: bool,
    /// 是否无头。
    headless: bool,
    /// 显示器列表。
    displays: Vec<DisplayInfo>,
}

impl SessionState {
    /// 构造会话状态。
    #[must_use]
    pub const fn new(
        locked: bool,
        remote: bool,
        headless: bool,
        displays: Vec<DisplayInfo>,
    ) -> Self {
        Self {
            locked,
            remote,
            headless,
            displays,
        }
    }

    /// 是否锁屏。
    #[must_use]
    pub const fn locked(&self) -> bool {
        self.locked
    }

    /// 是否远程会话。
    #[must_use]
    pub const fn remote(&self) -> bool {
        self.remote
    }

    /// 是否无头。
    #[must_use]
    pub const fn headless(&self) -> bool {
        self.headless
    }

    /// 显示器列表。
    #[must_use]
    pub fn displays(&self) -> &[DisplayInfo] {
        &self.displays
    }
}

/// 平台服务（会话级能力探测与健康检查）。
pub trait PlatformService: Send + Sync {
    /// 运行时能力探测（架构 v2 §13.1.2）。结果可缓存，但必须支持失效重探。
    ///
    /// # Errors
    /// 探测本身失败（如无障碍总线不可用）→ `PlatformPermission`；
    /// 探测到的矩阵自相矛盾 → `CapabilityMissing`（由 `CapabilityMatrix::validate` 判定）。
    fn probe_capabilities(&self) -> impl Future<Output = PlatformResult<CapabilityMatrix>> + Send;

    /// 平台/会话健康检查（是否锁屏、是否远程、显示是否可用）。
    ///
    /// # Errors
    /// 平台查询失败 → `PlatformPermission` / `Fatal`。
    fn session_state(&self) -> impl Future<Output = PlatformResult<SessionState>> + Send;
}
