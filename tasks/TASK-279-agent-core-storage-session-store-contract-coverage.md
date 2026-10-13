# TASK-279　agent-core storage-session-store 公共契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/tests/session_store_persistence.rs`、`tasks/TASK-279-agent-core-storage-session-store-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-5.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`docs/PARKING_LOT.md` PL-108、`crates/core/src/store.rs`、`crates/storage/src/conversations.rs`

**目标**

仅为 `assistant-agent-core` 的存储型 `SessionStore` 适配器（PL-108）增加公开契约测试，把
`apps/agent-core/src/storage_session_store.rs` 的行覆盖率从 76.19% 提升到 ≥80%（crate 独立行覆盖率保持
≥75%）。重点覆盖 `Ended` 状态与全部 role / retention 变体的往返、不可持久化的 revision / token
溢出，以及数据库互斥锁中毒的失败关闭。不修改生产逻辑，不新增依赖。

**背景**

2026-10-13 round 5 的 `storage_session_store.rs` 行覆盖率为 76.19%。既有
`tests/session_store_persistence.rs`（TASK-233）覆盖了 `Active` 往返、重复插入、revision 跳号、
缺失会话与篡改行的失败关闭，但没有覆盖 `Ended` 状态、`System` / `Tool` role 与
`Summarizable` / `Droppable` retention 的往返、revision / token 溢出与锁中毒。候选来自
`docs/automations/2026-10-12-report.md` §③（`apps/agent-core` 按模块拆卡）、按 §2 P2 派单。

**步骤**

1. 在既有 `apps/agent-core/tests/session_store_persistence.rs` 中追加用例（复用其临时库 setup）。
2. 覆盖 `Ended` 状态 + `System` / `User` / `Assistant` / `Tool` role + 三种 retention 的往返。
3. 覆盖 revision 与 token estimate 超出 `i64` 列时的 `snapshot_not_persistable`。
4. 覆盖数据库互斥锁中毒时的 `session_store_unavailable`。
5. 复测 `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅改测试文件）
- [ ] 新增测试只调用公开 API / 组装层入口，覆盖上述契约
- [ ] `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 通过，且 `storage_session_store.rs` 行覆盖率 ≥80%
- [ ] `cargo test -p assistant-agent-core` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75
cargo test -p assistant-agent-core
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-279 agent-core storage-session-store 公共契约覆盖率补齐
【目标】追加公开契约测试，把 `storage_session_store.rs` 行覆盖率从 76.19% 提升到 ≥80%
【write scope】仅：`apps/agent-core/tests/session_store_persistence.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实 GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 模块覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 5（**机制 = human**）
【疑问】无

### 2. 实际改动文件

- `apps/agent-core/tests/session_store_persistence.rs`（追加 4 个公开契约测试）
- `tasks/TASK-279-agent-core-storage-session-store-contract-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-5.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前独立实测：`apps/agent-core` 行覆盖率 76.02%，`storage_session_store.rs` 76.19%（region 76.49%）。
- `cargo test -p assistant-agent-core --test session_store_persistence`：7 passed（3 既有 + 4 新增）。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75`：EXIT 0；
  crate 行覆盖率 **76.28%**；`storage_session_store.rs` 行覆盖率 **84.92%**（region 84.59%）。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=409 / 0E / 128W / PASSED，`docscan` 0E/264W，
  `memory-counts`、`adr-index`、`check-ledger`、`card-check`、`refscan`(0E/0W)、
  `check-comments`、`verify-schemas`、`check-migrations` 全部 PASSED。
- PR #356；pull_request CI = **11/11 SUCCESS**；合并前 base=`main`、mergeable=`MERGEABLE`、
  mergeStateStatus=`CLEAN`；merge `6586821`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅追加 `apps/agent-core/tests/session_store_persistence.rs` 用例。
- 新增测试覆盖契约：满足；Ended + 全 role / retention 往返、溢出、锁中毒均断言精确 reason code。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 且模块 ≥80%：
  满足，crate 76.28%、模块 84.92%。
- `cargo test -p assistant-agent-core` 全绿：满足。
- `cargo fmt --all --check` / `clippy --all-targets -- -D warnings` / `test --workspace` 全绿：满足。
- 全部现行 xtask 硬门禁 PASSED：满足。
- 热点文件 guard：满足；`LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`（收口）与
  `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（回填）先 acquire 后 release。

### 5. 偏差

- 无。

### 6. 更合理做法

- 复用 TASK-233 的临时库 setup（`open_handle` / `TestDirectory`），把新用例追加进同一文件，
  避免重复 60 行夹具；同时只碰测试文件，生产源码零改动。

### 7. 遗留问题

- `storage_session_store.rs` 剩余未覆盖行主要是**schema 已约束**的字段（status / role / retention /
  revision 符号 / conversation_id 归属）在适配器内的第二层守卫；这些值写不进 SQLite，公开 API 不可达。
- `ui_server.rs`（67.20%）仍低，属 UI 传输层，需后续卡按模块单独评估。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `storage_session_store.rs` 行覆盖率 76.19% → 84.92%、crate 76.02% → 76.28%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API / 组装层入口，未改变生产控制流。
2. `Ended` + 全 role / retention 往返、溢出与锁中毒是否都断言了精确 reason code。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-agent-core` 实跑得到。
