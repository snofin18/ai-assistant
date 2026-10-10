# TASK-260　task-engine 契约测试覆盖率加固（章程 §5.3）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/task-engine/tests/**`、`tasks/TASK-260-task-engine-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-10-round-4.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/task-engine/README.md`

**目标**

仅为 `crates/task-engine` 增加白盒契约测试，把该 crate 的行覆盖率提升到 ≥90%，重点覆盖
checkpoint store 的有界错误语义、内存与 SQLite 适配器的一致性、恢复评估的全部
fail-closed 分支，以及 snapshot / Plan 对齐校验；不修改生产逻辑。

**背景**

自动化运行日 2026-10-10 round 4 的实测基线为 `assistant-task-engine` 行覆盖率
86.78%（1354 行中覆盖 1175 行）。它虽已高于章程 §5.3 的 85% 门槛，但
`checkpoint.rs`（76.44%）与 `recovery.rs`（80.12%）仍是该 crate 最薄弱的持久化与
崩溃恢复面，且当前 Ready 队列均需真实 GUI 或人工安全设计判断。因此按 §2 P2
执行可机器验证的测试强化，为后续真实恢复写入建立更厚的回归网。

**步骤**

1. 新增 `crates/task-engine/tests/coverage_contract.rs`，只调用现有公共 API。
2. 覆盖 `MemoryCheckpointStore` 的 create / duplicate / save-missing /
   monotonic-revision / load-missing 契约。
3. 用同一套 snapshot 输入对照 `MemoryCheckpointStore` 与
   `SqliteCheckpointStore`，覆盖真实关闭并重开后的持久化一致性。
4. 覆盖 `assess_recovery` 的 safe reset、completed、not-completed、unknown、
   missing evidence、取消请求、阻塞 step、告警完成与时钟回退分支。
5. 覆盖 `TaskSnapshot::validate` 的 schema、plan/task、step 对齐、重复 step 与
   current-step 校验，以及公开纯函数边界。
6. 复测 `cargo llvm-cov -p assistant-task-engine --tests`，确认 crate 行覆盖率
   ≥90%，再跑全局门禁。

**DoD**

- [ ] 生产代码 `crates/task-engine/src/**` 零改动
- [ ] 新增测试覆盖上述六类契约，正常 / 边界 / 错误路径均存在
- [ ] `cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 90` 通过
- [ ] `cargo test -p assistant-task-engine` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 90
cargo test -p assistant-task-engine
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

【任务】TASK-260 task-engine 覆盖率加固
【目标】仅新增契约测试，将 `crates/task-engine` 行覆盖率提升到 ≥90%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改 `src/**`、契约、依赖、CI/lint；操作真实 GUI/网络
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/task-engine/tests/coverage_contract.rs`（新增，仅测试）
- `tasks/TASK-260-task-engine-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-10-round-4.md`

### 3. 验收输出摘要

- 改动前 `cargo llvm-cov -p assistant-task-engine --tests --summary-only`：总行 1354 /
  覆盖 1175 / **86.78%**。
- `cargo test -p assistant-task-engine --test coverage_contract`：9 passed / 0 failed。
- `cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 90`：EXIT 0；
  总行 1354 / 覆盖 1224 / **90.40%**。
- `cargo test -p assistant-task-engine`：52 integration tests + 1 doctest，全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace --quiet`：EXIT 0；workspace 门禁全绿，xtask 469 passed。
- `cargo deny check`：`advisories ok, bans ok, licenses ok, sources ok`，EXIT 0。
- `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger /
  check-comments / verify-schemas / check-migrations / codegen --check / replay --suite core`：
  全部 exit 0；`hygiene` 0E/121W，`docscan` 0E/264W，`card-check` 0E/34W，
  `check-comments` 0E/73W，`replay` 2 step / 5 change / 0 mismatch。
- PR / CI / merge hash：待本轮 PR 收口。

### 4. DoD 逐条核对

- 生产源码零改动：满足，`crates/task-engine/src/**` 未改。
- 正常 / 边界 / 错误路径覆盖：满足，9 个契约测试覆盖 checkpoint store、SQLite 损坏 /
  重开、恢复全部分支与 snapshot 校验。
- crate 行覆盖率 ≥90%：满足，实测 90.40%。
- `cargo test -p assistant-task-engine` 与全 workspace 门禁：满足。
- 热点文件 guard：满足，六个热点文件先 acquire 后编辑并 release。
- 最终状态：**Review**；PR / CI / merge hash 待回填。

### 5. 偏差

- 非宿主 target clippy 对 `assistant-task-engine` 不适用：该 crate 依赖
  `assistant-storage` 的 SQLite C 构建链，ADR-0045 只允许对无 C 依赖的 pure Rust crate
  执行；因此不做 `--target` clippy，交由 CI 的三平台矩阵覆盖。
- 本轮多次只读/查询命令误用 `shell=bash` 并立即 exit 127；随后均改用
  `shell=powershell` 重跑。失败发生在写入前，未污染工作区，也未把未运行命令记作已通过。
- 单卡 diff 预计超过章程 §6 的 400 行审阅预算：测试文件约 330 行，另有卡片、状态、
  轮次留痕与内存条目。未拆轮的原因是这些测试共同覆盖同一组 checkpoint / recovery
  覆盖率缺口，拆开会使 `task-engine` 停留在 90% 以下。

### 6. 更合理做法

- 新增测试按“checkpoint store / SQLite / recovery / snapshot validation”分组，
  比继续堆入既有测试文件更易审阅，也避免触碰生产源码。

### 7. 遗留问题

- TASK-256 / TASK-257 / TASK-044 的真实 Paint GUI 验收仍由有人在场时执行。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-task-engine` 契约覆盖率从 86.78% 提升到 90.40%；
  同步更新 `MEMORY.md` 规模表并让 `memory-counts` 恢复 0E/0W。

### 9. 给审阅者的关注点

1. 新增文件是否仅调用现有公共 API，且没有改生产源码或放宽断言。
2. SQLite corruption / task-id mismatch 是否都保持显式 `Serialization` 失败。
3. 覆盖率数字是否由 `cargo llvm-cov -p assistant-task-engine --tests` 在同一提交上实跑得到。
