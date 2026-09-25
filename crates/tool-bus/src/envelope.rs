//! 统一返回信封的组装与**截断契约**（架构 v2 §5.3）。
//!
//! 形状的事实源是 `assistant_protocol::ToolEnvelope`（来自
//! `protocol/envelope/envelope-1.0.json`）—— 本 crate **不自建**第二套信封。
//!
//! ## 为什么组装要经 serde（而不是结构体字面量）
//!
//! `assistant_protocol` 的生成类型全部 `#[non_exhaustive]`（协议 crate 不变量 2，
//! 便于向后兼容扩展），而生成器**没有**为 `Source` / `Truncation` / `Evidence` /
//! `Metrics` 产出构造器。于是外部 crate 既不能用结构体字面量构造、也不能用字段赋值
//! 变出新值，只能经 serde 组装。这是真实缺口，已记 **DRIFT-020-3** 与
//! `docs/PARKING_LOT.md` PL-078（建议 codegen 为生成类型补构造器 / builder）。
//! 本文件把这段工作收敛在**一个**函数里，并对组装结果做后置条件校验（铁律 4）。
//!
//! ## 截断契约（不变量 3）
//!
//! `data` 的序列化大小超过 `max_bytes` 时：
//! 1. 统计 `data` 里的字符串叶子数量 `n`，把每个长度 > `max_bytes / n` 的字符串按
//!    **UTF-8 字符边界**截到该上限；重复直到序列化大小达标（最多 [`MAX_TRUNCATION_PASSES`] 轮）。
//! 2. 结果信封一定带 `truncated.occurred = true` + `reason = "max_bytes"` +
//!    `original_bytes = 原始字节数` —— 模型必须知道内容不完整（v2 §5.3 规则 3）。
//! 3. 仍然塞不下（例如全是短字符串而键名开销就超了）→ **调用失败**
//!    （[`ToolBusError::PayloadTooLarge`]），绝不悄悄返回超限载荷（铁律 1）。
//!
//! 为什么保留对象形状而不是把 `data` 换成字符串：`envelope-1.0.json` 规定
//! `data` 只能是 **object 或 null**，换成字符串会直接违反协议 schema。
//!
//! 相关：架构 v2 §5.3、`docs/spec/envelope.md`（跨进程 envelope，与本文的「工具返回
//! 信封」不是同一个东西，勿混）、`docs/spec/tool-schema.md` §4。

use assistant_protocol::{ErrorCode, SourceKind, ToolEnvelope};
use serde_json::{Map, Value};

use crate::error::{ToolBusError, ToolBusResult};

/// `data` 截断的最大轮数（每轮把超限字符串压到当时的平均值）。
const MAX_TRUNCATION_PASSES: usize = 8;

/// 截断时递归遍历 `data` 的深度上限（对抗性载荷不能把栈打穿）。
const MAX_TRUNCATION_DEPTH: usize = 64;

/// 数据的来源（信封 `source` 字段）。
///
/// 为什么用本地描述类型而不是直接收 `assistant_protocol::Source`：见模块头
/// （生成类型 `#[non_exhaustive]` 且无构造器）。字段语义与协议完全一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescriptor {
    /// 来源类别（`app_content` / `web_content` / `clipboard` …）。
    pub kind: SourceKind,
    /// 应用标识（如 `com.example.notepad`）。
    pub app_id: Option<String>,
    /// 应用内的目标标识（如 `doc_123`）。
    pub target: Option<String>,
}

impl SourceDescriptor {
    /// 应用内容（架构 v2 §12.4 的默认不可信来源）。
    #[must_use]
    pub fn app_content(app_id: impl Into<String>) -> Self {
        Self {
            kind: SourceKind::AppContent,
            app_id: Some(app_id.into()),
            target: None,
        }
    }

    /// 替换目标标识。
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
}

/// 证据（信封 `evidence` 字段）：让 undo / verify / replay 能引用动作后的状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EvidenceDescriptor {
    /// 树快照 id。
    pub snapshot_id: Option<String>,
    /// 截图路径或 URI。
    pub screenshot: Option<String>,
    /// 其它证据（开放字段）。
    pub extra: Option<Value>,
}

impl EvidenceDescriptor {
    /// 只带树快照 id 的证据。
    #[must_use]
    pub fn from_snapshot(snapshot_id: impl Into<String>) -> Self {
        Self {
            snapshot_id: Some(snapshot_id.into()),
            screenshot: None,
            extra: None,
        }
    }
}

/// 组装一封成功信封所需的全部输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeRequest {
    /// 工具名（`<app>.<domain>.<action>`）。
    pub tool: String,
    /// 任务 id。
    pub task_id: String,
    /// 步骤 id。
    pub step_id: String,
    /// 载荷（**必须是 JSON 对象或 null**，见 `envelope-1.0.json` 的 `data` 约束）。
    pub data: Value,
    /// 是否不可信（架构 v2 §12.4 的反提示注入标记）。
    pub untrusted: bool,
    /// 来源；`untrusted = true` 时**必填**。
    pub source: Option<SourceDescriptor>,
    /// 证据。
    pub evidence: Option<EvidenceDescriptor>,
    /// 载荷预算（字节）；`None` = 不截断。
    pub max_bytes: Option<usize>,
    /// 本次调用耗时（毫秒），进 `metrics.duration_ms`。
    pub duration_ms: u64,
    /// 本次调用的尝试次数（含 tool-bus 内部重试），进 `metrics.attempts`。
    pub attempts: u32,
}

impl EnvelopeRequest {
    /// 最小构造：其余字段用 `with_*` 补。
    #[must_use]
    pub fn new(
        tool: impl Into<String>,
        task_id: impl Into<String>,
        step_id: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            tool: tool.into(),
            task_id: task_id.into(),
            step_id: step_id.into(),
            data,
            untrusted: false,
            source: None,
            evidence: None,
            max_bytes: None,
            duration_ms: 0,
            attempts: 1,
        }
    }

    /// 标记为不可信内容并给出来源（两者必须同时给）。
    #[must_use]
    pub fn untrusted(mut self, source: SourceDescriptor) -> Self {
        self.untrusted = true;
        self.source = Some(source);
        self
    }

    /// 附加证据。
    #[must_use]
    pub fn with_evidence(mut self, evidence: EvidenceDescriptor) -> Self {
        self.evidence = Some(evidence);
        self
    }

    /// 设置载荷预算（字节）。
    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = Some(max_bytes);
        self
    }

    /// 设置 `metrics`。
    #[must_use]
    pub const fn with_metrics(mut self, duration_ms: u64, attempts: u32) -> Self {
        self.duration_ms = duration_ms;
        self.attempts = attempts;
        self
    }
}

/// 组装成功信封（含截断与后置条件校验）。
///
/// 幂等：给定同一 [`EnvelopeRequest`] 必得同一信封（无时钟读取、无随机）。
///
/// # Errors
/// - [`ToolBusError::UntrustedWithoutSource`]：`untrusted = true` 但没给 `source`
/// - [`ToolBusError::EnvelopeAssembly`]：`data` 不是对象 / null，或 serde 组装失败，
///   或组装结果不满足后置条件
/// - [`ToolBusError::PayloadTooLarge`]：超预算且按契约无法再截断
pub fn assemble_envelope(request: &EnvelopeRequest) -> ToolBusResult<ToolEnvelope> {
    if request.untrusted && request.source.is_none() {
        return Err(ToolBusError::UntrustedWithoutSource {
            tool: request.tool.clone(),
        });
    }
    if !(request.data.is_object() || request.data.is_null()) {
        return Err(ToolBusError::EnvelopeAssembly {
            tool: request.tool.clone(),
            reason: "envelope data must be a JSON object or null (envelope-1.0.json)".to_owned(),
        });
    }

    let (data, truncation) = apply_max_bytes(&request.tool, &request.data, request.max_bytes)?;

    let mut object = Map::new();
    object.insert("version".to_owned(), Value::String("1.0".to_owned()));
    object.insert("tool".to_owned(), Value::String(request.tool.clone()));
    object.insert("task_id".to_owned(), Value::String(request.task_id.clone()));
    object.insert("step_id".to_owned(), Value::String(request.step_id.clone()));
    object.insert("ok".to_owned(), Value::Bool(true));
    object.insert("data".to_owned(), data);
    object.insert("untrusted".to_owned(), Value::Bool(request.untrusted));
    object.insert("source".to_owned(), source_value(request.source.as_ref()));
    object.insert("truncated".to_owned(), truncation_value(truncation));
    object.insert(
        "evidence".to_owned(),
        evidence_value(request.evidence.as_ref()),
    );
    object.insert(
        "metrics".to_owned(),
        metrics_value(request.duration_ms, request.attempts),
    );
    // `error` 是 envelope-1.0.json 的必填字段（ok=true 时为 null）。
    object.insert("error".to_owned(), Value::Null);

    let envelope: ToolEnvelope =
        serde_json::from_value(Value::Object(object)).map_err(|error| {
            ToolBusError::EnvelopeAssembly {
                tool: request.tool.clone(),
                reason: error.to_string(),
            }
        })?;
    verify_postconditions(request, &envelope, truncation.is_some())?;
    Ok(envelope)
}

/// 组装一封失败信封。
///
/// 失败信封刻意**不带** `metrics` / `evidence`：它们的语义只在「工具真的跑过」时成立，
/// 而失败可能连 handler 都没进（参数非法）。`evidence_ref` 由
/// [`ToolEnvelope::error`] 从 `ErrorDefinition` 自动注入。
#[must_use]
pub fn error_envelope(
    tool: &str,
    task_id: &str,
    step_id: &str,
    code: ErrorCode,
    message: &str,
) -> ToolEnvelope {
    ToolEnvelope::error(
        tool.to_owned(),
        task_id.to_owned(),
        step_id.to_owned(),
        code,
        message.to_owned(),
    )
}

/// 后置条件校验（铁律 4）：组装结果必须与请求一致，否则宁可失败也不返回可疑信封。
fn verify_postconditions(
    request: &EnvelopeRequest,
    envelope: &ToolEnvelope,
    expect_truncation: bool,
) -> ToolBusResult<()> {
    let mismatched = envelope.tool != request.tool
        || envelope.task_id != request.task_id
        || envelope.step_id != request.step_id
        || !envelope.ok
        || envelope.untrusted != request.untrusted
        || envelope.source.is_some() != request.source.is_some()
        || envelope.truncated.is_some() != expect_truncation
        || envelope.error.is_some();
    if mismatched {
        return Err(ToolBusError::EnvelopeAssembly {
            tool: request.tool.clone(),
            reason: "assembled envelope failed its postcondition checks".to_owned(),
        });
    }
    Ok(())
}

/// 截断信息（内部表示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TruncationInfo {
    original_bytes: usize,
}

/// 按预算截断 `data`；返回 `(截断后的 data, 截断信息)`。
fn apply_max_bytes(
    tool: &str,
    data: &Value,
    max_bytes: Option<usize>,
) -> ToolBusResult<(Value, Option<TruncationInfo>)> {
    let Some(limit) = max_bytes else {
        return Ok((data.clone(), None));
    };
    let original_bytes = serialized_len(tool, data)?;
    if original_bytes <= limit {
        return Ok((data.clone(), None));
    }

    let mut truncated = data.clone();
    for _pass in 0..MAX_TRUNCATION_PASSES {
        let current = serialized_len(tool, &truncated)?;
        if current <= limit {
            return Ok((truncated, Some(TruncationInfo { original_bytes })));
        }
        let leaves = count_string_leaves(&truncated, 0);
        if leaves == 0 {
            break;
        }
        let cap = (limit / leaves).max(1);
        shorten_string_leaves(&mut truncated, cap, 0);
    }

    Err(ToolBusError::PayloadTooLarge {
        tool: tool.to_owned(),
        actual_bytes: serialized_len(tool, &truncated)?,
        max_bytes: limit,
    })
}

/// `data` 的紧凑序列化大小（字节）。
fn serialized_len(tool: &str, data: &Value) -> ToolBusResult<usize> {
    serde_json::to_vec(data)
        .map(|bytes| bytes.len())
        .map_err(|error| ToolBusError::EnvelopeAssembly {
            tool: tool.to_owned(),
            reason: format!("payload is not serializable: {error}"),
        })
}

/// 统计字符串叶子数量（超过深度上限即停，让调用方走 fail-closed 分支）。
fn count_string_leaves(value: &Value, depth: usize) -> usize {
    if depth > MAX_TRUNCATION_DEPTH {
        return 0;
    }
    match value {
        Value::String(_) => 1,
        Value::Array(items) => items
            .iter()
            .map(|item| count_string_leaves(item, depth + 1))
            .sum(),
        Value::Object(object) => object
            .values()
            .map(|inner| count_string_leaves(inner, depth + 1))
            .sum(),
        _ => 0,
    }
}

/// 把每个超过 `cap` 字节的字符串叶子截到 `cap`（按 UTF-8 字符边界）。
fn shorten_string_leaves(value: &mut Value, cap: usize, depth: usize) {
    if depth > MAX_TRUNCATION_DEPTH {
        return;
    }
    match value {
        Value::String(text) => {
            if text.len() > cap {
                let truncated = prefix_on_char_boundary(text, cap);
                text.truncate(truncated);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                shorten_string_leaves(item, cap, depth + 1);
            }
        }
        Value::Object(object) => {
            for inner in object.values_mut() {
                shorten_string_leaves(inner, cap, depth + 1);
            }
        }
        _ => {}
    }
}

/// 取不超过 `cap` 字节的**最长前缀**，且落点必须是 UTF-8 字符边界。
///
/// 用 `char_indices` 累加而不是切片下标：workspace 把 `clippy::indexing_slicing` 当拒绝项。
fn prefix_on_char_boundary(text: &str, cap: usize) -> usize {
    let mut end = 0usize;
    for (index, character) in text.char_indices() {
        let next = index + character.len_utf8();
        if next > cap {
            break;
        }
        end = next;
    }
    end
}

/// `source` → JSON（`None` → `null`）。
fn source_value(source: Option<&SourceDescriptor>) -> Value {
    let Some(source) = source else {
        return Value::Null;
    };
    let mut object = Map::new();
    object.insert("kind".to_owned(), kind_value(source.kind));
    if let Some(app_id) = &source.app_id {
        object.insert("app_id".to_owned(), Value::String(app_id.clone()));
    }
    if let Some(target) = &source.target {
        object.insert("target".to_owned(), Value::String(target.clone()));
    }
    Value::Object(object)
}

/// `SourceKind` → 稳定 token（与 envelope-1.0.json 的 `enum` 逐字对应）。
fn kind_value(kind: SourceKind) -> Value {
    let token = match kind {
        SourceKind::AppContent => "app_content",
        SourceKind::UserInput => "user_input",
        SourceKind::WebContent => "web_content",
        SourceKind::FileArtifact => "file_artifact",
        SourceKind::Screenshot => "screenshot",
        SourceKind::Ocr => "ocr",
        SourceKind::Clipboard => "clipboard",
        // `SourceKind::Unknown` 与协议将来新增的类别都走这里：`envelope-1.0.json` 里有
        // `unknown` 这个合法取值 —— 不猜具体类别，但也不让信封组装失败（铁律 1：宁可
        // 报「未知」也不冒充某个已有类别）。
        _ => "unknown",
    };
    Value::String(token.to_owned())
}

/// 截断信息 → JSON（`None` → `null`）。
fn truncation_value(truncation: Option<TruncationInfo>) -> Value {
    let Some(truncation) = truncation else {
        return Value::Null;
    };
    let mut object = Map::new();
    object.insert("occurred".to_owned(), Value::Bool(true));
    object.insert("reason".to_owned(), Value::String("max_bytes".to_owned()));
    object.insert(
        "original_bytes".to_owned(),
        Value::Number(serde_json::Number::from(
            u64::try_from(truncation.original_bytes).unwrap_or(u64::MAX),
        )),
    );
    Value::Object(object)
}

/// 证据 → JSON（`None` → `null`）。
fn evidence_value(evidence: Option<&EvidenceDescriptor>) -> Value {
    let Some(evidence) = evidence else {
        return Value::Null;
    };
    let mut object = Map::new();
    if let Some(snapshot_id) = &evidence.snapshot_id {
        object.insert("snapshot_id".to_owned(), Value::String(snapshot_id.clone()));
    }
    if let Some(screenshot) = &evidence.screenshot {
        object.insert("screenshot".to_owned(), Value::String(screenshot.clone()));
    }
    if let Some(extra) = &evidence.extra {
        object.insert("extra".to_owned(), extra.clone());
    }
    Value::Object(object)
}

/// 指标 → JSON（`duration_ms` 必填，`attempts` 最小 1）。
fn metrics_value(duration_ms: u64, attempts: u32) -> Value {
    let mut object = Map::new();
    object.insert(
        "duration_ms".to_owned(),
        Value::Number(serde_json::Number::from(duration_ms)),
    );
    object.insert(
        "attempts".to_owned(),
        Value::Number(serde_json::Number::from(attempts.max(1))),
    );
    Value::Object(object)
}
