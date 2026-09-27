-- TASK-206 迁移 0004：memory_fts（FTS5 虚表）的源表、索引与同步触发器。
--
-- 设计边界：
--   * memory_records 是检索内容的唯一事实源；memory_fts 只是可重建索引。
--   * 三个触发器让源表与索引在同一事务内同步；verify_memory_index() 仍提供显式自检，
--     用于发现外部篡改或索引损坏。
--   * record_kind / record_id / source_reference 作为 UNINDEXED 元数据保存，检索结果据此
--     可追溯到记录类型、主键与原文位置。
--
-- 只前进不回滚：本文件不得事后修改 —— 内容被 sha256 记账在 schema_migrations.checksum。

CREATE TABLE memory_records (
    id               INTEGER PRIMARY KEY,
    record_kind      TEXT    NOT NULL CHECK (record_kind IN ('task_history', 'preference', 'note')),
    record_id        TEXT    NOT NULL,
    source_reference TEXT    NOT NULL,
    content          TEXT    NOT NULL,
    content_hash     TEXT    NOT NULL CHECK (length(content_hash) = 64),
    updated_at       INTEGER NOT NULL,
    UNIQUE (record_kind, record_id)
);

CREATE INDEX idx_memory_records_kind ON memory_records(record_kind, updated_at DESC);

CREATE VIRTUAL TABLE memory_fts USING fts5(
    record_kind UNINDEXED,
    record_id UNINDEXED,
    source_reference UNINDEXED,
    content,
    content_hash UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER memory_records_after_insert
AFTER INSERT ON memory_records
BEGIN
    INSERT INTO memory_fts (rowid, record_kind, record_id, source_reference, content, content_hash)
    VALUES (new.id, new.record_kind, new.record_id, new.source_reference, new.content, new.content_hash);
END;

CREATE TRIGGER memory_records_after_delete
AFTER DELETE ON memory_records
BEGIN
    DELETE FROM memory_fts WHERE rowid = old.id;
END;

CREATE TRIGGER memory_records_after_update
AFTER UPDATE ON memory_records
BEGIN
    DELETE FROM memory_fts WHERE rowid = old.id;
    INSERT INTO memory_fts (rowid, record_kind, record_id, source_reference, content, content_hash)
    VALUES (new.id, new.record_kind, new.record_id, new.source_reference, new.content, new.content_hash);
END;
