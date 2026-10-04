# TASK-233　生产装配接入 storage 会话持久化（`SessionStore` 适配器；闭环 PL-108）

- 状态：**Done（2026-10-04）**
- 阶段：1　子阶段：1a 补救 / 治理　批次：治理池　依赖：TASK-028（trait 与内存实现）、TASK-230（storage 记录 API）、**PL-108**
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`crates/core/src/store.rs`（trait 契约）、`crates/storage/src/conversations.rs`、`crates/storage/src/conversation_messages.rs`、`apps/agent-core/src/assembly.rs`、`docs/PARKING_LOT.md` 的 `PL-108`

---

## 目标（一句话）

给 binary 装配层提供 **storage-backed `SessionStore` 适配器**，并把生产装配的两处 `MemorySessionStore`
换成它 —— 让「跨重启持久化」不再是口号。

## 背景（为什么现在做）

- `crates/core/src/store.rs` 的 trait 文档早就写明：「Production assembly implements this trait with the
  public `assistant-storage` record APIs. Core never holds a SQLite connection.」但实际生产装配
  （`apps/agent-core/src/production.rs`、`main.rs`）注入的是 `MemorySessionStore`。
- TASK-230 交付了 storage 层记录 API：`insert_conversation_snapshot` / `load_conversation_snapshot` /
  `replace_conversation_snapshot`（原子、revision 恰好 +1），正好是适配器需要的三个原语。
- `PL-108`（来源：TASK-230 轮的 automation memory）登记了这个缺口：装配层仍用内存 store 顶替生产存储，
  这正是 `PL-092` 原文禁止的形态。

## write scope

- `apps/agent-core/src/storage_session_store.rs`（新增适配器）、`apps/agent-core/src/lib.rs`（导出）
- `apps/agent-core/src/assembly.rs`（装配开关）、`apps/agent-core/src/production.rs`、`apps/agent-core/src/main.rs`（改用 storage 后端）
- `apps/agent-core/tests/session_store_persistence.rs`（新增证据）
- `tasks/TASK-233-storage-session-store-adapter.md`（本文件）与状态同步文件

## In scope

- 适配器实现 `SessionStore` 三个方法，全部经装配层注入的 `DatabaseHandle`（`Arc<Mutex<Database>>`）访问 SQLite：
  `insert_session` → `insert_conversation_snapshot`；`update_session` → `replace_conversation_snapshot`；
  `load_session` → `load_conversation_snapshot` + `SessionSnapshot::restore` 复校验。
- 映射失败/库内被篡改（未知 `role` / `retention` / `status`、revision 溢出）必须**显式失败**，不得猜。
- 装配层提供显式开关 `with_storage_session_store()`，生产装配两处改用它；**测试注入 `MemorySessionStore`
  的既有路径保持可用**（不静默改默认行为）。
- 证据：① 适配器级跨重开（关库 → 重开同一 data root → 逐字段一致，含分支消息）；② 负向（重复 insert /
  revision 非 +1 / 缺失会话 / 篡改行）；③ **装配级**：`HostAssembly` + `with_storage_session_store()`
  建会话与消息 → 关库 → 重开 → 能读回。

## Out of scope（做了算漂移）

- 改 `crates/core/**`、`crates/storage/**` 的公共形状（trait / 记录结构 / 迁移）；改 `docs/spec/**`、`AGENTS.md`。
- 新增依赖；把 SQLite 连接放进 core；实现 UI/会话列表等后续功能。

## 必须遵守

- **铁律 1 / 7**：失败必须显式；core 不碰平台/存储句柄，SQLite 只在 binary 层适配器里出现。
- **ADR-0063**：适配器自身不持有无界状态（无缓存；每次调用即读写，锁随作用域释放）。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。
- 提交前按 `docs/memory/pitfalls.md`（2026-10-04 补强）做状态行 ↔ LEDGER 比对并把结果贴进 PR。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test session_store_persistence
cargo test -p assistant-core
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check
```

## 完成定义（DoD）

- [ ] `StorageSessionStore` 实现 `SessionStore` 三方法，失败路径带稳定 `SessionStoreError` 原因码。
- [ ] 生产装配两处不再注入 `MemorySessionStore`；`with_storage_session_store()` 显式声明存储后端。
- [ ] 跨重开测试逐字段一致（含分支消息、`status`/`ended_at`/`revision`）。
- [ ] 负向四类（重复 insert / revision 非 +1 / 缺失会话 / 篡改行）显式失败，有原始输出。
- [ ] 装配级证据：`HostAssembly` 建会话并在重开库后读回。
- [ ] `PL-108` 在 `docs/PARKING_LOT.md` 标为已闭环；全套门禁绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-233　生产装配接入 storage 会话持久化（`SessionStore` 适配器；闭环 PL-108）
【目标】给 binary 装配层提供 storage-backed `SessionStore` 适配器，并把生产装配两处 `MemorySessionStore` 换成它
【write scope】仅：apps/agent-core/src/storage_session_store.rs（新增）、src/lib.rs、src/assembly.rs、src/production.rs、src/main.rs、tests/session_store_persistence.rs、tests/assembly_contract.rs、本卡 + 状态同步文件
【铁律】1 无静默失败 / 7 core 不碰平台或存储句柄、SQLite 只在 binary 层 / ADR-0063 适配器无无界状态 / ADR-0028 热点文件先取锁
【禁止】改 `crates/core/**`、`crates/storage/**` 公共形状；改 `docs/spec/**`、`AGENTS.md`；新增依赖；把 SQLite 连接放进 core
【验收】见 §3：fmt / clippy / workspace test / 两个专项测试 / 架构测试 / 全套 xtask 门禁 → 全绿
【依赖】TASK-028（trait 与内存实现）、TASK-230（storage 记录 API），已核对 LEDGER 均为 Done
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/storage_session_store.rs`（新增）：`StorageSessionStore` 实现 `SessionStore` 三方法，只经注入的 `DatabaseHandle` 访问 SQLite；`insert_session` → `insert_conversation_snapshot`，`update_session` → 先自查 revision 恰好 +1 再 `replace_conversation_snapshot`，`load_session` → `load_conversation_snapshot` + `SessionSnapshot::restore` 复校验。
- `apps/agent-core/src/lib.rs`：`mod storage_session_store;` + `pub use ... StorageSessionStore`。
- `apps/agent-core/src/assembly.rs`：`HostAssemblyInput` 新增 `storage_backed_session_store: bool`（默认 `false`）、新开关 `with_storage_session_store()`，装配点改走新增的 `select_session_store()`（注入 store 优先，其后 storage-backed，二者皆无则 `MissingComponent{session_store}`）。
- `apps/agent-core/src/production.rs`、`apps/agent-core/src/main.rs`：两处 `.with_session_store(Arc::new(MemorySessionStore::new()))` 改为 `.with_storage_session_store()`。
- `apps/agent-core/tests/session_store_persistence.rs`（新增）：适配器级跨重开 + 负向证据。
- `apps/agent-core/tests/assembly_contract.rs`：新增 `open_database_handle()` 与装配级 `test_storage_backed_session_store_survives_reopen`（不注入 store，靠开关）。

### 3. 验收输出摘要

全部 `EXIT 0`（原始输出见 PR 描述）：

- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（仅既有 `unknown lint: clippy::assert_is_empty` warning，与 TASK-224/230/231 轮一致，非本卡引入）。
- `cargo test --workspace` → EXIT 0（含 `438 passed` 的 xtask 二进制；最末 `test result: ok` 全绿）。
- `cargo test -p assistant-agent-core --test session_store_persistence` → **3 passed / 0 failed**。
- `cargo test -p assistant-agent-core --test assembly_contract` → **6 passed / 0 failed**（含新增 `test_storage_backed_session_store_survives_reopen`）。
- `cargo test -p assistant-core` → EXIT 0；`cargo test -p assistant-core arch::` → **5 passed / 0 failed**（依赖方向）。
- xtask 门禁：`hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` / `check-migrations` 全 PASSED（数值见 PR）。

负向四类证据（`session_store_persistence` 三个用例覆盖，`assistant-storage` schema 的 CHECK 先挡一类，适配器自层挡其余）：

- **重复 insert** → `reason_code = session_already_exists`；
- **revision 非 +1**（跳号 `5`）→ `reason_code = revision_conflict`；
- **缺失会话** → `update_session` 得 `session_not_found`，`load_session` 得 `None`（非错误）；
- **篡改行** → 改 `goal` / `content` 为空（schema 未约束、由记录层/适配器复校验）得 `session_store_failure`；改 `sequence` 为 `4294967296`（schema 允许、Core `u32` 装不下）得 `persisted_row_invalid`。全部显式失败，无一被默认值顶替。

### 4. DoD 逐条核对

- [x] `StorageSessionStore` 实现 `SessionStore` 三方法，失败路径带稳定原因码（`session_already_exists` / `session_not_found` / `revision_conflict` / `persisted_row_invalid` / `snapshot_not_persistable` / `session_store_failure` / `session_store_unavailable`）。
- [x] 生产装配两处不再注入 `MemorySessionStore`；`with_storage_session_store()` 显式声明存储后端（注入 store 仍优先，既有测试路径不变）。
- [x] 跨重开测试逐字段一致（含分支消息 `parent_id`、`status`、`ended_at`、`revision`）。
- [x] 负向四类显式失败，原始输出见 §3。
- [x] 装配级证据：`HostAssembly` + `with_storage_session_store()` 建会话 → `shutdown()` → 重开同一 data root → `load_session` 读回。
- [x] `PL-108` 在 `docs/PARKING_LOT.md` 标为已闭环；全套门禁绿。

### 5. 偏差

无卡面偏差。一处**观察**（非漂移）：`conversations` / `conversation_messages` 的 CHECK 约束会先挡下 `status` / `role` / `retention` / `revision` / `sequence`（负值）/ `token_estimate`（负值）等篡改，故「适配器自层」的复核用 schema 故意留空的 `goal` / `content` 与 schema 允许但 Core 无法表示的 `sequence = 4294967296` 取证，避免只测到 SQLite 而漏测适配器。未改任何断言以迁就实现。

### 6. 更合理做法

无色。适配器刻意不缓存、不加索引、不碰 schema，只调用 TASK-230 交付的四个记录原语；锁随单次调用作用域释放（ADR-0063）。

### 7. 遗留问题

无。`PL-108` 已闭环；`SessionStore` trait 与 `crates/core`、`crates/storage` 公共形状未改。

### 8. 新增长期记忆

无新增 FACT / PITFALL（本卡为 TASK-230 记录 API 的直接接线，无新跨应用坑；`MEMORY.md` 规模表无需变动）。

### 9. 给审阅者的关注点

1. `update_session` 的 revision 自查是**双保险**：适配器先查「恰好 +1」，`replace_conversation_snapshot` 在存储层再查一次，两处都不放行跳号。
2. `select_session_store()` 的优先级：显式注入 > storage-backed > `MissingComponent`；生产两处已切到开关，既有注入 `MemorySessionStore` 的测试路径不受影响（`test_host_assembly_rejects_missing_component` 仍按缺失报错）。
3. 篡改行证据刻意挑了 schema 不拦的字段，确保「适配器自层」真的被测到，而不是被 SQLite CHECK 掩盖。
