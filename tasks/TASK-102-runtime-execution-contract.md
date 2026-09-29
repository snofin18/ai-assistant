# TASK-102　运行执行链路契约：Planner → TaskEngine → Policy → ToolBus → Host → Verify → Undo

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：029、037、038、039
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：`docs/audits/stage-1a-integration-audit-2026-09-29.md`、架构 v2 §7/§8/§9/§12、ADR-0038 / 0053

## 目标

为阶段 1a 缺失的真实执行链路先建立契约：明确 Plan / Step 从装配层到 Policy、
ToolBus、Host、Verify、Undo 的调用顺序、所有权、取消/超时、错误传播和审计边界；
定义 `VerifyOutcome` 进入 task-engine 状态迁移的强制接口。

本卡只做 ADR + spec，不实现运行时代码。

## In scope

- 新增 `docs/adr/0056-runtime-execution-contract.md`（编号由执行时按登记表确认）。
- 新增 `docs/spec/runtime-execution.md`。
- `docs/adr/README.md`、`docs/memory/decisions.md` 的必要登记。
- 本卡执行记录区。

## Out of scope（做了算漂移）

- 修改任何 `crates/**`、`apps/**`、`fixtures/**`、`eval/**`。
- 实现 Host 分发、UI/IPC、Provider 或真实任务运行。
- 修改 task-engine / verify / policy / tool-bus 的代码或公共类型。
- 把未裁决的设计直接写死为 Accepted。

## 必须遵守

- 遵守架构 v2 的分层与 ADR-0053：`core` 只依赖白名单，组装点唯一在 binary。
- 每个写步骤必须要求 postcondition；验证失败不得进入成功状态。
- Host IPC 只传可序列化数据，句柄不跨进程。
- Policy 是唯一放行点；审计与撤销边界必须显式。
- 至少覆盖取消、超时、验证失败、Host 断连、用户接管与恢复。

## 验收命令

```powershell
cargo test --workspace
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- hygiene
```

## 完成定义（DoD）

- [ ] ADR 明确执行链路的组件所有权和接口边界。
- [ ] Spec 包含状态迁移、错误映射、取消/超时、审计与撤销契约。
- [ ] `VerifyOutcome` 强制接线方案不依赖模型自觉。
- [ ] ADR 登记、spec 索引和 memory 决策索引同步。
- [ ] 全部验收命令通过。
- [ ] 未修改 Out of scope 文件。

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
