//! `conversations`：会话行类型、生命周期投影与整棵消息树的持久化入口。
//!
//! 职责：
//!   * 保存 Core `SessionSnapshot` 的会话级字段：目标、状态、时间、revision；
//!   * 提供会话插入 / 读取，以及“插入快照”和“替换为下一 revision 快照”的原子操作；
//!   * 让 binary 装配层可以据此实现 `SessionStore`，而 storage 本身不依赖 Core。
//!
//! 边界（不做什么）：
//!   * 不定义上下文预算、压缩、分支选择或 UI 列表语义。
//!   * 不改变 `SessionStore` trait；trait 适配归唯一装配点。
//!   * 不提供多写者连接；连接生命周期仍由 `Database` / 调用方管理。
//!
//! 不变量：
//!   1. `insert_conversation_snapshot` 只创建 revision 0 的新会话；更新必须走
//!      `replace_conversation_snapshot`，且 revision 必须等于旧值 + 1。
//!   2. 整棵消息树在事务内替换；任一消息校验或写入失败都会回滚，不留下半个树。
//!   3. 读取时校验持久化状态与时间关系；库里的非法值不得伪装成合法会话。
//!
//! 相关：`docs/storage-design.md` §4 / §7、`crates/core/src/store.rs`、TASK-230。

use rusqlite::{Connection, OptionalExtension, params};

use super::conversation_messages::{
    ConversationMessageRecord, insert_conversation_message, load_conversation_messages,
    validate_message_tree_for_conversation,
};
use crate::error::{StorageError, StorageResult};

/// 会话 / 消息标识符允许的最大字符数（与 Core 的 identifier 上限对齐）。
const MAX_RECORD_IDENTIFIER_CHARS: usize = 128;

/// 会话目标的字节上限（与 Core 的 `MAX_GOAL_BYTES` 对齐）。
pub const MAX_CONVERSATION_GOAL_BYTES: usize = 4_096;

/// 活跃会话的持久化状态字符串。
pub const CONVERSATION_STATUS_ACTIVE: &str = "active";

/// 已结束会话的持久化状态字符串。
pub const CONVERSATION_STATUS_ENDED: &str = "ended";

/// `conversations` 表的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationRecord {
    /// 会话 id。
    pub id: String,
    /// 原始用户目标。
    pub goal: String,
    /// 可选标题；当前 Core 快照不生成标题，保留给后续 UI / 导入路径。
    pub title: Option<String>,
    /// 可选模型配置 id；当前不建外键，模型配置表由后续拥有者。
    pub model_config_id: Option<String>,
    /// 持久化状态：`active` / `ended`。
    pub status: String,
    /// 创建时间（Unix 毫秒）。
    pub created_at_unix_ms: i64,
    /// 结束时间（Unix 毫秒）；活跃会话必须为 `None`。
    pub ended_at_unix_ms: Option<i64>,
    /// 单调 revision；新建快照为 0。
    pub revision: i64,
    /// 是否已归档。
    pub archived: bool,
}

/// 插入一个新会话行；同 id 已存在时显式失败，不静默覆盖。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：字段形状非法或时间关系矛盾
/// - [`StorageError::Sqlite`]：主键冲突或底层写入失败
pub fn insert_conversation(
    connection: &Connection,
    conversation: &ConversationRecord,
) -> StorageResult<()> {
    validate_conversation(conversation)?;
    connection.execute(
        "INSERT INTO conversations
             (id, goal, title, model_config_id, status, created_at_unix_ms,
              ended_at_unix_ms, revision, archived)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            conversation.id,
            conversation.goal,
            conversation.title,
            conversation.model_config_id,
            conversation.status,
            conversation.created_at_unix_ms,
            conversation.ended_at_unix_ms,
            conversation.revision,
            i64::from(conversation.archived)
        ],
    )?;
    Ok(())
}

/// 读取一个会话行；不存在返回 `None`，不是错误。
///
/// 读取后仍会重新校验状态与时间关系；外部篡改出的非法行会返回显式错误。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：会话 id 非法，或库里的行不符合公开不变量
/// - [`StorageError::Sqlite`]：底层读取失败
pub fn load_conversation(
    connection: &Connection,
    conversation_id: &str,
) -> StorageResult<Option<ConversationRecord>> {
    validate_identifier("conversation_id", conversation_id)?;
    let mut statement = connection.prepare(
        "SELECT id, goal, title, model_config_id, status, created_at_unix_ms,
                ended_at_unix_ms, revision, archived
         FROM conversations
         WHERE id = ?1",
    )?;
    let mut rows = statement.query(params![conversation_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let archived: i64 = row.get(8)?;
    let conversation = ConversationRecord {
        id: row.get(0)?,
        goal: row.get(1)?,
        title: row.get(2)?,
        model_config_id: row.get(3)?,
        status: row.get(4)?,
        created_at_unix_ms: row.get(5)?,
        ended_at_unix_ms: row.get(6)?,
        revision: row.get(7)?,
        archived: archived != 0,
    };
    validate_conversation(&conversation)?;
    Ok(Some(conversation))
}

/// 原子插入一个 revision 0 的新会话与整棵消息树。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：会话不是 revision 0，或消息树形状非法
/// - [`StorageError::Sqlite`]：重复 id / sequence、外键失败或底层写入失败
pub fn insert_conversation_snapshot(
    connection: &Connection,
    conversation: &ConversationRecord,
    messages: &[ConversationMessageRecord],
) -> StorageResult<()> {
    if conversation.revision != 0 {
        return Err(StorageError::InvalidArgument {
            field: "revision",
            detail: format!("新建会话必须是 revision 0，收到 {}", conversation.revision),
        });
    }
    validate_conversation_snapshot(conversation, messages)?;

    let transaction = connection.unchecked_transaction()?;
    insert_conversation(&transaction, conversation)?;
    insert_messages_in_sequence_order(&transaction, messages)?;
    transaction.commit()?;
    Ok(())
}

/// 读取一个会话及整棵消息树；不存在返回 `None`。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：会话 id 或库中行非法
/// - [`StorageError::Sqlite`]：底层读取失败
pub fn load_conversation_snapshot(
    connection: &Connection,
    conversation_id: &str,
) -> StorageResult<Option<(ConversationRecord, Vec<ConversationMessageRecord>)>> {
    let transaction = connection.unchecked_transaction()?;
    let Some(conversation) = load_conversation(&transaction, conversation_id)? else {
        transaction.commit()?;
        return Ok(None);
    };
    let messages = load_conversation_messages(&transaction, conversation_id)?;
    transaction.commit()?;
    Ok(Some((conversation, messages)))
}

/// 原子替换一个已有会话及整棵消息树，并要求 revision 恰好递增 1。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：会话不存在、revision 不递增 1，或消息树形状非法
/// - [`StorageError::Sqlite`]：重复 id / sequence、外键失败或底层写入失败
pub fn replace_conversation_snapshot(
    connection: &Connection,
    conversation: &ConversationRecord,
    messages: &[ConversationMessageRecord],
) -> StorageResult<()> {
    validate_conversation_snapshot(conversation, messages)?;
    let transaction = connection.unchecked_transaction()?;

    let existing_revision: Option<i64> = transaction
        .query_row(
            "SELECT revision FROM conversations WHERE id = ?1",
            params![conversation.id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(existing_revision) = existing_revision else {
        return Err(StorageError::InvalidArgument {
            field: "conversation_id",
            detail: format!("会话 {} 不存在，不能替换", conversation.id),
        });
    };
    let expected_revision =
        existing_revision
            .checked_add(1)
            .ok_or_else(|| StorageError::InvalidArgument {
                field: "revision",
                detail: "旧 revision 已到 i64 上限".to_owned(),
            })?;
    if conversation.revision != expected_revision {
        return Err(StorageError::InvalidArgument {
            field: "revision",
            detail: format!(
                "会话 {} 期望 revision {expected_revision}，收到 {}",
                conversation.id, conversation.revision
            ),
        });
    }

    transaction.execute(
        "DELETE FROM conversation_messages WHERE conversation_id = ?1",
        params![conversation.id],
    )?;
    let changed = transaction.execute(
        "UPDATE conversations
         SET goal = ?2,
             title = ?3,
             model_config_id = ?4,
             status = ?5,
             created_at_unix_ms = ?6,
             ended_at_unix_ms = ?7,
             revision = ?8,
             archived = ?9
         WHERE id = ?1",
        params![
            conversation.id,
            conversation.goal,
            conversation.title,
            conversation.model_config_id,
            conversation.status,
            conversation.created_at_unix_ms,
            conversation.ended_at_unix_ms,
            conversation.revision,
            i64::from(conversation.archived)
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::InvalidArgument {
            field: "conversation_id",
            detail: format!("会话 {} 在替换过程中消失", conversation.id),
        });
    }
    insert_messages_in_sequence_order(&transaction, messages)?;
    transaction.commit()?;
    Ok(())
}

/// Validates the stable identifier syntax shared by conversations and messages.
///
/// The value must be 1..=128 ASCII letters, digits, `_`, `-`, or `.`. This
/// function is pure and idempotent; it never reads or writes SQLite.
///
/// # Errors
///
/// Returns [`StorageError::InvalidArgument`] when `value` is empty, oversized,
/// or contains an unsupported byte.
pub fn validate_identifier(field: &'static str, value: &str) -> StorageResult<()> {
    if value.is_empty()
        || value.chars().count() > MAX_RECORD_IDENTIFIER_CHARS
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(StorageError::InvalidArgument {
            field,
            detail: "必须为 1..=128 个 ASCII 字母、数字、下划线、连字符或点".to_owned(),
        });
    }
    Ok(())
}

fn validate_conversation_snapshot(
    conversation: &ConversationRecord,
    messages: &[ConversationMessageRecord],
) -> StorageResult<()> {
    validate_conversation(conversation)?;
    validate_message_tree_for_conversation(&conversation.id, messages)
}

fn validate_conversation(conversation: &ConversationRecord) -> StorageResult<()> {
    validate_identifier("conversation.id", &conversation.id)?;
    if conversation.goal.trim().is_empty() {
        return Err(StorageError::InvalidArgument {
            field: "goal",
            detail: "不得为空".to_owned(),
        });
    }
    if conversation.goal.len() > MAX_CONVERSATION_GOAL_BYTES {
        return Err(StorageError::InvalidArgument {
            field: "goal",
            detail: format!(
                "字节长度 {} 超过上限 {MAX_CONVERSATION_GOAL_BYTES}",
                conversation.goal.len()
            ),
        });
    }
    if conversation
        .goal
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(StorageError::InvalidArgument {
            field: "goal",
            detail: "包含不支持的控制字符".to_owned(),
        });
    }
    if !matches!(
        conversation.status.as_str(),
        CONVERSATION_STATUS_ACTIVE | CONVERSATION_STATUS_ENDED
    ) {
        return Err(StorageError::InvalidArgument {
            field: "status",
            detail: format!("未知会话状态 {:?}", conversation.status),
        });
    }
    if conversation.created_at_unix_ms < 0 {
        return Err(StorageError::InvalidArgument {
            field: "created_at_unix_ms",
            detail: "不得为负数".to_owned(),
        });
    }
    if conversation.revision < 0 {
        return Err(StorageError::InvalidArgument {
            field: "revision",
            detail: "不得为负数".to_owned(),
        });
    }
    match (conversation.status.as_str(), conversation.ended_at_unix_ms) {
        (CONVERSATION_STATUS_ACTIVE, Some(_)) => {
            return Err(StorageError::InvalidArgument {
                field: "ended_at_unix_ms",
                detail: "活跃会话不得设置结束时间".to_owned(),
            });
        }
        (CONVERSATION_STATUS_ENDED, None) => {
            return Err(StorageError::InvalidArgument {
                field: "ended_at_unix_ms",
                detail: "已结束会话必须设置结束时间".to_owned(),
            });
        }
        _ => {}
    }
    if let Some(ended_at) = conversation.ended_at_unix_ms
        && ended_at < conversation.created_at_unix_ms
    {
        return Err(StorageError::InvalidArgument {
            field: "ended_at_unix_ms",
            detail: "结束时间早于创建时间".to_owned(),
        });
    }
    Ok(())
}

fn insert_messages_in_sequence_order(
    connection: &Connection,
    messages: &[ConversationMessageRecord],
) -> StorageResult<()> {
    // 父节点必须在子节点之前插入；先按 sequence 排序，稳定覆盖调用方给出任意顺序的情况。
    let mut ordered: Vec<&ConversationMessageRecord> = messages.iter().collect();
    ordered.sort_by_key(|message| message.sequence);
    for message in ordered {
        insert_conversation_message(connection, message)?;
    }
    Ok(())
}
