# TASK-268　assistant-storage 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：M　**难度**：M
- **write scope**：`crates/storage/tests/coverage_contract.rs`、`tasks/TASK-268-storage-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-6.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/storage/README.md`

**目标**

仅为 `assistant-storage` 增加公共契约测试，把该 crate 的独立行覆盖率从 78.43% 提升到 ≥85%；
重点覆盖迁移注册表错误契约、会话 / 消息树字段校验、blob 未登记与引用幂等、记忆记录删除与查询
fail-closed、缺失记录读取和系统时钟。不修改生产逻辑，不读取真实桌面，不启动任何 GUI。

**背景**

2026-10-11 round 6 的独立实测基线为 `assistant-storage` 行覆盖率 78.43%
（1595 行中覆盖 1251 行）；`conversation_messages.rs` 68.16%、`conversations.rs` 70.37%、
`content.rs` 76.25%、`memory.rs` 76.68%、`migrations.rs` 74.49%、`time_source.rs` 0.00%。
当前 Ready 队列均为真实 GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/storage/tests/coverage_contract.rs`，复用现有 `tests/common` 临时目录夹具。
2. 覆盖 `MigrationSet` 的非法版本、重复版本、缺号、插入排序与迭代契约。
3. 覆盖会话、消息树、记忆记录的字段边界与 fail-closed 错误码。
4. 覆盖 blob 未登记、引用删除幂等、GC 缺文件与路径视图。
5. 覆盖缺失任务 / 步骤 / 检查点、用量计数、系统时钟和数据库视图。
6. 复测 `cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85`。
7. 跑 `cargo test -p assistant-storage` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试覆盖迁移注册、会话 / 消息树 / 记忆 / blob / 记录读取契约
- [ ] `cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-storage` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85
cargo test -p assistant-storage
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

【任务】TASK-268 assistant-storage 契约测试覆盖率加固
【目标】仅新增测试，把 `assistant-storage` 独立行覆盖率从 78.43% 提升到 ≥85%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/storage/tests/coverage_contract.rs`
- `tasks/TASK-268-storage-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`
- `docs/automations/2026-10-11-round-6.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-storage` 行覆盖率 78.43%（1595 行中覆盖 1251 行）。
- `cargo llvm-cov -p assistant-storage --tests --fail-under-lines 85`：EXIT 0；
  1595 行中覆盖 1419 行，行覆盖率 **88.97%**；`conversation_messages.rs` 94.53%、
  `conversations.rs` 92.18%、`memory.rs` 86.56%、`migrations.rs` 89.80%、
  `time_source.rs` 100.00%、`content.rs` 78.26%。
- `cargo test -p assistant-storage`：44 个已有集成测试、9 个新增契约测试与 2 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0，workspace 469 passed / 0 failed。
- `hygiene` scanned=401 / 0E / 127W / PASSED；相较基线新增 1 个
  `file-too-long` 软警告（新测试文件 608 行），0 Error。
- `memory-counts`、`adr-index`、`refscan`、`docscan`、`card-check`、`check-ledger`、
  `check-comments`、`verify-schemas`、`codegen --check`、`check-migrations` 全部 PASSED。
- `cargo deny check`、`cargo test -p assistant-core arch::`、`replay --suite core` 全绿。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 storage 集成测试文件。
- 迁移注册、会话 / 消息树 / 记忆 / blob / 记录读取契约：满足；9 个测试覆盖成功与失败路径。
- crate 独立行覆盖率 ≥85%：满足，实测 88.97%。
- `cargo test -p assistant-storage` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足，见本卡验收输出。
- 热点文件 guard：满足，六个公共热点文件均先 acquire 后编辑。
- PR / CI / merge hash：待本轮 PR 与合并回填后补记。

### 5. 偏差

- PowerShell 在本机再次零输出卡住；已按既有逃生路径改用 `cmd.exe` 执行
  git/cargo/gh、WSL 执行文本读取，并记录到本轮报告。
- 新测试文件 608 行，超过 600 行软阈值 8 行；`hygiene` 因此新增 1 条 Warning，
  0 Error。覆盖率测试保持单文件便于集中审阅，未为消除 Warning 扩大 write scope。
- 首次编译使用 `SystemClock::new()` 与未导入 `Clock` trait；已改为 `SystemClock`
  默认值并显式导入 trait。
- clippy 两次指出测试中的 `redundant_clone`；已改为独立变量或移动不再复用的值，
  未使用 `#[allow]`。

### 6. 更合理做法

- 对“错误字段矩阵”类测试，先按生产校验顺序建立 `assert_invalid` helper，再逐项喂入
  单字段变异；这样新增边界时不需要复制整段断言，也能保持字段名与 `StorageError` 契约一致。

### 7. 遗留问题

- PR / CI / merge hash 待本轮 PR 与合并回填后补记。
- 真实 Paint 验收仍由 TASK-256 / TASK-044 的有人值守流程承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-storage` 独立行覆盖率从 78.43% 提升到
  88.97%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，生产源码零改动。
2. 会话 / 消息树失败路径是否真实验证回滚，而非只断言错误码。
3. 1 MiB 级内容边界测试是否会在 CI 上产生不合理资源占用。
