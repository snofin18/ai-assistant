//! 工具声明（`ToolDefinition`）：协议 schema + MCP 注解 + 注册期校验。
//!
//! ## 名字规则（`docs/spec/tool-schema.md` §4 不变量 1）
//!
//! 默认 `<app>.<domain>.<action>` 三段式：全小写字母 / 数字 / 下划线，每段以字母开头；
//! 禁止裸名。**唯一例外**是两个元工具（两段式），细节与原因见 `meta.rs` 模块头
//! （DRIFT-020-4 / PL-080）。例外是闭集，其余非三段式名字一律拒绝（fail-closed）。
//!
//! ## 风险级 ↔ MCP 注解（架构 v2 §5.2 的双向映射）
//!
//! `destructiveHint = true → 风险级 ≥ high`；`readOnlyHint = true → 风险级 = low`。
//! 由风险级推导注解是**默认**路径；显式给注解（将来接第三方 MCP server 时）走
//! [`ToolDefinition::with_annotations`]，此时做一致性校验，冲突即拒绝注册 —— 不让
//! 「模型看到的注解」与「policy 用的风险级」（TASK-021）各说各话。
//!
//! ## 为什么所有校验都在注册期做
//!
//! schema 一旦进上下文就会影响模型的每一次决策；等到调用时才发现「这条约束本 crate
//! 根本强制不了」，就等于把约束变成了装饰（违反铁律 1 / 铁律 4）。宁可注册失败。
//!
//! 相关：架构 v2 §5.2、`docs/spec/tool-schema.md` §3 / §4 / §5、ADR-0021（受控词）。

use std::sync::Arc;

use assistant_protocol::{RiskLevel, ToolSchema};
use rmcp::model::{Tool, ToolAnnotations};
use serde_json::{Map, Value};

use crate::error::{ToolBusError, ToolBusResult};
use crate::fingerprint::FingerprintEntry;
use crate::meta::is_reserved_meta_tool_name;
use crate::schema::collect_unenforceable_constructs;

/// `ToolSchema.version` 的取值：本卡只支持 1.0（tool-schema.md §4 不变量 5 的起点）。
const TOOL_SCHEMA_VERSION: &str = "1.0";

/// 一个工具的完整声明。
///
/// 内部持 `assistant_protocol::ToolSchema`（**复用**协议类型，不自建第二套字段），
/// 路径是 serde —— 生成类型 `#[non_exhaustive]` 且没有构造器（DRIFT-020-3）。
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    schema: ToolSchema,
    annotations: ToolAnnotations,
}

impl ToolDefinition {
    /// 构造并**立即校验**一个工具声明。
    ///
    /// `output` 默认 `{}`（draft-07 里 `{}` = 任何值都合法），用
    /// [`with_output_schema`](Self::with_output_schema) 收紧。`requires_approval` 对
    /// `critical` 默认 `true`（保守默认；真正的放行判断归 TASK-021，本卡不做判断）。
    ///
    /// 幂等 / 无副作用：纯函数，只读入参。
    ///
    /// # Errors
    /// - [`ToolBusError::InvalidToolName`]：名字不是三段式，也不是两个保留的元工具名
    /// - [`ToolBusError::UnsupportedSchemaKeyword`]：根输入 schema 不是 JSON 对象，或含本
    ///   crate 无法强制的关键字 / 形状非法的关键字（fail-closed，列出 JSON pointer）
    /// - [`ToolBusError::SchemaAssembly`]：`ToolSchema` 组装失败（协议生成类型的构造缺口）
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        risk_level: RiskLevel,
        input_schema: Value,
    ) -> ToolBusResult<Self> {
        let name = name.into();
        if !is_admissible_tool_name(&name) {
            return Err(ToolBusError::InvalidToolName { name });
        }
        // MCP 的 `Tool.inputSchema` 必须是对象；布尔 schema 虽然合法（draft-07），但在这里
        // 无法表达 —— 与其丢掉这条信息，不如拒绝注册。
        if !input_schema.is_object() {
            return Err(ToolBusError::UnsupportedSchemaKeyword {
                tool: name,
                problems: vec![
                    "`: the root input schema must be a JSON object (MCP `Tool.inputSchema` is an object, not `true`/`false`)".to_owned(),
                ],
            });
        }
        let problems = collect_unenforceable_constructs(&input_schema);
        if !problems.is_empty() {
            return Err(ToolBusError::UnsupportedSchemaKeyword {
                tool: name,
                problems,
            });
        }

        // 逐字段插入而不是 `json!`：`json!` 会按引用读取表达式，让 `input_schema` 看起来
        // 没被消费，也掩盖了「哪个字段被移动」这件事。
        let mut schema_value = Map::new();
        schema_value.insert(
            "version".to_owned(),
            Value::String(TOOL_SCHEMA_VERSION.to_owned()),
        );
        schema_value.insert("name".to_owned(), Value::String(name.clone()));
        schema_value.insert("description".to_owned(), Value::String(description.into()));
        schema_value.insert("input".to_owned(), input_schema);
        schema_value.insert("output".to_owned(), Value::Object(Map::new()));
        // 风险级写**本文件自己的稳定 token**（与指纹同源），不依赖 serde 的 rename 策略；
        // 协议新增风险级时会得到 `unrecognized-risk-level` → 反序列化失败 → 注册失败
        // （fail-closed，而不是被悄悄当成某一级）。
        schema_value.insert(
            "risk_level".to_owned(),
            Value::String(risk_level_token(risk_level).to_owned()),
        );
        schema_value.insert(
            "requires_approval".to_owned(),
            Value::Bool(matches!(risk_level, RiskLevel::Critical)),
        );
        schema_value.insert("idempotent".to_owned(), Value::Bool(false));

        let schema: ToolSchema =
            serde_json::from_value(Value::Object(schema_value)).map_err(|error| {
                ToolBusError::SchemaAssembly {
                    tool: name,
                    reason: error.to_string(),
                }
            })?;

        Ok(Self {
            annotations: derive_annotations(risk_level, schema.idempotent),
            schema,
        })
    }

    /// 收紧输出 schema。
    ///
    /// 只做形状校验（对象或 `null`），**不**递归强制 —— 工具输出由工具自己负责，本 crate
    /// 的职责是「载荷与来源一起装箱」，不是替工具校验业务语义。
    ///
    /// # Errors
    /// [`ToolBusError::UnsupportedSchemaKeyword`]：`output_schema` 既不是对象也不是 `null`。
    pub fn with_output_schema(mut self, output_schema: Value) -> ToolBusResult<Self> {
        if !(output_schema.is_object() || output_schema.is_null()) {
            return Err(ToolBusError::UnsupportedSchemaKeyword {
                tool: self.schema.name,
                problems: vec!["`: the output schema must be a JSON object or null".to_owned()],
            });
        }
        self.schema.output = output_schema;
        Ok(self)
    }

    /// 声明幂等性，并让 MCP 注解跟着走（两处事实必须一致，见模块头）。
    ///
    /// # Errors
    /// 目前不会失败（推导出的注解按构造与事实一致）；保留 `Result` 是为了将来给
    /// 「显式注解与幂等声明冲突」留出统一的失败出口，不让调用方在版本升级后被迫改签名。
    pub fn with_idempotent(mut self, idempotent: bool) -> ToolBusResult<Self> {
        self.schema.idempotent = idempotent;
        self.annotations = derive_annotations(self.schema.risk_level, idempotent);
        Ok(self)
    }

    /// 显式声明需要人工确认（默认只有 `critical` 为 `true`）。
    #[must_use]
    pub const fn with_requires_approval(mut self, requires_approval: bool) -> Self {
        self.schema.requires_approval = requires_approval;
        self
    }

    /// 附加检索标签（`toolset.search` 会读它们）。
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.schema.tags = tags;
        self
    }

    /// 用外部给的 MCP 注解覆盖推导值，并**校验一致性**（架构 v2 §5.2 双向映射）。
    ///
    /// # Errors
    /// [`ToolBusError::RiskAnnotationMismatch`]：`readOnlyHint` / `destructiveHint` /
    /// `idempotentHint` 与已声明的事实冲突（含「既只读、又有破坏性」这种自相矛盾的组合）。
    pub fn with_annotations(mut self, annotations: ToolAnnotations) -> ToolBusResult<Self> {
        verify_annotations(&self.schema, &annotations)?;
        self.annotations = annotations;
        Ok(self)
    }

    /// 工具名（`<app>.<domain>.<action>`，或两个元工具名之一）。
    #[must_use]
    pub fn name(&self) -> &str {
        &self.schema.name
    }

    /// 给模型看的描述（进指纹：改了描述 = 模型看到的东西变了）。
    #[must_use]
    pub fn description(&self) -> &str {
        &self.schema.description
    }

    /// 风险级。
    #[must_use]
    pub const fn risk_level(&self) -> RiskLevel {
        self.schema.risk_level
    }

    /// 是否幂等。
    #[must_use]
    pub const fn is_idempotent(&self) -> bool {
        self.schema.idempotent
    }

    /// 输入 JSON Schema（`ToolSchema.input`，参数校验的唯一事实源）。
    #[must_use]
    pub const fn input_schema(&self) -> &Value {
        &self.schema.input
    }

    /// 输出 JSON Schema（`ToolSchema.output`）。
    #[must_use]
    pub const fn output_schema(&self) -> &Value {
        &self.schema.output
    }

    /// 协议层声明（`assistant_protocol::ToolSchema`）。
    #[must_use]
    pub const fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    /// MCP 注解（模型侧看到的风险提示）。
    #[must_use]
    pub const fn annotations(&self) -> &ToolAnnotations {
        &self.annotations
    }

    /// 检索标签（`meta.rs` 抽摘要用）。
    pub(crate) fn schema_tags(&self) -> &[String] {
        &self.schema.tags
    }

    /// 转成 MCP `tools/list` 的条目。
    ///
    /// # Errors
    /// [`ToolBusError::SchemaAssembly`]：根输入 schema 不是 JSON 对象。构造期已拒绝这种
    /// 输入，因此这里是**不可达**的错误路径 —— 保留它是为了不写 `unwrap`（铁律 1）。
    pub(crate) fn to_mcp_tool(&self) -> ToolBusResult<Tool> {
        let Some(input) = self.schema.input.as_object() else {
            return Err(ToolBusError::SchemaAssembly {
                tool: self.schema.name.clone(),
                reason: "root input schema is not a JSON object".to_owned(),
            });
        };
        let mut tool = Tool::new(
            self.schema.name.clone(),
            self.schema.description.clone(),
            Arc::new(input.clone()),
        );
        if let Some(output) = self.schema.output.as_object() {
            tool = tool.with_raw_output_schema(Arc::new(output.clone()));
        }
        Ok(tool.with_annotations(self.annotations.clone()))
    }

    /// 指纹输入视图（借用，不克隆 schema）。
    pub(crate) fn fingerprint_entry(&self) -> FingerprintEntry<'_> {
        FingerprintEntry {
            name: &self.schema.name,
            version: &self.schema.version,
            description: &self.schema.description,
            risk_level: self.schema.risk_level,
            input: &self.schema.input,
            output: &self.schema.output,
        }
    }
}

/// 名字是否可接受：三段式，或两个元工具字面量（模块头解释了为什么有例外）。
fn is_admissible_tool_name(name: &str) -> bool {
    if is_reserved_meta_tool_name(name) {
        return true;
    }
    let segments: Vec<&str> = name.split('.').collect();
    segments.len() == 3 && segments.iter().all(|segment| is_name_segment(segment))
}

/// 一段名字：首字符小写字母，其余小写字母 / 数字 / 下划线（naming.md §7）。
fn is_name_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// 由风险级推导 MCP 注解（架构 v2 §5.2 的双向映射）。
///
/// 四个 hint **全部显式写出**：MCP 的 `destructiveHint` / `openWorldHint` 在缺省时默认
/// `true`，留空会让一个只读低风险工具在模型眼里变成「有破坏性、面向开放世界」。
fn derive_annotations(risk_level: RiskLevel, idempotent: bool) -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(matches!(risk_level, RiskLevel::Low))
        .destructive(matches!(risk_level, RiskLevel::High | RiskLevel::Critical))
        .idempotent(idempotent)
        .open_world(false)
}

/// 校验显式注解与工具声明是否自洽（违反即拒绝注册）。
fn verify_annotations(schema: &ToolSchema, annotations: &ToolAnnotations) -> ToolBusResult<()> {
    let tool = schema.name.clone();
    let read_only = annotations.read_only_hint == Some(true);
    let destructive = annotations.destructive_hint == Some(true);

    if read_only && destructive {
        return Err(ToolBusError::RiskAnnotationMismatch {
            tool,
            declared: risk_level_token(schema.risk_level).to_owned(),
            required: "readOnlyHint and destructiveHint cannot both be true".to_owned(),
        });
    }
    if read_only && schema.risk_level != RiskLevel::Low {
        return Err(ToolBusError::RiskAnnotationMismatch {
            tool,
            declared: risk_level_token(schema.risk_level).to_owned(),
            required: "low (readOnlyHint = true)".to_owned(),
        });
    }
    if destructive && matches!(schema.risk_level, RiskLevel::Low | RiskLevel::Medium) {
        return Err(ToolBusError::RiskAnnotationMismatch {
            tool,
            declared: risk_level_token(schema.risk_level).to_owned(),
            required: "high or higher (destructiveHint = true)".to_owned(),
        });
    }
    if let Some(idempotent_hint) = annotations.idempotent_hint
        && idempotent_hint != schema.idempotent
    {
        return Err(ToolBusError::RiskAnnotationMismatch {
            tool,
            declared: format!("idempotentHint = {idempotent_hint}"),
            required: format!("idempotent = {}", schema.idempotent),
        });
    }
    Ok(())
}

/// 风险级 → 稳定 token。
///
/// 进 `toolset.*` 的输出与错误文本。刻意**不**走 serde 的 `rename_all`：那段文本是契约，
/// 不能因为将来给枚举加 `#[serde(rename)]` 就漂移（指纹同源：`fingerprint.rs` 也自带一份）。
pub const fn risk_level_token(risk_level: RiskLevel) -> &'static str {
    match risk_level {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Critical => "critical",
        // 协议新增风险级时：给一个与任何真实取值都不同的 token，绝不冒充已有级别
        // （`_` 是 `#[non_exhaustive]` 枚举的强制要求，不是「顺手加的兜底」）。
        _ => "unrecognized-risk-level",
    }
}
