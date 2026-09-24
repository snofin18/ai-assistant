//! 能力矩阵本体与它的校验（架构 v2 §13.1.2）。
//!
//! 职责：装下"**这台机器现在能做什么**"的完整快照，并把 spec §4 的不变量变成**会失败的返回值**。
//! 边界：**不做策略判定**（放行点是 `crates/policy`，TASK-021）。
//!
//! ## 为什么名字叫 `CapabilityMatrix`
//! 架构 v2 §13.1.2 的运行时探测结果。**注意**：`protocol/capability-matrix/capability-1.0.json`
//! 是另一件事（稳定能力标识目录 `<layer>.<capability>`）—— 两者同名不同物，
//! 改名要改契约 → 需 ADR（见 `crates/platform/api/README.md`「已知限制」）。
//!
//! 相关：`docs/spec/capability-matrix.md` §4、架构 v2 §13.1.2。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::capability::{ApprovalRequirement, CapabilityEntry, RiskLevel};
use crate::error::PlatformError;

/// 通道可用性（架构 v2 §13.1.2 的 `channels.*.available`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChannelAvailability {
    /// 可用。
    Available,
    /// 不可用。
    Unavailable,
    /// 需要用户授权（`needs_consent`）。
    NeedsConsent,
}

/// 一条通道的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ChannelState {
    /// 可用性。
    availability: ChannelAvailability,
    /// 补充说明（如"GNOME 下第三方截图接口受限"）。
    detail: Option<String>,
}

impl ChannelState {
    /// 构造通道状态。
    #[must_use]
    pub const fn new(availability: ChannelAvailability, detail: Option<String>) -> Self {
        Self {
            availability,
            detail,
        }
    }

    /// 可用性。
    #[must_use]
    pub const fn availability(&self) -> ChannelAvailability {
        self.availability
    }

    /// 补充说明。
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// 一条降级说明 —— **必须**翻译成自然语言进模型上下文（§13.1.2 的关键设计）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Degradation {
    /// 降级 id（如 `no_silent_screenshot`）。
    id: String,
    /// 影响（人类可读；会被注入模型上下文）。
    impact: String,
}

impl Degradation {
    /// 构造降级说明。
    #[must_use]
    pub const fn new(id: String, impact: String) -> Self {
        Self { id, impact }
    }

    /// 降级 id。
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 影响。
    #[must_use]
    pub fn impact(&self) -> &str {
        &self.impact
    }
}

/// 能力矩阵的校验失败（**每一条都对应 spec §4 的一条不变量**）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityMatrixError {
    /// 能力列表为空 —— 空矩阵会被下游当成"什么都不能做"，而不是"没探测"。
    EmptyCapabilityList,
    /// 能力 id 为空白。
    BlankCapabilityId,
    /// 能力 id 重复。
    DuplicateCapabilityId(String),
    /// 风险降级（不变量 1）。
    RiskDowngrade {
        /// 原等级。
        from: RiskLevel,
        /// 试图改成的新等级。
        to: RiskLevel,
    },
    /// 审批要求与风险级不一致（不变量 2）。
    ApprovalMismatch {
        /// 能力 id。
        id: String,
        /// 风险级。
        risk: RiskLevel,
        /// 声明的审批要求。
        declared: ApprovalRequirement,
        /// 该风险级要求的审批要求。
        expected: ApprovalRequirement,
    },
}

impl CapabilityMatrixError {
    /// 对应的错误码（供 `From` 转换使用）。
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        ErrorCode::CapabilityMissing
    }
}

impl From<CapabilityMatrixError> for PlatformError {
    fn from(error: CapabilityMatrixError) -> Self {
        Self::new(
            error.code(),
            format!("capability matrix invalid: {error:?}"),
        )
    }
}

impl std::fmt::Display for CapabilityMatrixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCapabilityList => f.write_str("capability list is empty"),
            Self::BlankCapabilityId => f.write_str("capability id is blank"),
            Self::DuplicateCapabilityId(id) => write!(f, "duplicate capability id `{id}`"),
            Self::RiskDowngrade { from, to } => {
                write!(f, "risk downgrade from {from:?} to {to:?}")
            }
            Self::ApprovalMismatch {
                id,
                risk,
                declared,
                expected,
            } => write!(
                f,
                "capability `{id}` declares {declared:?} for {risk:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for CapabilityMatrixError {}

/// 一次探测的上下文（架构 v2 §13.1.2 的 `probed_at` + `platform` + `session` 三块）。
///
/// 为什么单独成一个类型而不是摊成 5 个参数：
/// 1. 这 5 个字段是"**同一次探测**的原子事实" —— 时间、平台、会话状态必须来自同一时刻，
///    摊成散参数容易被误拼（把上一次的时间配这一次的平台），而拼错的矩阵会被下游当有效值用；
/// 2. AGENTS.md §5.3 规定函数参数 ≤ 6，`CapabilityMatrix::new` 若摊开就是 8 个。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProbeContext {
    /// 探测时刻（RFC 3339，UTC）。
    probed_at: String,
    /// 操作系统标识（如 `windows` / `macos` / `linux`）。
    platform_os: String,
    /// 会话是否已锁定（锁屏时合成输入不可用，必须显式告诉模型）。
    session_locked: bool,
    /// 是否远程会话（RDP / VNC：合成输入与截图的语义都不同）。
    session_remote: bool,
    /// 是否无头（没有交互式显示会话）。
    session_headless: bool,
}

impl ProbeContext {
    /// 构造探测上下文。
    #[must_use]
    pub const fn new(
        probed_at: String,
        platform_os: String,
        session_locked: bool,
        session_remote: bool,
        session_headless: bool,
    ) -> Self {
        Self {
            probed_at,
            platform_os,
            session_locked,
            session_remote,
            session_headless,
        }
    }

    /// 探测时刻（RFC 3339，UTC）。
    #[must_use]
    pub fn probed_at(&self) -> &str {
        &self.probed_at
    }

    /// 操作系统标识。
    #[must_use]
    pub fn platform_os(&self) -> &str {
        &self.platform_os
    }

    /// 会话是否已锁定。
    #[must_use]
    pub const fn session_locked(&self) -> bool {
        self.session_locked
    }

    /// 是否远程会话。
    #[must_use]
    pub const fn session_remote(&self) -> bool {
        self.session_remote
    }

    /// 是否无头。
    #[must_use]
    pub const fn session_headless(&self) -> bool {
        self.session_headless
    }
}

/// 运行时能力矩阵（架构 v2 §13.1.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CapabilityMatrix {
    /// 探测上下文（时刻 / 平台 / 会话状态）。
    probe: ProbeContext,
    /// 通道名 → 状态（`BTreeMap` = 确定性遍历顺序，便于审计与回放）。
    channels: BTreeMap<String, ChannelState>,
    /// 能力声明列表。
    capabilities: Vec<CapabilityEntry>,
    /// 降级说明列表。
    degradations: Vec<Degradation>,
}

impl CapabilityMatrix {
    /// 构造能力矩阵。
    #[must_use]
    pub const fn new(
        probe: ProbeContext,
        channels: BTreeMap<String, ChannelState>,
        capabilities: Vec<CapabilityEntry>,
        degradations: Vec<Degradation>,
    ) -> Self {
        Self {
            probe,
            channels,
            capabilities,
            degradations,
        }
    }

    /// 探测上下文（时刻 / 平台 / 会话状态）。
    #[must_use]
    pub const fn probe(&self) -> &ProbeContext {
        &self.probe
    }

    /// 探测时刻。
    #[must_use]
    pub fn probed_at(&self) -> &str {
        self.probe.probed_at()
    }

    /// 操作系统标识。
    #[must_use]
    pub fn platform_os(&self) -> &str {
        self.probe.platform_os()
    }

    /// 会话是否已锁定。
    #[must_use]
    pub const fn session_locked(&self) -> bool {
        self.probe.session_locked()
    }

    /// 是否远程会话。
    #[must_use]
    pub const fn session_remote(&self) -> bool {
        self.probe.session_remote()
    }

    /// 是否无头。
    #[must_use]
    pub const fn session_headless(&self) -> bool {
        self.probe.session_headless()
    }

    /// 通道状态。
    #[must_use]
    pub const fn channels(&self) -> &BTreeMap<String, ChannelState> {
        &self.channels
    }

    /// 能力声明列表。
    #[must_use]
    pub fn capabilities(&self) -> &[CapabilityEntry] {
        &self.capabilities
    }

    /// 降级说明列表。
    #[must_use]
    pub fn degradations(&self) -> &[Degradation] {
        &self.degradations
    }

    /// 校验 4 条不变量（**纯函数**：同样的矩阵必得同样的结论）。
    ///
    /// 覆盖面刻意与 spec §4 一一对应，**不发明新规则**：
    /// ① 能力列表非空 + id 非空且唯一（可追溯性前提）
    /// ② 风险不得降级（`RiskLevel::escalate` 的矩阵级用法）
    /// ③ `L3+ ⇒ required`、`L5 ⇒ forbidden`（不变量 2）
    /// ④ `Resource` / `SideEffect` 的枚举性由类型系统 + `parse` 保证（不变量 3 / 4）
    ///
    /// # Errors
    /// 任一不变量被违反 → 对应的 `CapabilityMatrixError`。
    pub fn validate(&self) -> Result<(), CapabilityMatrixError> {
        if self.capabilities.is_empty() {
            return Err(CapabilityMatrixError::EmptyCapabilityList);
        }
        for (index, entry) in self.capabilities.iter().enumerate() {
            if entry.id().trim().is_empty() {
                return Err(CapabilityMatrixError::BlankCapabilityId);
            }
            if self
                .capabilities
                .iter()
                .enumerate()
                .any(|(other_index, other)| other_index != index && other.id() == entry.id())
            {
                return Err(CapabilityMatrixError::DuplicateCapabilityId(
                    entry.id().to_string(),
                ));
            }
            let expected = entry.risk().required_approval();
            // 不变量 2 的两条硬方向：L3/L4 必须 required；L5 必须 forbidden。
            // L1/L2 只要求"不是 forbidden"（保守方向允许升级为人工确认，但把 L1 当永久拒绝是错的）。
            let consistent = match entry.risk() {
                RiskLevel::L1 | RiskLevel::L2 => entry.approval() != ApprovalRequirement::Forbidden,
                RiskLevel::L3 | RiskLevel::L4 | RiskLevel::L5 => entry.approval() == expected,
            };
            if !consistent {
                return Err(CapabilityMatrixError::ApprovalMismatch {
                    id: entry.id().to_string(),
                    risk: entry.risk(),
                    declared: entry.approval(),
                    expected,
                });
            }
        }
        Ok(())
    }
}
