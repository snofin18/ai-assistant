# TASK-206　`crates/storage`：`memory_fts`（FTS5 虚表）迁移 + 检索 API + 存储侧测试

- 状态：**Ready**
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内；它是 **TASK-208** 的前置卡）　依赖：012（✅ Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`；契约见 `docs/spec/core-orchestration.md` 与 `docs/adr/0053-core-orchestration-layer-interface.md`。
- 号段依据 **ADR-0037 D1**（200~299 = 治理池）+ **ADR-0053 D6 / D7**（先例 = TASK-201 / 202 / 203 / 205：阶段 1 结构件的前置 / 拆分卡）。
- 来源：**DRIFT-028-1**（原 TASK-028 §5；触发器 ⑤）+ **ADR-0053 D6 / D7**（人类 2026-09-26 裁决「drift-028 的 5 点都按照你的建议做」→ 采纳「立前置卡」）
- **DRIFT-206-1 裁决（2026-09-26，方案 A）**：write scope 增列 `docs/storage-design.md`（**仅 §3.4**）→ 本卡**由 Blocked 回 Ready**，可原样开工

---

## 目标（一句话）

在 `crates/storage` 落地 `memory_fts`（FTS5 虚表）的**迁移**与**检索 API**，并配套存储侧测试 —— 让 `assistant-core` 的 Memory（TASK-208）只**消费**一个已经存在、已被测试的存储接口，而不是自己去拼 SQL。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | `memory_fts`（FTS5 虚表）被 `docs/storage-design.md` 明确定为 **L1 SQLite 层** | `docs/storage-design.md` §3（`+ FTS5: memory_fts（任务历史与偏好检索）`）与 §4 归属表（`memory_fts`（FTS5 虚表）→ L1） |
| 2 | `crates/storage/src/error.rs` 的模块注释**早已预告**本卡：「后续卡（TASK-013 审计、**TASK-028 FTS5**）会新增变体」 | `crates/storage/src/error.rs` 顶部 `#[non_exhaustive]` 说明 |
| 3 | 原 TASK-028 的 write scope 只有 `crates/core/**`，**装不下**存储层的迁移与 SQL | `tasks/TASK-028-core-session-context-planner-memory.md` §5 的 **DRIFT-028-1**（触发器 ⑤） |
| 4 | `crates/storage` 现有公开面**没有任何检索 API**（只有 `Database::open` / `clock` / `blob_store` / `schema_version` / `close` + 记录类型） | `crates/storage/src/lib.rs` 的 re-export 清单 |
| 5 | **不需要新第三方依赖**：`libsqlite3-sys` 的 `bundled` 构建已带 `-DSQLITE_ENABLE_FTS5` | 本机实测：`build.rs` 的 bundled 编译参数含 `-DSQLITE_ENABLE_FTS5`（DRIFT-028-1 的「佐证 ③」） |
| 6 | 人类 2026-09-26 已把「**任何 crate 往迁移链加表，都得顺手改 storage 的测试**」上升为正式机制 | 人类 chat 2026-09-26；本卡 DoD 第 2 / 3 条 |

## write scope

- `crates/storage/src/**`（迁移链 + 检索 API + 记录类型；新增模块属漂移触发器 ②，**本卡的存在即 ADR-0053 D6 的授权**）
- `crates/storage/migrations/**`（若迁移以文件形式登记；与 `crates/storage/src/schema.rs` 的 `MIGRATIONS` 保持一致）
- `crates/storage/tests/**`（**新增**测试文件；不得改既有断言 —— 漂移触发器 ⑦）
- `crates/storage/README.md`（职责 / 边界 / 不变量 / 已知限制随新 API 同步）
- `docs/DEPENDENCIES.md`（**仅当**确实新增使用方；FTS5 走既有 `libsqlite3-sys`，预期**零新增**）
- `docs/storage-design.md`（**仅 §3.4 全局迁移登记表**：占号 0004 + 必要的历史注记 —— **DRIFT-206-1 方案 A**，2026-09-26 裁决）
- `tasks/TASK-206-storage-memory-fts5-search.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步

## Out of scope（写了就停）

- **不做**向量检索 / 语义检索 / 外部索引（阶段 1 Out of scope；见 `plans/stage-1-pilots.md` 的 Out of scope 清单）
- **不做** `core` 侧的 Memory 组件、App Map 加载、片段拼装（归 **TASK-208**）
- **不改**任何既有表的结构与语义（`audit_logs` / `tasks` / `task_steps` / `evidence` 等）；本卡只**新增** FTS5 虚表与其同步机制
- **不引第三方依赖**（FTS5 走既有 bundled 构建）；确实需要时必须先登记 + 走漂移触发器 ①
- **不放宽**任何 lint、不加 `#[allow]`（`#[cfg(test)]` 内的既有豁免口径除外）、不加 `unsafe`
- **不改** `assistant-protocol` 的 schema / `ErrorCode`（要改 → ADR，漂移触发器 ③④）
- **不顺手重构** storage 既有模块

## 必须遵守

- **迁移链一致性**：新表必须进迁移链并**同时**改存储侧测试（人类 2026-09-26 的正式机制）；`cargo run -p xtask -- check-migrations` 必须绿
- **fail-closed 检索**：非法查询（空查询、超长、未支持的语法）一律带 `ErrorCode` 返回，**不得**退化成「返回全表」或「返回空集冒充成功」（铁律 1 / 2）
- **检索结果必须带来源与可追溯标识**（至少：命中的记录类型 + 主键 + 原文引用位置），供上层判断可信度 —— 内容本身是**不可信输入**（铁律 2）
- **无静默失败**：FTS5 虚表与主表不一致时必须**可检测**（要么由触发器/同步机制保证，要么提供一致性自检 API 并在不一致时报错）
- **错误语义**：新增的 `StorageError` 变体必须给稳定 `reason_code()`（snake_case，不得随版本漂移）并映射到架构 v2 §8.7 的 13 类之一；存储层**不发明**新 `ErrorCategory`
- **只追加文件**（`LEDGER.md` / `docs/PARKING_LOT.md`）**不改写既有行**

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-storage
cargo run -p xtask -- check-migrations
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo deny check
```

## DoD

- [ ] `memory_fts`（FTS5 虚表）随迁移链建立；**新库**与**既有库升级**两条路径都能到达同一 schema 版本
- [ ] 存储侧测试覆盖：建表 / 写入同步 / 检索命中 / 检索未命中 / 非法查询 fail-closed / 一致性自检（≥ 6 个用例）
- [ ] **既有测试零改动通过**（漂移触发器 ⑦：改断言 = 缺陷）
- [ ] 公开检索 API 有完整文档注释（语义 / 参数 / 返回 / **错误语义** / 副作用 / 是否幂等 / 超时与取消行为）
- [ ] `StorageError` 新增变体带稳定 `reason_code()` 与 13 类映射；`#[non_exhaustive]` 保持
- [ ] 零新增第三方依赖（若确实新增 → 已在 `docs/DEPENDENCIES.md` 登记且人类已批准）
- [ ] 上列 14 条验收命令全绿；`hygiene` / `card-check` / `docscan` 的 warning **不新增**（对照既有基线）
- [ ] §11.1 进度同步：`PLAN.md` 当前状态块 / `README.md` 三处 / `LEDGER.md` / `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md` 规模表
- [ ] 若产生新 FACT / PITFALL → 追加 `docs/memory/{facts,pitfalls}.md`

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-206 `crates/storage`：`memory_fts`（FTS5 虚表）迁移 + 检索 API + 存储侧测试
【目标】让 TASK-208 只消费一个已建表、已测试、fail-closed、可追溯的 storage 检索接口。
【write scope】仅：crates/storage/src/**、crates/storage/migrations/**、crates/storage/tests/**、
crates/storage/README.md、docs/DEPENDENCIES.md（仅确需新增依赖时）、docs/storage-design.md 仅 §3.4、
本卡记录区，以及 AGENTS.md §11.1 指定进度文件。
【铁律】1 无静默失败；2 不可信输入先校验；9 不得静默扩大范围；10 契约先行。
【禁止】向量/语义检索；core Memory；改既有表；新第三方依赖；放宽 lint / 加 allow / unsafe；
改 protocol schema / ErrorCode；顺手重构。
【验收】卡内 14 条命令 → 全绿，警告相对既有基线不新增。
【依赖】TASK-012 Done（已核对 LEDGER 与当前 schema）；TASK-208 依赖本卡。
【疑问】0004 与 audit 0002/0003 交错；不修改 audit 测试的前提下，storage 用 MIGRATIONS（0001）+
MEMORY_MIGRATIONS（0004）两个片段暴露给唯一装配点。
```

### 2. 实际改动文件

- `crates/storage/migrations/0004_memory_fts.sql`（新增）
- `crates/storage/src/memory.rs`（新增）
- `crates/storage/src/error.rs`
- `crates/storage/src/lib.rs`
- `crates/storage/src/schema.rs`
- `crates/storage/tests/storage_memory_fts.rs`（新增）
- `crates/storage/README.md`
- `docs/storage-design.md`（仅 §3.4）
- `docs/memory/pitfalls.md`
- `tasks/TASK-206-storage-memory-fts5-search.md`（仅记录区）
- `MEMORY.md`（仅规模表：pitfalls 行数 / 条目数）

既有 `crates/storage/tests/common/mod.rs`、`storage_migrations.rs` 与 audit 测试**均未改动**。

### 3. 验收输出摘要

- `cargo fmt --all --check` → PASS（0 diff）
- `cargo clippy --all-targets -- -D warnings` → PASS
- `cargo test --workspace` → PASS
- `cargo test -p assistant-storage` → PASS；新增 FTS 测试 10 个，storage 合计 38 测试 + 2 doctest
- `cargo run -p xtask -- check-migrations` → PASSED（4 迁移 / 4 登记 / 2 crate）
- `cargo run -p xtask -- verify-schemas` → PASSED（5 schema）
- `cargo run -p xtask -- codegen --check` → PASSED（0 drift）
- `cargo run -p xtask -- hygiene` → PASSED（0E / 4W，既有基线）
- `cargo run -p xtask -- docscan` → PASSED（0E / 468W，既有基线）
- `cargo run -p xtask -- card-check` → PASSED（0E / 27W，既有基线）
- `cargo run -p xtask -- memory-counts` → PASSED
- `cargo run -p xtask -- adr-index` → PASSED（39）
- `cargo run -p xtask -- check-ledger` → PASSED
- `cargo deny check` → PASS（advisories / bans / licenses / sources）

### 4. DoD 逐条核对

- [x] `memory_fts`（FTS5 虚表）随迁移链建立；新库与既有库升级两条路径都到达同一 schema 版本
- [x] 存储侧测试覆盖：建表 / 写入同步 / 命中 / 未命中 / 非法查询 fail-closed / 一致性自检（10 个用例）
- [x] 既有测试零改动通过（未改任何既有断言或既有测试文件）
- [x] 公开检索 API 有完整文档注释（语义 / 参数 / 返回 / 错误语义 / 副作用 / 幂等 / 超时与取消）
- [x] `StorageError` 新增 `invalid_memory_query` / `memory_index_inconsistent`，保留 `#[non_exhaustive]` 与 Fatal 映射
- [x] 零新增第三方依赖
- [x] 上列 14 条验收命令全绿；warning 与既有基线一致
- [ ] §11.1 进度同步：实现 PR 合并后由 closeout 提交完成状态与当前进度块
- [x] 新增 PITFALL：全局迁移片段不能单独 `validate()`

### 5. 偏差

无行为偏差。实现中确认 0004 与 audit 0002/0003 交错，若把 0004 并入 storage 的 `MIGRATIONS` 会要求修改
audit 既有测试（超出本卡 write scope）；因此在 storage 增加公开片段常量 `MEMORY_MIGRATIONS`，唯一装配点
注册 storage 两个片段 + audit 两个片段。`check-migrations`、storage 测试与 audit 测试均全绿。

### 6. 更合理做法

迁移片段按“拥有者 + 变更批次”暴露，而不是强迫一个 crate 的所有迁移挤进一个连续集合；这保留了
ADR-0038 的全局连续校验，同时避免跨 crate 测试被无关迁移牵动。

### 7. 遗留问题

- TASK-208 消费本 API 时只依赖记录/查询/结果类型，不拼 SQL。
- 当前只支持字面量词项检索；FTS5 运算符不暴露给模型/用户。
- 当前 `unicode61` 不切分无空格 CJK 序列；“短中文子串”检索需另立卡评估 trigram / 分段器 / 辅助索引。
- 未做向量或语义检索（阶段 1 Out of scope）。

### 8. 新增长期记忆

`docs/memory/pitfalls.md`：单个 crate 的迁移常量是全局序列片段，不能单独 `validate()`；storage 的
0001 / 0004 与 audit 的 0002 / 0003 必须在装配点合并后校验连续。

### 9. 给审阅者的关注点

1. 最高风险：新增公开常量 `MEMORY_MIGRATIONS` 是否接受为 ADR-0038 下的“多片段拥有者”约定。
2. 次风险：FTS 查询把用户输入全部解释为字面量词项，并拒绝空 / 超长 / 无有效词项 / limit 越界。
3. 一致性自检只报告源行缺索引、索引孤儿、字段快照不一致，不自动重建索引。
