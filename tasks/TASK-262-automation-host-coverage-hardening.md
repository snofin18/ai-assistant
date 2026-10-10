# TASK-262　automation-host 契约测试覆盖率加固（章程 §5.3）

- 状态：**Done**（2026-10-11；PR #300 / merge `81f196a`）
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/automation-host/src/main.rs`（仅 `#[cfg(test)]` 模块）、`apps/automation-host/src/lib.rs`（仅 `#[cfg(test)]` 模块）、`tasks/TASK-262-automation-host-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-1.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`apps/automation-host/Cargo.toml`

**目标**

仅为 `assistant-automation-host` 增加白盒契约测试，把该 crate 的行覆盖率提升到 ≥75%，
重点覆盖命令行参数提取、会话心跳 / 超时 / 非请求消息分支，以及已知配置错误路径；
不修改生产逻辑，不启动真实 NamedPipe 或任何 GUI。

**背景**

自动化运行日 2026-10-11 round 1 的独立实测基线为 `assistant-automation-host`
行覆盖率 69.05%（420 行中覆盖 290 行）；其中 `main.rs` 24 行完全未覆盖，
`run_session` 的心跳与超时分支也未执行。当前 Ready 队列均为真实 GUI 或人工安全
设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 在 `main.rs` 的 `#[cfg(test)]` 模块覆盖 `argument_value` 的命中 / 缺失 / 相邻选项边界。
2. 在 `lib.rs` 的既有测试模块覆盖协商心跳、错误会话心跳、非请求消息与超时后断开。
3. 覆盖配置解析的未知参数、空 `--allow-peer`、零心跳超时等错误路径。
4. 复测 `cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 75`。
5. 跑 `cargo test -p assistant-automation-host` 与全局硬门禁。

**DoD**

- [ ] 测试代码之外的生产逻辑零改动（仅新增 `#[cfg(test)]` 模块）
- [ ] 新增测试覆盖参数边界、会话心跳 / 超时 / 非请求消息与配置错误路径
- [ ] `cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 75` 通过
- [ ] `cargo test -p assistant-automation-host` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 75
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

【任务】TASK-262 automation-host 覆盖率加固
【目标】仅新增 cfg(test) 测试，把 `apps/automation-host` 行覆盖率提升到 ≥75%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；启动真实 NamedPipe / GUI / 网络
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `apps/automation-host/src/main.rs`（仅测试模块）
- `apps/automation-host/src/lib.rs`（仅测试模块）
- `tasks/TASK-262-automation-host-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-11-round-1.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-automation-host` 行覆盖率 69.05%（420 行中覆盖 290 行）。
- `cargo llvm-cov -p assistant-automation-host --tests --fail-under-lines 75`：EXIT 0；
  578 行中覆盖 482 行，行覆盖率 **83.39%**；`lib.rs` 84.40%、`main.rs` 71.74%。
- `cargo test -p assistant-automation-host`：14 个 lib 测试 + 2 个 main 测试 + 1 个
  既有 acceptance 测试全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- PR / CI / merge hash：PR #300；pull_request CI run `38081164145` = 11/11 SUCCESS；
  merge `81f196a`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；新增内容仅在 `#[cfg(test)]` 模块，未改生产分支。
- 参数边界 / 会话分支 / 配置错误路径：满足；覆盖精确选项匹配、缺失值、未知参数、
  空 peer、零超时、非数字超时、协商心跳、错误会话心跳、非请求消息与接收超时。
- crate 行覆盖率 ≥75%：满足，独立 `cargo llvm-cov` 实测 83.39%。
- `cargo test -p assistant-automation-host` 与全 workspace 门禁：满足。
- 热点文件 guard：满足，六个热点文件先 acquire 后编辑并 release。
- 最终状态：Done；PR #300 / CI `38081164145` / merge `81f196a` 已回填。

### 5. 偏差

- 选卡过程中先尝试 `assistant-ipc` 与 `assistant-secrets` 的独立覆盖率门槛；
  IPC 受 Windows 传输模块拖累，secrets 受真实 keychain 不可达路径拖累。两者都在
  写入前完整回退，最终选择可在一轮内达到 75% 的 `assistant-automation-host`。
- `apps/automation-host/src/lib.rs` 测试模块使其跨过 600 行软阈值，`hygiene`
  warning 从 121 变为 122；0 Error、verdict 仍为 PASSED。

### 6. 更合理做法

- 测试放在既有 `lib.rs` 测试模块内，直接复用 `ScriptedTransport`，避免另造跨模块
  fixture；`main.rs` 只测试私有 `argument_value` 的纯参数提取边界。

### 7. 遗留问题

- 真实 NamedPipe / GUI 验收仍由既有的有人值守靶机流程承接；本轮没有启动任何真实应用。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-automation-host` 独立行覆盖率从 69.05%
  提升到 83.39%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部位于 `#[cfg(test)]` 模块，且没有改变生产控制流。
2. 心跳、超时、错误会话和非请求消息是否都断言了显式失败，而不是只检查“没有 panic”。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-automation-host` 实跑得到。
