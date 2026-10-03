//! `conversation_messages`：会话消息树的行类型、校验与最小读写。
//!
//! 职责：
//!   * 把 `SessionSnapshot` 的消息节点投影为可持久化的一行；
//!   * 保证 `(conversation_id, sequence)` 唯一、父子同会话且父 sequence 严格更小；
//!   * 提供单条插入与按 sequence 读取，供 Core 装配层的 `SessionStore` adapter 使用。
//!
//! 边界（不做什么）：
//!   * 不定义上下文裁剪、压缩或 tokenizer 策略；本层只保存已给出的 estimate。
//!   * 不把 `content` 当可信文本；上层仍按铁律 2 做角色 / 来源校验。
//!   * 不依赖 `assistant-core`，避免 storage → core 的依赖环。
//!
//! 不变量：
//!   1. `parent_id IS NULL` 表示根；非空父节点必须属于同一会话且 sequence 更小。
//!   2. 单棵会话树消息数有硬上限，防止一次写入把内存与数据库拖垮（ADR-0063）。
//!   3. 读取时重新校验持久化字符串；外部篡改出的未知 role / retention 必须报错。
//!
//! 相关：`docs/storage-design.md` §4 / §7、`crates/core/src/message.rs`、TASK-230。

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, params};

use super::conversations::validate_identifier;
use crate::error::{StorageError, StorageResult};

/// 单条消息正文的字节上限（与 Core 的 `MAX_TEXT_BYTES` 对齐）。
pub const MAX_MESSAGE_CONTENT_BYTES: usize = 1_048_576;

/// 单棵会话消息树的节点硬上限。
///
/// `100_000` 足以覆盖正常任务历史，同时把一次整树替换的内存 / SQLite 写入限定在可审计范围内。
pub const MAX_MESSAGES_PER_CONVERSATION: usize = 100_000;

const MESSAGE_ROLE_SYSTEM: &str = "system";
const MESSAGE_ROLE_USER: &str = "user";
const MESSAGE_ROLE_ASSISTANT: &str = "assistant";
const MESSAGE_ROLE_TOOL: &str = "tool";

const RETENTION_REQUIRED: &str = "required";
const RETENTION_SUMMARIZABLE: &str = "summarizable";
const RETENTION_DROPPABLE: &str = "droppable";

/// `conversation_messages` 表的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationMessageRecord {
    /// 消息 id；同一会话内唯一。
    pub id: String,
    /// 所属会话 id。
    pub conversation_id: String,
    /// 父消息 id；`None` 表示根。
    pub parent_id: Option<String>,
    /// 会话内顺序键；父节点必须严格更小。
    pub sequence: i64,
    /// 持久化角色：`system` / `user` / `assistant` / `tool`。
    pub role: String,
    /// 消息正文。
    pub content: String,
    /// 调用方给出的 token 估算，必须为正。
    pub token_estimate: i64,
    /// 持久化裁剪策略：`required` / `summarizable` / `droppable`。
    pub retention: String,
}

/// 写入一条消息。
///
/// 本函数会先校验消息形状、会话存在性与父节点顺序，再执行普通 `INSERT`。重复主键 / sequence
/// 或缺失会话都会显式失败，不会覆盖已有行。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：字段为空 / 越界，role / retention 未知，或父节点缺失 / 顺序非法
/// - [`StorageError::Sqlite`]：主键或唯一约束冲突，或底层写入失败
pub fn insert_conversation_message(
    connection: &Connection,
    message: &ConversationMessageRecord,
) -> StorageResult<()> {
    validate_message_shape(message)?;

    let conversation_exists: i64 = connection.query_row(
        "SELECT COUNT(*) FROM conversations WHERE id = ?1",
        params![message.conversation_id],
        |row| row.get(0),
    )?;
    if conversation_exists == 0 {
        return Err(StorageError::InvalidArgument {
            field: "conversation_id",
            detail: format!("会话 {} 不存在", message.conversation_id),
        });
    }

    if let Some(parent_id) = message.parent_id.as_deref() {
        let parent_sequence: Option<i64> = connection
            .query_row(
                "SELECT sequence
                 FROM conversation_messages
                 WHERE conversation_id = ?1 AND id = ?2",
                params![message.conversation_id, parent_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(parent_sequence) = parent_sequence else {
            return Err(StorageError::InvalidArgument {
                field: "parent_id",
                detail: format!(
                    "父消息 {parent_id} 不存在于会话 {}",
                    message.conversation_id
                ),
            });
        };
        if parent_sequence >= message.sequence {
            return Err(StorageError::InvalidArgument {
                field: "sequence",
                detail: format!(
                    "消息 {} 的 sequence {} 必须大于父消息的 {}",
                    message.id, message.sequence, parent_sequence
                ),
            });
        }
    }

    connection.execute(
        "INSERT INTO conversation_messages
             (id, conversation_id, parent_id, sequence, role, content, token_estimate, retention)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            message.id,
            message.conversation_id,
            message.parent_id,
            message.sequence,
            message.role,
            message.content,
            message.token_estimate,
            message.retention
        ],
    )?;
    Ok(())
}

/// 按 `sequence` 升序读取一个会话的全部消息。
///
/// 不存在的会话会得到空列表；调用方需要用它区分“会话不存在”时，应先调用
/// [`crate::load_conversation`]。读取时会把未知 role / retention 当成显式错误。
///
/// # Errors
/// - [`StorageError::InvalidArgument`]：库里存在本版本不认识的持久化字段
/// - [`StorageError::Sqlite`]：底层读取失败
pub fn load_conversation_messages(
    connection: &Connection,
    conversation_id: &str,
) -> StorageResult<Vec<ConversationMessageRecord>> {
    validate_identifier("conversation_id", conversation_id)?;
    let mut statement = connection.prepare(
        "SELECT id, conversation_id, parent_id, sequence, role, content, token_estimate, retention
         FROM conversation_messages
         WHERE conversation_id = ?1
         ORDER BY sequence",
    )?;
    let rows = statement.query_map(params![conversation_id], |row| {
        Ok(ConversationMessageRecord {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            parent_id: row.get(2)?,
            sequence: row.get(3)?,
            role: row.get(4)?,
            content: row.get(5)?,
            token_estimate: row.get(6)?,
            retention: row.get(7)?,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        let message = row?;
        validate_message_shape(&message)?;
        messages.push(message);
    }
    Ok(messages)
}

/// 校验一棵完整消息树，确保它可以在一个事务里替换进一个会话。
///
/// 该函数是纯逻辑校验，不访问数据库；调用后仍由外键与迁移触发器做最后一层保护。
pub fn validate_message_tree_for_conversation(
    conversation_id: &str,
    messages: &[ConversationMessageRecord],
) -> StorageResult<()> {
    if messages.len() > MAX_MESSAGES_PER_CONVERSATION {
        return Err(StorageError::InvalidArgument {
            field: "messages",
            detail: format!(
                "消息数 {} 超过上限 {MAX_MESSAGES_PER_CONVERSATION}",
                messages.len()
            ),
        });
    }

    let mut ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    let mut parent_sequences = BTreeMap::new();
    for message in messages {
        validate_message_shape(message)?;
        if message.conversation_id != conversation_id {
            return Err(StorageError::InvalidArgument {
                field: "conversation_id",
                detail: format!(
                    "消息 {} 属于会话 {}，不能写入 {conversation_id}",
                    message.id, message.conversation_id
                ),
            });
        }
        if !ids.insert(message.id.as_str()) {
            return Err(StorageError::InvalidArgument {
                field: "message.id",
                detail: format!("消息 id {} 重复", message.id),
            });
        }
        if !sequences.insert(message.sequence) {
            return Err(StorageError::InvalidArgument {
                field: "message.sequence",
                detail: format!("消息 sequence {} 重复", message.sequence),
            });
        }
        parent_sequences.insert(message.id.as_str(), message.sequence);
    }

    for message in messages {
        let Some(parent_id) = message.parent_id.as_deref() else {
            continue;
        };
        let Some(parent_sequence) = parent_sequences.get(parent_id).copied() else {
            return Err(StorageError::InvalidArgument {
                field: "parent_id",
                detail: format!("消息 {} 的父消息 {parent_id} 不在同一棵树", message.id),
            });
        };
        if parent_sequence >= message.sequence {
            return Err(StorageError::InvalidArgument {
                field: "sequence",
                detail: format!(
                    "消息 {} 的 sequence {} 必须大于父消息 {parent_id} 的 {parent_sequence}",
                    message.id, message.sequence
                ),
            });
        }
    }
    Ok(())
}

fn validate_message_shape(message: &ConversationMessageRecord) -> StorageResult<()> {
    validate_identifier("message.id", &message.id)?;
    validate_identifier("conversation_id", &message.conversation_id)?;
    if let Some(parent_id) = message.parent_id.as_deref() {
        validate_identifier("parent_id", parent_id)?;
    }
    if message.sequence < 0 {
        return Err(StorageError::InvalidArgument {
            field: "sequence",
            detail: "不得为负数".to_owned(),
        });
    }
    if !matches!(
        message.role.as_str(),
        MESSAGE_ROLE_SYSTEM | MESSAGE_ROLE_USER | MESSAGE_ROLE_ASSISTANT | MESSAGE_ROLE_TOOL
    ) {
        return Err(StorageError::InvalidArgument {
            field: "role",
            detail: format!("未知消息角色 {:?}", message.role),
        });
    }
    if message.content.trim().is_empty() {
        return Err(StorageError::InvalidArgument {
            field: "content",
            detail: "不得为空".to_owned(),
        });
    }
    if message.content.len() > MAX_MESSAGE_CONTENT_BYTES {
        return Err(StorageError::InvalidArgument {
            field: "content",
            detail: format!(
                "字节长度 {} 超过上限 {MAX_MESSAGE_CONTENT_BYTES}",
                message.content.len()
            ),
        });
    }
    if message
        .content
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(StorageError::InvalidArgument {
            field: "content",
            detail: "包含不支持的控制字符".to_owned(),
        });
    }
    if message.token_estimate <= 0 {
        return Err(StorageError::InvalidArgument {
            field: "token_estimate",
            detail: "必须为正数".to_owned(),
        });
    }
    if !matches!(
        message.retention.as_str(),
        RETENTION_REQUIRED | RETENTION_SUMMARIZABLE | RETENTION_DROPPABLE
    ) {
        return Err(StorageError::InvalidArgument {
            field: "retention",
            detail: format!("未知裁剪策略 {:?}", message.retention),
        });
    }
    Ok(())
}
