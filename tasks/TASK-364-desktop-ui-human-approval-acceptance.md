# TASK-364 desktop-ui human approval acceptance

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 产品化
- 依赖：ADR-0101、TASK-056、TASK-343、TASK-356、TASK-358、TASK-359、TASK-361、TASK-363
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1`
- `eval/tasks/edge/t5.2/README.md`
- `eval/tasks/edge/t5.2/evidence/2026-11-12-task-364-desktop-ui-human-approval*/**`
- `apps/agent-core/src/browser_task_support.rs`
- `apps/agent-core/src/browser_task_ui_approval_tests.rs`
- `apps/desktop-ui/src-tauri/src/event_bridge.rs`
- `apps/desktop-ui/src/ipc/client.ts`
- `tasks/TASK-364-desktop-ui-human-approval-acceptance.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

使用真实的本地 loopback T5.2 表单、专用 Edge profile、`assistant-agent-core --ui-approval` 和正式
desktop-ui/Tauri 窗口完成一次人类在场的 L3 表单提交验收。人类必须在 UI 中看到脱敏字段 diff、
来源归因与 point-of-no-return 提示，并亲自完成一次性的 approve 操作。

## In scope

- 新增可重复的 T5.2 desktop-ui 人工验收入口，启动 fixture、Core、Edge 与 desktop-ui 并收集证据。
- Core 与 UI 使用同一条受控 NamedPipe 和同一个 token 环境变量；UI peer 必须固定为 desktop-ui 二进制。
- 人类决定前，Core 不得调用 `form_submit`；人类在 UI 中批准后，`form_submit` 最多调用一次。
- 验证 UI 收到 `step_approval_requested`，展示脱敏字段 diff、`once` 授权和不可逆提示。
- 验证最终 JSON `submitted=true`、query-free 本地 `post_url`、成功页 marker。
- 验证 fixture listener、Core、desktop-ui 与专用 Edge profile 进程全部收敛。
- 记录运行命令、UI/edge/core 版本、结果摘要与失败日志路径。

## Out of scope

- 不访问真实外部站点、真实账号、密码、文件上传、支付、验证码或跨源 OAuth。
- 不修改 Tool/UI IPC/DB schema、Core 审批状态机或 Policy 判据。
- 不提供自动 approve 捷径；agent 不得代替人类点击 L3 确认或输入 `once`。
- 不把本卡验收自动等同于 TASK-057 / TASK-058 Done；两张卡仍由治理流程收口。

## 必须遵守

1. 只使用 `127.0.0.1:8766` 的三字段合成表单与专用 Edge profile。
2. 人类决定前 submit calls 必须为 0；批准后最多为 1，未知结果不重试。
3. UI 展示的数据必须来自已校验的 approval event，不得用截图文本或模型摘要替代 diff。
4. token 不得进入命令行、日志、截图、错误消息或证据文件。
5. 任一步失败必须保留 stdout/stderr、进程状态和错误摘要，并清理进程树。

## 验收命令

```powershell
pnpm --dir apps/desktop-ui build
cargo build --manifest-path apps/desktop-ui/src-tauri/Cargo.toml
cargo build -p assistant-agent-core
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/2026-11-12-task-364-desktop-ui-human-approval
cargo test --manifest-path apps/desktop-ui/src-tauri/Cargo.toml
cargo test --workspace
cargo run -p xtask -- check-ledger
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
```

期望：人类在正式 desktop-ui 审批卡完成一次确认；提交恰好一次；成功 marker 可见；所有进程与
端口收敛；证据文件可追溯且不含 token 或未脱敏字段值。

## DoD

- [ ] 正式 desktop-ui 显示 T5.2 脱敏 diff、归因、once scope 与不可逆提示。
- [ ] 人类确认前无 `form_submit`，确认后恰好提交一次。
- [ ] JSON、成功页 marker、进程树与端口清理证据齐全。
- [ ] desktop-ui / workspace / 文档门禁通过。
- [ ] PR 合并并记录 checks / merge / post-merge main CI；进度同步。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-364 desktop-ui human approval acceptance
【目标】用正式 desktop-ui 完成一次本地 T5.2 人类确认提交并形成证据
【write scope】仅：T5.2 验收脚本/README/evidence、本卡、facts/pitfalls、PLAN/plans/README/LEDGER
【铁律】无静默失败；审批数据先校验；L3 必须人类确认；点击后不重试；证据不得泄露 token
【禁止】外部站点/真实账号/文件/支付/验证码；不改 schema、Policy、Core 审批状态机；不自动 approve
【验收】正式 UI 显示 diff/归因/once；人类确认后 submit 恰好 1 次；JSON/marker/cleanup 全绿
【依赖】TASK-056/343/356/358/359/361/363 已核对为 Done
【疑问】无；人类已明确本会话可进行真实确认
```

### 2. 实际改动文件

- `eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1`：新增正式 desktop-ui 人工验收启动器，
  启动 fixture、Core、专用 Edge 与 Tauri UI，等待 `awaiting_human_confirmation`，批准后验证
  JSON、成功页与清理；token 仅存在于子进程环境。
- `eval/tasks/edge/t5.2/README.md`：记录 UI 人工审批运行方式和安全边界。
- `apps/agent-core/src/browser_task_support.rs`：browser `--ui-approval` handshake window
  从默认 5 秒调整为 30 秒，并加回归断言。
- `apps/desktop-ui/src-tauri/src/event_bridge.rs`：改为无 Tauri event ACL 的 sequenced
  自定义命令事件桥；保留最新事件供 StrictMode 新 listener 重放。
- `apps/desktop-ui/src-tauri/src/lib.rs`：注册事件读取命令。
- `apps/desktop-ui/src/ipc/eventStream.ts`、`eventStream.test.mjs`、`client.ts`：
  轮询并严格校验 sequenced bridge，不开放 `core:event` 权限。
- `eval/tasks/edge/t5.2/evidence/2026-11-12-task-364-desktop-ui-human-approval-v7/`：
  保存成功 run 的命令、pid、pre-approval、result、summary、server shutdown 与人工观察。

### 3. 验收输出摘要

- 失败轮次依次暴露并修复：Start-Process 空格参数、Tauri debug dev URL、Core 5 秒 accept
  timeout、Tauri event ACL 与 StrictMode listener 重放竞态；未发生人工批准前提交。
- v7：CUA 在 `awaiting_human_confirmation` 阶段检测到 Assistant 窗口中的真实用户输入；
  Core 随后写出 `completed`，`submit_form` 恰好一个 `committed`。
- v7 JSON：`submitted=true`，`post_url=http://127.0.0.1:8766/t5.2/submitted`，
  previous / post fingerprint 均存在且不同。
- v7 成功页：HTTP 200 且包含 `submitted-marker`。
- v7 pre-approval：result 不存在；最终 submit step count=1。
- v7 清理：专用 Edge profile 进程 0；fixture pid 已停；8766 listener=false；
  Core / desktop-ui 进程退出。
- 证据目录中 token 环境赋值匹配 0；Edge 版本 `157.0.4341.46`。
- PR #714 checks **11/11 success**，base=`main`，`MERGEABLE` / `CLEAN`；
  merge `59fd555d1973599784604df2b07db2a8e379db35`。
- post-merge main CI `43132393138`：success。

### 4. DoD 逐条核对

- [x] 正式 desktop-ui 显示 T5.2 脱敏 diff、归因、once scope 与不可逆提示。
- [x] 人类确认前无 `form_submit`，确认后恰好提交一次。
- [x] JSON、成功页 marker、进程树与端口清理证据齐全。
- [x] desktop-ui / workspace / 文档门禁通过。
- [x] PR #714 合并并记录 checks / merge / post-merge main CI；进度同步。

### 5. 偏差

- `DRIFT-364-1`：正式 UI 首次实跑发现 5 秒 accept timeout 不足，且 Tauri event ACL
  与 StrictMode 重放使审批卡无法出现。已按 write scope 修复为 30 秒握手窗口与无 ACL
  的自定义命令轮询；未批准前 submit calls 为 0。
- TASK-364 证据轮次最终机器统计为 `shell_misuse=bash=54` / `cmd=0`；此前记录的 44 是
  提交前快照，新增命令均为只读检查、测试/构建、guard 锁操作与 push/poll，未超出本卡
  预期变更。当前批准/收口轮最终为 `bash=3` / `cmd=0`：三次均为误填 shell 参数且被
  ADR-0100 hook 重写，其中一次是 `plans/stage-1-pilots.md` guard release，两次是
  只读的 main CI / PR `gh` 查询；没有产生额外仓库写入。
- `DRIFT-364-2`：实现 PR #714 还改动了 `apps/desktop-ui/src-tauri/src/lib.rs`、
  `apps/desktop-ui/src/ipc/eventStream.ts` 与 `eventStream.test.mjs`，但这三项不在本卡
  write scope。改动用于注册无 ACL 的事件读取命令及其严格校验轮询客户端，未触及
  Tool/UI IPC/DB schema、审批状态机或依赖。人类已于 2026-11-12 明确批准将其作为
  TASK-364 的 write scope 例外，裁决已同步到 `LEDGER.md`。

### 6. 实施中发现的更合理做法

- 正式 desktop-ui 必须通过 `tauri build --debug --no-bundle` 生成嵌入前端资源的二进制；
  直接 `cargo build` 的 debug 窗口会等待开发 URL，不会启动 React/event client。
- 轮询桥保留 sequence；StrictMode 新 listener 可重放最新审批事件，同时不开放 event 插件
  权限，维持空 capabilities。

### 7. 遗留问题

- TASK-057 / TASK-058 的治理状态不在本卡自动改写。

### 8. 新增长期记忆

- FACT：desktop-ui 真实人类 approval 已通过本地 Edge T5.2 一次提交。
- PITFALL：Tauri debug 资产、event ACL 与 StrictMode listener 间隙会共同伪装成 Core pipe 故障。

### 9. 给审阅者的关注点

- 人类看到的是否确为同一份字段 diff 与 once point-of-no-return。
- submit 次数、postcondition、进程树和端口清理是否都有原始证据。
