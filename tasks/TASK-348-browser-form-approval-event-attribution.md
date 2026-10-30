# TASK-348　浏览器表单 approval 事件与来源归因

- 状态：**Ready**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-346、TASK-347、ADR-0101、ADR-0082
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：待派单前按实际 UI/IPC 接点展开。

## 目标

将已校验 `InstructionAttribution`、字段级 `DiffPreview::Fields`、risk / reversibility /
scope / evidence / expiry 通过 `step_approval_requested` 送到 UI，并保持 strict parse、
`app_content` 默认拒绝与 policy deny 不可覆盖。

## Out of scope

- 不新增浏览器工具 handler；
- 不执行真实表单或真实站点；
- 不把 UI 覆盖当作 policy allow；
- 不新增 InstructionOrigin token。

## DoD

- [ ] approval event 契约与 Core/UI 双向契约测试；
- [ ] field diff 只来自 form_read + 已验证 readback；
- [ ] `app_content` 默认拒绝、显式覆盖不越权；
- [ ] 全量 Rust/UI 与文档门禁全绿。
