# TASK-230　storage 补齐 conversations / message-tree 持久化记录 API

- 状态：**Done（2026-10-04，PR merge hash 待回填；`PL-092` 闭环）**
- 阶段：1　子阶段：1a 补救 / 治理　批次：治理池　依赖：TASK-012、TASK-028、TASK-206
- 关联：`docs/storage-design.md` §3.4 / §4 / §7、`crates/core/src/store.rs`、`crates/core/src/message.rs`、`docs/PARKING_LOT.md` 的 `PL-092`
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。

---

## 目标（一句话）

在 `crates/storage` 增加 `conversations` 与 `conversation_messages` 的真实 SQLite 读写 API，使 Core 的
`SessionStore` 后续可由 binary 装配层接上跨重启持久化，而不是继续用 `MemorySessionStore` 冒充生产存储。

## 背景（为什么现在做）

`0001_init.sql` 的 `tasks.conversation_id` 注释指向 `conversations` 表，TASK-028 也把真实持久化放进了
`SessionStore` 注入边界；但 storage 既没有 `conversations` 表，也没有会话 / 消息树记录 API。
`PL-092` 因此一直阻塞生产装配。本卡只补存储层机制与行 API，不改 `SessionStore` trait，也不在 core 内打开连接。

## write scope

- `crates/storage/**`（源码、迁移、测试、README、`MIGRATIONS` 清单）
- `crates/audit/tests/audit_integration.rs`（仅同步全局迁移期望版本；不改进生产行为）
- `docs/storage-design.md`（仅 §3.4 迁移登记表及相邻过时口径）
- `docs/PARKING_LOT.md`（仅追加 `PL-092` 闭环行）
- `docs/memory/pitfalls.md`（仅追加本次发现的迁移版本测试坑）、`MEMORY.md`（仅规模表相应行）
- `tasks/TASK-230-storage-conversation-message-tree.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（本卡行与当前进度句）

## In scope

- 新增全局迁移 `0005_conversations_message_tree.sql`，建立 `conversations` 与 `conversation_messages`：
  会话行保存 `SessionSnapshot` 所需的目标、状态、时间、revision；消息行保存父子、sequence、role、
  content、token estimate、retention。
- 在 `crates/storage` 公开 `ConversationRecord` / `ConversationMessageRecord` 与会话 / 消息的插入、读取、
  整树替换 API；写操作必须显式返回成功或带 `StorageError` 的失败。
- 提供跨重启式真实测试：连接 A 写入 → 显式关闭 → 同一路径重新打开 → 读回逐字段一致。
- 提供缺失会话或非法父子关系的负向测试：外键 / 顺序校验必须显式失败，不得静默写入或返回假树。
- 同步 `MIGRATIONS`、`docs/storage-design.md` §3.4 与 `crates/storage/README.md` 的不变量 / 已知限制。

## Out of scope（做了算漂移）

- 修改 `SessionStore` trait、core 的 `SessionSnapshot` / `MessageNode` 公共形状，或让 `crates/storage`
  反向依赖 `assistant-core`。
- 修改 `0001_init.sql` 或任何已发布迁移；修改 `crates/platform/**` / `xtask/**` / `docs/spec/**`。
- 新增第三方依赖、crate、顶层目录、抽象层；添加 `unsafe`、`#[allow]` 或放宽 lint。
- 实现审计 hash chain、UI 会话列表、模型配置表、冷归档或加密。

## 必须遵守

- **铁律 1 / 2 / 4**：无静默失败；持久化输入先校验；每个写操作必须有可验证 postcondition。
- **铁律 9 / 10**：不扩卡外范围；新增迁移先登记，再让 `Database::open` 能装配。
- **ADR-0038**：storage 只拥有自己的迁移片段，版本号全局唯一；本 crate 的 `MIGRATIONS` 必须包含 0005。
- **资源生命周期（ADR-0063）**：本卡不新增进程内长期增长容器；消息行随会话删除级联，数据库行数由业务生命周期控制。
- **ADR-0028**：写热点文件前 `guard acquire`，写完立刻 `guard release`。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-storage
cargo run -p xtask -- check-migrations
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

并附：`0005` 迁移 diff；跨重启测试的第一次 / 关闭 / 重开 / 二次读取四步证据；缺失会话与非法父子关系的负向输出。

## 完成定义（DoD）

- [ ] 新增 `0005_conversations_message_tree.sql`，且 `0001_init.sql` 未被修改。
- [ ] `assistant_storage::MIGRATIONS` 登记 0005；`check-migrations` 全绿；§3.4 表逐行一致。
- [ ] `ConversationRecord` / `ConversationMessageRecord` 及插入、读取、整树替换 API 已公开，错误 fail-closed。
- [ ] 跨重启测试证明：首次连接写入、关闭、重新打开同一数据库后内容逐字段一致。
- [ ] 负向测试证明：缺失会话与非法父子关系显式失败，失败后不留下半个会话 / 消息树。
- [ ] `crates/storage/README.md` 同步职责、边界、不变量与已知限制。
- [ ] 全套验收命令全绿；`PL-092` 已追加闭环行；状态同步文件已更新。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-230 storage 补齐 conversations / message-tree 持久化记录 API
【目标】新增 0005 迁移，提供会话与消息树的真实 SQLite 读写 API、跨重启测试与显式负向失败
【write scope】仅：crates/storage/**、crates/audit/tests/audit_integration.rs（仅全局迁移期望同步）、
              docs/storage-design.md §3.4、docs/PARKING_LOT.md、docs/memory/pitfalls.md、MEMORY.md 规模表、
              本卡、LEDGER / PLAN 当前状态块 / README 三处 / plans 本卡行与进度句
【铁律】1 无静默失败；2 持久化输入先校验；4 写操作有 postcondition；9 不扩范围；10 迁移先登记；12 有界资源
【禁止】改 0001_init.sql、platform / xtask / spec、SessionStore trait；新增第三方依赖或放宽 lint
【验收】fmt / clippy / workspace test / storage test；xtask 迁移、文档、schema 与 ledger 门禁全绿
【依赖】TASK-012、TASK-028、TASK-206 已 Done；当前全局迁移号最大为 0004
【疑问】无；保持 storage 与 core 解耦，由后续 binary 装配层做 SessionStore adapter
```

### 2. 实际改动文件

- `crates/storage/migrations/0005_conversations_message_tree.sql`（新增 `conversations` / `conversation_messages` / 索引 / 父子顺序触发器）
- `crates/storage/src/conversations.rs`、`conversation_messages.rs`、`lib.rs`、`schema.rs`
- `crates/storage/tests/storage_conversations.rs`、`tests/storage_memory_fts.rs`
- `crates/storage/README.md`
- `crates/audit/tests/audit_integration.rs`（仅全局迁移期望版本同步，见 §5）
- `docs/storage-design.md` §3.4、`docs/PARKING_LOT.md`、`docs/memory/pitfalls.md`、`MEMORY.md` 规模表
- `tasks/TASK-230-storage-conversation-message-tree.md`、`plans/stage-1-pilots.md`、`LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处

### 3. 验收输出摘要

**按退出码核**

- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0；只有仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → EXIT 0；xtask 438 passed，audit 13 passed，storage 全目标通过。
- `cargo test -p assistant-storage` → EXIT 0；新增会话测试 5 passed / 0 failed。
- `cargo run -p xtask -- check-migrations` → `scanned_migration_files=5 registry_entries=5`，0 error / 0 warning，PASSED。
- `hygiene` → 0 error / 101 warning，PASSED；`memory-counts` / `adr-index` / `refscan` / `docscan` / `check-comments` / `verify-schemas` / `codegen --check` → 全 PASSED。

**跨重启与负向证据**

- `test_conversation_snapshot_survives_close_and_reopen`：连接 A 写入 revision 0 → `Database::close()` → 同一路径重开，会话字段与 3 条消息逐字段一致；再替换到 revision 1 → 再关闭/重开 → 4 条消息仍一致。
- `test_insert_message_rejects_missing_conversation`：缺失会话 → `invalid_argument`，消息行数为 0。
- `test_insert_message_rejects_invalid_parent_relationships`：缺失父节点 / 父 sequence 不小于子 sequence → `invalid_argument`，只保留合法根消息。
- `test_insert_snapshot_rejects_invalid_tree_without_partial_rows`：整树非法 → 会话表与消息表都为 0。
- `test_replace_snapshot_rejects_revision_gap_and_keeps_old_tree`：revision 0 → 2 跳号 → `invalid_argument`，原快照逐字段不变。

### 4. DoD 逐条核对

- [x] 新增 `0005_conversations_message_tree.sql`，`0001_init.sql` 未改。
- [x] `MIGRATIONS` 登记 0005，`check-migrations` 5/5，`docs/storage-design.md` §3.4 同步。
- [x] 公开 conversation / message 记录类型与插入、读取、整树替换 API；非法输入 / 引用 / revision 均显式失败。
- [x] 跨重启测试证明真实关闭与重开同一 SQLite 后内容一致，不使用内存 store 冒充。
- [x] 缺失会话、非法父子关系、整树原子性与 revision 跳号均有负向测试。
- [x] `crates/storage/README.md` 已同步职责、边界、不变量与已知限制。
- [x] 全套验收命令通过；`PL-092` 已闭环；状态同步文件已更新。

### 5. 偏差

**DRIFT-230-1（触发器 #5 scope / #7 测试断言；本批已解决，未阻塞）**

- **现象**：新增 storage `0005` 后，`crates/audit/tests/audit_integration.rs` 的全局迁移期望版本写死为 4，workspace 测试会因迁移链新增而失败。
- **影响**：不改该断言就无法满足 `cargo test --workspace` 全绿；但该改动落在任务提示强调的 `crates/storage/**` 之外。
- **建议/处理**：只把期望值 4 同步为 5，并写明新链由 storage 0001 + 0004 + 0005 + audit 0002 + 0003 组成；不改 audit 生产代码、不删除断言、不放宽判据。预授权自动化按“自行解决最优方案”原则在本批完成，并新增 PITFALL。

### 6. 更合理做法

- storage 不依赖 `assistant-core`：记录类型使用持久化字符串保存 Core 语义，避免 crate 依赖环，也避免在 storage 复制 core enum。
- 外层主键用 `(conversation_id, id)`，并对 `(conversation_id, parent_id)` 加复合外键；再加 `parent.sequence < child.sequence` 触发器，使“父节点同会话且更早”不仅依赖 Rust 调用方。
- 整树写入使用 `unchecked_transaction`，失败时由事务回滚；新建强制 revision 0，替换强制旧 revision + 1。

### 7. 遗留问题

- `SessionStore` 的真实 adapter 仍需在 binary 装配层编写；本卡只交付 storage 公开记录 API，未改 trait，也不把 `MemorySessionStore` 冒充持久化。
- 会话列表 / 归档切换 / 保留策略 API 未实现，`archived` 字段先持久化，后续按装配需求另立卡。

### 8. 新增长期记忆

- PITFALL（已落 `docs/memory/pitfalls.md`）：跨 crate 集成测试不能把全局迁移上限写死成当前快照数字；新增迁移时必须同批同步具体断言或改用 `migrations().expected_version()`。

### 9. 给审阅者的关注点

- `conversation_messages` 的复合外键 + `BEFORE INSERT/UPDATE` 触发器是否正确覆盖“父同会话且 sequence 更小”；这是防静默坏树的主要数据库侧证据。
- 将 storage 记录适配为 `SessionSnapshot` 时，`i64` revision / token 与 core `u64` 的边界必须在 adapter 层显式处理，不能在 storage API 中放宽。
- `crates/audit/tests/audit_integration.rs` 的 4→5 是 `DRIFT-230-1` 的一行机械同步；请确认该 scope 扩张可接受。
