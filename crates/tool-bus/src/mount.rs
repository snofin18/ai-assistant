//! 注册表与**动态挂载**：选择 → 挂载集 → 指纹 + 超限告警 + 审计事件。
//!
//! 为什么挂载是单独一步（而不是「注册即挂载」）：架构 v2 §5.5 第 1 条要求「只把当前任务
//! 涉及的应用的工具放进上下文」。注册是**能不能用**，挂载是**这一轮给不给模型看** ——
//! 两件事必须分开，否则工具数会随注册表增长而失控。
//!
//! 不变量：
//! 1. 挂载结果**要么完整，要么失败**：选择里出现未知工具名 / 匹配不到任何工具的应用前缀
//!    时直接报 [`ToolBusError::UnknownTool`]，绝不「静默少挂几个」。
//! 2. 指纹只由**最终挂载集**决定（同一集合任意顺序同指纹；任一字节变化即变指纹）。
//! 3. 工具数 > [`MAX_MOUNTED_TOOLS_WITHOUT_WARNING`] 时**必须**留下结构化的
//!    [`ToolsetOversizeWarning`] + [`AuditEvent`]；只打一行日志不算（铁律 1）。
//!
//! 相关：架构 v2 §5.5、`docs/spec/audit-event.md` §4、ADR-0021（受控词）、ADR-0040（hash 口径）。

use std::collections::BTreeMap;
use std::sync::Arc;

use assistant_protocol::AuditEvent;

use crate::clock::Clock;
use crate::error::{ToolBusError, ToolBusResult};
use crate::fingerprint::{FingerprintEntry, ToolsetFingerprint, compute_toolset_fingerprint};
use crate::handler::ToolHandler;
use crate::meta::{ToolSummary, is_reserved_meta_tool_name, meta_tools};
use crate::registry::ToolDefinition;

/// 单次挂载的工具数**超过**它就告警（架构 v2 §5.5 第 5 条）。
///
/// 计数口径 = **最终挂载集**（含两个元工具）：这条告警要回答的是「模型上下文里能看到
/// 几个工具」，元工具同样占上下文。
pub const MAX_MOUNTED_TOOLS_WITHOUT_WARNING: usize = 40;

/// 挂载选择：这一轮模型能看到哪些工具（架构 v2 §5.5 第 1 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountSelection {
    /// 全部已注册工具 + 两个元工具。
    All,
    /// 按工具名精确挂载；名字不存在即报错（**不**静默跳过）。
    Names(Vec<String>),
    /// 按应用前缀（工具名第一段，如 `notepad`）挂载；某个前缀一个都没匹配上即报错。
    Apps(Vec<String>),
}

impl MountSelection {
    /// 全量挂载（测试与「单应用场景」用）。
    #[must_use]
    pub const fn all() -> Self {
        Self::All
    }

    /// 按工具名挂载。
    #[must_use]
    pub const fn names(names: Vec<String>) -> Self {
        Self::Names(names)
    }

    /// 按应用挂载。
    #[must_use]
    pub const fn apps(apps: Vec<String>) -> Self {
        Self::Apps(apps)
    }
}

/// 挂载工具数超限时的结构化告警。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolsetOversizeWarning {
    /// 实际挂载的工具数（含元工具）。
    pub mounted_count: usize,
    /// 触发告警的上限。
    pub limit: usize,
}

/// 挂载报告：这一轮到底挂上了什么、指纹是什么、有没有告警。
#[derive(Debug, Clone)]
pub struct MountReport {
    /// 实际挂载的工具名（升序，含两个元工具）。
    pub mounted: Vec<String>,
    /// 工具集指纹（审计里记它 = 事后能复现「模型当时看到了什么」）。
    pub fingerprint: ToolsetFingerprint,
    /// 超限告警；未超限为 `None`。
    pub warning: Option<ToolsetOversizeWarning>,
    /// 超限时同时产出的审计事件。
    ///
    /// **未串链**：`prev_hash` / `self_hash` 留空，由 `crates/audit` 追加时按
    /// `docs/spec/audit-event.md` §4 重算并写入（本 crate 不持有审计库，也不该持有）。
    pub audit_event: Option<AuditEvent>,
}

/// 已注册工具（定义 + handler）。
struct RegisteredTool {
    definition: ToolDefinition,
    handler: Arc<dyn ToolHandler>,
}

/// 工具注册表：`name -> (definition, handler)`，名字唯一。
///
/// 线程安全：`register` 需要 `&mut self`（注册是启动期动作），挂载后
/// [`MountedToolset`] 是只读的，可安全共享给 server 的并发调用。
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

impl ToolRegistry {
    /// 空注册表。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个工具。
    ///
    /// # Errors
    /// - [`ToolBusError::DuplicateTool`]：同名工具已注册。两个元工具名
    ///   （`toolset.list` / `toolset.search`）**视同已被总线占用** —— 它们每次挂载都会
    ///   无条件加入，适配器占用这两个名字只会让「谁赢」变得不确定。
    /// - [`ToolBusError::InvalidToolName`]：名字为空（防御性检查；正常路径下
    ///   [`ToolDefinition::new`] 已拦下所有不合规名字）
    pub fn register(
        &mut self,
        definition: ToolDefinition,
        handler: Arc<dyn ToolHandler>,
    ) -> ToolBusResult<()> {
        let name = definition.name().to_owned();
        if name.is_empty() {
            return Err(ToolBusError::InvalidToolName { name });
        }
        if is_reserved_meta_tool_name(&name) || self.tools.contains_key(&name) {
            return Err(ToolBusError::DuplicateTool { tool: name });
        }
        self.tools.insert(
            name,
            RegisteredTool {
                definition,
                handler,
            },
        );
        Ok(())
    }

    /// 是否已注册某个工具名。
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 已注册工具数（不含两个元工具）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 注册表是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 解析挂载选择 → 升序去重的工具名 → 加上两个元工具 → 算出报告。
    ///
    /// 幂等：同一注册表 + 同一选择 + 同一时钟必得同一结果（指纹与挂载集确定；只有
    /// 超限时的 `AuditEvent.ts` 随时钟变化）。
    ///
    /// # Errors
    /// - [`ToolBusError::UnknownTool`]：选择引用了未注册的名字 / 匹配不到工具的应用前缀
    /// - [`ToolBusError::FingerprintSerialization`]：指纹序列化失败（内部缺陷）
    /// - [`ToolBusError::AuditEventAssembly`]：超限审计事件组装失败（内部缺陷）
    /// - `meta_tools` 的注册期错误（两个元工具的定义本身不合规）
    pub(crate) fn mount(
        &self,
        selection: &MountSelection,
        session_id: &str,
        clock: &dyn Clock,
    ) -> ToolBusResult<MountedToolset> {
        let mut tools: BTreeMap<String, Arc<MountedTool>> = BTreeMap::new();
        for name in self.resolve_selection(selection)? {
            let registered = self
                .tools
                .get(&name)
                .ok_or_else(|| ToolBusError::UnknownTool { tool: name.clone() })?;
            tools.insert(
                name,
                Arc::new(MountedTool {
                    definition: registered.definition.clone(),
                    handler: Arc::clone(&registered.handler),
                }),
            );
        }

        // 元工具 handler 只持有这份**纯数据**目录（不是 MountedToolset）→ 不产生 Arc 环。
        // 目录内容 = 最终挂载集，含元工具自己：模型问「有哪些工具」时应当看到全部。
        let mut catalog: Vec<ToolSummary> = tools
            .values()
            .map(|tool| ToolSummary::from_definition(&tool.definition))
            .collect();
        catalog.push(ToolSummary::meta_list());
        catalog.push(ToolSummary::meta_search());
        for (definition, handler) in meta_tools(Arc::new(catalog))? {
            tools.insert(
                definition.name().to_owned(),
                Arc::new(MountedTool {
                    definition,
                    handler,
                }),
            );
        }

        let report = build_report(&tools, session_id, clock)?;
        Ok(MountedToolset { tools, report })
    }

    /// 解析挂载选择 → 升序、去重的工具名。
    ///
    /// 重复名字被去重（`Names(["a","a"])` 与 `Names(["a"])` 等价）；未知名字 / 无匹配前缀
    /// 一律报错（不变量 1）。
    fn resolve_selection(&self, selection: &MountSelection) -> ToolBusResult<Vec<String>> {
        let mut selected: Vec<String> = match selection {
            MountSelection::All => self.tools.keys().cloned().collect(),
            MountSelection::Names(names) => {
                let mut picked = Vec::with_capacity(names.len());
                for name in names {
                    if !self.tools.contains_key(name) {
                        return Err(ToolBusError::UnknownTool { tool: name.clone() });
                    }
                    picked.push(name.clone());
                }
                picked
            }
            MountSelection::Apps(apps) => {
                let mut picked = Vec::new();
                for app in apps {
                    let matched: Vec<String> = self
                        .tools
                        .keys()
                        .filter(|name| app_of(name) == Some(app.as_str()))
                        .cloned()
                        .collect();
                    if matched.is_empty() {
                        return Err(ToolBusError::UnknownTool {
                            tool: format!("{app}.*"),
                        });
                    }
                    picked.extend(matched);
                }
                picked
            }
        };
        selected.sort();
        selected.dedup();
        Ok(selected)
    }
}

/// 已挂载的工具（定义 + handler）。
pub struct MountedTool {
    pub(crate) definition: ToolDefinition,
    pub(crate) handler: Arc<dyn ToolHandler>,
}

/// 已挂载工具集 + 它的挂载报告。
pub struct MountedToolset {
    tools: BTreeMap<String, Arc<MountedTool>>,
    report: MountReport,
}

impl MountedToolset {
    /// 按名字取工具（`None` = 未挂载 → server 侧返回 `TargetNotFound` 信封）。
    pub(crate) fn get(&self, name: &str) -> Option<&Arc<MountedTool>> {
        self.tools.get(name)
    }

    /// 遍历已挂载工具（按名字升序；`BTreeMap` 保证顺序确定，`tools/list` 因此可复现）。
    pub(crate) fn tools(&self) -> impl Iterator<Item = &Arc<MountedTool>> {
        self.tools.values()
    }

    /// 已挂载工具数。
    pub(crate) fn len(&self) -> usize {
        self.tools.len()
    }

    /// 挂载报告。
    pub const fn report(&self) -> &MountReport {
        &self.report
    }
}

/// 由挂载集算出报告（指纹 + 告警 + 审计事件）。
///
/// `tools` 必须是**最终**挂载集（含元工具），否则指纹与告警计数都会偏 —— 这正是把
/// 这一步放在 `mount` 末尾（而不是开头）的原因。
fn build_report(
    tools: &BTreeMap<String, Arc<MountedTool>>,
    session_id: &str,
    clock: &dyn Clock,
) -> ToolBusResult<MountReport> {
    let entries: Vec<FingerprintEntry<'_>> = tools
        .values()
        .map(|tool| tool.definition.fingerprint_entry())
        .collect();
    let fingerprint = compute_toolset_fingerprint(&entries)?;

    let mounted_count = tools.len();
    let warning = if mounted_count > MAX_MOUNTED_TOOLS_WITHOUT_WARNING {
        Some(ToolsetOversizeWarning {
            mounted_count,
            limit: MAX_MOUNTED_TOOLS_WITHOUT_WARNING,
        })
    } else {
        None
    };
    let audit_event = match &warning {
        Some(warning) => Some(oversize_audit_event(
            session_id,
            warning,
            &fingerprint,
            clock,
        )?),
        None => None,
    };

    Ok(MountReport {
        mounted: tools.keys().cloned().collect(),
        fingerprint,
        warning,
        audit_event,
    })
}

/// 超限 → 审计事件。
///
/// `event_type` 用协议里**已有**的 `incident.reported`（审计事件的 `event_type` 是封闭
/// 枚举，新增取值 = 改 schema = 漂移触发器 ③），具体原因写进 `args.reason`。
///
/// 为什么经 serde 而不是结构体字面量：[`AuditEvent`] 是 `#[non_exhaustive]` 的生成类型且
/// **没有构造器**（DRIFT-020-3）。
///
/// # Errors
/// [`ToolBusError::AuditEventAssembly`]：`serde_json` 拒收该形状（内部缺陷）。
fn oversize_audit_event(
    session_id: &str,
    warning: &ToolsetOversizeWarning,
    fingerprint: &ToolsetFingerprint,
    clock: &dyn Clock,
) -> ToolBusResult<AuditEvent> {
    let value = serde_json::json!({
        "version": "1.0",
        "event_type": "incident.reported",
        "ts": clock.now_rfc3339(),
        "session_id": session_id,
        "actor": "system",
        "action": "toolset.mount",
        "args": {
            "reason": "toolset_oversize",
            "mounted_count": warning.mounted_count,
            "limit": warning.limit,
            "fingerprint": fingerprint.as_str(),
        },
        "prev_hash": "",
        "self_hash": "",
    });
    serde_json::from_value(value).map_err(|error| ToolBusError::AuditEventAssembly {
        reason: error.to_string(),
    })
}

/// 工具名的第一段 = 应用标识；不足一段时为 `None`。
fn app_of(name: &str) -> Option<&str> {
    match name.split('.').next() {
        Some(app) if !app.is_empty() => Some(app),
        _ => None,
    }
}
