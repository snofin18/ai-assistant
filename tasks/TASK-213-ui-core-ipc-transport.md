# TASK-213　UI↔Core 真实传输：Core 侧监听端 + UI 侧 client + 事件推送

- 状态：**Ready**（**开工前置**：ADR-0057 必须已由人类转 Accepted）
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：0057、104、103、019
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：**PL-095**、`docs/adr/0057-ui-core-ipc-transport-contract.md`、
  `docs/spec/ui-ipc-protocol.md`、TASK-104、TASK-105、TASK-019、`crates/ipc`

## 目标

把 TASK-104 已经落地的 UI↔Core **契约形状**接上**真实传输**：Core 侧起 UI 命令监听端，
UI 侧 `src-tauri` 用真实 client 替换 `core_transport_unavailable`，并打通 Core → UI 的事件推送。
完成后 TASK-105 才可能从 UI 触发 T1.x 真实运行。

## In scope

- `apps/agent-core/src/**`：Core 侧 UI 命令监听端与装配（binary 装配层，不进 `crates/core`）。
- `apps/desktop-ui/src-tauri/**`：`CoreCommandTransport` 的真实实现（NamedPipe client）。
- `crates/ipc/**`：**仅在确有必要时**复用小改动（例如把已有的帧/握手原语暴露得更合适）。
- 相关测试、`.github/workflows/**` 的新增门禁与负向验证、README。
- 本卡记录。

## Out of scope

- 修改 `crates/ipc` 的**公共工具信封**（`RequestMessage` / `ResponseMessage`）—— ADR-0057 D2 明文禁止复用/改动它们。
- UI 新功能（审批卡 / 拾取器 / 策略面板已由 TASK-030~032 交付）。
- T1.x 真实运行与 10 次成功率（归 TASK-105）。
- macOS/Linux 的 UDS 实现（归后续平台卡）。
- 加密层与远端访问（ADR-0057 D3 明确不做）。

## 必须遵守

- **ADR-0057 未转 Accepted 前不得开工**；开工前重读该 ADR 与 `docs/spec/ui-ipc-protocol.md`。
- 版本漂移 / 未知字段 / 未知 kind / 未知 correlation / 缺响应 / 事件字段缺失 —— 六个 fail-closed 点全部要拒绝并带 `ErrorCode`。
- 断连后**不得自动重放命令**（命令可能已产生副作用）。
- UI 进程不得链接 `assistant-agent-core` / `assistant-policy` / `assistant-task-engine`。
- 不得跨进程传句柄：线上只有可序列化数据（铁律 8）。
- 新门禁必须配负向验证（ADR-0019），且**先证明会红再接阻断**。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core
cargo test -p assistant-automation-host
cd apps/desktop-ui/src-tauri; cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test
pnpm --dir apps/desktop-ui lint; pnpm --dir apps/desktop-ui typecheck; pnpm --dir apps/desktop-ui test
cargo run -p xtask -- hygiene
cargo run -p xtask -- check-comments
```

## 完成定义（DoD）

- [ ] Core 侧真实起监听端：一次性 token + 对端进程镜像白名单 + 版本握手。
- [ ] UI 侧 `CoreCommandTransport` 真实实现；`core_transport_unavailable` 不再是默认路径。
- [ ] 真实管道端到端：UI 发 `submit_intent` → Core 返回同一 `correlation_id` 的 `intent_accepted`。
- [ ] Core → UI 事件推送：`step_state_changed` 经 UI 侧 zod 校验后落到时间线模型。
- [ ] 六个 fail-closed 点各有负向用例，且负向用例断言**具体 ErrorCode**（不是"非零"）。
- [ ] 断连：kill Core 后 UI 在 2 s 内检测断连。
- [ ] 新门禁接入 CI 且带负向验证（ADR-0019）。
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
