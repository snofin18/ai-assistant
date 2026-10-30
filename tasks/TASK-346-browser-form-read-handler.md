# TASK-346　浏览器 form_read 生产 handler
- 状态：**Review**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-343、ADR-0101、`docs/spec/browser-form-confirmation.md`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。

---
- **write scope**：`apps/agent-core/src/browser_handlers.rs`、
  `apps/agent-core/src/browser_form_read.rs`、`apps/agent-core/src/browser_form_read_tests.rs`、
  `apps/agent-core/src/browser_registry_tests.rs`、`docs/memory/{facts,pitfalls}.md`、
  `MEMORY.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`LEDGER.md`
## 目标
实现只读 `edge|chrome.cdp.form_read` handler 的受限 DOM 快照与 Rust 侧校验；不接 adapter
registry、提交、UI、approval 或 fixture。
## In scope
- 唯一 form / 字段命中、允许控件、单值与总量上限；untrusted 快照、fingerprint、直接 handler
  测试与 Review 状态同步。
## Out of scope（做了算漂移）
- 不修改 adapter tool schema / registry / runtime 计数；不实现 `form_submit`、UI/IPC、
  approval、fixture 或真实浏览器，不改 `form_fill` 合同。
## 必须遵守
1. 页面值始终是不可信输入，返回结构化、有界数据。
2. 歧义、非法 action、超界、控制字符与禁止控件全部 fail-closed。
3. 不新增 `unsafe`、`unwrap`、`expect`、`panic` 或测试断言放宽。
## 验收
`cargo test -p assistant-agent-core form_read`、`browser_registry`、`browser_runtime`、
`browser_task` 专项全绿；`AGENTS.md` §6 的 fmt、clippy、workspace、schema/codegen、文档、
deny 与 replay 门禁全绿。
## 完成定义（DoD）
- [ ] handler 只发出一次受限 DOM 读取表达式。
- [ ] 唯一命中、允许控件、大小边界和 action 校验有专项测试。
- [ ] 输出标记 untrusted 且不包含整页 DOM。
- [ ] 全量门禁通过，后续 registry 接线另卡完成。
<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写；改动分界线以上任何一行 = 漂移触发器 ⑤。 -->
## 执行记录
### 1. 约束回执
```text
【任务】TASK-346 浏览器 form_read 生产 handler
【目标】实现只读快照 handler，不接 registry/提交/UI/fixture
【write scope】见卡片正文
【铁律】不可信输入先校验；只读操作证明状态未变；不得静默扩范围
【禁止】adapter schema、registry/runtime 接线、form_submit、真实浏览器
【验收】form_read/browser 专项、workspace、fmt/clippy、schema/codegen、文档门禁
【依赖】TASK-343 / ADR-0101
【疑问】无
```
### 2. 实际改动文件
`browser_handlers.rs`、`browser_form_read.rs`、`browser_form_read_tests.rs`、
`browser_registry_tests.rs`、facts/pitfalls、MEMORY、PLAN、阶段计划、LEDGER。
### 3. 验收输出摘要
form_read 5/5、registry 5/5、runtime 5/5、task 27/27、workspace 510/510、arch 10/10；fmt、clippy、schema 5/5、codegen 0 drift、文档门禁、deny、replay 与 llvm-cov 79.70% 全绿；净差异 400/10。
### 4. DoD 逐条核对
DoD 4/4：只读表达式、边界测试、untrusted 输出与全量门禁（registry 接线另卡）。
### 5. 偏差
初版把 handler、adapter schema 与 registry/runtime 接线合在一卡，超过 400 行预算；现已拆分。
### 6. 更合理做法
先冻结 handler 的内部安全边界，再由独立卡完成 adapter schema 与生产挂载。
### 7. 遗留问题
integration 卡负责 Edge/Chrome 挂载；`form_submit`、approval/diff、fixture 与真实 Edge 未完成。
### 8. 新增长期记忆
facts 记录 handler-only 边界；pitfalls 记录错误载荷与 untrusted 有界快照的 fail-closed 要求。
### 9. 给审阅者的关注点
DOM 表达式是否确实只读且不返回整页内容；所有错误路径是否在 Rust 返回前 fail-closed。
