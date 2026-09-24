//! 不透明句柄（**铁律 8：element / 句柄对象不得跨进程**）。
//!
//! 职责：给"Host 内已完成定位的那个目标"一个本地标识，使上层可以引用它，
//! 又**无法**把它序列化出去、**无法**把它当成平台对象使用。
//! 边界：不做缓存淘汰、不做失效重解析 —— 那些是 Host 内的事（TASK-017 / 025）。
//!
//! ## 不变量（本文件是**唯一**的句柄定义处）
//! 1. **不派生 `Serialize` / `Deserialize`**：句柄跨进程即失效，能序列化就等于给了
//!    "不小心把它传出去"的机会（架构 v2 §3.2 / §6.1：handle 是缓存，不是身份）。
//!    机器校验：`tests/handles_not_serializable.rs` 扫本文件源码，出现 `Serialize` 即红。
//! 2. **内部不持有平台对象**：只有 `LocalHandleId` + 可展示的元数据；
//!    真正的 `IUIAutomationElement` 之类的对象**永远**留在平台实现里。
//! 3. 句柄**不是身份**：判定"还是不是同一个目标"必须用 `Fingerprint`（§6.1 / §7.3），
//!    不能用句柄相等 —— 句柄会随重解析变化。

/// Host 本地的句柄标识（**不是** OS 句柄，也不是 `RuntimeId`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct LocalHandleId {
    /// 单调递增的本地序号（Host 内唯一）。
    value: u64,
}

impl LocalHandleId {
    /// 构造本地句柄 id。
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    /// 取值。
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.value
    }
}

/// 已解析的窗口（Host 本地句柄）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResolvedWindow {
    /// 本地句柄 id。
    id: LocalHandleId,
    /// 人类可读标签（日志/审计用；**不得**作 selector，ADR-0022 D4）。
    display_label: String,
}

impl ResolvedWindow {
    /// 构造已解析窗口。
    #[must_use]
    pub const fn new(id: LocalHandleId, display_label: String) -> Self {
        Self { id, display_label }
    }

    /// 本地句柄 id。
    #[must_use]
    pub const fn id(&self) -> LocalHandleId {
        self.id
    }

    /// 人类可读标签（**仅供日志/审计**）。
    #[must_use]
    pub fn display_label(&self) -> &str {
        &self.display_label
    }
}

/// 已解析的元素（Host 本地句柄）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResolvedElement {
    /// 本地句柄 id。
    id: LocalHandleId,
    /// 父元素的本地句柄 id（自愈重解析时用来校验层级未变）。
    parent: LocalHandleId,
    /// 无障碍角色（**非本地化**的那一个，如 `Document`；ADR-0022 D4）。
    role: String,
}

impl ResolvedElement {
    /// 构造已解析元素。
    #[must_use]
    pub const fn new(id: LocalHandleId, parent: LocalHandleId, role: String) -> Self {
        Self { id, parent, role }
    }

    /// 本地句柄 id。
    #[must_use]
    pub const fn id(&self) -> LocalHandleId {
        self.id
    }

    /// 父元素句柄 id。
    #[must_use]
    pub const fn parent(&self) -> LocalHandleId {
        self.parent
    }

    /// 无障碍角色。
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }
}
