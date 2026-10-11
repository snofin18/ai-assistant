# TASK-269　assistant-model-gateway 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：M　**难度**：M
- **write scope**：`crates/model-gateway/tests/coverage_contract.rs`、`tasks/TASK-269-model-gateway-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-7.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/model-gateway/README.md`

**目标**

仅为 `assistant-model-gateway` 增加公共契约测试，把该 crate 的独立行覆盖率从 75.54% 提升到 ≥85%；
重点覆盖请求与流事件校验、路由条件、重试边界、能力缺失、取消与预算 fail-closed、错误分类和账本聚合。
不修改生产逻辑，不新增依赖，不读取真实网络、凭据或 GUI。

**背景**

2026-10-11 round 7 的独立实测基线为 `assistant-model-gateway` 行覆盖率 75.54%
（1206 行中覆盖 911 行）；`gateway.rs` 72.66%、`model.rs` 60.16%、`retry.rs` 67.35%、
`execution.rs` 64.29%。当前 Ready 队列均为真实 GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/model-gateway/tests/coverage_contract.rs`，复用现有 `tests/common` 测试替身。
2. 覆盖请求、缓存提示、用量、工具调用与流事件的字段边界和错误码。
3. 覆盖路由全部条件、规则与 fallback 重复校验，以及 provider capability 预检。
4. 覆盖构造失败、重试与 jitter 边界、取消、预算、缺失终止事件和重复 usage。
5. 覆盖错误显示、整数溢出、账本空值与聚合上限契约。
6. 复测 `cargo llvm-cov -p assistant-model-gateway --tests --fail-under-lines 85`。
7. 跑 `cargo test -p assistant-model-gateway` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试覆盖请求 / 流事件 / 路由 / capability / retry / cancellation / budget 契约
- [ ] `cargo llvm-cov -p assistant-model-gateway --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-model-gateway` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-model-gateway --tests --fail-under-lines 85
cargo test -p assistant-model-gateway
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

【任务】TASK-269 assistant-model-gateway 契约测试覆盖率加固
【目标】仅新增测试，把 `assistant-model-gateway` 独立行覆盖率从 75.54% 提升到 ≥85%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/model-gateway/tests/coverage_contract.rs`
- `tasks/TASK-269-model-gateway-coverage-hardening.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-model-gateway` 行覆盖率 75.54%（1206 行中覆盖 911 行）。
- `cargo llvm-cov -p assistant-model-gateway --tests --fail-under-lines 85`：EXIT 0；
  1206 行中覆盖 1131 行，行覆盖率 **93.78%**；`gateway.rs` 88.97%、
  `model.rs` 95.22%、`retry.rs` 86.73%、`execution.rs` 100.00%、`router.rs` 100.00%。
- `cargo test -p assistant-model-gateway`：5 个 cost、13 个新增 coverage、3 个 error、
  7 个 provider/router、6 个 retry/cancel 集成测试与 1 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- `hygiene` scanned=402 / 0E / **128W** / PASSED（基线 127W，仅新增 1 条
  `file-too-long` 软警告）；`memory-counts`、`adr-index`、`refscan`、`docscan`、
  `card-check`、`check-ledger`、`check-comments`、`verify-schemas`、
  `codegen --check`、`check-migrations` 全部 PASSED。
- `cargo deny check`、`cargo test -p assistant-core arch::`、
  `cargo run -p xtask -- replay --suite core` 全绿。
- 新增动作为仅新增测试文件；生产源码、依赖、schema、trait、ErrorCode、IPC 与 CI 均未改动。
- PR #316：pull_request CI run `38152902565` = **11/11 SUCCESS**；合并前
  base=`main`、mergeable=`MERGEABLE`、mergeStateStatus=`CLEAN`；
  merge `fc20a44`；post-merge main push CI run `38153403739` = **success**。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 model-gateway 集成测试文件。
- 请求 / 流事件 / 路由 / capability / retry / cancellation / budget 契约：满足；
  13 个新增测试覆盖成功、失败与 fallback 路径。
- crate 独立行覆盖率 ≥85%：满足，实测 93.78%。
- `cargo test -p assistant-model-gateway` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足，见本卡验收输出。
- 热点文件 guard：满足，状态同步前 acquire，编辑后 release。
- PR / CI / merge hash：满足；PR #316 / CI `38152902565` / merge `fc20a44`。

### 5. 偏差

- **DRIFT-269-1（契约与实测不一致，未修）**：README 不变量与 `CapabilityMissing`
  错误路径要求首候选能力不足且无 fallback 时返回 `CapabilityMissing`；实测
  `start_available_attempt` 在 `handle_stream_failure` 已取走 `last_error` 后再次
  `take()`，最终返回 `InternalInvariant`。该路径属于生产源码，超出本卡仅测试
  write scope，本轮未改。建议另立缺陷卡修正，并补一条“无 fallback 的 capability
  missing 保留原错误码”的负向测试。
- PowerShell 在本机再次零输出卡住；已按既有逃生路径改用 `cmd.exe`
  执行 git/cargo/gh、WSL 执行文本读取，并记录到本轮报告。
- 新测试文件 867 行，超过 600 行软阈值但低于 900 行硬阈值；`hygiene`
  新增一条 `file-too-long` Warning，0 Error。
- 本轮实现提交 diff = 1098 insertions，超过章程 §6 的单卡 400 行审阅预算；
  属于 §2 P2 覆盖加固回退路径，已拆为 13 个聚焦测试和 3 个 <80 行 helper，
  未引入生产代码，供审阅者按文件一次性核对。

### 6. 更合理做法

- 用 `gateway_with_events`、`collect_single_error` 与 `route_primary` 小 helper
  表达重复场景，再把失败路径拆成独立测试，既保持每个测试低于 80 行，也让
  覆盖缺口与断言意图一一对应。

### 7. 遗留问题

- PR #316 / CI `38152902565` / merge `fc20a44` 已回填；post-merge main CI
  `38153403739` success。
- `DRIFT-269-1` 需要 Orchestrator 开卡修复；当前只留测试证据与建议。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-model-gateway` 独立行覆盖率从 75.54%
  提升到 93.78%，并记录 `DRIFT-269-1` 的无 fallback 错误码覆盖缺陷。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，生产源码零改动。
2. capability 与 provider 启动失败场景是否通过 fallback 保持 fail-closed。
3. `DRIFT-269-1` 是否应优先于后续覆盖加固处理。
