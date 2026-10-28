# TASK-337　Browser navigate postcondition deadline 对齐

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-107、`DRIFT-056-9`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`apps/agent-core/src/task_package_render.rs`、
  `apps/agent-core/tests/task_package.rs`、`crates/verify/tests/postcondition_evaluation.rs`

## 目标

将 Edge/Chrome `cdp.navigate` 的 `state_changed` postcondition deadline 从固定
3000 ms 调整为现有 20000 ms step execute timeout，使真实外部页面只要在允许的
导航执行时间内完成状态变化就能通过验证。

## 必须遵守

1. browser navigate 的 deadline 直接复用 `STEP_EXECUTE_TIMEOUT_MS`，不新增重复常量。
2. 仅调整 browser navigate；`dom_read` / `wait_network_idle` 仍为 `state_unchanged`。
3. 增加 20000 ms 边界正负测试：小于等于 deadline 通过，超过 deadline 失败。
4. 不改 Tool schema、公共 trait、ErrorCode、依赖、Policy、CSV 契约或 TASK-056 断言内容。

## Out of scope

- 本卡不执行外部 `books.toscrape.com` smoke；修复合并后由 TASK-056 重跑并记录。
- 不改变 `state_changed` 的通用 verifier 实现或 Observation 语义。
- 不放宽本地 10/10、CSV validator、Step approval 或任何其他 postcondition。

## 验收命令

```powershell
cargo test -p assistant-agent-core --test task_package
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

- [ ] Edge/Chrome navigate 渲染出的 `within_ms` 等于 20000。
- [ ] 19999 ms 的慢导航状态变化通过，20001 ms 被显式 falsify。
- [ ] `dom_read` / `wait_network_idle` 断言保持 `state_unchanged`。
- [ ] 上述验收命令全绿；TASK-056 外部 3/3 在修复合并后另行执行。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-337 Browser navigate postcondition deadline 对齐
【目标】将 browser navigate state_changed deadline 从 3000 ms 改为 20000 ms
【write scope】仅：task_package_render、task_package/verify 测试、TASK-337 与状态同步
【铁律】1 无静默失败 / 4 每步有 postcondition / 9 不扩范围 / 10 契约先行
【禁止】不改公共 schema/API/ErrorCode/依赖/Policy；不改 dom_read 与其他断言
【验收】mapping=20000、慢导航正负边界、workspace 与全量门禁 → 全绿
【依赖】TASK-107、TASK-056 人工验收；DRIFT-056-9 已获用户批准
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/tests/task_package.rs`
- `crates/verify/tests/postcondition_evaluation.rs`
- `tasks/TASK-337-browser-navigate-postcondition-deadline.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/facts.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --test task_package` → **23/23 passed**；
  Edge/Chrome navigate 均渲染 `within_ms=20000`，dom.read / network-idle 仍为
  `state_unchanged`。
- `cargo test -p assistant-verify --test postcondition_evaluation`
  → **29/29 passed**；19999 ms 慢导航满足，20001 ms 显式 falsify。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint warning。
- `cargo test --workspace` → exit 0，全部测试套件 failed=0。
- `cargo test -p assistant-core arch::` → 10/10 passed。
- `verify-schemas` / `codegen --check` → 0 error / 0 drift。
- `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
  `card-check` / `check-ledger` / `check-comments` / `check-migrations`
  → 全部 PASSED；仅既有 warning。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。

### 4. DoD 逐条核对

- [x] Edge/Chrome navigate 渲染出的 `within_ms` 等于 20000。
- [x] 19999 ms 慢导航通过，20001 ms 显式失败。
- [x] `dom_read` / `wait_network_idle` 保持 `state_unchanged`。
- [x] 验收命令全绿；TASK-056 外部 3/3 在合并后另行执行。

### 5. 偏差

- 无新 DRIFT。`DRIFT-056-9` 的目标 deadline 差异已按用户批准修复。

### 6. 更合理做法

- browser navigate 的 postcondition 窗口直接复用 `STEP_EXECUTE_TIMEOUT_MS`。
  已成功的导航不应只因耗时超过旧 3 秒常量而在后置验证阶段被反向判失败。

### 7. 遗留问题

- TASK-337 合并后回到 TASK-056分支，重跑外部 `books.toscrape.com` 3 次；
  每轮仍必须三页、60 行并通过严格 CSV validator。

### 8. 新增长期记忆

- FACT：browser navigate 的 `state_changed` 窗口与 step execute timeout 对齐为
  20000 ms；真实外部页面首次暴露出旧 3000 ms 会误杀 4514 ms 的成功导航。

### 9. 给审阅者的关注点

- 重点确认只改了 browser navigate 的 deadline，没有改通用 verifier、dom.read、
  state_unchanged、Tool schema 或策略边界。
- 本卡没有执行真实外部 smoke；合并后由 TASK-056 继续并记录。
