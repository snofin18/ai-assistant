# TASK-342　PowerShell 强制 guard 与 OpenAI 文档代理回退

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：治理修复
- 依赖：ADR-0099、ADR-0100
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`.codex/hooks.json`、`.codex/hooks/powershell-guard.ps1`、
  `.codex/hooks/user-prompt-reset.ps1`、`tools/fetch-openai-doc.ps1`、
  `docs/adr/0100-powershell-guard-and-openai-proxy-fallback.md`、`docs/adr/README.md`、
  `AGENTS.md`、`docs/automation-charter.md`、
  `docs/automations/codex-automations-operations.md`、`docs/memory/facts.md`、
  `docs/memory/pitfalls.md`、`docs/memory/decisions.md`、
  `tasks/TASK-342-powershell-guard-and-openai-proxy-fallback.md`、`PLAN.md`、
  `plans/stage-1-pilots.md`、`README.md`、`MEMORY.md`、`LEDGER.md`

## 目标

把 Windows 上频繁误用 `shell: "bash"` 从“依赖模型记得写对”改为宿主层可执行护栏：
所有 shell 命令在 `PreToolUse` 中被重写为显式 Windows PowerShell 进程；同时规定 OpenAI
官方文档直连失败时的有界代理回退流程。

## 必须遵守

1. 不依赖模型在每次工具调用里正确填写 `shell`；hook 必须在命令真正执行前重写。
2. `PreToolUse` 看不到 `shell` 字段，不得伪造“能检测并拒绝 bash”的实现。
3. 重写必须使用 UTF-16LE Base64 `-EncodedCommand`，避免 shell 引号、换行与转义歧义。
4. Windows hook 必须从 Git 根定位脚本，不能假设 Codex 从仓库根启动。
5. OpenAI 官方文档直连失败时，使用 `HTTPS_PROXY` / `HTTP_PROXY`，再回退
   `http://127.0.0.1:30000`；代理请求必须有界重试，不得静默伪造内容。
6. 不新增依赖、不改产品公共 API/schema、不读取或记录凭据。

## Out of scope

- 不修改 Codex 应用二进制、工具 schema 或全局代理配置。
- 不自动信任项目 hooks；首次 trust 仍由人类完成。
- 不抓取或保存 OpenAI 文档正文到仓库。

## 验收命令

```powershell
Get-Content -Raw .codex/hooks.json | ConvertFrom-Json | Out-Null
'{"tool_name":"Bash","tool_input":{"command":"Get-Date"}}' |
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .codex/hooks/powershell-guard.ps1 |
  ConvertFrom-Json
powershell.exe -NoLogo -NoProfile -NonInteractive -OutputFormat Text `
  -ExecutionPolicy Bypass -EncodedCommand JABQAHIAbwBnAHIAZQBzAHMAUAByAGUAZgBlAHIAZQBuAGMAZQAgAD0AIAAnAFMAaQBsAGUAbgB0AGwAeQBDAG8AbgB0AGkAbgB1AGUAJwA7ACAARwBlAHQALQBEAGEAdABlAA==
tools/fetch-openai-doc.ps1 -Url https://developers.openai.com/codex/hooks/
codex exec --dangerously-bypass-hook-trust --dangerously-bypass-approvals-and-sandbox `
  -C . "Use the shell tool exactly once and deliberately set shell to bash. Run Get-Date."
cargo fmt --all --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
```

## DoD

- [ ] 事件探针证明 `PreToolUse` 只提供 `tool_name` 与 `tool_input.command`，不提供 `shell`。
- [ ] 故意设置 `shell: "bash"` 的端到端调用仍由显式 PowerShell 成功执行。
- [ ] OpenAI 文档直连失败后，代理回退至少一次返回 HTTP 200 且正文非空。
- [ ] hook 从子目录启动仍可定位脚本。
- [ ] 全部门禁全绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-342 PowerShell 强制 guard 与 OpenAI 文档代理回退
【目标】宿主级重写所有 shell 命令为 PowerShell，并规定官方文档代理回退
【write scope】见卡片正文
【铁律】1 无静默失败 / 2 不可信输入 / 9 不扩范围 / 10 契约先行
【禁止】不依赖模型每次都写对 shell；不修改 Codex 二进制；不记录凭据
【验收】事件探针 + 故意 bash 端到端 + 直连失败代理回退 + 子目录 hook + 全量门禁
【依赖】ADR-0099、ADR-0100
【疑问】无
```

### 2. 实际改动文件

- `.codex/hooks.json`
- `.codex/hooks/powershell-guard.ps1`
- `tools/fetch-openai-doc.ps1`
- `docs/adr/0100-powershell-guard-and-openai-proxy-fallback.md`
- `docs/adr/README.md`
- `AGENTS.md`
- `docs/automation-charter.md`
- `docs/automations/codex-automations-operations.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `docs/memory/decisions.md`
- `tasks/TASK-342-powershell-guard-and-openai-proxy-fallback.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `MEMORY.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `PreToolUse` 探针输入只有 `tool_name=Bash`、`tool_input.command` 等元数据，
  确认没有 `shell` 字段；返回 `updatedInput.shell=powershell` 后宿主仍启动 bash。
- guard 单测样例对 `Get-Date` 返回 `permissionDecision=allow` 和
  `powershell.exe ... -EncodedCommand ...`；损坏 JSON 返回 `permissionDecision=deny`。
- `codex exec` 从 `tasks/` 子目录故意使用 `shell=bash` 运行 `Get-Date`：
  rollout 中原始命令为 `Get-Date`，hook 后实际命令为
  `powershell.exe ... -EncodedCommand ...`，成功返回 `2026年10月29日 11:05:07`。
- `tools/fetch-openai-doc.ps1` 实测 direct 403，proxy-attempt-1 HTTP 200，
  redirect 到 `https://learn.chatgpt.com/docs/hooks`，正文 528794 字符。
- 三个 PowerShell 脚本 parser errors = 0、non-ASCII bytes = 0。
- `hygiene` 0E / 151W；`memory-counts` 0E / 1 existing warning；
  `adr-index`、`docscan`、`card-check`、`check-ledger` 均 0E。

### 4. DoD 逐条核对

- [x] 事件探针证明 `PreToolUse` 只提供 `tool_name` 与 `tool_input.command`，不提供 `shell`。
- [x] 故意设置 `shell: "bash"` 的端到端调用仍由显式 PowerShell 成功执行。
- [x] OpenAI 文档直连失败后，代理回退返回 HTTP 200 且正文非空。
- [x] hook 从 `tasks/` 子目录启动仍可定位脚本。
- [x] 全部门禁全绿（fmt / clippy / workspace / 9 项 xtask 文档门禁 / deny / replay）。

### 5. 偏差

- 本 turn 机器审计到 10 次 `shell: "bash"`：7 次为误填（读取、`rg`、脚本解析、
  memory-counts、fmt/clippy、arch/schema），另 3 次是为验证 interop 与 hook 重写而故意执行；
  所有实际工作随后均以显式 PowerShell 重跑。新 guard 已通过故意 `shell=bash` 的端到端验证。
- `fetch-openai-doc.ps1` 在 Windows PowerShell 5.1 下无法跟随代理链 308，因此明确要求
  PowerShell 7；PowerShell 7 实测通过。

### 6. 更合理做法

- 不再尝试从 hook 猜测或修改不可见的 `shell` 参数；直接重写命令为显式 PowerShell 进程，
  这样对 bash / cmd / powershell 外层选择都成立。

### 7. 遗留问题

- 项目 hooks 首次使用仍需在 Codex `/hooks` 完成持久 trust。
- 若外层 bash 在 sandbox 启动前即被宿主拒绝，guard 无法修复该启动错误；只能留痕，不能伪称执行成功。
- 当前提交后还需重新运行 PR CI 并回填 PR / CI / merge evidence。

### 8. 新增长期记忆

- FACT：`PreToolUse` 不暴露 shell，且 `updatedInput.shell` 被忽略；编码命令重写有效。
- PITFALL：桌面官方文档 5.1 跟随 308 失败，抓取工具必须 PowerShell 7。
- DECISION：ADR-0100。

### 9. 给审阅者的关注点

- 确认 hook 的 deny/allow JSON 合约与官方 PreToolUse 输出一致。
- 确认 `tools/fetch-openai-doc.ps1` 只允许三个官方 host，最终重定向仍校验。
- 确认 hook trust 是部署前置条件，不能把 bypass e2e 当成已持久 trust。
