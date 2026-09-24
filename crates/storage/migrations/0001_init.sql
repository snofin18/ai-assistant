-- TASK-012 迁移 0001：核心表（W1/W4/W5/W6 最小集）。
--
-- 表名与列名以 `cross-platform-ai-assistant-architecture-v2.md` §15.1 为单一事实源（复数 snake_case）。
-- 本卡正文 In scope 用单数简称（`task` / `step` / `usage`），此处按 §15.1 的权威名落地，
-- 差异与依据登记在 tasks/TASK-012-*.md §5（DRIFT-012-1）。
--
-- 只前进不回滚：本文件**不得事后修改** —— 内容被 sha256 记账在 `schema_migrations.checksum`，
-- 改动会让下一次启动直接拒绝（避免"同一个版本号对应两份不同 schema"）。
--
-- 时间列一律为 Unix 毫秒（由注入的 Clock 提供，AGENTS.md §5.3：时钟一律 trait 注入）。
-- 跨卡的引用（conversations / undo_anchors / model_configs）**不加 FOREIGN KEY**：
-- 那些表分别归 TASK-028 / 后续卡，SQLite 的 FK 指向不存在的表会在写入时报错。
-- 本卡内部的引用（task_steps → tasks 等）加 FK + ON DELETE CASCADE。

CREATE TABLE tasks (
    id                  TEXT    PRIMARY KEY,
    conversation_id     TEXT,                          -- → conversations(id)：该表归 TASK-028
    goal                TEXT    NOT NULL,
    plan_json           TEXT,
    status              TEXT    NOT NULL,
    reversibility_worst TEXT,
    budget_json         TEXT,
    started_at          INTEGER NOT NULL,
    ended_at            INTEGER,
    cost_usd            REAL,
    tokens_in           INTEGER NOT NULL DEFAULT 0,
    tokens_out          INTEGER NOT NULL DEFAULT 0,
    error_code          TEXT
);

CREATE INDEX idx_tasks_status ON tasks(status, started_at);

CREATE TABLE task_steps (
    id                  TEXT    PRIMARY KEY,
    task_id             TEXT    NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    seq                 INTEGER NOT NULL,
    tool                TEXT    NOT NULL,
    args_json           TEXT,
    status              TEXT    NOT NULL,
    attempts            INTEGER NOT NULL DEFAULT 0,
    pre_fingerprint     TEXT,
    post_fingerprint    TEXT,
    verify_result_json  TEXT,
    anchor_id           TEXT,                          -- → undo_anchors(id)：该表归后续卡
    started_at          INTEGER NOT NULL,
    ended_at            INTEGER,
    duration_ms         INTEGER,
    error_code          TEXT,
    UNIQUE (task_id, seq)
);

CREATE INDEX idx_steps_task ON task_steps(task_id, seq);

-- W1 的"每步一次检查点"（storage-design.md §3.1）：恢复时读最近一条即可，
-- 不必重放全部 step。热点状态仍以内存为权威副本，这里只存可恢复的最小状态。
CREATE TABLE checkpoints (
    id              TEXT    PRIMARY KEY,
    task_id         TEXT    NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    last_step_seq   INTEGER NOT NULL,
    state_json      TEXT    NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE INDEX idx_checkpoints_task ON checkpoints(task_id, created_at DESC);

-- L2 内容寻址 blob 池的**元数据**（文件本体在 <root>/blobs/<前2位>/<sha256>）。
-- bytes = 解压后长度，读取时用作解压上限（防解压炸弹）并与实测长度比对。
CREATE TABLE blobs (
    blob_id          TEXT    PRIMARY KEY CHECK (length(blob_id) = 64),
    kind             TEXT    NOT NULL,
    bytes            INTEGER NOT NULL CHECK (bytes >= 0),
    compressed_bytes INTEGER NOT NULL CHECK (compressed_bytes >= 0),
    created_at       INTEGER NOT NULL
);

CREATE INDEX idx_blobs_created ON blobs(created_at);

-- 引用计数用"一行一个引用"表达（而不是一个计数字段）：
-- 同一个 owner 重复引用同一 blob 天然幂等（PRIMARY KEY 去重），
-- 也不会出现"多减一次导致计数错乱"的经典 bug。引用数 = COUNT(*)。
CREATE TABLE blob_refs (
    blob_id     TEXT    NOT NULL REFERENCES blobs(blob_id) ON DELETE CASCADE,
    owner_kind  TEXT    NOT NULL,
    owner_id    TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (blob_id, owner_kind, owner_id)
);

CREATE INDEX idx_blob_refs_owner ON blob_refs(owner_kind, owner_id);

CREATE TABLE usage_records (
    id              TEXT    PRIMARY KEY,
    ts              INTEGER NOT NULL,
    task_id         TEXT,
    step_id         TEXT,
    model_config_id TEXT,
    tokens_in       INTEGER NOT NULL DEFAULT 0,
    tokens_out      INTEGER NOT NULL DEFAULT 0,
    cached_tokens   INTEGER NOT NULL DEFAULT 0,
    cost_usd        REAL,
    latency_ms      INTEGER,
    cache_hit       INTEGER NOT NULL DEFAULT 0 CHECK (cache_hit IN (0, 1))
);

CREATE INDEX idx_usage_ts ON usage_records(ts);
