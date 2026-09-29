# TASK-103　真实任务执行器：Host 分发、上下文装配与 VerifyOutcome 接线

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：102
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-102 的 ADR/spec、`apps/agent-core/**`、`apps/automation-host/**`、`crates/task-engine/**`

## 目标

按 TASK-102 的契约实现一条真实的后端执行链：从 `Plan` / `Step` 进入 task-engine，
经 Policy 放行、ToolBus 调用、Host 分发、结果校验与 VerifyOutcome，再按结果推进状态；
失败、取消、超时和未知结果必须 fail-closed。

## In scope

- `apps/agent-core/src/**` 的执行器与装配代码。
- `apps/automation-host/src/**` 的请求分发。
- `crates/task-engine/**` 的 `VerifyOutcome` 接线。
- `crates/ipc/**` 的必要测试适配。
- 相关 README、测试与本卡记录。

## Out of scope

- UI、审批卡、桌面端 typed command。
- 真实 Notepad 运行和 10 次成功率报告。
- 新 Provider、新 MCP server 或新公共 schema。

## 必须遵守

- 以一个已声明步骤为最小闭环，不得一次实现多渠道抽象。
- task-engine 不得在缺 VerifyOutcome 时进入成功终态。
- Host 断连、未知响应、校验失败都返回带 ErrorCode 的失败。
- 所有超时可取消，取消后不得继续写。
- 不在 core 直接依赖平台实现或 IPC。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core
cargo test -p assistant-task-engine
cargo run -p xtask -- check-migrations
cargo run -p xtask -- hygiene
```

## 完成定义（DoD）

- [ ] Agent-core 能装配并执行一个真实 `Plan`。
- [ ] Host 能接收并分发工具调用，返回结构化结果。
- [ ] task-engine 强制接收 `VerifyOutcome` 后才推进成功。
- [ ] 断连、超时、拒绝、验证失败均有负向测试。
- [ ] 无静默失败、无句柄跨进程。
- [ ] README 与测试同步，验收全绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
