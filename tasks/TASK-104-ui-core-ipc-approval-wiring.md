# TASK-104　UI ↔ Core typed IPC 与审批接线

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：102、103
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-029/030/031/032、TASK-102/103、`apps/desktop-ui/**`

## 目标

把桌面 UI 与 Core 真正接起来：typed IPC command、审批请求/结果、执行时间线、
暂停/接管操作只通过受校验的边界传递，禁止 UI 自行判断权限或直接调用平台层。

## In scope

- `apps/agent-core/src/**` 的 IPC command / event 适配层。
- `apps/desktop-ui/src/**` 的调用、状态与审批界面接线。
- `apps/desktop-ui/src-tauri/**` 的必要的 command 注册。
- 相关测试与 README。

## Out of scope

- 真实 Notepad 10 次运行验收。
- 修改 Policy 或 Host 核心逻辑。
- 新 Provider、DLP 或浏览器功能。

## 必须遵守

- UI 不直接调用 Host 或平台 provider。
- 审批 scope、diff、来源归因、point-of-no-return 全部经运行时校验。
- IPC 输入是不可信输入，必须拒绝未知字段/非法状态。
- 审批拒绝或接管后必须停止对应执行。
- 仍保持 UI 无系统权限和严格 CSP。

## 验收命令

```powershell
cargo test --workspace
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
cargo run -p xtask -- docscan
```

## 完成定义（DoD）

- [ ] 至少一条 UI 发起的执行请求能到达 Agent-core。
- [ ] 审批卡能收到并回传结构化决策。
- [ ] 时间线能显示真实执行与验证结果。
- [ ] IPC 拒绝非法输入且不产生静默失败。
- [ ] 前端 lint/test/typecheck/build 全绿。
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
