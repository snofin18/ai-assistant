-- TASK-013 迁移 0002：审计表 `audit_logs`（列名与顺序以架构 v2 §15.1 line 2594 为单一事实源）。
--
-- 为什么由 TASK-013 而不是 TASK-012 建：storage 的边界明写「不建 audit_logs」，
-- 审计语义（hash chain / 只追加）属于 audit crate；但**表结构必须走 storage 的迁移链** ——
-- 迁移框架把全部迁移编译期内嵌在 crates/storage/src/schema.rs 的 MIGRATIONS 常量里，
-- 没有第二处入口（这正是 TASK-013 原 write scope 的缺口，见该卡 §5 DRIFT-013-1）。
--
-- 只前进不回滚：本文件**不得事后修改** —— 内容被 sha256 记账在 schema_migrations.checksum，
-- 改动会让下一次启动直接拒绝（避免"同一个版本号对应两份不同 schema"）。
--
-- 时间列一律为 Unix 毫秒（由注入的 Clock 提供，AGENTS.md §5.3：时钟一律 trait 注入）。
-- 跨卡的引用（tasks / task_steps）**不加 FOREIGN KEY**：与 0001 同口径 —— 审计行必须在
-- 目标任务行被清理后**依然留存**（审计的价值就是"事后还能查"），加 CASCADE 会违背该目的。
--
-- append-only 用**两道**保证：
--   ① 代码侧：crates/audit 里没有任何 UPDATE / DELETE 语句（测试反证 + 评审核对）
--   ② 数据库侧：下面两个 BEFORE 触发器 RAISE(ABORT) —— 防「绕过 crate 直接改库」
-- INSERT 是唯一合法写路径。

CREATE TABLE audit_logs (
    id          TEXT    PRIMARY KEY,                                  -- = 本条 self_hash（见 TASK-013 §5 DRIFT-013-2）
    prev_hash   TEXT    NOT NULL,                                     -- 首事件 = 空串（GENESIS_PREV_HASH）
    ts          INTEGER NOT NULL,                                     -- Unix 毫秒
    actor       TEXT    NOT NULL,                                     -- AuditActor 的 serde 名：user/agent/system/tool
    task_id     TEXT,                                                 -- 可空：会话级 / 系统级事件不属于任何任务
    step_id     TEXT,                                                 -- 可空：同上
    event_type  TEXT    NOT NULL,                                     -- naming.md §7：noun.past_verb（如 tool.called）
    detail_json TEXT    NOT NULL,                                     -- AuditEvent 的完整 JSON（含 prev_hash / self_hash）
    hash        TEXT    NOT NULL CHECK (length(hash) = 64)            -- 本条 self_hash（sha256 小写 hex，64 字符）
);

CREATE INDEX idx_audit_ts ON audit_logs(ts);

-- 追加不可改的数据库侧护栏。为什么用触发器而不是权限：本地单文件 SQLite 没有
-- 「语句级权限」可用（不是服务端），触发器是本地唯一能表达"这张表只许 INSERT"的机制。
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
