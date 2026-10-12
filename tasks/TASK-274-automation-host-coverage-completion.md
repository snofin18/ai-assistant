# TASK-274　automation-host 契约测试覆盖率补齐（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/automation-host/tests/coverage_contract.rs`、`tasks/TASK-274-automation-host-coverage-completion.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-12-round-2.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`apps/automation-host/README.md`、`docs/spec/testing.md`

**目标**

仅为 `assistant-automation-host` 增加公开 API 集成契约测试，把该 crate 的独立行覆盖率从
83.39% 提升到 ≥85%；本轮重点覆盖 `HostConfig::parse` 的可选 status-file 参数，
以及 `write_status` 的成功、错误文本规范化与文件写入失败契约。
不修改生产逻辑，不启动真实 NamedPipe、GUI、网络或读取凭据。

**背景**

2026-10-12 round 2 的独立 crate 基线为行覆盖率 83.39%（578 行中覆盖 482 行）。
TASK-262 已把该 crate 从 69.05% 提升到 83.39%，但尚未达到质量门槛；
未覆盖面主要剩真实 NamedPipe 启动路径和状态文件写入契约，后者可由临时文件测试
在不接触真实应用、网络或凭据的情况下机器验证。
当前 Ready 队列仍为真实 GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `apps/automation-host/tests/coverage_contract.rs`。
2. 覆盖 `HostConfig::parse` 的 `--status-file` 分支与解析后字段保留。
3. 覆盖 `write_status` 的成功输出、错误文本换行规范化与写入失败显式错误。
4. 复测 `cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 85`。
5. 跑 crate 测试与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试覆盖 status-file 解析与 `write_status` 成功/失败契约
- [ ] `cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-automation-host` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 85
cargo test -p assistant-automation-host
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

【任务】TASK-274 automation-host 契约测试覆盖率补齐
【目标】仅新增集成契约测试，把 `assistant-automation-host` 独立行覆盖率从 83.39% 提升到 ≥85%
【write scope】仅：新建 `apps/automation-host/tests/coverage_contract.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实 NamedPipe / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `apps/automation-host/tests/coverage_contract.rs`（新增 3 个公开契约测试）
- `tasks/TASK-274-automation-host-coverage-completion.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `docs/automations/2026-10-12-round-2.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-automation-host` 行覆盖率 83.39%
  （578 行中覆盖 482 行）。
- `cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 85`：
  EXIT 0；578 行中覆盖 501 行，行覆盖率 **86.68%**；`lib.rs` 87.97%、
  `main.rs` 71.74%。
- `cargo test -p assistant-automation-host`：14 个 lib 测试 + 2 个 main 测试 +
  1 个既有 acceptance 测试 + 3 个新增 coverage_contract 测试全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`、`cargo test -p assistant-core arch::`：EXIT 0。
- xtask：`hygiene` scanned=405 / 0E / 128W / PASSED，`memory-counts`、
  `adr-index`、`refscan`、`docscan`、`card-check`、`check-ledger`、
  `check-comments`、`verify-schemas`、`codegen --check`、`check-migrations`
  全部 PASSED。
- `cargo deny check`、`cargo run -p xtask -- replay --suite core`、双平台
  纯 Rust clippy 全部 PASSED。
- PR / CI / merge hash：待本轮 PR 收口后回填。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增集成测试文件。
- status-file 解析与 `write_status` 成功 / 失败契约：满足；分别覆盖成功文本、
  Display 换行清洗、目录写入失败与缺失 token 在 NamedPipe 创建前失败。
- crate 独立行覆盖率 ≥85%：满足，实测 86.68%。
- `cargo test -p assistant-automation-host` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足。
- 热点文件 guard：满足；状态同步前 acquire，编辑后 release。
- PR / CI / merge hash：待回填。

### 5. 偏差

- 本会话误用 WSL bash 13 次；均为只读查询、覆盖率命令或帮助命令，
  返回 `command not found` / shell 语法错误，无文件或锁影响；随后均用
  PowerShell 重跑。
- 第一版 `write_status` 断言未包含 `IpcError::Transport` 的 Display 前缀，
  测试真实失败；按公开错误展示契约修正为
  `IPC transport failure: line one line two` 后通过。

### 6. 更合理做法

- 新增集成测试单独放在 `tests/coverage_contract.rs`，避免像 TASK-262 那样
  继续扩大 `lib.rs` 内 `#[cfg(test)]` 模块的行数与覆盖分母。

### 7. 遗留问题

- 真实 NamedPipe 与真实 peer 身份路径仍由既有 acceptance / 有人值守靶机流程
  承接；本轮未启动真实 NamedPipe、GUI、网络或读取凭据。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-automation-host` 独立行覆盖率从
  83.39% 提升到 86.68%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，未改变生产控制流或公共接口。
2. `write_status` 的成功、错误展示与写入失败是否都断言了精确可观察结果。
3. 覆盖率数字是否由同一提交上的独立
   `cargo llvm-cov -p assistant-automation-host` 实跑得到。
