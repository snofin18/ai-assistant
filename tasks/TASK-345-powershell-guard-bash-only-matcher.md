# TASK-345　PowerShell guard 仅订阅 Bash 工具

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：治理修复
- 依赖：ADR-0100、TASK-342
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`.codex/hooks.json`、`.codex/hooks/powershell-guard.ps1`、
  `docs/memory/pitfalls.md`、`MEMORY.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`LEDGER.md`

## 目标

把 PowerShell guard 的 `PreToolUse` matcher 从所有工具收窄为仅 `Bash`，并让脚本在
非 Bash 原始 payload 上直接退出，避免 `automation_update` 等 MCP 长输入被无关 hook
解析并 fail-closed。

## In scope

- `.codex/hooks.json` 的 matcher 精确改为 `Bash`；
- guard 脚本先做原始 `tool_name` 快速判断，仅 Bash 才进入 JSON 解析与命令重写；
- 记录该 host bridge 长度/序列化坑，并同步 Review 状态与门禁结果。

## Out of scope

- 不改变 Bash 命令重写、长度上限、PowerShell `-EncodedCommand` 或 deny 语义；
- 不修改 `automation_update`、任何 automation prompt 或 automation 数据；
- 不改变 UserPromptSubmit reset hook；
- 不新增测试框架、依赖或公共 API。

## 必须遵守

1. 非 Bash 输入必须零输出退出，不能返回 allow/deny 决策或修订 `updatedInput`。
2. Bash 输入仍必须经过原 guard；超长、空命令、解析失败继续 fail-closed。
3. matcher 与脚本快速路径必须同时存在，避免配置或脚本单点回归。
4. 不改动 hook 以外的产品行为；不绕过自动化授权或项目治理。

## 验收命令

```powershell
Get-Content -Raw .codex/hooks.json | ConvertFrom-Json | Out-Null
'{"tool_name":"mcp__codex_app__automation_update","tool_input":{"prompt":"not json for bash"}}' |
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .codex/hooks/powershell-guard.ps1
'{"tool_name":"Bash","tool_input":{"command":"Get-Date"}}' |
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .codex/hooks/powershell-guard.ps1 |
  ConvertFrom-Json
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- memory-counts
```

## 完成定义（DoD）

- [ ] matcher 精确为 `Bash`，非 Bash 输入零输出退出。
- [ ] 伪造 Bash `Get-Date` 仍返回 `permissionDecision=allow` 与 `-EncodedCommand` 改写。
- [ ] automation_update 长 prompt 可穿过 PreToolUse，不再被无关 guard 解析。
- [ ] 文档门禁全绿，任务卡与 LEDGER 同步。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-345 PowerShell guard 仅订阅 Bash 工具
【目标】消除 automation_update 长输入被非 Bash guard 误解析并 fail-closed
【write scope】见卡片正文
【铁律】无静默失败；不可信输入先校验；不得放宽 Bash 改写；不得静默扩范围
【禁止】不改 Bash 包装与 deny 语义、不改 automation 数据、不新增依赖
【验收】JSON 解析 + 非 Bash 快退 + Bash 改写 + 5 项文档门禁
【依赖】ADR-0100 / TASK-342 Done
【疑问】无
```

### 2. 实际改动文件

- `.codex/hooks.json`：PreToolUse matcher 由 `.*` 收窄为 `Bash`。
- `.codex/hooks/powershell-guard.ps1`：增加原始 `tool_name` 快退，非 Bash 不解析 JSON。
- `docs/memory/pitfalls.md`、`MEMORY.md`：记录长 MCP payload 被无关 guard 误拒的事实，
  并把 pitfalls 规模同步为 308 行 / 197 条。
- `tasks/TASK-345-powershell-guard-bash-only-matcher.md`、`PLAN.md`、
  `plans/stage-1-pilots.md`、`LEDGER.md`：Review 状态与进度留痕。

### 3. 验收输出摘要

- `.codex/hooks.json` 解析成功，`matcher=Bash`。
- 5 KB 非 Bash payload：脚本 exit 0 且零输出，PASSED。
- 伪造 Bash `Get-Date`：返回 `permissionDecision=allow`，命令包含 `-EncodedCommand`，PASSED。
- `git diff --check`：PASSED。
- `hygiene`：0 error / 153 warnings，PASSED。
- `docscan`：0 error / 175 warnings，PASSED。
- `card-check`：0 error / 35 warnings，PASSED。
- `check-ledger`：0 error / 0 warning，PASSED。
- `memory-counts`：0 error / 1 warning，PASSED（仅既有 Paint 归档阈值告警）。

### 4. DoD 逐条核对

- [x] matcher 精确为 `Bash`，非 Bash 输入零输出退出。
- [x] 伪造 Bash `Get-Date` 仍返回 allow 与 `-EncodedCommand` 改写。
- [x] 长 MCP payload 不再进入 guard 解析；修复合并后将用真实 `automation_update` 复验。
- [x] 文档门禁全绿，任务卡与 LEDGER 同步。

### 5. 偏差

- 创建 automation 时连续三次长 prompt 被旧 matcher 误拒；用短 `mode=view` 确认不是
  scheduling 本身失败，再以本卡修复。真实五任务创建在合并后执行。
- 本会话还存在模型侧误填 `shell: "bash"` 的记录；guard 已重写执行且未通过 bash
  写入产品文件，相关历史计数不回写成零。

### 6. 更合理做法

- 通过 matcher 与脚本快速路径双层收窄，既减少无关 hook 调用，也保证误配置时直接调用
  脚本仍不会解析 MCP 长输入。

### 7. 遗留问题

- 修复合并后重新创建五个一次性 automation，并用真实 create 调用验证长 prompt。

### 8. 新增长期记忆

- `docs/memory/pitfalls.md`：记录 matcher 必须与脚本处理范围一致，避免无关 MCP 输入进入 Bash guard。

### 9. 给审阅者的关注点

- matcher 是否只匹配 `Bash`，以及非 Bash 原始 payload 是否真的零输出退出。
- 原 Bash deny / rewrite 行为是否完全不变。
