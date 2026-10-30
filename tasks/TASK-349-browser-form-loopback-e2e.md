# TASK-349　浏览器表单 loopback fixture 与脚本化端到端

- 状态：**Ready**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-346、TASK-347、TASK-348、ADR-0101
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：待派单前按 fixture / task-package / eval 接点展开。

## 目标

建立 `127.0.0.1` 三字段表单 fixture 与脚本化 ToolBus/RuntimeExecutor 路径，验证
`form_read → form_fill → diff/approval → form_submit`，以及 deny / timeout / replay /
字段漂移时提交次数为 0、正常批准时点击恰好一次。

## Out of scope

- 不启动真实 Edge/Chrome；
- 不使用外部站点、真实账号或敏感数据；
- 不替代 TASK-057 的人类在场真机验收。

## DoD

- [ ] fixture 可本地启动且提交结果确定；
- [ ] 正常路径与全部负向矩阵有自动化测试；
- [ ] 测试证明点击次数为 0 或 1；
- [ ] 全量 workspace、schema、replay 与文档门禁全绿。
