# TASK-347　浏览器 form_submit 生产 handler

- 状态：**Ready**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-346、ADR-0101
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：待派单前按 TASK-346 的 registry / handler 模式展开。

## 目标

新增 Edge/Chrome 高风险 L3 `cdp.form_submit`：在同一 DOM 表达式内复验 form、submit、
同源 action 与 expected fields，只点击一次，并以 CDP target identity 变化作为成功证据。

## Out of scope

- 不新增通用 click；
- 不修改 `form_read` / `form_fill` 合同；
- 不接 UI/approval/fixture；
- 不执行真实浏览器或外部站点。

## DoD

- [ ] high / L3 irreversible / requires_approval / 非幂等 schema；
- [ ] 同源、action、字段漂移和歧义全部在点击前 fail-closed；
- [ ] 源码级测试证明一次 click，点击后 identity 未变或不可读不成功；
- [ ] registry / runtime / task package 计数与断言同步。
