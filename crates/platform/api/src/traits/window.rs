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
use crate::handle::ResolvedWindow;
use crate::target::TargetDescriptor;

/// 窗口过滤条件（架构 v2 §13.1.1 的 `WindowFilter`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct WindowFilter {
    /// 只看某个应用。
    app_id: Option<String>,
    /// 标题正则（**本地化相关**，只作辅助过滤）。
    title_regex: Option<String>,
}

impl WindowFilter {
    /// 无过滤（列出全部）。
    #[must_use]
    pub const fn any() -> Self {
        Self {
            app_id: None,
            title_regex: None,
        }
    }

    /// 按应用过滤。
    #[must_use]
    pub fn for_app(app_id: impl Into<String>) -> Self {
        Self {
            app_id: Some(app_id.into()),
            title_regex: None,
        }
    }

    /// 应用过滤条件。
    #[must_use]
    pub fn app_id(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    /// 标题正则过滤条件。
    #[must_use]
    pub fn title_regex(&self) -> Option<&str> {
        self.title_regex.as_deref()
    }
}

/// 窗口信息（架构 v2 §13.1.1 的 `WindowInfo`）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WindowInfo {
    /// 已解析的窗口句柄。
    window: ResolvedWindow,
    /// 所属应用。
    app_id: String,
    /// 标题（**仅供展示**；不得作 selector，ADR-0022 D4）。
    title: String,
}

impl WindowInfo {
    /// 构造窗口信息。
    #[must_use]
    pub const fn new(window: ResolvedWindow, app_id: String, title: String) -> Self {
        Self {
            window,
            app_id,
            title,
        }
    }

    /// 窗口句柄。
    #[must_use]
    pub const fn window(&self) -> &ResolvedWindow {
        &self.window
    }

    /// 所属应用。
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// 标题（仅供展示）。
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
}

/// 窗口状态（最小化 / 前台 / 遮挡）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WindowState {
    /// 是否最小化（最小化时坐标无效，§6.9）。
    minimized: bool,
    /// 是否前台。
    foreground: bool,
    /// 是否被遮挡。
    occluded: bool,
}

impl WindowState {
    /// 构造窗口状态。
    #[must_use]
    pub const fn new(minimized: bool, foreground: bool, occluded: bool) -> Self {
        Self {
            minimized,
            foreground,
            occluded,
        }
    }

    /// 是否最小化。
    #[must_use]
    pub const fn minimized(&self) -> bool {
        self.minimized
    }

    /// 是否前台。
    #[must_use]
    pub const fn foreground(&self) -> bool {
        self.foreground
    }

    /// 是否被遮挡。
    #[must_use]
    pub const fn occluded(&self) -> bool {
        self.occluded
    }
}

/// 抢焦点策略（架构 v2 §13.1.1 的 `FocusPolicy`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FocusPolicy {
    /// 允许抢焦点（用户在场且已授权）。
    AllowSteal,
    /// 需要用户确认后才抢。
    RequireUserConsent,
    /// 永不抢焦点（无人值守的默认）。
    NeverSteal,
}

/// 截图选项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CaptureOptions {
    /// 是否脱敏（密码框 / 正则命中区域遮挡）。
    redact: bool,
}

impl CaptureOptions {
    /// 构造截图选项。
    #[must_use]
    pub const fn new(redact: bool) -> Self {
        Self { redact }
    }

    /// 是否脱敏。
    #[must_use]
    pub const fn redact(&self) -> bool {
        self.redact
    }
}

/// 截图引用（**不是**像素本身：像素走内容寻址 blob，架构 v2 §15.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImageRef {
    /// blob 标识。
    blob_id: String,
    /// 宽（像素）。
    width: u32,
    /// 高（像素）。
    height: u32,
}

impl ImageRef {
    /// 构造截图引用。
    #[must_use]
    pub const fn new(blob_id: String, width: u32, height: u32) -> Self {
        Self {
            blob_id,
            width,
            height,
        }
    }

    /// blob 标识。
    #[must_use]
    pub fn blob_id(&self) -> &str {
        &self.blob_id
    }

    /// 宽。
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// 高。
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
}

/// 窗口提供者（枚举 / 解析 / 状态 / 前台 / 截图）。
pub trait WindowProvider: Send + Sync {
    /// 枚举窗口。
    ///
    /// # Errors
    /// 枚举通道不可用 → `PlatformPermission`（如 Wayland 无全局窗口列表）。
    fn list_windows(
        &self,
        filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send;

    /// 按描述解析窗口（**不做**视觉兜底 —— 兜底是调用方的策略）。
    ///
    /// # Errors
    /// 全部候选失败 → `TargetNotFound`；多匹配且策略是 `ErrorAndAsk` → `TargetAmbiguous`；
    /// 描述自身不合法 → `ToolInvalidArgs`（先调 `TargetDescriptor::validate`）。
    fn resolve_window(
        &self,
        descriptor: &TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send;

    /// 查询窗口状态。
    ///
    /// # Errors
    /// 句柄已失效（窗口已关闭）→ `TargetNotFound`。
    fn window_state(
        &self,
        window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send;

    /// 把窗口带到前台（**受策略约束**：`NeverSteal` 时不得抢焦点）。
    ///
    /// # Errors
    /// 策略禁止 → `PolicyDenied`；平台拒绝（如 Windows 前台锁定）→ `TargetUnresponsive`。
    fn bring_to_front(
        &self,
        window: &ResolvedWindow,
        policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 截取窗口（**不是**全屏；脱敏由 `CaptureOptions` 决定）。
    ///
    /// # Errors
    /// 截图通道需授权而未授权 → `PlatformPermission`；窗口最小化 → `TargetUnresponsive`。
    fn capture(
        &self,
        window: &ResolvedWindow,
        options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send;
}
