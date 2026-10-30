# TASK-348　浏览器 form_submit 生产 handler

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-343、TASK-346、TASK-347、ADR-0101、`docs/spec/browser-form-confirmation.md`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。

---

- **write scope**：`apps/agent-core/src/browser_handlers.rs`、
  `apps/agent-core/src/browser_form_submit.rs`、`apps/agent-core/src/browser_form_submit_tests.rs`、
  `docs/memory/{facts,pitfalls}.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`

## 目标

实现 handler-only 的专用 `edge|chrome.cdp.form_submit`：同一 DOM 表达式内复验 form、submit、
origin、action 与字段状态后最多点击一次；点击后必须观察目标页变化，未知状态显式进入
`UserInteraction`，禁止自动重试。

## In scope

- 输入边界、同源 action、无 credentials、`_self` target 与字段状态复验；
- 最多一次 `.click()`、bounded network-idle、post_url + navigation marker postcondition；
- 点击前错误映射、点击后 unknown outcome、直接 handler 测试与长期记忆。

## Out of scope（做了算漂移）

- 不接 adapter schema / registry / runtime 计数 / task package；
- 不实现 approval/diff、UI/IPC、fixture、真实浏览器或通用 click；
- 不改 protocol、数据库、依赖、`form_fill` / `form_read` 合同，不宣称 TASK-057 完成。

## 必须遵守

1. 任一 selector 歧义、跨 origin、URL credentials、非 `_self` target、formaction 覆盖或字段漂移
   都必须在点击前失败。
2. 表达式只允许一次 `submit.click()`；点击后等待或读取失败不得返回成功，也不得重试。
3. 成功只在 URL 仍为同源 HTTP(S) 且 `performance.timeOrigin` 变化后返回。

## 验收

`cargo test -p assistant-agent-core form_submit`、`browser_registry`、`browser_runtime`、
`browser_task` 全绿；`AGENTS.md` §6 全量门禁、deny、replay 与覆盖率全绿。

## 完成定义（DoD）

- [x] handler-only 实现，未接入 adapter/registry；
- [x] 点击前所有契约检查在单一表达式内完成，`.click()` 仅一次；
- [x] 点击后 unknown / 未观察变化均返回 `UserInteraction`；
- [x] 成功输出仅 `{submitted:true,post_url}` 且标记 untrusted；
- [x] 全量门禁通过，TASK-057 保持未完成。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写；改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-348 Browser form_submit 生产 handler
【目标】同源复验后最多点击一次，未知结果进入 UserInteraction
【write scope】见卡片正文
【铁律】不可信输入先校验；写操作 postcondition；Policy 唯一放行；不得静默扩范围
【禁止】adapter/registry、approval/UI、fixture、真实浏览器、通用 click、公共协议/依赖
【验收】form_submit/browser 专项、workspace、fmt/clippy、schema/codegen、文档门禁
【依赖】TASK-343 / TASK-346 / TASK-347 / ADR-0101
【疑问】无
```

### 2. 实际改动文件

- `browser_handlers.rs`：handler-only 模块注册与工具名，不加入 registry expected list；
- `browser_form_submit.rs`、`browser_form_submit_tests.rs`：提交 handler、preflight/postcondition
  校验与直接测试；
- facts/pitfalls、MEMORY、PLAN、阶段计划、README 与 LEDGER。

### 3. 验收输出摘要

form_submit 5/5；browser registry 5/5；runtime 5/5；browser task 27/27；workspace 510/510；
fmt、clippy、schema/codegen、文档门禁、deny、replay 与 coverage 79.72% 全绿。

### 4. DoD 逐条核对

- [x] 只注册到内部 handler map，`BrowserToolNames::as_vec` 仍保持五个 adapter 工具。
- [x] 单一表达式内完成唯一命中、URL、target、formaction/formnovalidate、form 归属、字段与 validity 检查；`.click()` 一次。
- [x] network-idle 或 identity read 失败、timeOrigin 未变化、post_url 不安全均 `UserInteraction`。
- [x] 成功数据仅 `submitted` 与 `post_url`，ToolOutput 标记 untrusted。
- [x] 全量门禁通过；TASK-057 未完成。

### 5. 偏差

- 无产品范围偏差。clippy 首轮仅发现测试中的 redundant closure，已改为
  `std::sync::PoisonError::into_inner`，未新增 allow。

### 6. 更合理做法

继续沿用 handler-only 拆分，registry、approval 与 fixture 独立成卡，避免不可逆动作和审批链路
挤入同一 review。

### 7. 遗留问题

adapter schema、registry/runtime 接线、approval/diff、fixture 与真实 Edge 人工验收仍待后续卡。

### 8. 新增长期记忆

- facts：记录 handler-only 安全提交边界与 timeOrigin postcondition；
- pitfalls：记录不可把 click 返回或点击后网络失败当成可重试成功。

### 9. 给审阅者的关注点

确认所有检查都在 `.click()` 之前、表达式只有一个 click，且点击后任何不确定路径都不会返回成功。

### 10. 合并证据

- PR #662 checks 11/11 success；Mergeable / Clean；merge `f043f76`；
- post-merge main CI `41000801189` success。
