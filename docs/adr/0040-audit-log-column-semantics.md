# ADR-0040　`audit_logs` 列语义去重 + 显式链序（PL-045 + PL-043 闭环）

状态：**Accepted**（2026-09-24，人类 chat 裁决「PL-045 语义重复部分，留下有用且语义明显的唯一部分。PL-042/043/044 这些，检查下来没有问题，或者搭建没有问题，就落地」）　日期：2026-09-24　Supersedes：—　Superseded by：—

来源：**PL-045**（`id` 与 `hash` 两列语义相同）+ **PL-043**（链尾查询依赖 `rowid`）；
关联：架构 v2 §15.1（`audit_logs` 列清单）、`docs/storage-design.md` §3.4 / §4、
`crates/audit/migrations/0002_audit_logs.sql`（**只读，不得改动**）、
`crates/audit/src/{log,verify,lib}.rs`、`tasks/TASK-013-audit-append-hash-chain-flush.md` §5 DRIFT-013-2、
ADR-0038（迁移注册表：DDL 与拥有者同处）、`tasks/TASK-203-audit-log-column-semantics.md`（落地卡）。

---

## 背景（为什么现在要决定）

TASK-013 建 `audit_logs` 时，架构 v2 §15.1 给了**两列** `id` 与 `hash`，但**没有说明二者的区别**。
TASK-013 的取值是 `id = hash = self_hash`，并把「两列必须一致」当成一条额外校验
（记 DRIFT-013-2 → PL-045）。两处症状：

| # | 症状 | 证据 |
|---|---|---|
| 1 | **两列存同一个值** —— 每行多 64 字节 + 索引，且读者必须问「它俩差在哪」 | `0002_audit_logs.sql`：`id TEXT PRIMARY KEY` 与 `hash TEXT NOT NULL CHECK(length=64)` 注释都写「= 本条 self_hash」 |
| 2 | 「两列一致」这条校验是**同义反复**：真正的判据是「重算 hash == 存下来的值」，而两列都只是那个值的副本 | `verify.rs` 的 `row.id != row.hash` 分支与 `recomputed != row.hash` 分支 |
| 3 | 链尾查询用 `ORDER BY rowid DESC LIMIT 1` | `log.rs` `SELECT_CHAIN_TAIL_SQL`；PL-043：`VACUUM` 可能重排无 `INTEGER PRIMARY KEY` 表的 rowid |

第 3 条不是理论问题：本表**没有** `INTEGER PRIMARY KEY`，SQLite 文档明说
`VACUUM` 可以重排 rowid。虽然本表 append-only（无 `DELETE` 路径）故「实际不会发生」，
但「链的物理顺序」与「行的可见顺序」是**同一件事的两种描述**，而链的判据必须**只依赖链本身**。

## 决策（一句话）

**`audit_logs` 只保留一个「本条 self_hash」列（`id`），并新增一列显式链序（`sequence`）**；
删掉与之语义重复的 `hash` 列，链尾查询与读取顺序改按 `sequence`，不再依赖 `rowid`。

拆成六条：

**D1　目标列集合（新 schema）**

```text
audit_logs(sequence, id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json)
```

- **删** `hash`（PL-045 闭环：语义重复，唯一保留 `id`）
- **加** `sequence`（PL-043 闭环：显式链序，替代 `rowid`）
- 其余 8 列**逐字不变**（列名 / 类型 / 可空性 / 注释口径）

**D2　`sequence INTEGER PRIMARY KEY AUTOINCREMENT` = 显式链序**

- 本表 append-only（代码侧无 `UPDATE`/`DELETE`；库侧触发器兜底）⇒ `sequence` 单调递增 == 链顺序
- `AUTOINCREMENT` 保证「**不复用**已用过的值」—— 即便将来引入归档删除，新行也不会回填旧号
  （普通 `INTEGER PRIMARY KEY` 在删掉尾部行后会复用 `max+1`，那会让「链序」在归档场景下说谎）
- 链尾查询：`SELECT id FROM audit_logs ORDER BY sequence DESC LIMIT 1`（不再是 `rowid`）
- 读取顺序：`ORDER BY sequence`（不再是 `rowid`）

**D3　`id TEXT NOT NULL UNIQUE` = 本条 `self_hash`**

- 语义唯一：**链位置 + 内容**共同决定（`sha256(HASH_DOMAIN ‖ 0x1F ‖ prev_hash ‖ 0x1F ‖ 规范化 JSON)`）
- `UNIQUE` 保留原 `PRIMARY KEY` 的「重复 self_hash 必须报错」行为（`INSERT` 仍不是 `OR REPLACE`）
- **不再有**「两列一致」这条校验：只剩一列时它是同义反复；真正判据「重算 == 存下来的值」保留

**D4　迁移 `0003_audit_logs_semantics`（重建表）**

- 新文件 `crates/audit/migrations/0003_audit_logs_semantics.sql`，**先**在 `docs/storage-design.md` §3.4 占号
- 步骤（全部在一个迁移事务内）：`CREATE TABLE audit_logs_new(...)` → `INSERT ... SELECT`（`ORDER BY rowid`，
  让 `sequence` 按**当时的链序**分配）→ `DROP TABLE audit_logs`（连带旧索引与两个触发器）→
  `ALTER TABLE audit_logs_new RENAME TO audit_logs` → 重建 `idx_audit_ts` + 两个 append-only 触发器
- **`0002_audit_logs.sql` 一字不改**：它的 sha256 已记在已有库的 `schema_migrations.checksum`，
  改一个字节 → 下一次启动 `migration_checksum_mismatch`（ADR-0038 不变量 2）
- 已有 v2 库的升级路径由新迁移承担；**旧数据的链不重建**（只换列形状），故 `verify_chain` 对升级后的库仍自洽

**D5　代码侧的对应改动（`crates/audit`）**

| 项 | 旧 | 新 |
|---|---|---|
| `INSERT_RECORD_SQL` | 9 列（含 `hash`） | 8 列（不含 `sequence`：由 SQLite 分配） |
| `SELECT_RECORDS_SQL` | `ORDER BY rowid` | `ORDER BY sequence` |
| `SELECT_RAW_ROWS_SQL` | 选 `id, hash, prev_hash, detail_json` | 选 `id, prev_hash, detail_json` |
| `SELECT_CHAIN_TAIL_SQL` | `ORDER BY rowid DESC` | `ORDER BY sequence DESC` |
| `RawAuditRow` | 有 `hash` 字段 | **删** `hash` 字段 |
| `verify_rows` | 先比 `id`/`hash` 两列，再比重算值 | 只比「重算值 == `id`」 |

**D6　架构 v2 §15.1 的列清单同步更新（本 ADR 授权）**

`cross-platform-ai-assistant-architecture-v2.md` line 2594 的
`audit_logs(id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json, hash)`
改为 D1 的形状，并就地注明 `sequence` / `id` 的语义。
**这是改「架构已决事项」（漂移触发器 ④）+ 改 DB schema（③）** —— 本 ADR 即授权，
不另开 ADR；除这一行外不动该文件任何内容。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 保持现状（`id` + `hash` 同值） | ❌ | PL-045：每行多 64 字节 + 读者必须问「差在哪」；校验是同义反复 |
| 2 | **删 `hash`、加 `sequence`**（本 ADR 采纳） | ✅ | 一列一个语义；链序显式；迁移可原地完成；无新依赖 |
| 3 | 删 `id`、保留 `hash` 作主键 | ❌ | 「主键叫 `hash`」让「这是行标识」这层语义消失；且 `id` 已被 `AuditRecord.id` 等公共面引用 |
| 4 | 保留两列，把 `id` 改成 UUIDv7 代理键 | ❌ | 引 `uuid` 依赖（漂移 ①）；且 UUIDv7 是**时间**有序、不是**链**有序，链尾查询仍要排序 |
| 5 | 不加 `sequence`，改用「无后继者的那一行」求链尾（图查询） | ❌ | 依赖 `prev_hash` 索引 + 分叉时要显式报错；而本表是**线性链**，一列单调序号更简单、更快、更好审 |
| 6 | 直接改 `0002_audit_logs.sql` 的 DDL | ❌ | 破坏已有库的 checksum 记账（`migration_checksum_mismatch`，拒绝启动） |
| 7 | 加 `sequence` 但用普通 `INTEGER PRIMARY KEY`（非 `AUTOINCREMENT`） | ❌ | 删尾行后会复用号；本表虽无删除路径，但归档动作（PL-043 的触发场景）正是它 |

## 影响范围（本 ADR 实施时）

| 类别 | 文件 | 改动 |
|---|---|---|
| 迁移 | `crates/audit/migrations/0003_audit_logs_semantics.sql` | **新增**（重建表 + 重建索引与触发器） |
| 代码 | `crates/audit/src/lib.rs` | `MIGRATIONS` 加 0003；模块文档不变量 5 改述 |
| 代码 | `crates/audit/src/log.rs` | `INSERT` / 3 条 `SELECT` 常量 + `RawAuditRow` + 注释 |
| 代码 | `crates/audit/src/verify.rs` | 去掉 `id`/`hash` 两列一致性分支；模块文档改述 |
| 测试 | `crates/audit/tests/audit_integration.rs` | 列清单断言（`sequence` 在首、无 `hash`）+ 期望版本 2 → 3 |
| 测试 | `crates/audit/tests/audit_tamper.rs` | 新增「`VACUUM` 后链尾仍正确」的等价断言 |
| 文档 | `cross-platform-ai-assistant-architecture-v2.md` §15.1 | line 2594 列清单（D6） |
| 文档 | `docs/storage-design.md` | §3.4 加 0003 行；§4 表映射行去 `hash` |
| 文档 | `crates/audit/README.md` | 不变量 6 / 已知限制（PL-043、PL-045 两条改为「已闭环」） |
| 文档 | `docs/PARKING_LOT.md` | PL-043 / PL-045 关闭 |
| 文档 | `docs/adr/README.md` / `docs/memory/decisions.md` | 登记 0040 |
| 任务卡 | `tasks/TASK-203-audit-log-column-semantics.md` | 本 ADR 的落地卡 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 重建表时丢行 / 乱序 | `INSERT ... SELECT ... ORDER BY rowid` 在**同一迁移事务**内完成；集成测试断言「升级后行数不变 + 链自洽 + 逐行 `prev_hash` 与升级前一致」 |
| `DROP TABLE` 把 append-only 触发器一并带走 | 迁移里**显式重建**两个触发器；集成测试断言升级后两个触发器都在，且 `UPDATE`/`DELETE` 仍被拒 |
| `ALTER TABLE ... RENAME` 连带重命名索引 | 先 rename、后建 `idx_audit_ts`（顺序固定，不依赖旧索引存续） |
| 已有库的 checksum 记账被破坏 | 0002 一字不改；只**新增** 0003（ADR-0038 不变量 1） |
| 删掉「两列一致」校验会削弱篡改检出 | 该分支是同义反复（两列都只是 `self_hash` 的副本）；真正的判据「重算 == 存下来的值」保留且是**唯一**权威判据 |
| `AUTOINCREMENT` 带来 `sqlite_sequence` 表 | 它匹配 `sqlite_%` 前缀，`schema.rs` 的 `business_table_count` 已排除，不影响「有业务表却无版本表」的判据 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **列形状**：`pragma_table_info('audit_logs')` 逐字等于 D1 的 9 列（顺序一致）。
2. **升级路径**：v2 库（有数据）→ 打开即升到 v3；行数不变、`verify_chain` 自洽、
   `sequence` 从 1 连续、两个触发器与 `idx_audit_ts` 都在。
3. **不再依赖 rowid**：`crates/audit` 与 `crates/audit/migrations/0003_*.sql` 里
   `grep rowid` 只在 0003 的注释里出现（迁移需要 `ORDER BY rowid` 来**读旧序**），
   运行期 SQL 归零。
4. **`hash` 归零**：`grep -w hash` 在 `crates/audit/src/` 只应命中 `prev_hash` / `self_hash` / 注释。
5. **`VACUUM` 不改变链尾**：`VACUUM` 之后 `persisted_hash()` 与 append 的 `prev_hash` 仍接得上
   （PL-043 的原始触发场景的等价断言）。
6. 重新评估触发条件：① 引入归档删除 → `AUTOINCREMENT` 的「不复用」语义成为硬依赖，需补测；
   ② 引入第二个拥有 `audit_logs` 的写入者 → 链序分配的并发语义需重新设计。

## 相关 ADR

- ADR-0038（迁移注册表：`audit_logs` 的 DDL 归 `crates/audit`；只前进不回滚；checksum 记账）
- ADR-0030（机器校验优于手工回填 —— §3.4 登记表先占号）
- ADR-0033（单文件行数口径）
- ADR-0039（任务结束状态同步 —— 本 ADR 的落地卡同批更新 `PLAN.md` / `README.md`）
