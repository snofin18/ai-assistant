//! 核心表的行类型与最小读写（W1 任务/步骤/检查点、W6 用量）。
//!
//! 边界：本模块**只**做"结构体 ⇄ 一行"的映射。状态机、重试、预算判定、撤销锚点管理
//! 都**不**在这里（它们属于 task-engine / audit 等后续卡）。
//!
//! 不变量：
//!   1. 结构体字段与 `migrations/0001_init.sql` 的列**一一对应**（新增列 = 新增迁移）
//!   2. 写入用普通 `INSERT`（**不是** `INSERT OR REPLACE`）：主键冲突 = 报错，
//!      不静默覆盖已有行（铁律 1：禁止静默覆盖）
//!   3. 时间列一律 Unix 毫秒，由调用方传入（时钟注入在 `Clock`，本模块不读系统时间）
//!
//! 相关：架构 v2 §15.1、`docs/storage-design.md` §4

use rusqlite::{Connection, params};

use crate::error::StorageResult;

/// `tasks` 表的一行（任务级状态与检查点元数据）。
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRecord {
    /// 任务 id（`t_` 前缀由上游生成，本层不校验格式）。
    pub id: String,
    /// 所属会话；`conversations` 表归 TASK-028，故此处可空。
    pub conversation_id: Option<String>,
    /// 用户目标（自然语言）。
    pub goal: String,
    /// 计划 JSON；大于 64 KB 时应外置到 blob（`storage-design.md` §4）。
    pub plan_json: Option<String>,
    /// 任务状态（枚举取值归 task-engine，本层存字符串）。
    pub status: String,
    /// 最差可逆性等级（用于风险升级判定）。
    pub reversibility_worst: Option<String>,
    /// 预算 JSON。
    pub budget_json: Option<String>,
    /// 开始时间（Unix 毫秒）。
    pub started_at: i64,
    /// 结束时间（Unix 毫秒）。
    pub ended_at: Option<i64>,
    /// 累计成本（USD）。
    pub cost_usd: Option<f64>,
    /// 累计输入 token。
    pub tokens_in: i64,
    /// 累计输出 token。
    pub tokens_out: i64,
    /// 失败时的错误码（`ErrorCategory` 的字符串形式）。
    pub error_code: Option<String>,
}

/// `task_steps` 表的一行（步骤级状态与指纹）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStepRecord {
    /// 步骤 id。
    pub id: String,
    /// 所属任务 id。
    pub task_id: String,
    /// 任务内序号（从 0 或 1 开始由上游决定，本层只要求任务内唯一）。
    pub seq: i64,
    /// 工具名（`<app>.<domain>.<action>`）。
    pub tool: String,
    /// 工具参数 JSON。
    pub args_json: Option<String>,
    /// 步骤状态。
    pub status: String,
    /// 已尝试次数。
    pub attempts: i64,
    /// 执行前状态指纹。
    pub pre_fingerprint: Option<String>,
    /// 执行后状态指纹。
    pub post_fingerprint: Option<String>,
    /// 后置条件校验结果 JSON。
    pub verify_result_json: Option<String>,
    /// 撤销锚点 id；`undo_anchors` 表归后续卡，故不加外键。
    pub anchor_id: Option<String>,
    /// 开始时间（Unix 毫秒）。
    pub started_at: i64,
    /// 结束时间（Unix 毫秒）。
    pub ended_at: Option<i64>,
    /// 耗时（毫秒）。
    pub duration_ms: Option<i64>,
    /// 失败时的错误码。
    pub error_code: Option<String>,
}

/// `checkpoints` 表的一行（W1 的"每步一次检查点"）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointRecord {
    /// 检查点 id。
    pub id: String,
    /// 所属任务 id。
    pub task_id: String,
    /// 本检查点覆盖到的最后一步序号。
    pub last_step_seq: i64,
    /// 可恢复的最小状态（JSON）。
    pub state_json: String,
    /// 创建时间（Unix 毫秒）。
    pub created_at: i64,
}

/// `usage_records` 表的一行（W6 用量与成本）。
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRecord {
    /// 记录 id。
    pub id: String,
    /// 发生时间（Unix 毫秒）。
    pub ts: i64,
    /// 关联任务（可空：模型调用可能发生在任务之外）。
    pub task_id: Option<String>,
    /// 关联步骤（可空）。
    pub step_id: Option<String>,
    /// 模型配置 id（`model_configs` 表归后续卡）。
    pub model_config_id: Option<String>,
    /// 输入 token。
    pub tokens_in: i64,
    /// 输出 token。
    pub tokens_out: i64,
    /// 命中缓存的 token。
    pub cached_tokens: i64,
    /// 成本（USD）。
    pub cost_usd: Option<f64>,
    /// 端到端延迟（毫秒）。
    pub latency_ms: Option<i64>,
    /// 是否命中 provider 侧缓存（存 0/1）。
    pub cache_hit: bool,
}

/// 写入一条任务。
///
/// # Errors
/// - [`crate::StorageError::Sqlite`]：主键冲突（同 id 已存在）或约束失败 —— 不静默覆盖
pub fn insert_task(conn: &Connection, task: &TaskRecord) -> StorageResult<()> {
    conn.execute(
        "INSERT INTO tasks (id, conversation_id, goal, plan_json, status, reversibility_worst,
                            budget_json, started_at, ended_at, cost_usd, tokens_in, tokens_out, error_code)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            task.id,
            task.conversation_id,
            task.goal,
            task.plan_json,
            task.status,
            task.reversibility_worst,
            task.budget_json,
            task.started_at,
            task.ended_at,
            task.cost_usd,
            task.tokens_in,
            task.tokens_out,
            task.error_code
        ],
    )?;
    Ok(())
}

/// 读取一条任务（不存在返回 `None`，不是错误）。
///
/// # Errors
/// 仅 [`crate::StorageError::Sqlite`]。
pub fn load_task(conn: &Connection, task_id: &str) -> StorageResult<Option<TaskRecord>> {
    let mut statement = conn.prepare(
        "SELECT id, conversation_id, goal, plan_json, status, reversibility_worst, budget_json,
                started_at, ended_at, cost_usd, tokens_in, tokens_out, error_code
         FROM tasks WHERE id = ?1",
    )?;
    let mut rows = statement.query(params![task_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(TaskRecord {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            goal: row.get(2)?,
            plan_json: row.get(3)?,
            status: row.get(4)?,
            reversibility_worst: row.get(5)?,
            budget_json: row.get(6)?,
            started_at: row.get(7)?,
            ended_at: row.get(8)?,
            cost_usd: row.get(9)?,
            tokens_in: row.get(10)?,
            tokens_out: row.get(11)?,
            error_code: row.get(12)?,
        })),
        None => Ok(None),
    }
}

/// 写入一条步骤。
///
/// # Errors
/// - [`crate::StorageError::Sqlite`]：主键冲突，或 `(task_id, seq)` 重复（UNIQUE 约束）
pub fn insert_task_step(conn: &Connection, step: &TaskStepRecord) -> StorageResult<()> {
    conn.execute(
        "INSERT INTO task_steps (id, task_id, seq, tool, args_json, status, attempts,
                                 pre_fingerprint, post_fingerprint, verify_result_json, anchor_id,
                                 started_at, ended_at, duration_ms, error_code)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            step.id,
            step.task_id,
            step.seq,
            step.tool,
            step.args_json,
            step.status,
            step.attempts,
            step.pre_fingerprint,
            step.post_fingerprint,
            step.verify_result_json,
            step.anchor_id,
            step.started_at,
            step.ended_at,
            step.duration_ms,
            step.error_code
        ],
    )?;
    Ok(())
}

/// 按序号升序读取某任务的全部步骤。
///
/// # Errors
/// 仅 [`crate::StorageError::Sqlite`]。
pub fn load_task_steps(conn: &Connection, task_id: &str) -> StorageResult<Vec<TaskStepRecord>> {
    let mut statement = conn.prepare(
        "SELECT id, task_id, seq, tool, args_json, status, attempts, pre_fingerprint,
                post_fingerprint, verify_result_json, anchor_id, started_at, ended_at,
                duration_ms, error_code
         FROM task_steps WHERE task_id = ?1 ORDER BY seq",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        Ok(TaskStepRecord {
            id: row.get(0)?,
            task_id: row.get(1)?,
            seq: row.get(2)?,
            tool: row.get(3)?,
            args_json: row.get(4)?,
            status: row.get(5)?,
            attempts: row.get(6)?,
            pre_fingerprint: row.get(7)?,
            post_fingerprint: row.get(8)?,
            verify_result_json: row.get(9)?,
            anchor_id: row.get(10)?,
            started_at: row.get(11)?,
            ended_at: row.get(12)?,
            duration_ms: row.get(13)?,
            error_code: row.get(14)?,
        })
    })?;
    let mut steps = Vec::new();
    for row in rows {
        steps.push(row?);
    }
    Ok(steps)
}

/// 写入一条检查点。
///
/// # Errors
/// - [`crate::StorageError::Sqlite`]：主键冲突，或 `task_id` 指向不存在的任务（外键约束）
pub fn insert_checkpoint(conn: &Connection, checkpoint: &CheckpointRecord) -> StorageResult<()> {
    conn.execute(
        "INSERT INTO checkpoints (id, task_id, last_step_seq, state_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            checkpoint.id,
            checkpoint.task_id,
            checkpoint.last_step_seq,
            checkpoint.state_json,
            checkpoint.created_at
        ],
    )?;
    Ok(())
}

/// 读取某任务**最新**的检查点（恢复路径：只读这一条，不必重放全部步骤）。
///
/// # Errors
/// 仅 [`crate::StorageError::Sqlite`]。
pub fn load_latest_checkpoint(
    conn: &Connection,
    task_id: &str,
) -> StorageResult<Option<CheckpointRecord>> {
    let mut statement = conn.prepare(
        "SELECT id, task_id, last_step_seq, state_json, created_at
         FROM checkpoints WHERE task_id = ?1
         ORDER BY created_at DESC, last_step_seq DESC LIMIT 1",
    )?;
    let mut rows = statement.query(params![task_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(CheckpointRecord {
            id: row.get(0)?,
            task_id: row.get(1)?,
            last_step_seq: row.get(2)?,
            state_json: row.get(3)?,
            created_at: row.get(4)?,
        })),
        None => Ok(None),
    }
}

/// 写入一条用量记录。
///
/// # Errors
/// - [`crate::StorageError::Sqlite`]：主键冲突，或 `cache_hit` 不是 0/1（CHECK 约束）
pub fn insert_usage_record(conn: &Connection, usage: &UsageRecord) -> StorageResult<()> {
    conn.execute(
        "INSERT INTO usage_records (id, ts, task_id, step_id, model_config_id, tokens_in,
                                    tokens_out, cached_tokens, cost_usd, latency_ms, cache_hit)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            usage.id,
            usage.ts,
            usage.task_id,
            usage.step_id,
            usage.model_config_id,
            usage.tokens_in,
            usage.tokens_out,
            usage.cached_tokens,
            usage.cost_usd,
            usage.latency_ms,
            usage.cache_hit
        ],
    )?;
    Ok(())
}

/// 用量记录条数（最小聚合，证明 W6 表可查）。
///
/// # Errors
/// 仅 [`crate::StorageError::Sqlite`]。
pub fn count_usage_records(conn: &Connection) -> StorageResult<i64> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM usage_records", [], |row| row.get(0))?;
    Ok(count)
}
