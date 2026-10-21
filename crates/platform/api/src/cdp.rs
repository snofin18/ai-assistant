//! CDP 专属平台 contract（架构 v2 §13.2.7 / §13.7）。
//!
//! 职责：给 Windows provider 定义浏览器 profile / 会话 / 目标 / 命令的**
//! 连接形状与失败语义**；provider 只负责把 CDP 原语做完，
//! 不做策略审批、不做 UI 状态机。
//!
//! 边界：
//! - 不句柄窗口 / UIA 树；`ResolvedCdpTarget` 只往下传給 CDP 层。
//! - 不做任何 HTTP / WebSocket / 浏览器启动 IO；provider 负责那些子层。
//! - 错误永远复用 `assistant_protocol::ErrorCode`；本 contract 不新增 `ErrorCode`。

use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::error::{PlatformError, PlatformResult};

/// CDP 侧点击 / 输入所用的物理坐标（架构 v2 §6.9 统一为全局虚拟屏物理像素）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CdpPoint {
    /// 物理像素 x。
    x: i64,
    /// 物理像素 y。
    y: i64,
}

impl CdpPoint {
    /// 构造一个可校验的 CDP 坐标。
    #[must_use]
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }

    /// 物理像素 x。
    #[must_use]
    pub const fn x(&self) -> i64 {
        self.x
    }

    /// 物理像素 y。
    #[must_use]
    pub const fn y(&self) -> i64 {
        self.y
    }

    /// 验证一只 normalized CDP point 是否在给定渲染矩形内。
    /// Provider 有义务先把物理坐标换算到设备空间再做这一步。
    ///
    /// # Errors
    /// 坐标越界 → `ErrorCode::ToolInvalidArgs`；矩形负宽 / 高 → 同上。
    pub fn check_in_bounds(
        &self,
        left: i64,
        top: i64,
        right: i64,
        bottom: i64,
    ) -> PlatformResult<()> {
        if left >= right || top >= bottom {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("CDP bounds ({left}, {top})..({right}, {bottom}) are not positive."),
            ));
        }
        if self.x < left || self.x >= right || self.y < top || self.y >= bottom {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!(
                    "CDP point ({}, {}) is outside the page rect {}..{}",
                    self.x, self.y, left, right
                ),
            ));
        }
        Ok(())
    }
}

/// 已解析的 CDP 会话句柄（本地不透明；不得跨进程）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResolvedCdpSession {
    id: u64,
}

impl ResolvedCdpSession {
    /// 构造 opaque session id。
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self { id }
    }

    /// 本地会话 id。
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }
}

/// 已解析的 CDP 页面目标句柄（本地不透明；不得跨进程）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResolvedCdpTarget {
    session: ResolvedCdpSession,
    target_id: u64,
}

impl ResolvedCdpTarget {
    /// 构造 opaque session + target id。
    #[must_use]
    pub const fn new(session: ResolvedCdpSession, target_id: u64) -> Self {
        Self { session, target_id }
    }

    /// 该 target 所在 session。
    #[must_use]
    pub const fn session(&self) -> ResolvedCdpSession {
        self.session
    }

    /// 本地 target id。
    #[must_use]
    pub const fn target_id(&self) -> u64 {
        self.target_id
    }
}

/// CDP 目标稳定身份（URL + 标题 + 可选标记，禁止 tab index）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CdpTargetIdentity {
    url: String,
    title: String,
    marker: Option<String>,
}

impl CdpTargetIdentity {
    /// 构造 URL + 标题身份（无 marker）。
    #[must_use]
    pub fn new(target_url: impl Into<String>, target_title: impl Into<String>) -> Self {
        Self {
            url: target_url.into(),
            title: target_title.into(),
            marker: None,
        }
    }

    /// 设置 site-specific marker。
    #[must_use]
    pub fn with_marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    /// 读取 marker。
    #[must_use]
    pub fn marker(&self) -> Option<&str> {
        self.marker.as_deref()
    }
}

/// CDP 会话 / 目标 / 页面元数据三重结果。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CdpSessionSnapshot {
    session: ResolvedCdpSession,
    target: ResolvedCdpTarget,
    target_identity: CdpTargetIdentity,
}

impl CdpSessionSnapshot {
    /// 构造 session/target/identity 三元组。
    #[must_use]
    pub const fn new(
        session: ResolvedCdpSession,
        target: ResolvedCdpTarget,
        target_identity: CdpTargetIdentity,
    ) -> Self {
        Self {
            session,
            target,
            target_identity,
        }
    }

    /// 会话句柄。
    #[must_use]
    pub const fn session(&self) -> ResolvedCdpSession {
        self.session
    }

    /// 目标句柄。
    #[must_use]
    pub const fn target(&self) -> ResolvedCdpTarget {
        self.target
    }

    /// 目标身份。
    #[must_use]
    pub const fn target_identity(&self) -> &CdpTargetIdentity {
        &self.target_identity
    }
}

/// CDP DOM 快照的窗口身份，用作 postcondition 的 URL/title 标定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CdpDomSnapshot {
    url: String,
    title: String,
    marker: Option<String>,
    dom: String,
}

impl CdpDomSnapshot {
    /// 构造 DOM snapshot + identity。
    #[must_use]
    pub fn new(
        target_url: &str,
        target_title: &str,
        target_marker: Option<&str>,
        dom: impl Into<String>,
    ) -> Self {
        Self {
            url: target_url.to_string(),
            title: target_title.to_string(),
            marker: target_marker.map(ToString::to_string),
            dom: dom.into(),
        }
    }

    /// 目标 URL。
    #[must_use]
    pub fn target_url(&self) -> &str {
        &self.url
    }

    /// 目标标题。
    #[must_use]
    pub fn target_title(&self) -> &str {
        &self.title
    }

    /// 目标 marker。
    #[must_use]
    pub fn target_marker(&self) -> Option<&str> {
        self.marker.as_deref()
    }

    /// DOM 文本或序列化内容。
    #[must_use]
    pub fn dom(&self) -> &str {
        &self.dom
    }
}

/// CDP `Page.navigate` 的 provider 参数与等待语义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CdpLoadCondition {
    /// `Page.loadEventFired` 之后。
    Load,
    /// `Page.loadEventFired` + 网络 idle 判定。
    NetworkIdle {
        /// network idle 连续静默窗（毫秒）。
        quiet_ms: u64,
    },
}

impl CdpLoadCondition {
    /// 供 provider 生成等待语义。
    #[must_use]
    pub const fn expected_state(&self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::NetworkIdle { quiet_ms: _ } => "network-idle",
        }
    }
}

/// CDP `Page.captureScreenshot` 的输出（PNG）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CdpScreenshot {
    /// 内容寻址 blob id；provider 负责算地址但不负责写存储。
    blob_id: String,
    width: u32,
    height: u32,
    format: String,
}

impl CdpScreenshot {
    /// 构造 PNG 截图元数据。
    #[must_use]
    pub fn new(blob_id: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            blob_id: blob_id.into(),
            width,
            height,
            format: "png".to_string(),
        }
    }

    /// 内容地址。
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

    /// 格式。
    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }
}

/// CDP 选项（profile 侧所有值都必须 provider 校验；本层不注入）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CdpConnectOptions {
    host: String,
    port: u16,
    page_url: String,
    page_title: Option<String>,
    page_marker: Option<String>,
    session_timeout_ms: u64,
}

impl CdpConnectOptions {
    /// 构造 CDP 连接选项。
    ///
    /// # Errors
    ///
    /// `host` / `page_url` 为空 → `ToolInvalidArgs`。
    pub fn new(
        host: impl Into<String>,
        port: u16,
        page_url: impl Into<String>,
        page_title: Option<String>,
        page_marker: Option<String>,
        session_timeout_ms: u64,
    ) -> PlatformResult<Self> {
        let host = host.into();
        let page_url = page_url.into();
        if host.trim().is_empty() || page_url.trim().is_empty() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "host / page_url must be non-empty",
            ));
        }
        Ok(Self {
            host,
            port,
            page_url,
            page_title,
            page_marker,
            session_timeout_ms,
        })
    }

    /// 主机名。
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// 端口。
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// 页面 URL。
    #[must_use]
    pub fn page_url(&self) -> &str {
        &self.page_url
    }

    /// 页面标题。
    #[must_use]
    pub fn page_title(&self) -> Option<&str> {
        self.page_title.as_deref()
    }

    /// 页面 marker。
    #[must_use]
    pub fn page_marker(&self) -> Option<&str> {
        self.page_marker.as_deref()
    }

    /// 会话超时毫秒。
    #[must_use]
    pub const fn session_timeout_ms(&self) -> u64 {
        self.session_timeout_ms
    }
}

/// CDP 专属平台 provider 的单一 trait 形状。
///
/// 所有 CDP 能力都通过本 trait 暴露；它不向窗口 / UIA 层泄漏句柄，
/// 统一用 opaque `ResolvedCdpTarget` + `ResolvedCdpSession`。
pub trait CdpProvider: Send + Sync {
    /// Establish a CDP session and attach one page target.
    fn connect(
        &self,
        options: &CdpConnectOptions,
    ) -> impl Future<Output = PlatformResult<CdpSessionSnapshot>> + Send;

    /// Navigate a target and wait according to `condition`.
    fn navigate(
        &self,
        target: &ResolvedCdpTarget,
        url: &str,
        condition: &CdpLoadCondition,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// Extract / evaluate a DOM primitive; the caller supplies the script shape.
    fn evaluate_dom(
        &self,
        target: &ResolvedCdpTarget,
        expression: &str,
    ) -> impl Future<Output = PlatformResult<CdpDomSnapshot>> + Send;

    /// Dispatch a mouse click at `point`.
    fn click(
        &self,
        target: &ResolvedCdpTarget,
        point: &CdpPoint,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// Capture the target page as PNG.
    fn capture_screenshot(
        &self,
        target: &ResolvedCdpTarget,
    ) -> impl Future<Output = PlatformResult<CdpScreenshot>> + Send;

    /// Configure the download directory for this dedicated profile.
    fn set_download_directory(
        &self,
        target: &ResolvedCdpTarget,
        directory: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// Close the page target (and, as needed, kill its browser process).
    fn close(&self, target: &ResolvedCdpTarget) -> impl Future<Output = PlatformResult<()>> + Send;
}
