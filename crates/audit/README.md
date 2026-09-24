# assistant-audit crate (TASK-013)

> 阶段 1A1 基础设施：**追加不可改的审计流**。SHA-256 hash chain + ring buffer 批量 flush + `durability` 可配。

## 职责

- **迁移 `0002_audit_logs` + `0003_audit_logs_semantics`**（DDL 在 `crates/audit/migrations/`，经
  `pub const MIGRATIONS` 暴露；**ADR-0038**）：建 `audit_logs`（列名 / 顺序按架构 v2 §15.1）+ `idx_audit_ts` +
  **数据库侧** append-only 护栏（`BEFORE UPDATE` / `BEFORE DELETE` 触发器 `RAISE(ABORT)`）。
  `0003`（**ADR-0040**）= 重建表：删与 `id` 同义的 `hash` 列（PL-045）+ 加显式链序 `sequence`（PL-043）；
  **本 crate 自己测自己的表**（ADR-0038 D2）
- **`AuditLog`**：`append`（串链）→ 缓冲 → 单事务批量 `flush`；`verify_chain` / `verify_chain_strict` 重算整条链
- **`Durability`**：`batched`（默认 100 条 / 200 ms）/ `immediate` / `separate_db_full`（见"已知限制"）
- 提供**注入点**：连接与 `Clock` 都从外面传入（测试用固定时钟 + 临时目录即可回放）

## 边界（不做什么）

- **不做审批 / 放行判定**：审计只**记录**发生过什么，判定在 policy（铁律 6）
- 不做保留期 / 容量轮转 / 合规导出；不做 `detail_json` 外置 blob（`storage-design.md` §4）
- 不引后台线程 / 定时器（flush 由 `append` 驱动，故不依赖任何 async 运行时）
- 不调任何平台 API；**不定义第二份事件结构体**（载荷类型来自 `crates/protocol`，铁律 10）
- 不持有、也不关闭连接（连接归 `crates/storage` 的 `Database`；`docs/storage-design.md` §7 单写者）

## 不变量

1. **只追加**：本 crate 内**没有**任何 `UPDATE` / `DELETE` 语句；库侧另有触发器兜底。
   两道锁是**互补**的：代码侧防"自己写错"，触发器防"别人绕过 crate 直接改库"
2. **链不可断**：`self_hash = sha256(HASH_DOMAIN ‖ 0x1F ‖ prev_hash ‖ 0x1F ‖ 规范化 JSON)`，
   规范化 JSON 里 `self_hash` 置空 —— 改任何一字节都会让 `verify_chain` 报错
3. **flush 要么全成要么全不成**：单事务；失败时缓冲**不清空**、链尾**不前进**（不丢事件，也不假装写过）
4. **时钟注入**：`ts` 与"200 ms 到点"都取自注入的 `Clock`（`AGENTS.md` §5.3：时钟一律 trait 注入）
5. **校验不依赖行序**：`ts` 是毫秒粒度，同毫秒可以有很多条 —— 故 `verify_chain` 把链当**图**
   （`prev_hash → 行` 建索引，从创世沿链前进），而不是"按 `ts` 排序后逐行比对"
6. **`id` = 本条 `self_hash`**：链位置 + 内容共同决定，天然唯一。它是**唯一**的 self_hash 落点 ——
   与之同义的 `hash` 列已由迁移 0003 删除（ADR-0040 D3 / PL-045 闭环）
7. **链序显式化**：`sequence INTEGER PRIMARY KEY AUTOINCREMENT` 单调且**永不复用**已用号，
   链尾查询与读取顺序都按它 —— 与可能被 `VACUUM` 重排的 `rowid` 解耦（ADR-0040 D2 / PL-043 闭环）
8. **迁移归自己**：`audit_logs` 的 DDL 在 `crates/audit/migrations/`，经 `MIGRATIONS` 暴露；
   装配点（本轮 = 测试夹具，正式 = Host）把各 crate 的集合合并后交给 `Database::open`（ADR-0038）

## 典型用法

```rust
use std::sync::Arc;

use assistant_audit::{AuditLog, AuditSubject, Durability};
use assistant_storage::{
    Database, MIGRATIONS as STORAGE_MIGRATIONS, MigrationSet, StoragePaths, SystemClock,
};

// 唯一装配点（ADR-0038 D3）：storage 自己的表 + 本 crate 的 audit_logs
let mut migrations = MigrationSet::new();
migrations.register_all(STORAGE_MIGRATIONS)?;
migrations.register_all(assistant_audit::MIGRATIONS)?;

let database = Database::open(
    &StoragePaths::new("D:/data/assistant"),
    Arc::new(SystemClock),
    &migrations,
)?;
let mut log = AuditLog::new(database.connection(), database.clock(), Durability::default())?;

// 高风险动作：用 immediate 档，崩机不丢已确认的事件
let mut strict = AuditLog::new(database.connection(), database.clock(), Durability::Immediate)?;
let _ = strict.append(&event, &AuditSubject::step("t_1", "s_1"))?;

// 批量档：攒满 100 条或距上次 flush 超过 200 ms 自动落库
let _ = log.append(&event, &AuditSubject::unattached())?;
assert_eq!(log.flush()?, 0); // 缓冲空时是 no-op

// 取证：链不自洽时 verify_chain_strict 直接报错
let verification = log.verify_chain()?;
assert!(verification.is_intact(), "{}", verification.summary());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 已知限制

- **`separate_db_full` 只解析不落地**：调用 `AuditLog::new(.., Durability::SeparateDbFull)`
  返回 `AuditError::UnsupportedDurability`（**显式失败**，绝不静默降级成 batched）→ **PL-042**
- **整表清空 / 截断无法从库内检出**：链的判据全在库内，删掉**全部**行后"没有行"与"链自洽"不可区分。
  要检出它需要**外部锚点**（把链尾写到只追加的外部介质 / 另一台机器）→ **PL-044**
- ~~链尾查询依赖 `rowid` 顺序~~ **已闭环（PL-043 / ADR-0040 D2）**：迁移 0003 引入显式
  `sequence INTEGER PRIMARY KEY AUTOINCREMENT`，链尾与读取顺序都按它；`VACUUM` 不再能影响链序
  （回归断言见 `tests/audit_tamper.rs` 的 `test_vacuum_does_not_change_chain_tail`）
- ~~`id` 与 `hash` 两列在 v1 语义上相同~~ **已闭环（PL-045 / ADR-0040 D3）**：迁移 0003 删掉 `hash`，
  只留语义明显的 `id`（= 本条 `self_hash`）；「两列必须一致」那条同义反复的校验随之消失，
  权威判据只剩「重算 == `id`」
- **`actor` 列是反规范化副本**：协议新增 `AuditActor` 变体时该列落 `unknown`，权威值始终在 `detail_json`
- **无加密、无签名**：审计库与主库同为明文；签名 / 远程不可篡改存储归后续卡
- **单写连接**：本 crate 借用一个写连接，不提供多写者并发入口

## 相关文档

- 架构 v2 §15.1（表结构）/ §15.3（加密、保留与容量）
- `docs/storage-design.md` §3.2（PRAGMA 与 `audit.durability`）/ §3.3（ring buffer 200 ms / 100 条）/ **§3.4（迁移登记表）** / §4（表映射）
- **`docs/adr/0038-storage-migration-registry.md`**（迁移注册表：DDL 与拥有者同处）
- **`docs/adr/0040-audit-log-column-semantics.md`**（列语义去重 + 显式链序；PL-043 / PL-045 闭环）
- `docs/spec/audit-event.md`、`docs/spec/error-codes.md`
- `tasks/TASK-013-audit-append-hash-chain-flush.md`（本卡正文 + 执行记录）
