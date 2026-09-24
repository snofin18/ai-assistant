//! 目标描述与有序候选选择器链（架构 v2 §6.2）。
//!
//! 职责：把"怎么找到那个目标"写成**可序列化、可学习、可降权**的数据，而不是一个句柄。
//! 边界：**不做解析** —— 真正去试候选链、处理歧义的是平台实现（TASK-017）与自愈定位（§6.3）。
//!
//! ## 不变量
//! 1. `app_id` 非空；每个候选链**至少一个**候选（空链 = 永远找不到，必须当场拒绝）。
//! 2. `score` ∈ `[0, 1]` 且有限；`id` 在**同一链内唯一**。
//! 3. `locale_dependent` 的候选在多语言环境下**自动降权**（§6.3：`score *= 0.5`），
//!    降权只影响**顺序**，绝不自动新增候选（§6.3 自愈规则：避免不可解释的行为）。
//! 4. **禁用可见文本作主 selector**（AGENTS.md §7 + ADR-0022 D4）：`TitleRegex` / `NameRegex` /
//!    `A11yPath` / `VisualAnchor` 都只能作低分兜底，且**必须**标 `locale_dependent = true`。
//!
//! 相关：架构 v2 §6.2 / §6.3 / §6.4（定位禁止事项）、ADR-0022 D4、`docs/spec/naming.md` §7。

use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::error::{PlatformError, PlatformResult};

/// 候选选择器的种类（对应架构 v2 §6.2 的 `kind`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectorKind {
    /// 运行时 id（UIA `RuntimeId`）—— 最高分，但进程重启即失效（`ttl_ms` 通常为 0）。
    RuntimeId,
    /// 自动化 id（UIA `AutomationId` / AX `identifier`）—— 主选，**非本地化**。
    AutomationId,
    /// 无障碍标识符（AX / AT-SPI 侧的稳定 id）。
    AxIdentifier,
    /// class + role 组合。
    ClassAndRole,
    /// role + 父元素引用。
    RoleAndParent,
    /// 无障碍树路径 —— **本地化相关**（路径里可能含本地化节点名）。
    A11yPath,
    /// 窗口标题正则 —— **本地化相关**，只作兜底。
    TitleRegex,
    /// 控件可见文本正则 —— **本地化相关**，最低分兜底（ADR-0022 D4 明令）。
    NameRegex,
    /// 视觉锚点（OCR 文本 + 区域）—— 最低分兜底。
    VisualAnchor,
}

/// 候选选择器的取值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectorValue {
    /// 单值（id / 正则 / 路径片段等）。
    Text(String),
    /// class + role 组合。
    ClassAndRole {
        /// 控件类名。
        class: String,
        /// 无障碍角色。
        role: String,
    },
    /// role + 父元素引用。
    RoleAndParent {
        /// 无障碍角色。
        role: String,
        /// 父候选的 `id`（同一链内引用，不是平台句柄）。
        parent_id: String,
    },
    /// 无障碍树路径（自根到叶的节点名列表）。
    Path(Vec<String>),
    /// 视觉锚点。
    VisualAnchor {
        /// OCR 文本。
        ocr_text: String,
        /// 区域提示（如 `titlebar`）。
        region: String,
    },
}

/// 一个候选选择器。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectorCandidate {
    /// 链内唯一 id（用于 `RoleAndParent` 的 `parent_id` 引用与自愈回写）。
    id: String,
    /// 种类。
    kind: SelectorKind,
    /// 取值。
    value: SelectorValue,
    /// 经验值 + 学习值（§6.2：每次成功/失败都回写，逐步收敛）。
    score: f64,
    /// 是否本地化相关 —— `true` 时在多语言环境下自动降权（§6.3）。
    locale_dependent: bool,
    /// 有效期（毫秒）；`None` = 不过期，`Some(0)` = 一次性（如 `RuntimeId`）。
    ttl_ms: Option<u64>,
}

impl SelectorCandidate {
    /// 构造候选。
    ///
    /// # Errors
    /// - `id` 为空白 → `ToolInvalidArgs`
    /// - `score` 非有限或不在 `[0, 1]` → `ToolInvalidArgs`
    pub fn new(
        id: impl Into<String>,
        kind: SelectorKind,
        value: SelectorValue,
        score: f64,
        locale_dependent: bool,
    ) -> PlatformResult<Self> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "selector candidate id must not be blank",
            ));
        }
        if !score.is_finite() || !(0.0..=1.0).contains(&score) {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("selector score must be finite and within [0, 1], got {score}"),
            ));
        }
        Ok(Self {
            id,
            kind,
            value,
            score,
            locale_dependent,
            ttl_ms: None,
        })
    }

    /// 设置有效期（`RuntimeId` 这类一次性候选应设 `Some(0)`）。
    #[must_use]
    pub const fn with_ttl_ms(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = Some(ttl_ms);
        self
    }

    /// 链内唯一 id。
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 种类。
    #[must_use]
    pub const fn kind(&self) -> SelectorKind {
        self.kind
    }

    /// 取值。
    #[must_use]
    pub const fn value(&self) -> &SelectorValue {
        &self.value
    }

    /// 静态分数（未做本地化降权）。
    #[must_use]
    pub const fn score(&self) -> f64 {
        self.score
    }

    /// 是否本地化相关。
    #[must_use]
    pub const fn is_locale_dependent(&self) -> bool {
        self.locale_dependent
    }

    /// 有效期（毫秒）。
    #[must_use]
    pub const fn ttl_ms(&self) -> Option<u64> {
        self.ttl_ms
    }

    /// 本地化降权后的**实际**分数（架构 v2 §6.3）。
    ///
    /// 规则：`locale_dependent` 且 authored locale ≠ current locale → `score * 0.5`；
    /// 其余情况原样返回。**只降权、不新增候选**（§6.3 自愈规则）。
    #[must_use]
    pub fn effective_score(&self, authored_locale: &str, current_locale: &str) -> f64 {
        if self.locale_dependent && authored_locale != current_locale {
            self.score * 0.5
        } else {
            self.score
        }
    }
}

/// 多匹配时的语义（架构 v2 §6.6）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OnAmbiguous {
    /// 报 `TargetAmbiguous` 并升级给人 —— **默认**（铁律 1：不猜）。
    ErrorAndAsk,
    /// 取最高分候选（**仅在 Adapter 显式声明可容忍时使用**）。
    HighestScore,
}

/// 未找到时的重试与升级策略（架构 v2 §6.2 的 `resolution_policy.on_not_found`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OnNotFound {
    /// 逐次重试前的等待毫秒数（空 = 不重试）。
    retry_after_ms: Vec<u64>,
    /// 重试耗尽后是否升级给人（`false` = 直接返回 `TargetNotFound`）。
    then_escalate: bool,
}

impl OnNotFound {
    /// 构造策略。
    #[must_use]
    pub const fn new(retry_after_ms: Vec<u64>, then_escalate: bool) -> Self {
        Self {
            retry_after_ms,
            then_escalate,
        }
    }

    /// 逐次重试前的等待毫秒数。
    #[must_use]
    pub fn retry_after_ms(&self) -> &[u64] {
        &self.retry_after_ms
    }

    /// 重试耗尽后是否升级给人。
    #[must_use]
    pub const fn then_escalate(&self) -> bool {
        self.then_escalate
    }
}

/// 解析策略（架构 v2 §6.2 的 `resolution_policy`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResolutionPolicy {
    /// 多匹配时的语义。
    on_ambiguous: OnAmbiguous,
    /// 未找到时的策略。
    on_not_found: OnNotFound,
    /// 单次解析的总超时（毫秒）。
    max_resolve_ms: u64,
    /// 低于此分数的候选**不再尝试**（§6.2 的 `min_score_to_try`）。
    min_score_to_try: f64,
}

impl ResolutionPolicy {
    /// 构造策略。
    ///
    /// # Errors
    /// - `max_resolve_ms == 0` → `ToolInvalidArgs`（0 超时等于必然失败，属配置错误）
    /// - `min_score_to_try` 非有限或不在 `[0, 1]` → `ToolInvalidArgs`
    pub fn new(
        on_ambiguous: OnAmbiguous,
        on_not_found: OnNotFound,
        max_resolve_ms: u64,
        min_score_to_try: f64,
    ) -> PlatformResult<Self> {
        if max_resolve_ms == 0 {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "max_resolve_ms must be > 0",
            ));
        }
        if !min_score_to_try.is_finite() || !(0.0..=1.0).contains(&min_score_to_try) {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!(
                    "min_score_to_try must be finite and within [0, 1], got {min_score_to_try}"
                ),
            ));
        }
        Ok(Self {
            on_ambiguous,
            on_not_found,
            max_resolve_ms,
            min_score_to_try,
        })
    }

    /// 多匹配时的语义。
    #[must_use]
    pub const fn on_ambiguous(&self) -> OnAmbiguous {
        self.on_ambiguous
    }

    /// 未找到时的策略。
    #[must_use]
    pub const fn on_not_found(&self) -> &OnNotFound {
        &self.on_not_found
    }

    /// 单次解析的总超时（毫秒）。
    #[must_use]
    pub const fn max_resolve_ms(&self) -> u64 {
        self.max_resolve_ms
    }

    /// 低于此分数的候选不再尝试。
    #[must_use]
    pub const fn min_score_to_try(&self) -> f64 {
        self.min_score_to_try
    }
}

/// 目标描述（架构 v2 §6.2 的 `TargetDescriptor` v2）。
///
/// **它是数据，不是句柄**：可以序列化、可以存库、可以跨进程传递；
/// 真正的句柄（`ResolvedWindow` / `ResolvedElement`）永远留在 Host 内（铁律 8）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TargetDescriptor {
    /// 描述版本（架构 v2 的口径是 `"2.0"`）。
    descriptor_version: String,
    /// 应用标识（如 `com.microsoft.notepad`）。
    app_id: String,
    /// 窗口候选链（**有序**：按 `effective_score` 降序尝试）。
    window_candidates: Vec<SelectorCandidate>,
    /// 元素候选链（同上；空 = 只定位到窗口）。
    element_candidates: Vec<SelectorCandidate>,
    /// 解析策略。
    resolution_policy: ResolutionPolicy,
}

impl TargetDescriptor {
    /// 构造描述。
    #[must_use]
    pub const fn new(
        descriptor_version: String,
        app_id: String,
        window_candidates: Vec<SelectorCandidate>,
        element_candidates: Vec<SelectorCandidate>,
        resolution_policy: ResolutionPolicy,
    ) -> Self {
        Self {
            descriptor_version,
            app_id,
            window_candidates,
            element_candidates,
            resolution_policy,
        }
    }

    /// 描述版本。
    #[must_use]
    pub fn descriptor_version(&self) -> &str {
        &self.descriptor_version
    }

    /// 应用标识。
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// 窗口候选链。
    #[must_use]
    pub fn window_candidates(&self) -> &[SelectorCandidate] {
        &self.window_candidates
    }

    /// 元素候选链（空 = 只定位到窗口）。
    #[must_use]
    pub fn element_candidates(&self) -> &[SelectorCandidate] {
        &self.element_candidates
    }

    /// 解析策略。
    #[must_use]
    pub const fn resolution_policy(&self) -> &ResolutionPolicy {
        &self.resolution_policy
    }

    /// 校验描述自身的一致性（**纯函数**：不解析、不碰平台）。
    ///
    /// # Errors
    /// - `descriptor_version` / `app_id` 为空白 → `ToolInvalidArgs`
    /// - 窗口候选链为空 → `ToolInvalidArgs`（空链 = 永远找不到）
    /// - 同一链内 `id` 重复 → `ToolInvalidArgs`（会让 `parent_id` 引用产生歧义）
    pub fn validate(&self) -> PlatformResult<()> {
        if self.descriptor_version.trim().is_empty() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "descriptor_version must not be blank",
            ));
        }
        if self.app_id.trim().is_empty() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "app_id must not be blank",
            ));
        }
        if self.window_candidates.is_empty() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "window_candidates must not be empty",
            ));
        }
        for (label, chain) in [
            ("window_candidates", &self.window_candidates),
            ("element_candidates", &self.element_candidates),
        ] {
            for (index, candidate) in chain.iter().enumerate() {
                let duplicated = chain.iter().enumerate().any(|(other_index, other)| {
                    other_index != index && other.id() == candidate.id()
                });
                if duplicated {
                    return Err(PlatformError::new(
                        ErrorCode::ToolInvalidArgs,
                        format!("{label} has duplicate candidate id `{}`", candidate.id()),
                    ));
                }
            }
        }
        Ok(())
    }
}
