//! 工具集元工具：`toolset.list` / `toolset.search`（架构 v2 §5.5 第 2 条）。
//!
//! 为什么需要它们：一个应用 30 个工具、绑定 5 个应用就是 150 个工具，全塞进上下文会
//! token 爆炸 + 模型选择准确率下降 + 幻觉调用。正确做法是先给模型**两个**检索入口，
//! 让它按需拉取清单。
//!
//! ## 名字为什么是两段式（唯一例外）
//!
//! `docs/spec/tool-schema.md` §4 不变量 1 要求 `<app>.<domain>.<action>` 三段式，而
//! 任务卡 TASK-020 交付物 8（与架构 v2 §5.5 第 2 条的写法一致）要求 `toolset.list` /
//! `toolset.search`。两者冲突 → 记 **DRIFT-020-4** 与 `docs/PARKING_LOT.md` PL-080，
//! 本卡按任务卡执行（裁决顺序：任务卡 > 代码现状），并把例外做成**闭集**：
//! [`is_reserved_meta_tool_name`] 只认这两个字面量，其余非三段式名字一律拒绝。
//!
//! ## 目录里不放 schema
//!
//! 元工具只回摘要（名字 / 描述 / 风险级 / 标签）。完整 schema 由 MCP `tools/list` 提供
//! —— 目录里塞 schema 会把上下文撑爆，正好违背 §5.5 的初衷。
//!
//! 相关：架构 v2 §5.5、`tasks/TASK-020-tool-bus-mcp-rmcp-server.md` 交付物 8。

use std::sync::Arc;

use assistant_protocol::RiskLevel;
use serde_json::{Map, Value};

use crate::error::ToolBusResult;
use crate::handler::{CallContext, ToolHandler, ToolOutput};
use crate::registry::{ToolDefinition, risk_level_token};

/// 元工具一：列出当前挂载的工具集。
pub const META_TOOL_LIST: &str = "toolset.list";

/// 元工具二：按关键词检索当前挂载的工具集。
pub const META_TOOL_SEARCH: &str = "toolset.search";

/// 元工具一给模型看的描述（描述是模型看到的指令性文本，会进指纹）。
const META_TOOL_LIST_DESCRIPTION: &str =
    "列出当前这一轮挂载的工具（可按 app_id 过滤）。不确定某个能力是否可用时先调用它。";

/// 元工具二给模型看的描述。
const META_TOOL_SEARCH_DESCRIPTION: &str =
    "按关键词检索当前这一轮挂载的工具（匹配名字 / 描述 / 标签），返回摘要而不是完整 schema。";

/// 元工具检索结果里的一条记录（**纯数据**，不持有 handler / schema）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSummary {
    name: String,
    description: String,
    risk_level: RiskLevel,
    tags: Vec<String>,
}

impl ToolSummary {
    /// 从工具定义抽摘要（注册期就抽好，元工具调用时不再碰 `ToolDefinition`）。
    pub fn from_definition(definition: &ToolDefinition) -> Self {
        Self {
            name: definition.name().to_owned(),
            description: definition.description().to_owned(),
            risk_level: definition.risk_level(),
            tags: definition.schema_tags().to_vec(),
        }
    }

    /// `toolset.list` 自己的摘要（目录必须包含元工具自己：它们确实是挂载着的）。
    pub fn meta_list() -> Self {
        meta_summary(META_TOOL_LIST, META_TOOL_LIST_DESCRIPTION)
    }

    /// `toolset.search` 自己的摘要。
    pub fn meta_search() -> Self {
        meta_summary(META_TOOL_SEARCH, META_TOOL_SEARCH_DESCRIPTION)
    }

    /// 是否属于某个应用（名字第一段）。两段式的元工具第一段是 `toolset`。
    fn belongs_to_app(&self, app: &str) -> bool {
        self.name.split('.').next() == Some(app)
    }

    /// 关键词是否命中：名字 / 描述 / 标签的**大小写不敏感**子串匹配。
    ///
    /// 为什么只做子串匹配：本卡不引入全文检索依赖（新增依赖 = 漂移触发器 ①），
    /// 40~150 个工具的规模用子串足够；将来要更好用再单开卡。
    fn matches_query(&self, lowercase_needle: &str) -> bool {
        self.name.to_lowercase().contains(lowercase_needle)
            || self.description.to_lowercase().contains(lowercase_needle)
            || self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(lowercase_needle))
    }

    /// 元工具输出里的一条记录。
    fn to_value(&self) -> Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "risk_level": risk_level_token(self.risk_level),
            "tags": self.tags,
        })
    }
}

/// 两个元工具共用的摘要构造。
fn meta_summary(name: &str, description: &str) -> ToolSummary {
    ToolSummary {
        name: name.to_owned(),
        description: description.to_owned(),
        // 元工具只读当前挂载集，不改任何应用状态 → low（与 `derive_annotations` 的口径一致）。
        risk_level: RiskLevel::Low,
        tags: vec!["toolset".to_owned(), "meta".to_owned()],
    }
}

/// 是否是两个保留的元工具名（名字规则的**闭集**例外，见模块头）。
pub fn is_reserved_meta_tool_name(name: &str) -> bool {
    name == META_TOOL_LIST || name == META_TOOL_SEARCH
}

/// 构造两个元工具的 `(定义, handler)`。
///
/// `catalog` 是**最终挂载集**的摘要（含元工具自己）。handler 只持有这份纯数据，
/// 不持有 `MountedToolset` → 不会形成 `Arc` 环。
///
/// # Errors
/// [`ToolBusError::InvalidToolName`](crate::ToolBusError::InvalidToolName) /
/// [`UnsupportedSchemaKeyword`](crate::ToolBusError::UnsupportedSchemaKeyword) /
/// [`SchemaAssembly`](crate::ToolBusError::SchemaAssembly)：两个元工具的定义本身不合规
/// （只可能是本文件改坏了，属内部缺陷，不吞错）。
pub fn meta_tools(
    catalog: Arc<Vec<ToolSummary>>,
) -> ToolBusResult<Vec<(ToolDefinition, Arc<dyn ToolHandler>)>> {
    let list_definition = ToolDefinition::new(
        META_TOOL_LIST,
        META_TOOL_LIST_DESCRIPTION,
        RiskLevel::Low,
        serde_json::json!({
            "type": "object",
            "properties": { "app_id": { "type": "string" } },
            "additionalProperties": false
        }),
    )?
    .with_idempotent(true)?
    .with_tags(vec!["toolset".to_owned(), "meta".to_owned()]);

    let search_definition = ToolDefinition::new(
        META_TOOL_SEARCH,
        META_TOOL_SEARCH_DESCRIPTION,
        RiskLevel::Low,
        serde_json::json!({
            "type": "object",
            "properties": { "query": { "type": "string", "minLength": 1 } },
            "required": ["query"],
            "additionalProperties": false
        }),
    )?
    .with_idempotent(true)?
    .with_tags(vec!["toolset".to_owned(), "meta".to_owned()]);

    let list_catalog = Arc::clone(&catalog);
    let list_handler: Arc<dyn ToolHandler> = Arc::new(
        move |_call: &CallContext, arguments: &Map<String, Value>| -> ToolBusResult<ToolOutput> {
            let app_filter = arguments.get("app_id").and_then(Value::as_str);
            let listed: Vec<Value> = list_catalog
                .iter()
                .filter(|summary| app_filter.is_none_or(|app| summary.belongs_to_app(app)))
                .map(ToolSummary::to_value)
                .collect();
            Ok(ToolOutput::json(serde_json::json!({
                "count": listed.len(),
                "tools": listed,
            })))
        },
    );

    let search_handler: Arc<dyn ToolHandler> = Arc::new(
        move |_call: &CallContext, arguments: &Map<String, Value>| -> ToolBusResult<ToolOutput> {
            // 参数校验已保证 `query` 存在且为非空字符串；`unwrap_or_default` 只是不给 panic
            // 留机会（空串会命中所有工具，仍然是一个**可解释**的结果，不是静默失败）。
            let needle = arguments
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase();
            let matched: Vec<Value> = catalog
                .iter()
                .filter(|summary| summary.matches_query(&needle))
                .map(ToolSummary::to_value)
                .collect();
            Ok(ToolOutput::json(serde_json::json!({
                "count": matched.len(),
                "tools": matched,
            })))
        },
    );

    Ok(vec![
        (list_definition, list_handler),
        (search_definition, search_handler),
    ])
}
