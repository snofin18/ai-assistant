//! `memory_fts`：任务历史、偏好与笔记的 FTS5 检索接口。
//
//! 职责：
//!   * `memory_records` 是内容的**唯一事实源**；`memory_fts` 是 contentless 候选索引。
//!   * 0004 迁移用三个触发器同步 INSERT / UPDATE / DELETE，避免 API 与索引两套写入路径。
//!   * 对外只暴露“记录写入 + 检索 + 一致性自检”，不暴露 SQL、不持有连接。
//!
//! 边界（不做什么）：
//!   * 不做向量检索 / 语义检索 / 外部索引（阶段 1 Out of scope）。
//!   * 不做 `core` 的 Memory、App Map 加载或上下文拼装（归 TASK-208）。
//!   * 不把查询文本当 FTS5 语法执行：所有用户词项都会转义为字面量，未知语法 fail-closed。
//!
//! 不变量：
//!   1. `memory_fts` 是 contentless 候选索引；每个候选必须再由源正文逐词复核，索引命中而
//!      源文不含时返回 [`StorageError::MemoryIndexInconsistent`]，不能返回 stale 结果。
//!   2. 检索结果携带 `record_kind` / `record_id` / `source_reference`；调用方因此能追溯
//!      到原始记录，而不是只拿到一段不可信文本。
//!   3. 空查询、超长查询、无字母/数字的词项与超限 limit 都返回带稳定 `reason_code()` 的
//!      [`StorageError`]，不得退化成“返回空集”。
//!
//! 相关：`docs/storage-design.md` §3.4 / §4 / §8、架构 v2 §15.1、TASK-206。

use std::fmt::Write as _;

use rusqlite::types::Value;
use rusqlite::{Connection, params, params_from_iter};

use crate::error::{StorageError, StorageResult};

/// 默认检索结果上限。
pub const DEFAULT_MEMORY_SEARCH_LIMIT: usize = 20;

/// 单次检索允许请求的最大结果数。
pub const MAX_MEMORY_SEARCH_LIMIT: usize = 100;

/// 查询文本的字符数上限（按 Unicode scalar value 计，不用字节数）。
pub const MAX_MEMORY_QUERY_CHARS: usize = 512;

/// 查询允许的最大词项数（按 Unicode 空白切分）。
pub const MAX_MEMORY_QUERY_TERMS: usize = 32;

/// `memory_records.record_id` 的字符数上限。
pub const MAX_MEMORY_RECORD_ID_CHARS: usize = 256;

/// `memory_records.source_reference` 的字符数上限。
pub const MAX_MEMORY_SOURCE_REFERENCE_CHARS: usize = 1024;

/// 单条记忆内容的字节上限。上限存在的意义是避免一次写入把整个索引拖垮。
pub const MAX_MEMORY_CONTENT_BYTES: usize = 1_048_576;

/// 记忆记录类型。字符串形式是持久化契约，**不得随版本改名**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MemoryRecordKind {
    /// 过去执行过的任务历史。
    TaskHistory,
    /// 用户偏好。
    Preference,
    /// 领域笔记。
    Note,
}

impl MemoryRecordKind {
    /// 全部受支持的类型（按稳定字符串排序）。
    pub const ALL: [Self; 3] = [Self::TaskHistory, Self::Preference, Self::Note];

    /// 持久化字符串。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaskHistory => "task_history",
            Self::Preference => "preference",
            Self::Note => "note",
        }
    }

    /// 解析持久化字符串。
    ///
    /// # Errors
    /// 未知类型返回 [`StorageError::InvalidArgument`]，不静默映射到 `Note` 或其它默认值。
    pub fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "task_history" => Ok(Self::TaskHistory),
            "preference" => Ok(Self::Preference),
            "note" => Ok(Self::Note),
            other => Err(StorageError::InvalidArgument {
                field: "record_kind",
                detail: format!("未知记忆记录类型 {other:?}"),
            }),
        }
    }
}

/// 一条可检索的记忆记录。
///
/// `source_reference` 是原文位置的可追溯引用（例如 `app-map:notepad#L42`）；
/// 它由写入方提供，存储层只校验形状，不把它当可信内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    /// 记录类型。
    pub record_kind: MemoryRecordKind,
    /// 记录在来源域内的稳定主键。
    pub record_id: String,
    /// 原文位置引用。
    pub source_reference: String,
    /// 可检索正文。
    pub content: String,
    /// 写入时间（Unix 毫秒）。
    pub updated_at: i64,
}

/// 检索请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryQuery {
    /// 用户查询文本；被解释为空白分隔的字面量词项，不暴露 FTS5 运算符。
    pub text: String,
    /// 可选的记录类型过滤器。
    pub kind: Option<MemoryRecordKind>,
    /// 返回结果上限。
    pub limit: usize,
}

impl MemoryQuery {
    /// 用默认 limit 创建查询。
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: None,
            limit: DEFAULT_MEMORY_SEARCH_LIMIT,
        }
    }

    /// 限定记录类型。
    #[must_use]
    pub const fn with_kind(mut self, kind: MemoryRecordKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// 覆盖结果上限；非法值由 [`search_memory`] fail-closed。
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// 一条带来源信息的检索结果。
#[derive(Debug, Clone, PartialEq)]
pub struct MemorySearchResult {
    /// 命中的记录类型。
    pub record_kind: MemoryRecordKind,
    /// 命中的记录主键。
    pub record_id: String,
    /// 原文位置引用。
    pub source_reference: String,
    /// 源正文前 160 个字符的片段（不是完整正文，也不含高亮标记）。
    pub snippet: String,
    /// BM25 分数；越小表示越相关。
    pub score: f64,
}

/// 一致性自检发现的索引问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIndexIssue {
    /// 出问题的源表 rowid / FTS rowid。
    pub row_id: i64,
    /// 问题类别。
    pub kind: MemoryIndexIssueKind,
}

/// 一致性自检的问题类别。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemoryIndexIssueKind {
    /// 源记录存在，但对应 FTS 行不存在。
    SourceRowMissingFromIndex,
    /// FTS 行存在，但对应源记录不存在。
    IndexRowMissingFromSource,
}

/// 写入一条记忆记录；0004 的触发器会在同一事务中同步 `memory_fts`。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：主键 / 原文引用 / 正文为空，或长度超过公开上限
/// - [`StorageError::Sqlite`]：`(record_kind, record_id)` 重复或底层写入失败；不静默覆盖
pub fn insert_memory_record(connection: &Connection, record: &MemoryRecord) -> StorageResult<()> {
    validate_memory_record(record)?;
    connection.execute(
        "INSERT INTO memory_records (record_kind, record_id, source_reference, content, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            record.record_kind.as_str(),
            record.record_id,
            record.source_reference,
            record.content,
            record.updated_at
        ],
    )?;
    Ok(())
}

/// 删除一条记忆记录；触发器同步删除索引行。
///
/// 返回值：`true` = 确实删除了一条记录；`false` = 记录不存在（幂等 no-op）。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：主键为空或超过上限
/// - [`StorageError::Sqlite`]：底层删除失败
pub fn delete_memory_record(
    connection: &Connection,
    record_kind: MemoryRecordKind,
    record_id: &str,
) -> StorageResult<bool> {
    validate_record_id(record_id)?;
    let changed = connection.execute(
        "DELETE FROM memory_records WHERE record_kind = ?1 AND record_id = ?2",
        params![record_kind.as_str(), record_id],
    )?;
    Ok(changed == 1)
}

/// 检索记忆记录。
///
/// 查询文本被当作**字面量词项**：调用方输入的 FTS5 运算符不会被执行；不支持的空白 /
/// 控制字符形状会在查询前被拒绝。结果按 BM25 升序（越相关越靠前）。
///
/// 副作用：只读，不改索引。是否幂等：相同数据库状态 + 相同查询返回相同结果。
///
/// # Errors
/// - [`StorageError::InvalidMemoryQuery`]：空查询 / 超长 / 无字母数字词项 / limit 非法
/// - [`StorageError::MemoryIndexInconsistent`]：命中的索引行与源表快照不一致，或记录类型无法解析
/// - [`StorageError::Sqlite`]：底层查询失败
pub fn search_memory(
    connection: &Connection,
    query: &MemoryQuery,
) -> StorageResult<Vec<MemorySearchResult>> {
    let validated = validate_query(query)?;
    let limit = i64::try_from(query.limit).map_err(|error| StorageError::InvalidMemoryQuery {
        field: "limit",
        detail: format!("无法转换为 SQLite 整数：{error}"),
    })?;
    let kind = query.kind.map(MemoryRecordKind::as_str);

    let (sql, values) = build_search_statement(&validated, kind, limit)?;

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), |row| {
        Ok(SearchRow {
            row_id: row.get(0)?,
            record_kind: row.get(1)?,
            record_id: row.get(2)?,
            source_reference: row.get(3)?,
            snippet: row.get(4)?,
            source_matches: row.get(5)?,
            score: row.get(6)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        let row = row?;
        if !row.source_matches {
            return Err(StorageError::MemoryIndexInconsistent {
                detail: format!(
                    "memory_fts rowid {} 命中查询，但源正文不包含全部查询词项",
                    row.row_id
                ),
            });
        }
        let record_kind = MemoryRecordKind::parse(&row.record_kind).map_err(|error| {
            StorageError::MemoryIndexInconsistent {
                detail: format!(
                    "memory_fts 行包含未知 record_kind {:?}: {}",
                    row.record_kind,
                    error.reason_code()
                ),
            }
        })?;
        results.push(MemorySearchResult {
            record_kind,
            record_id: row.record_id,
            source_reference: row.source_reference,
            snippet: row.snippet,
            score: row.score,
        });
    }
    Ok(results)
}

fn build_search_statement(
    validated: &ValidatedQuery,
    kind: Option<&str>,
    limit: i64,
) -> StorageResult<(String, Vec<Value>)> {
    let mut sql = String::from(
        "SELECT memory_fts.rowid, m.record_kind, m.record_id, m.source_reference,
                substr(m.content, 1, 160),
                CASE WHEN ",
    );
    for (offset, _) in validated.terms.iter().enumerate() {
        if offset > 0 {
            sql.push_str(" AND ");
        }
        write!(sql, "instr(lower(m.content), lower(?{})) > 0", offset + 3).map_err(|error| {
            StorageError::InvalidMemoryQuery {
                field: "text",
                detail: format!("构造查询失败：{error}"),
            }
        })?;
    }
    let limit_placeholder = validated.terms.len() + 3;
    write!(
        sql,
        " THEN 1 ELSE 0 END AS source_matches, bm25(memory_fts)
         FROM memory_fts
         JOIN memory_records AS m ON m.id = memory_fts.rowid
         WHERE memory_fts MATCH ?1
           AND (?2 IS NULL OR m.record_kind = ?2)
         ORDER BY bm25(memory_fts), m.record_kind, m.record_id
         LIMIT ?{limit_placeholder}"
    )
    .map_err(|error| StorageError::InvalidMemoryQuery {
        field: "text",
        detail: format!("构造查询失败：{error}"),
    })?;

    let mut values = Vec::with_capacity(validated.terms.len() + 3);
    values.push(Value::Text(validated.match_expression.clone()));
    values.push(kind.map_or(Value::Null, |value| Value::Text(value.to_owned())));
    for term in &validated.terms {
        values.push(Value::Text(term.clone()));
    }
    values.push(Value::Integer(limit));
    Ok((sql, values))
}

/// `search_memory` 的原始行快照；`source_matches` 表示 FTS 候选是否被源正文复核通过。
struct SearchRow {
    row_id: i64,
    record_kind: String,
    record_id: String,
    source_reference: String,
    snippet: String,
    source_matches: bool,
    score: f64,
}

/// 自检 `memory_records` 与 `memory_fts` 的一致性。
///
/// 正常状态返回空 Vec；发现源行缺索引或索引孤儿时返回带行号的报告。
/// 该 API 只报告事实，不自动重建索引，也不把不一致静默修好。
///
/// # Errors
/// 仅 [`StorageError::Sqlite`]（读取源表或虚表失败）。
pub fn verify_memory_index(connection: &Connection) -> StorageResult<Vec<MemoryIndexIssue>> {
    let mut issues = Vec::new();

    let mut missing = connection.prepare(
        "SELECT m.id
         FROM memory_records AS m
         LEFT JOIN memory_fts AS f ON f.rowid = m.id
         WHERE f.rowid IS NULL
         ORDER BY m.id",
    )?;
    let rows = missing.query_map([], |row| row.get::<_, i64>(0))?;
    for row in rows {
        issues.push(MemoryIndexIssue {
            row_id: row?,
            kind: MemoryIndexIssueKind::SourceRowMissingFromIndex,
        });
    }

    let mut orphan = connection.prepare(
        "SELECT f.rowid
         FROM memory_fts AS f
         LEFT JOIN memory_records AS m ON m.id = f.rowid
         WHERE m.id IS NULL
         ORDER BY f.rowid",
    )?;
    let rows = orphan.query_map([], |row| row.get::<_, i64>(0))?;
    for row in rows {
        issues.push(MemoryIndexIssue {
            row_id: row?,
            kind: MemoryIndexIssueKind::IndexRowMissingFromSource,
        });
    }

    issues.sort_by_key(|issue| issue.row_id);
    Ok(issues)
}

fn validate_memory_record(record: &MemoryRecord) -> StorageResult<()> {
    validate_record_id(&record.record_id)?;
    if record.source_reference.trim().is_empty() {
        return Err(StorageError::InvalidArgument {
            field: "source_reference",
            detail: "不得为空".to_owned(),
        });
    }
    if record.source_reference.chars().count() > MAX_MEMORY_SOURCE_REFERENCE_CHARS {
        return Err(StorageError::InvalidArgument {
            field: "source_reference",
            detail: format!(
                "长度 {} 超过上限 {MAX_MEMORY_SOURCE_REFERENCE_CHARS}",
                record.source_reference.chars().count()
            ),
        });
    }
    if record.content.trim().is_empty() {
        return Err(StorageError::InvalidArgument {
            field: "content",
            detail: "不得为空".to_owned(),
        });
    }
    if record.content.len() > MAX_MEMORY_CONTENT_BYTES {
        return Err(StorageError::InvalidArgument {
            field: "content",
            detail: format!(
                "字节长度 {} 超过上限 {MAX_MEMORY_CONTENT_BYTES}",
                record.content.len()
            ),
        });
    }
    Ok(())
}

fn validate_record_id(record_id: &str) -> StorageResult<()> {
    if record_id.trim().is_empty() {
        return Err(StorageError::InvalidArgument {
            field: "record_id",
            detail: "不得为空".to_owned(),
        });
    }
    if record_id.chars().count() > MAX_MEMORY_RECORD_ID_CHARS {
        return Err(StorageError::InvalidArgument {
            field: "record_id",
            detail: format!(
                "长度 {} 超过上限 {MAX_MEMORY_RECORD_ID_CHARS}",
                record_id.chars().count()
            ),
        });
    }
    Ok(())
}

struct ValidatedQuery {
    match_expression: String,
    terms: Vec<String>,
}

fn validate_query(query: &MemoryQuery) -> StorageResult<ValidatedQuery> {
    if query.text.trim().is_empty() {
        return Err(StorageError::InvalidMemoryQuery {
            field: "text",
            detail: "不得为空或全为空白".to_owned(),
        });
    }
    if query.text.chars().count() > MAX_MEMORY_QUERY_CHARS {
        return Err(StorageError::InvalidMemoryQuery {
            field: "text",
            detail: format!(
                "长度 {} 超过上限 {MAX_MEMORY_QUERY_CHARS}",
                query.text.chars().count()
            ),
        });
    }
    if query.limit == 0 || query.limit > MAX_MEMORY_SEARCH_LIMIT {
        return Err(StorageError::InvalidMemoryQuery {
            field: "limit",
            detail: format!("必须在 1..={MAX_MEMORY_SEARCH_LIMIT}，收到 {}", query.limit),
        });
    }

    let mut terms = Vec::new();
    let mut match_terms = Vec::new();
    for term in query.text.split_whitespace() {
        if term.chars().any(char::is_control) {
            return Err(StorageError::InvalidMemoryQuery {
                field: "text",
                detail: "不得包含控制字符".to_owned(),
            });
        }
        if !term.chars().any(char::is_alphanumeric) {
            return Err(StorageError::InvalidMemoryQuery {
                field: "text",
                detail: format!("词项 {term:?} 不含字母或数字"),
            });
        }
        let escaped = term.replace('"', "\"\"");
        terms.push(term.to_owned());
        match_terms.push(format!("\"{escaped}\""));
        if terms.len() > MAX_MEMORY_QUERY_TERMS {
            return Err(StorageError::InvalidMemoryQuery {
                field: "text",
                detail: format!("词项数超过上限 {MAX_MEMORY_QUERY_TERMS}"),
            });
        }
    }
    if terms.is_empty() {
        return Err(StorageError::InvalidMemoryQuery {
            field: "text",
            detail: "至少需要一个字面量词项".to_owned(),
        });
    }
    Ok(ValidatedQuery {
        match_expression: match_terms.join(" AND "),
        terms,
    })
}
