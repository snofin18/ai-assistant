//! 能力矩阵与它的 4 条不变量（架构 v2 §13.1.2 + `docs/spec/capability-matrix.md` §4）。
//!
//! 职责：描述"**这台机器现在能做什么**"，供三处消费 —— 策略引擎判断、模型上下文、UI 展示。
//! 边界：**不做策略判定**（放行点是 `crates/policy`，TASK-021）；本 crate 只给数据 + 不变量。
//!
//! ## 不变量（`CapabilityMatrix::validate` 逐条机器校验）
//! 1. **风险单调升级**：L1 → L5 只升不降（`RiskLevel::escalate` 拒绝降级）
//! 2. **Approval 强制**：`L3+ ⇒ required`、`L5 ⇒ forbidden`
//! 3. **Resource 类型枚举**：`read / write / send / invoke / destroy` 恰好 5 类（`parse` 拒绝 `modify` 这类模糊词）
//! 4. **`SideEffect` 必填**：必须明确声明（`parse` 拒绝空串与未知词）
//!
//! 本文件只放**枚举与单条声明**；矩阵本体与它的校验在 [`crate::matrix`]。

use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::error::{PlatformError, PlatformResult};
use crate::matrix::CapabilityMatrixError;

/// 资源访问类别（`docs/spec/capability-matrix.md` §4 不变量 3：**恰好 5 类**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResourceAccess {
    /// 只读。
    Read,
    /// 写入（可能覆盖既有内容）。
    Write,
    /// 发送（出网 / 对外发消息）。
    Send,
    /// 调用（触发某个动作，如按钮点击）。
    Invoke,
    /// 销毁（不可逆删除）。
    Destroy,
}

impl ResourceAccess {
    /// 全部 5 类（用于遍历与测试；数量本身是契约）。
    pub const ALL: [Self; 5] = [
        Self::Read,
        Self::Write,
        Self::Send,
        Self::Invoke,
        Self::Destroy,
    ];

    /// 解析字符串（**唯一的反序列化入口**，因此模糊词在这里被拦住）。
    ///
    /// # Errors
    /// 不在这 5 类里 → `CapabilityMissing`（例：`"modify"` 这类模糊词 —— spec §4 明令不允许）。
    pub fn parse(raw: &str) -> PlatformResult<Self> {
        match raw {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "send" => Ok(Self::Send),
            "invoke" => Ok(Self::Invoke),
            "destroy" => Ok(Self::Destroy),
            other => Err(PlatformError::new(
                ErrorCode::CapabilityMissing,
                format!(
                    "unknown resource access `{other}` (allowed: read/write/send/invoke/destroy)"
                ),
            )),
        }
    }

    /// 规范字符串（与 `parse` 互逆）。
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Send => "send",
            Self::Invoke => "invoke",
            Self::Destroy => "destroy",
        }
    }
}

/// 副作用类别（`docs/spec/capability-matrix.md` §4 不变量 4：**必填**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SideEffect {
    /// 无副作用（只读探测）。
    None,
    /// 改变 UI 焦点。
    UiFocus,
    /// 落盘。
    Disk,
    /// 启动进程。
    ProcessSpawn,
    /// 出网。
    NetworkEgress,
    /// 改剪贴板。
    Clipboard,
    /// 改变外部系统状态（资金 / 对外发布类，**永久禁止自动化**，铁律 6）。
    ExternalSystem,
}

impl SideEffect {
    /// 解析字符串。
    ///
    /// # Errors
    /// 空串或未知词 → `CapabilityMissing`（"没写"与"写了 none"必须能区分，铁律 1）。
    pub fn parse(raw: &str) -> PlatformResult<Self> {
        match raw {
            "none" => Ok(Self::None),
            "ui_focus" => Ok(Self::UiFocus),
            "disk" => Ok(Self::Disk),
            "process_spawn" => Ok(Self::ProcessSpawn),
            "network_egress" => Ok(Self::NetworkEgress),
            "clipboard" => Ok(Self::Clipboard),
            "external_system" => Ok(Self::ExternalSystem),
            other => Err(PlatformError::new(
                ErrorCode::CapabilityMissing,
                format!("unknown side effect `{other}` (must be declared explicitly)"),
            )),
        }
    }

    /// 规范字符串。
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::UiFocus => "ui_focus",
            Self::Disk => "disk",
            Self::ProcessSpawn => "process_spawn",
            Self::NetworkEgress => "network_egress",
            Self::Clipboard => "clipboard",
            Self::ExternalSystem => "external_system",
        }
    }
}

/// 风险级（架构 v2 的 L1~L5；数字越大越危险）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RiskLevel {
    /// L1：只读、无副作用。
    L1,
    /// L2：可逆写入（有 undo / 快照）。
    L2,
    /// L3：不可逆或需人工确认。
    L3,
    /// L4：出网 / 影响外部。
    L4,
    /// L5：资金 / 对外发布 —— **永久禁止自动化**（铁律 6）。
    L5,
}

impl RiskLevel {
    /// 该风险级**必须**的审批要求（不变量 2）。
    #[must_use]
    pub const fn required_approval(&self) -> ApprovalRequirement {
        match self {
            Self::L1 | Self::L2 => ApprovalRequirement::Auto,
            Self::L3 | Self::L4 => ApprovalRequirement::Required,
            Self::L5 => ApprovalRequirement::Forbidden,
        }
    }

    /// 单调升级：只允许"升"或"持平"，**降级直接报错**（不变量 1）。
    ///
    /// # Errors
    /// `proposed` 的等级低于 `current` → `CapabilityMatrixError::RiskDowngrade`
    /// （例：把 L3 的 `FileDelete` 声明成 L2 —— spec §4 明令禁止）。
    pub const fn escalate(current: Self, proposed: Self) -> Result<Self, CapabilityMatrixError> {
        let current_rank = current.rank();
        let proposed_rank = proposed.rank();
        if proposed_rank < current_rank {
            Err(CapabilityMatrixError::RiskDowngrade {
                from: current,
                to: proposed,
            })
        } else {
            Ok(proposed)
        }
    }

    /// 等级数值（1~5），用于比较与展示。
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
            Self::L4 => 4,
            Self::L5 => 5,
        }
    }
}

/// 审批要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ApprovalRequirement {
    /// 自动放行（仍需过策略引擎）。
    Auto,
    /// 必须人工确认。
    Required,
    /// 永久禁止（= 强制拒绝，铁律 6）。
    Forbidden,
}

/// 一条能力声明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CapabilityEntry {
    /// 能力 id。
    id: String,
    /// 资源访问类别。
    resource: ResourceAccess,
    /// 副作用（必填）。
    side_effect: SideEffect,
    /// 风险级。
    risk: RiskLevel,
    /// 声明的审批要求（必须与 `risk.required_approval()` 一致）。
    approval: ApprovalRequirement,
}

impl CapabilityEntry {
    /// 构造一条能力声明。
    #[must_use]
    pub const fn new(
        id: String,
        resource: ResourceAccess,
        side_effect: SideEffect,
        risk: RiskLevel,
        approval: ApprovalRequirement,
    ) -> Self {
        Self {
            id,
            resource,
            side_effect,
            risk,
            approval,
        }
    }

    /// 能力 id。
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 资源访问类别。
    #[must_use]
    pub const fn resource(&self) -> ResourceAccess {
        self.resource
    }

    /// 副作用。
    #[must_use]
    pub const fn side_effect(&self) -> SideEffect {
        self.side_effect
    }

    /// 风险级。
    #[must_use]
    pub const fn risk(&self) -> RiskLevel {
        self.risk
    }

    /// 声明的审批要求。
    #[must_use]
    pub const fn approval(&self) -> ApprovalRequirement {
        self.approval
    }
}
