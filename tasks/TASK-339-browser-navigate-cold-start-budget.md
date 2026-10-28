# TASK-339　Browser navigate 冷启动预算

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-337、TASK-338、`DRIFT-056-11`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`apps/agent-core/src/task_package_render.rs`、
  `apps/agent-core/src/browser_task_runtime.rs`、
  `apps/agent-core/src/browser_task_runtime_tests.rs`、
  `apps/agent-core/tests/task_package.rs`、
  `crates/verify/tests/postcondition_evaluation.rs`、
  `eval/tasks/edge/t5.1/run_real_edge_acceptance.ps1`

## 目标

让 Edge/Chrome `cdp.navigate` 的 execute 与 `state_changed` 窗口覆盖真实冷启动
页面 load 耗时，同时保持所有非 navigate 步骤的 20 秒预算不变。

## 必须遵守

1. 仅 Edge/Chrome `cdp.navigate` 使用 60000 ms execute 与 postcondition 窗口。
2. `dom_read`、`wait_network_idle` 和其他步骤继续使用 20000 ms execute。
3. 浏览器任务 Plan 预算与验收脚本 session timeout 同步为 180000 ms；
   外部任务包元数据由 TASK-056 在其分支同步。
4. 补 59999 ms 通过、60001 ms 失败边界测试，并断言 navigate / 非 navigate timeouts。
5. 不删除断言、不改 load 语义、不使用 warm-cache，不放宽 CSV 或字段完整性判据。

## Out of scope

- 不执行真实外部 3/3；修复合并后由 TASK-056 重跑。
- 不改 Tool schema、公共 API、ErrorCode、依赖、Policy 或 CDP transport。
- 不调整通用 production Plan 预算；只改私有 browser task runner。

## 验收命令

```powershell
cargo test -p assistant-agent-core --test task_package
cargo test -p assistant-agent-core browser_task_runtime
cargo test -p assistant-verify --test postcondition_evaluation
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- check-migrations
cargo deny check
```

## DoD

- [ ] navigate 渲染 `execute_ms=60000`，其他步骤 `execute_ms=20000`。
- [ ] navigate postcondition `within_ms=60000`，其他 browser postcondition 不变。
- [ ] 59999 ms 通过、60001 ms 显式失败。
- [ ] browser runner 与 acceptance script 使用 180000 ms 总预算。
- [ ] 上述验收命令全绿；真实外部 3/3 留给 TASK-056。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-339 Browser navigate 冷启动预算
【目标】navigate 使用 60 秒执行/验证窗口，其他步骤保持 20 秒
【write scope】renderer / browser runtime / 对应测试 / eval 脚本 / external package / 状态同步
【铁律】1 无静默失败 / 4 可验证 postcondition / 8 有界资源 / 9 不扩范围 / 10 契约先行
【禁止】不删断言、不改 load 语义、不用 warm-cache、不改公共 schema/API/依赖/Policy
【验收】59999/60001 边界、navigate 与 dom_read timeout 分离、workspace 与全量门禁 → 全绿
【依赖】TASK-337/TASK-338、DRIFT-056-11、用户批准
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/src/browser_task_runtime.rs`
- `apps/agent-core/src/browser_task_runtime_tests.rs`
- `apps/agent-core/tests/task_package.rs`
- `crates/verify/tests/postcondition_evaluation.rs`
- `eval/tasks/edge/t5.1/run_real_edge_acceptance.ps1`
- `tasks/TASK-339-browser-navigate-cold-start-budget.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/pitfalls.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --test task_package` → **23/23 passed**；
  Edge/Chrome navigate 为 60000 ms，dom_read / wait_network_idle 为 20000 ms。
- `cargo test -p assistant-agent-core browser_task_runtime` → **24 passed**；
  browser Plan 总预算 180000 ms 与六步执行路径通过。
- `cargo test -p assistant-verify --test postcondition_evaluation`
  → **29/29 passed**；59999 ms 通过、60001 ms 显式 falsify。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint warning。
- `cargo test --workspace` → **497 passed / 0 failed**，doc-tests 全绿。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `verify-schemas` → 5/5；`codegen --check` → 0 drift / 0 error。
- `hygiene` → 0E / 151W；`memory-counts` → 0E / 1W；
  `adr-index` / `refscan` / `check-ledger` / `check-migrations` → 0E。
- `docscan` → 0E / 175W；`card-check` → 0E / 35W；
  `check-comments` → 0E / 75W。
- `cargo deny check` → advisories / bans / licenses / sources all ok。

### 4. DoD 逐条核对

- [x] navigate 渲染 `execute_ms=60000`，其他步骤 `execute_ms=20000`。
- [x] navigate postcondition `within_ms=60000`，其他 browser postcondition 不变。
- [x] 59999 ms 通过、60001 ms 显式失败。
- [x] browser runner 与 acceptance script 使用 180000 ms 总预算。
- [x] 验收命令全绿；真实外部 3/3 留给 TASK-056。

### 5. 偏差

- 无功能漂移。当前 rollout shell misuse 机器审计更新为 **158** 次；
  本卡新增 10 次均为只读查询或格式化/门禁命令的 shell 参数误填，在执行前即因
  command-not-found 或语法错误失败，未写产品文件；均已用 PowerShell 重跑。

### 6. 更合理做法

- 导航是唯一会等待远端页面 `load` 的 browser 步骤，应与本地 DOM 读取分离预算；
  60000 ms 单步窗口和 180000 ms 总窗口为三页冷启动留出有界余量。

### 7. 遗留问题

- TASK-339 合并后，TASK-056 同步 external package 的 `time_budget_ms=180000`，
  使用全新 profile 重跑 `books.toscrape.com` 3/3。

### 8. 新增长期记忆

- PITFALL：真实外部页面冷启动 `load` 可超过 20 秒；browser navigate 必须使用
  与通用本地步骤不同的有界执行和 postcondition 预算。

### 9. 给审阅者的关注点

- 确认只有 Edge/Chrome `cdp.navigate` 使用 60000 ms；所有其他步骤仍为 20000 ms。
- 确认 60001 ms 仍 fail-closed，没有删除或放宽通用 verifier。
- 确认总预算 180000 ms 仍为有界值，且没有修改公共 schema/API/依赖/Policy。
