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
//! 5. **元素解析必须有 scope**（ADR-0043）：`resolve_element` / `wait_for` 的搜索起点是传入的
//!    `&ResolvedWindow`，**禁止**从桌面根搜元素（代价与栈溢出风险见 ADR-0043）。

use std::future::Future;

use crate::error::PlatformResult;
use crate::fingerprint::Fingerprint;
use crate::geometry::NormalizedPoint;
use crate::handle::{ResolvedElement, ResolvedWindow};
use crate::target::SelectorCandidate;

/// 树快照选项（裁剪预算，架构 v2 §7.1 / §7.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TreeOptions {
    /// 最大深度（`None` = 不限，但平台实现仍须受上下文预算约束）。
    max_depth: Option<u32>,
    /// 是否包含离屏节点。
    include_offscreen: bool,
}

impl TreeOptions {
    /// 构造树选项。
    #[must_use]
    pub const fn new(max_depth: Option<u32>, include_offscreen: bool) -> Self {
        Self {
            max_depth,
            include_offscreen,
        }
    }

    /// 最大深度。
    #[must_use]
    pub const fn max_depth(&self) -> Option<u32> {
        self.max_depth
    }

    /// 是否包含离屏节点。
    #[must_use]
    pub const fn include_offscreen(&self) -> bool {
        self.include_offscreen
    }
}

/// 树快照（**可序列化**：它跨进程进模型上下文 / 回放，与句柄不同）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TreeSnapshot {
    /// 被快照的窗口句柄。
    window: ResolvedWindow,
    /// 快照时的状态指纹（§7.3：回放对齐用）。
    fingerprint: Fingerprint,
    /// 节点数（裁剪后）。
    node_count: u32,
}

impl TreeSnapshot {
    /// 构造树快照。
    #[must_use]
    pub const fn new(window: ResolvedWindow, fingerprint: Fingerprint, node_count: u32) -> Self {
        Self {
            window,
            fingerprint,
            node_count,
        }
    }

    /// 被快照的窗口句柄。
    #[must_use]
    pub const fn window(&self) -> &ResolvedWindow {
        &self.window
    }

    /// 快照时的状态指纹。
    #[must_use]
    pub const fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// 节点数。
    #[must_use]
    pub const fn node_count(&self) -> u32 {
        self.node_count
    }
}

/// 选择器链（有序候选）。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SelectorChain {
    /// 候选列表（调用方须按 `effective_score` 降序传入，§6.3）。
    candidates: Vec<SelectorCandidate>,
}

impl SelectorChain {
    /// 构造选择器链。
    #[must_use]
    pub const fn new(candidates: Vec<SelectorCandidate>) -> Self {
        Self { candidates }
    }

    /// 候选列表。
    #[must_use]
    pub fn candidates(&self) -> &[SelectorCandidate] {
        &self.candidates
    }
}

/// 元素查询条件（架构 v2 §13.1.1 的 `ElementQuery`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ElementQuery {
    /// 无障碍角色（**非本地化**）。
    role: Option<String>,
    /// 自动化 id（**非本地化**，主选）。
    automation_id: Option<String>,
}

impl ElementQuery {
    /// 按自动化 id 查询（主选路径）。
    #[must_use]
    pub fn by_automation_id(automation_id: impl Into<String>) -> Self {
        Self {
            role: None,
            automation_id: Some(automation_id.into()),
        }
    }

    /// 无障碍角色。
    #[must_use]
    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    /// 自动化 id。
    #[must_use]
    pub fn automation_id(&self) -> Option<&str> {
        self.automation_id.as_deref()
    }
}

/// 元素**期望**状态（`wait_for` 的目标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ElementState {
    /// 期望可用。
    enabled: bool,
    /// 期望可见。
    visible: bool,
    /// 期望获得焦点。
    focused: bool,
}

impl ElementState {
    /// 构造期望状态。
    #[must_use]
    pub const fn new(enabled: bool, visible: bool, focused: bool) -> Self {
        Self {
            enabled,
            visible,
            focused,
        }
    }

    /// 期望可用。
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// 期望可见。
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// 期望获得焦点。
    #[must_use]
    pub const fn focused(&self) -> bool {
        self.focused
    }
}

/// 超时（**显式传参**，不用平台默认值 —— §13.1.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Timeout {
    /// 毫秒。
    millis: u64,
}

impl Timeout {
    /// 构造超时。
    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self { millis }
    }

    /// 毫秒数。
    #[must_use]
    pub const fn millis(&self) -> u64 {
        self.millis
    }
}

/// 文本编辑操作（`edit_text` 的语义）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextEditOp {
    /// 在指定偏移插入。
    Insert {
        /// 字符偏移（0 = 开头）。
        at: usize,
        /// 要插入的文本。
        text: String,
    },
    /// 删除区间 `[start, end)`。
    Delete {
        /// 起始偏移。
        start: usize,
        /// 结束偏移（不含）。
        end: usize,
    },
    /// 替换区间 `[start, end)`。
    Replace {
        /// 起始偏移。
        start: usize,
        /// 结束偏移（不含）。
        end: usize,
        /// 替换成的文本。
        text: String,
    },
}

/// 选择项（列表 / 下拉框）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Selection {
    /// 按下标选（0 起）。
    ByIndex(u32),
    /// 按**非本地化**的稳定值选（如 UIA `SelectionItemPattern` 的 item id）。
    ByStableValue(String),
}

/// 滚动目标。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScrollTarget {
    /// 水平百分比（0.0~1.0）。
    horizontal_percent: f64,
    /// 垂直百分比（0.0~1.0）。
    vertical_percent: f64,
}

impl ScrollTarget {
    /// 构造滚动目标。
    #[must_use]
    pub const fn new(horizontal_percent: f64, vertical_percent: f64) -> Self {
        Self {
            horizontal_percent,
            vertical_percent,
        }
    }

    /// 水平百分比。
    #[must_use]
    pub const fn horizontal_percent(&self) -> f64 {
        self.horizontal_percent
    }

    /// 垂直百分比。
    #[must_use]
    pub const fn vertical_percent(&self) -> f64 {
        self.vertical_percent
    }
}

/// 指针动作（**最后手段**：优先用 `set_value` / `invoke_action`）。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PointerAction {
    /// 移动。
    Move,
    /// 单击。
    Click,
    /// 双击。
    DoubleClick,
    /// 从当前位置拖到指定点（拖拽期间必须持有独占租约，架构 v2 §9）。
    DragTo {
        /// 释放点。
        drop_at: NormalizedPoint,
    },
}

/// 修饰键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyModifier {
    /// Ctrl。
    Control,
    /// Alt。
    Alt,
    /// Shift。
    Shift,
    /// Win / Cmd。
    Meta,
}

/// 按键组合。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct KeyChord {
    /// 主键（如 `V`、`Enter`）。
    key: String,
    /// 修饰键。
    modifiers: Vec<KeyModifier>,
}

impl KeyChord {
    /// 构造按键组合。
    #[must_use]
    pub const fn new(key: String, modifiers: Vec<KeyModifier>) -> Self {
        Self { key, modifiers }
    }

    /// 主键。
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// 修饰键。
    #[must_use]
    pub fn modifiers(&self) -> &[KeyModifier] {
        &self.modifiers
    }
}

/// 键盘输入的目标（**必须显式**：焦点错位会打到别的应用上）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyTarget {
    /// 发给指定窗口。
    Window(ResolvedWindow),
    /// 发给指定元素（平台实现负责先确认焦点）。
    Element(ResolvedElement),
    /// 发给当前前台窗口（**仅在已校验前台**时使用）。
    Foreground,
}

/// 指纹范围（架构 v2 §7.3 的 `scope`）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FingerprintScope {
    /// 整窗。
    WholeWindow,
    /// 某棵子树。
    Element(ResolvedElement),
}

/// 无障碍自动化提供者（树 / 元素 / 动作 / 合成输入 / 指纹）。
///
/// **调用优先级**（§13.1.1 设计要点，调用方义务）：
/// `set_value` / `edit_text` / `invoke_action` / `select` / `scroll`
/// **优先于** `pointer_action` / `key_action`。
pub trait UiAutomationProvider: Send + Sync {
    /// 抓取树快照（按 `TreeOptions` 裁剪）。
    ///
    /// # Errors
    /// 无障碍接口不可用 → `PlatformPermission`；窗口已失效 → `TargetNotFound`。
    fn snapshot_tree(
        &self,
        root: &ResolvedWindow,
        options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send;

    /// 在 `scope` 窗口内按候选链解析元素（**ADR-0043**：元素解析必须有 scope）。
    ///
    /// 搜索起点是 `scope` 的 UIA 根元素，**不是**桌面根 —— 桌面级搜索的代价随桌面规模增长
    /// （TASK-017 真机实测中位数 1.53 s），且违反 ADR-0022 E6 引用的官方要求。
    /// 调用顺序：先 `resolve_window` 定窗口，再在这里定元素（架构 v2 §6.2）。
    ///
    /// # Errors
    /// 未找到 → `TargetNotFound`；多匹配 → `TargetAmbiguous`；链为空 → `ToolInvalidArgs`；
    /// `scope` 的窗口已关闭 / 句柄失效 → `TargetNotFound`。
    fn resolve_element(
        &self,
        scope: &ResolvedWindow,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send;

    /// 在 `scope` 窗口内等待元素进入期望状态（**ADR-0043**）。
    ///
    /// # Errors
    /// 超时 → `TargetUnresponsive`（**不是** `TargetNotFound`：元素可能一直存在但状态没到）；
    /// `scope` 的窗口已关闭 / 句柄失效 → `TargetNotFound`。
    fn wait_for(
        &self,
        scope: &ResolvedWindow,
        query: &ElementQuery,
        state: &ElementState,
        timeout: Timeout,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send;

    /// 读文本。
    ///
    /// # Errors
    /// 元素不支持取值 → `CapabilityMissing`；句柄失效 → `TargetNotFound`。
    fn read_text(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send;

    /// 设置取值（**优先于键盘模拟**）。
    ///
    /// # Errors
    /// 元素只读 → `CapabilityMissing`；平台拒绝 → `PlatformPermission`。
    fn set_value(
        &self,
        element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 编辑文本（插入 / 删除 / 替换）。
    ///
    /// # Errors
    /// 区间越界 → `ToolInvalidArgs`；元素不支持编辑 → `CapabilityMissing`。
    fn edit_text(
        &self,
        element: &ResolvedElement,
        operation: &TextEditOp,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 触发动作（Invoke / Toggle / Expand…）。
    ///
    /// # Errors
    /// 元素不支持该动作 → `CapabilityMissing`。
    fn invoke_action(
        &self,
        element: &ResolvedElement,
        action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 选择项。
    ///
    /// # Errors
    /// 目标不是可选项 → `CapabilityMissing`；下标越界 → `ToolInvalidArgs`。
    fn select(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 滚动到指定位置。
    ///
    /// # Errors
    /// 元素不可滚动 → `CapabilityMissing`；百分比不在 `[0,1]` → `ToolInvalidArgs`。
    fn scroll(
        &self,
        element: &ResolvedElement,
        target: &ScrollTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 指针动作（**最后手段**；入参必须是已归一化的点，§6.9）。
    ///
    /// # Errors
    /// 坐标未校准 → `PlatformPermission`（§6.9 规则 4：校准失败即禁用坐标通道）；
    /// 窗口最小化 → `TargetUnresponsive`。
    fn pointer_action(
        &self,
        point: NormalizedPoint,
        action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 键盘动作（**必须显式给目标**：焦点错位会打到别的应用上）。
    ///
    /// # Errors
    /// 目标不在前台且策略禁止抢焦点 → `PolicyDenied`。
    fn key_action(
        &self,
        chord: &KeyChord,
        target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send;

    /// 计算状态指纹（§7.3：一等接口）。
    ///
    /// # Errors
    /// 窗口已失效 → `TargetNotFound`；范围不可读 → `CapabilityMissing`。
    fn fingerprint(
        &self,
        window: &ResolvedWindow,
        scope: &FingerprintScope,
    ) -> impl Future<Output = PlatformResult<Fingerprint>> + Send;
}
