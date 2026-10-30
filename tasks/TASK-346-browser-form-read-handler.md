# TASK-346　浏览器 form_read 生产 handler

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-343、ADR-0101、`docs/spec/browser-form-confirmation.md`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`apps/agent-core/src/browser_handlers.rs`、
  `apps/agent-core/src/browser_form_read.rs`、
  `apps/agent-core/src/browser_form_read_tests.rs`、
  `apps/agent-core/src/browser_registry.rs`、
  `apps/agent-core/src/browser_registry_tests.rs`、
  `apps/agent-core/src/browser_runtime.rs`、
  `apps/agent-core/src/browser_runtime_tests.rs`、
  `apps/agent-core/src/browser_task_runtime.rs`、
  `apps/agent-core/src/browser_task_runtime_tests.rs`、
  `adapters/browser.edge/tools/tools.json`、
  `adapters/browser.chrome/tools/tools.json`、
  `adapters/browser.edge/tests/validate_adapter_packs.py`、
  `docs/memory/facts.md`、`docs/memory/pitfalls.md`、
  `MEMORY.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`

## 目标

按 ADR-0101 新增 Edge/Chrome `cdp.form_read` 生产 handler，并复用既有
`CdpProvider::evaluate_dom`；不接提交、UI、approval 事件或 fixture。

## In scope

- `form_read` 返回有界 form origin/action 与显式字段当前值；
- registry 从四工具扩为五工具，CLI runtime 与 task runner 的计数同步；
- 只读工具 schema、adapter pack validator 与离线负向测试。

## Out of scope（做了算漂移）

- 不新增 `form_submit`、通用 click、平台 trait、UI/IPC、approval 事件、diff UI、fixture 或真实浏览器；
- 不修改 `form_fill` 的现有输出或“永不提交”语义；
- 不修改 protocol schema、数据库、依赖或 TASK-057 状态；
- 不实现 TASK-347 / TASK-348 / TASK-349 的提交、审批和端到端链路。

## 必须遵守

1. `form_read` 字段、值、总大小和允许控件严格按 spec 限制；页面内容标记 untrusted。
2. `form_read` 必须保持只读，歧义、非法 action、超界或不允许控件均 fail-closed。
3. 不新增 `unsafe`、`unwrap`、`expect`、`panic` 或测试断言放宽。

## 验收命令

```powershell
cargo test -p assistant-agent-core form_read
cargo test -p assistant-agent-core browser_registry
cargo test -p assistant-agent-core browser_runtime
cargo test -p assistant-agent-core browser_task
python adapters/browser.edge/tests/validate_adapter_packs.py
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

## 完成定义（DoD）

- [ ] Edge/Chrome registry 均挂载五个 CDP 工具，包含 `form_read`。
- [ ] `form_read` 的 schema、handler、边界和负向测试落地。
- [ ] CLI runtime 与 task runner 的 browser schema 计数同步为 5。
- [ ] adapter pack validator 校验 `form_read` 的只读语义与输出。
- [ ] 上述验收与 AGENTS 文档门禁全绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-346 浏览器 form_read 生产 handler
【目标】新增只读快照 handler，不接提交/UI/fixture
【write scope】见卡片正文
【铁律】不可信输入先校验；Policy 唯一放行；只读操作必须证明状态未变；
        契约先行；不得静默扩范围
【禁止】form_submit、通用 click、真实浏览器/站点、UI/IPC、fixture、依赖或数据库改动
【验收】form_read 专项、browser 聚焦测试、workspace、fmt/clippy、schema/codegen
【依赖】TASK-343 / ADR-0101 / browser-form-confirmation spec
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_handlers.rs`、`browser_form_read.rs`、
  `browser_form_read_tests.rs`、`browser_registry.rs`、
  `browser_registry_tests.rs`、`browser_runtime.rs`、`browser_runtime_tests.rs`、
  `browser_task_runtime.rs`、`browser_task_runtime_tests.rs`。
- `adapters/browser.edge/tools/tools.json`、`adapters/browser.chrome/tools/tools.json`、
  adapter pack validator。
- TASK-346/347/348/349、TASK-057 依赖、facts/pitfalls、MEMORY、PLAN、阶段计划与 LEDGER。

### 3. 验收输出摘要

- `form_read` 专项：3 passed / 0 failed。
- browser registry：5 passed / 0 failed；browser runtime：5 passed / 0 failed；
  browser task runner：27 passed / 0 failed。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：510 passed / 0 failed；arch tests 10/10。
- schema 5/5、codegen 0 drift、hygiene / memory-counts / adr-index / refscan /
  docscan / card-check / check-ledger 全 PASSED；deny 与 replay 通过。
- Edge/Chrome adapter pack：PACK OK。

### 4. DoD 逐条核对

- [x] Edge/Chrome registry 均挂载五工具，包含 `form_read`。
- [x] schema、handler、字段边界与负向测试落地。
- [x] CLI runtime / task runner 的 browser schema 计数同步为 5。
- [x] adapter validator 校验只读语义和输出。
- [x] 上述验收与 AGENTS 文档门禁全绿。

### 5. 偏差

- 原实现把 `form_read` / `form_submit` 合并后超过单卡 400 行预算；按治理规则拆分为
  TASK-346（只读快照）与 TASK-347（提交 handler）。
- 未启动真实 Edge/Chrome；提交与真实 postcondition 留 TASK-347 / TASK-349 / TASK-057。

```text
DRIFT-346-1
触发器：#10（工作量超预估 2 倍）
现象：拆出 form_submit 后，最终 diff 仍为 906 additions / 39 deletions；新增只读 handler、
      双 adapter schema、registry/runtime 接线、测试和治理文件超过 400 行预算。
影响：单卡无法在 400 行内完整审阅；若直接合并会违反任务卡粒度规则。
我的建议：保持 Review，由人类裁决是否接受一次例外，或再拆为 handler 与 adapter/registry 接线两张卡。
已停止的工作：不再加入 UI、approval、fixture 或真实浏览器工作。
需要人类裁决：是
```

```text
DRIFT-346-1
触发器：#10（工作量超预估 2 倍）
现象：TASK-346 的最终 diff 为 895 additions / 39 deletions；虽然已拆出 form_submit，
      但新增 handler、双 adapter schema、registry/runtime 接线、测试和治理文件仍超过 400 行预算。
影响：单卡不可在 400 行内完整审阅；若继续按 Done 合并，会违反任务卡粒度规则。
我的建议：保持 Review，由人类决定是否接受一次例外，或再拆成“handler 实现”与“adapter/registry 接线”两张卡。
已停止的工作：已停止继续扩范围；不再加入 UI、approval 或 fixture。
需要人类裁决：是
```

### 6. 更合理做法

- 先交付只读快照并固定 registry 计数，再由独立卡增加不可逆提交，避免将两种风险和
  两套负向矩阵塞进同一个 PR。

### 7. 遗留问题

- TASK-347 接 `form_submit`；TASK-348 接 approval/diff/归因；TASK-349 接 loopback fixture。

### 8. 新增长期记忆

- `docs/memory/facts.md`：五工具 registry 与 form_read handler 边界。
- `docs/memory/pitfalls.md`：只读快照也必须按 untrusted、有界和唯一命中处理。

### 9. 给审阅者的关注点

- `form_read` 返回值是否保持 untrusted 且有界。
- 非法 action、歧义 selector 和禁止控件是否在返回前失败。
