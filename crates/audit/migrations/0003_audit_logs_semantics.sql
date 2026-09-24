-- TASK-203 迁移 0003：`audit_logs` 列语义去重 + 显式链序（ADR-0040；PL-045 + PL-043 闭环）。
--
-- 为什么重建整张表而不是 ALTER TABLE：SQLite 的 ALTER 只支持加列 / 改名 / 改列名，
-- **删列**（DROP COLUMN）在 3.35 才有且带一串限制（不能是 PRIMARY KEY / UNIQUE / 被索引引用），
-- 本表要删的 `hash` 恰好同时被 UNIQUE 语义与「主键的孪生列」牵连 —— 重建是唯一确定性的走法。
--
-- 形状变化（ADR-0040 D1）：
--   删 `hash`     —— 它与 `id` 在 v1 语义上**完全相同**（都是本条 self_hash），属语义重复（PL-045）
--   加 `sequence` —— 显式链序，替代对 `rowid` 的隐式依赖（PL-043：VACUUM 可能重排无 INTEGER PRIMARY KEY 表的 rowid）
--   其余 8 列逐字不变。
--
-- `AUTOINCREMENT` 而非裸 `INTEGER PRIMARY KEY`：前者保证「已用过的号**永不复用**」——
-- 归档删除（PL-043 的触发场景）之后，新行不会回填旧号把「链序」讲成假话。
--
-- `0002_audit_logs.sql` **一字不改**：它的 sha256 已记在已有库的 `schema_migrations.checksum`，
-- 改一个字节就会让下一次启动报 `migration_checksum_mismatch`（ADR-0038 不变量 2）。
--
-- 迁移内容本身**不得事后修改**（同一套 checksum 记账规则）。

CREATE TABLE audit_logs_new (
    sequence    INTEGER PRIMARY KEY AUTOINCREMENT,                    -- 显式链序（ADR-0040 D2）
    id          TEXT    NOT NULL UNIQUE,                              -- = 本条 self_hash（ADR-0040 D3）
    prev_hash   TEXT    NOT NULL,                                     -- 首事件 = 空串（GENESIS_PREV_HASH）
    ts          INTEGER NOT NULL,                                     -- Unix 毫秒
    actor       TEXT    NOT NULL,                                     -- AuditActor 的 serde 名：user/agent/system/tool
    task_id     TEXT,                                                 -- 可空：会话级 / 系统级事件不属于任何任务
    step_id     TEXT,                                                 -- 可空：同上
    event_type  TEXT    NOT NULL,                                     -- naming.md §7：noun.past_verb（如 tool.called）
    detail_json TEXT    NOT NULL                                      -- AuditEvent 的完整 JSON（含 prev_hash / self_hash）
);

-- 旧行按**当时的物理序**（rowid = 当时的链序，因为本表 append-only 且此刻仍是旧形状）拷贝，
-- 于是新 `sequence` 1..N 恰好复刻旧链序。旧 `hash` 列**故意不拷贝**：它与 `id` 同值。
INSERT INTO audit_logs_new (id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json)
    SELECT id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json
    FROM audit_logs
    ORDER BY rowid;

-- DROP 会连带带走旧表的 `idx_audit_ts` 与两个 append-only 触发器 —— 下面全部显式重建。
DROP TABLE audit_logs;

ALTER TABLE audit_logs_new RENAME TO audit_logs;

CREATE INDEX idx_audit_ts ON audit_logs(ts);

-- append-only 的两道护栏之一（数据库侧）。语义与 0002 完全一致，只是重建一遍。
CREATE TRIGGER audit_logs_no_update
BEFORE UPDATE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is append-only: UPDATE is forbidden');
END;

CREATE TRIGGER audit_logs_no_delete
BEFORE DELETE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is append-only: DELETE is forbidden');
END;
