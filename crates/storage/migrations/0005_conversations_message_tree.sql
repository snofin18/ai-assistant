-- TASK-230 迁移 0005：会话行与消息树（W1 会话恢复的最小持久化形状）。
--
-- 设计边界：
--   * conversations 保存会话生命周期与 revision；conversation_messages 保存一棵可校验的消息树。
--   * 父节点必须属于同一会话且 sequence 严格小于子节点，因此既有 parent 外键也有顺序触发器。
--   * 消息 content 仍是不透明文本；模型输出 / UI 输入的语义校验归 core 或上层，本层只保证形状与引用完整。
--
-- 只前进不回滚：本文件不得事后修改 —— 内容被 sha256 记账在 schema_migrations.checksum。

CREATE TABLE conversations (
    id                  TEXT    PRIMARY KEY,
    goal                TEXT    NOT NULL,
    title               TEXT,
    model_config_id     TEXT,
    status              TEXT    NOT NULL CHECK (status IN ('active', 'ended')),
    created_at_unix_ms  INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    ended_at_unix_ms    INTEGER,
    revision            INTEGER NOT NULL CHECK (revision >= 0),
    archived            INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    CHECK (
        (status = 'active' AND ended_at_unix_ms IS NULL)
        OR (status = 'ended' AND ended_at_unix_ms IS NOT NULL)
    ),
    CHECK (ended_at_unix_ms IS NULL OR ended_at_unix_ms >= created_at_unix_ms)
);

CREATE INDEX idx_conversations_status_created
    ON conversations(status, created_at_unix_ms DESC);

CREATE INDEX idx_conversations_archived_created
    ON conversations(archived, created_at_unix_ms DESC);

CREATE TABLE conversation_messages (
    id              TEXT    NOT NULL,
    conversation_id TEXT    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    parent_id       TEXT,
    sequence        INTEGER NOT NULL CHECK (sequence >= 0),
    role            TEXT    NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content         TEXT    NOT NULL,
    token_estimate  INTEGER NOT NULL CHECK (token_estimate > 0),
    retention       TEXT    NOT NULL CHECK (
        retention IN ('required', 'summarizable', 'droppable')
    ),
    PRIMARY KEY (conversation_id, id),
    UNIQUE (conversation_id, sequence),
    FOREIGN KEY (conversation_id, parent_id)
        REFERENCES conversation_messages(conversation_id, id)
        ON DELETE CASCADE
);

CREATE INDEX idx_conversation_messages_sequence
    ON conversation_messages(conversation_id, sequence);

-- SQLite 外键只能证明 parent_id 存在，不能证明父节点在同一会话且更早；触发器补上后，
-- 即使有人绕过公开 API 直接写 SQL，也不会留下会造成无环 / 顺序失败的树。
CREATE TRIGGER conversation_messages_parent_order_insert
BEFORE INSERT ON conversation_messages
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM conversation_messages AS parent
        WHERE parent.conversation_id = NEW.conversation_id
          AND parent.id = NEW.parent_id
          AND parent.sequence < NEW.sequence
    ) THEN RAISE(
        ABORT,
        'conversation message parent must exist in the same conversation with lower sequence'
    ) END;
END;

CREATE TRIGGER conversation_messages_parent_order_update
BEFORE UPDATE OF id, conversation_id, parent_id, sequence
ON conversation_messages
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM conversation_messages AS parent
        WHERE parent.conversation_id = NEW.conversation_id
          AND parent.id = NEW.parent_id
          AND parent.sequence < NEW.sequence
    ) THEN RAISE(
        ABORT,
        'conversation message parent must exist in the same conversation with lower sequence'
    ) END;
END;
