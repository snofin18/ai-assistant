# TASK-349　Browser form_submit 生产接线

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-348、TASK-347、TASK-343、ADR-0101、`docs/spec/browser-form-confirmation.md`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。

---

- **write scope**：`adapters/browser.edge/{app_map.json,tools/tools.json,tests/validate_adapter_packs.py}`、
  `adapters/browser.chrome/{app_map.json,tools/tools.json}`、
  `apps/agent-core/src/{browser_handlers.rs,browser_registry.rs,browser_registry_tests.rs,browser_runtime.rs,browser_runtime_tests.rs,browser_task_runtime.rs,browser_task_runtime_tests.rs}`、
  `docs/memory/{facts,pitfalls}.md`、`PLAN.md`、
  `plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`

## 目标

把已合并的 handler-only `form_submit` 接入 Edge/Chrome adapter schema 与六工具
registry/runtime；不实现 task-package 执行断言、approval/diff、UI/IPC、fixture 或真实浏览器。

## In scope

- Edge/Chrome `cdp.form_submit` schema、app-map intent 与 adapter pack 校验；
- browser registry、runtime、task runner 从五工具同步为六工具；
- 文档和状态同步。

## Out of scope（做了算漂移）

- 不接 task package / approval/diff、UI/IPC、loopback fixture 或真实浏览器；
- 不修改 `form_submit` handler 的提交安全合同、`form_read` / `form_fill` 合同、protocol、
  数据库或依赖；
- 不放宽 adapter pack、registry、runtime 或 task package 的 fail-closed 判据；
- 不宣称 TASK-057 或 TASK-058 完成。

## 必须遵守

1. `form_submit` schema 固定 `effect=write`、`reversibility=l3_irreversible`、`risk_level=high`、
   `requires_approval=true`、`idempotent=false`，输入必须包含唯一 form/submit selector、
   expected origin/action 与 expected fields。
2. Edge/Chrome 都只能挂载六个已实现工具；download/screenshot 继续拒绝。
3. `form_submit` 先不进入 task-package assertion table；其执行观测必须与 approval/diff 卡一起设计。

## 验收

`cargo test -p assistant-agent-core form_submit`、`browser_registry`、`browser_runtime` 与
`browser_task` 全绿；adapter pack 校验 `PACK OK`；`AGENTS.md` §6 全量门禁、deny、replay 与
覆盖率全绿。

## 完成定义（DoD）

- [ ] Edge/Chrome adapter pack 都声明 `form_submit` 且高风险语义正确。
- [ ] 两个 browser registry 都挂载六个生产工具。
- [ ] CLI runtime 与 task runner 的 schema 计数同步为六。
- [ ] `form_submit` 不进入 task-package assertion table，执行观测留给后续 approval/diff 卡。
- [ ] 全量门禁通过，TASK-057 / TASK-058 仍未完成。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写；改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-349 Browser form_submit 生产接线
【目标】接入 adapter schema 与六工具 registry/runtime
【write scope】见卡片正文
【铁律】不可信输入先校验；契约先行；写操作 postcondition；不得静默扩范围
【禁止】approval/diff、UI/IPC、fixture、真实浏览器、公共协议或依赖改动
【验收】form_submit/browser 专项、adapter pack、workspace 与全量门禁
【依赖】TASK-348 / TASK-347 / TASK-343 / ADR-0101
【疑问】无
```

### 2. 实际改动文件

- Edge/Chrome adapter `app_map.json`、`tools.json` 与 Edge adapter pack validator；
- browser registry/runtime/task runner 的六工具计数与对应测试；
- facts/pitfalls、MEMORY、PLAN、阶段计划、README 与 LEDGER。

### 3. 验收输出摘要

- form_submit 5/5；browser registry 5/5；browser runtime 5/5；browser task 27/27；
  workspace 510/510；adapter pack 双绿；fmt、clippy、schema/codegen、文档门禁、deny、replay 与
  coverage 79.71% 全绿。

### 4. DoD 逐条核对

- [x] Edge/Chrome adapter pack 都声明 `form_submit` 且高风险语义正确。
- [x] 两个 browser registry 都挂载六个生产工具。
- [x] CLI runtime 与 task runner 的 schema 计数同步为六。
- [x] `form_submit` 不进入 task-package assertion table，执行观测留给后续 approval/diff 卡。
- [x] 全量门禁通过；TASK-057 / TASK-058 未标记完成。

### 5. 偏差

- 未添加 task-package assertion：`form_submit` 成功输出没有 fingerprint，直接接线会使
  `EnvelopeObservationCollector` 进入 observation unavailable。该边界已按卡片收口，未伪造观测。

### 6. 更合理做法

- 保持六工具 schema/registry 接线与 approval/diff/observation 分层；先让 adapter pack 的
  高风险语义可机器校验，再由 TASK-057 设计 task package 的执行断言。

### 7. 遗留问题

- approval/diff、UI/IPC、loopback fixture、真实 Edge 人工验收与 task-package observation
  仍待后续卡。

### 8. 新增长期记忆

- facts：记录 form_submit 六工具生产链路与 task-package observation 边界；
- pitfalls：记录不能为无 fingerprint 的 form_submit 伪造 task-package 断言。

### 9. 给审阅者的关注点

- 确认 Edge/Chrome schema 的 write / high / l3_irreversible / approval / non-idempotent
  五项语义一致；确认六工具计数在 registry、runtime、task runner 与测试中同步；确认没有把
  task-package assertion 作为本卡的隐含扩展。
