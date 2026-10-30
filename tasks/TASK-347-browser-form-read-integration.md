# TASK-347　Browser form_read 生产接线

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-346、TASK-343、ADR-0101、`docs/spec/browser-form-confirmation.md`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。

---

- **write scope**：`adapters/browser.edge/{app_map.json,tools/tools.json,tests/validate_adapter_packs.py}`、
  `adapters/browser.chrome/{app_map.json,tools/tools.json}`、
  `apps/agent-core/src/{browser_registry.rs,browser_registry_tests.rs,browser_runtime.rs,browser_runtime_tests.rs,browser_task_runtime.rs,browser_task_runtime_tests.rs,task_package_render.rs}`、
  `apps/agent-core/tests/task_package.rs`、`docs/memory/{facts,pitfalls}.md`、
  `MEMORY.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`

## 目标

把已合并的只读 `form_read` handler 接入 Edge/Chrome adapter schema、五工具 registry/runtime
与任务包 `state_unchanged` 后置断言；不实现提交、审批或 UI。

## In scope

- Edge/Chrome `cdp.form_read` schema、app-map intent 与 adapter pack 校验；
- browser registry、runtime、task runner 从四工具同步为五工具；
- `form_read` 固定 `state_unchanged` 的渲染测试、文档和状态同步。

## Out of scope（做了算漂移）

- 不实现 `form_submit`、approval/diff、UI/IPC、fixture 或真实浏览器；
- 不修改 `form_fill`、`form_read` handler 合同、protocol、数据库或依赖；
- 不放宽 adapter pack、registry 或 task package 的 fail-closed 判据。

## 必须遵守

1. `form_read` 固定 `effect=read`、`reversibility=none_readonly`、无需审批且幂等。
2. Edge/Chrome 都只能挂载五个已实现工具，download/screenshot 继续拒绝。
3. 任务包只能生成固定 `state_unchanged` 后置断言，不接受模型自定义断言。

## 验收

`cargo test -p assistant-agent-core form_read`、`browser_registry`、`browser_runtime`、
`browser_task` 与 `task_package` 全绿；adapter pack 校验 PACK OK；`AGENTS.md` §6 全量门禁、
deny、replay 与覆盖率全绿。

## 完成定义（DoD）

- [x] Edge/Chrome adapter pack 都声明 `form_read` 且只读语义正确。
- [x] 两个 browser registry 都挂载五个生产工具。
- [x] CLI runtime 与 task runner 的五工具计数同步。
- [x] `form_read` 任务包后置断言固定为 `state_unchanged`。
- [x] 全量门禁通过，TASK-057 仍不标记完成。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写；改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-347 Browser form_read 生产接线
【目标】接入 adapter schema、五工具 registry/runtime 与固定只读后置断言
【write scope】见卡片正文
【铁律】不可信输入先校验；契约先行；不得静默扩范围；fail-closed
【禁止】form_submit、审批/UI、fixture、真实浏览器、公共协议或依赖改动
【验收】form_read/browser/task_package 专项、adapter pack、workspace 与全量门禁
【依赖】TASK-346 / TASK-343 / ADR-0101
【疑问】无
```

### 2. 实际改动文件

- Edge/Chrome adapter `app_map.json`、`tools.json` 与 Edge adapter pack validator；
- browser registry、runtime、task runner、task-package render 与对应测试；
- 本卡、facts/MEMORY、PLAN、阶段计划、README 与 LEDGER。

### 3. 验收输出摘要

form_read 5/5、browser registry 5/5、runtime 5/5、browser task 27/27、task_package 25/25；
adapter pack 双绿；workspace 510/510；fmt、clippy、schema/codegen、文档门禁、deny、replay 与
coverage 79.69% 全绿。
- Merge evidence：PR #660 / merge `a5ba47e`；post-merge main CI `40990836783` success。

### 4. DoD 逐条核对

- [x] Edge/Chrome adapter pack 声明 `form_read`，固定 read / none_readonly / no approval / idempotent。
- [x] 两个 browser registry 都挂载五个生产工具。
- [x] runtime 与 task runner 的 schema 计数同步为五。
- [x] 任务包 `form_read` 后置断言固定为 `state_unchanged`。
- [x] 全量门禁通过；TASK-057 未标记完成。

### 5. 偏差

- 首轮 all-targets clippy 因 `call_edge_tools` 达 89 行失败；已按初版结构拆出
  `call_edge_form_tools`，未新增 allow 或放宽判据。

### 6. 更合理做法

复用 TASK-346 初版已有的接线形状，不重新设计 adapter 或工具 schema。

### 7. 遗留问题

`form_submit`、approval/diff、fixture 与真实 Edge 仍由后续卡承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 Edge/Chrome `form_read` 已进入五工具生产链路及固定
  `state_unchanged` 后置断言。

### 9. 给审阅者的关注点

确认五工具计数、只读 schema 与固定 `state_unchanged` 没有放宽任何 fail-closed 判据。
