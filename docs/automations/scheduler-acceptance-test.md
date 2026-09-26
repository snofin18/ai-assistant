# 夜间自动化验收测试（任务计划程序 + `codex exec`）

> ## ⚠ 本清单已被取代（当前**非**主方案），保留为**回退方案**的验收清单
>
> **ADR-0029（2026-09-18）** 把夜间自动化的投递机制**改回 Codex 原生 scheduled tasks**，
> 本文件所依据的 **ADR-0018**（Windows 任务计划程序 + `codex exec`）状态已改为
> `Accepted → Superseded by ADR-0029`，**但方案本身保留可用**（ADR-0029 D4）。
>
> - **当前主方案的验收清单在别处**：`docs/nightly/codex-automations-operations.md` §2（GATE-0）与 §8，
>   纪律部分见 `docs/overnight-automation-charter.md` §11.9（章程 **v1.4**）。
> - **什么时候才用本文件**：只有 GATE-0 确认「本机端点吞不下 automation 的投递方式」、
>   人类决定启用回退方案时。**届时本清单原样可用** —— 下面 10 条 `codex exec` 本机实测
>   已全部保留在章程 §11.8（并在表头注明归属）。
> - **正文不改**：本文件按 ADR-0029 D4「顶部加横幅、正文不改」处理，所以下文仍然按
>   「ADR-0018 是现行方案」的口吻书写；凡与主方案冲突处，**以本横幅与章程 §11 为准**。

> **上位**：ADR-0018（Accepted，**生效前提 = 本清单全绿**）、`docs/overnight-automation-charter.md` §11。
> **当前状态（2026-09-18）：⏳ 未验收** —— 本机还**没有**跑过一次完整的端到端运行。
> **谁执行**：**必须人类在场**。理由：① 首次运行要现场记录退出码基线（§4）；
> ② `codex exec` 若行为异常需要能立刻 Ctrl-C；③ 注册/注销计划任务本身需要人类授权。
> **在此之前不得注册正式任务计划**（章程 §11.9）。

---

## 0. 前置条件（不满足就不要开始）

| # | 条件 | 检查命令 | 期望 |
|---|---|---|---|
| P1 | Codex CLI 可用且版本已知 | `codex --version` | `codex-cli 0.154.0-alpha.6.2`（**变了就必须重跑本清单** —— ADR-0018 风险表：CLI 升级可能改变 `exec` 行为） |
| P2 | 仓库路径已在 `config.toml` 的 `[projects]` 里标为 trusted | 读 `C:\Users\fexfe\.codex\config.toml` | `trust_level = "trusted"` |
| P3 | `model_context_window` 已配置 | 同上 | `1000000`（PL-014：`qwen3.8-max` metadata 缺失时会被 fallback 成 828400） |
| P4 | 工作区干净、当前在 `main` | `git status --porcelain` / `git branch --show-current` | 空输出 / `main` |
| P5 | 没有残留锁 | `Test-Path <repo>\.nightly.lock` | `False` |
| P6 | 全库 rollout 无缺 `call_id` 的毒项 | `python tools\fix_codex_callid.py --dry-run`（或等价扫描） | `total 0` |

---

## 1. 冒烟 A：`codex exec` 最小往返（**不碰仓库**）

目的：先证明"CLI 能正常往返、投递机制干净"，再谈调度。任何一步失败都**不要**继续。

```powershell
# 只读沙箱 + JSONL 事件流 + 落最后一条消息；不落任何仓库改动
codex exec -C D:\csart\ai-assistant --skip-git-repo-check `
  -s read-only --json `
  -o $env:TEMP\nightly-smoke-lastmsg.txt `
  -c model_context_window=1000000 `
  "回报你实际使用的 model 与 model_context_window 数值，然后只输出 OK"
$code = $LASTEXITCODE
```

| # | 断言 | 判定方法 | 期望 |
|---|---|---|---|
| A1 | 退出码 | `$code` | **记入 §4 基线表**（官方无文档，见章程 §11.8 第 10 条）。预期 `0` |
| A2 | 有最后一条消息 | `Get-Content $env:TEMP\nightly-smoke-lastmsg.txt` | 非空，且含 `OK` |
| A3 | JSONL 事件流里**没有**畸形投递 | 在 `%CODEX_HOME%\sessions\**\rollout-*.jsonl` 里找本次 session，统计 `function_call_output` | **计数 0**；且无 `name:"automation_update"`、无 `at_` 前缀 id |
| A4 | 实际上下文窗口没被 fallback | 日志/事件里的 `model_context_window` | **1000000**（不是 828400 / 258400） |
| A5 | 仓库未被改动 | `git status --porcelain` | 空输出 |
| A6 | 新建了独立 session（不复用） | 本次 rollout 文件名里的 session id ≠ 任何既有会话 | 全新 id |

> **A3 是整份清单里最重要的一条**：它直接证明"换成 CLI 之后，`Invalid 'call_id'` 这个故障模式消失了"。
> 原始故障见 ADR-0018 背景节与 `LEDGER.md` 2026-09-17。

---

## 2. 冒烟 B：包装脚本的锁与超时语义（**可以不调 CLI**）

目的：`scripts/nightly-run.ps1`（ADR-0018 影响 #2）的三条控制流必须各自被单独验证，
否则"跑通一次"只能证明快乐路径。用 `-DryRun` 参数把 `codex exec` 换成 `Start-Sleep`。

| # | 场景 | 构造方法 | 期望 |
|---|---|---|---|
| B1 | 正常路径 | `-DryRun -SleepSeconds 5` | 建锁 → 干活 → **删锁**；`Test-Path .nightly.lock` 结束为 `False`；退出码 0 |
| B2 | 干活的进程失败 | `-DryRun -FakeExitCode 3` | 脚本**仍然删锁**（`try/finally`）；退出码 = 3（**必须透传**，否则任务计划的 `LastTaskResult` 永远是 0 → §11.6 的①层信号失效） |
| B3 | 硬超时 | `-DryRun -SleepSeconds 600 -TimeoutMinutes 1` | 到点 kill 子进程；**删锁**；退出码非 0；日志里写明"硬超时" |
| B4 | 锁冲突（新鲜锁） | 先手工写一个 `run_started_at` = 当前时刻的 `.nightly.lock`，再跑 `-DryRun` | **立即结束**，不改任何代码、不提交；`docs/nightly/<date>-skipped.md` 追加一行；结束后**原锁仍在**（不能把别人的锁删了） |
| B5 | 陈旧锁 | 手工写一个 `run_started_at` = 4 小时前的锁，再跑 `-DryRun` | 旧锁内容**原样抄进** skipped 记录 → 覆盖为新锁 → 继续 → 结束删锁 |
| B6 | 日志首行内容 | 任一上述场景 | 日志首行含：`codex --version` 输出、开始时刻、`-s` 沙箱值、`model_context_window` 实际值 |

---

## 3. 冒烟 C：任务计划注册与真实触发（**需要人类授权**）

```powershell
$action    = New-ScheduledTaskAction -Execute "powershell.exe" `
               -Argument "-NoProfile -ExecutionPolicy Bypass -File D:\csart\ai-assistant\scripts\nightly-run.ps1" `
               -WorkingDirectory "D:\csart\ai-assistant"          # ← schtasks.exe 做不到这一条（章程 §11.8 第 9 条）
$trigger   = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(3)   # 冒烟用一次性触发；正式用 Daily 23:30 / 02:30
$settings  = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 90) `
               -MultipleInstances IgnoreNew                        # StartWhenAvailable 刻意不设（保持默认 False）
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName "ai-assistant-nightly-SMOKE" `
               -Action $action -Trigger $trigger -Settings $settings -Principal $principal
```

| # | 断言 | 判定方法 | 期望 |
|---|---|---|---|
| C1 | 注册成功且设置真的落地 | `Get-ScheduledTask -TaskName ai-assistant-nightly-SMOKE \| Select -Expand Settings` | `ExecutionTimeLimit = PT1H30M`、`StartWhenAvailable = False`、`MultipleInstances = IgnoreNew`、`RestartCount = 0` |
| C2 | 到点真的触发 | `Get-ScheduledTaskInfo -TaskName ai-assistant-nightly-SMOKE` | `LastRunTime` ≈ 触发时刻；`LastTaskResult` = **0** |
| C3 | 工作目录正确 | 日志里打印 `(Get-Location).Path` | `D:\csart\ai-assistant`（**不是** `C:\Windows\System32` —— 这是 `-WorkingDirectory` 缺失时的典型症状） |
| C4 | 端到端产物齐全 | 检查文件 | `docs/nightly/logs/<date>-<HHMM>.log` + `<date>-round-1.md` + `LEDGER.md` 追加行 + **`.nightly.lock` 不存在** |
| C5 | 投递机制干净 | 同 A3 | `function_call_output` 计数 **0** |
| C6 | **睡眠/关机不补跑** | 把触发时间设到过去、机器睡 5 分钟再唤醒，看是否自动跑 | **不跑**（`StartWhenAvailable=False` 的预期行为，章程 §11.7 ①） |
| C7 | 冒烟任务已清理 | `Unregister-ScheduledTask -TaskName ai-assistant-nightly-SMOKE -Confirm:$false` | `Get-ScheduledTask` 查不到 |

> **C6 是"反向断言"**：它验证的是"我们**故意**不要补跑"。若不写这一条，将来有人
> 把 `StartWhenAvailable` 打开时会以为"改进了"，实际上是引入了 §11.5 的锁冲突风险。

---

## 4. `codex exec` 退出码基线（**自建事实源**，章程 §11.8 第 10 条）

官方文档没有退出码语义 → **首次冒烟时把观察值填进来，此后本表就是本项目的事实源**。
任何与本表不符的新退出码 = 需要调查的异常，不是"正常波动"。

| 退出码 | 观察到的场景 | 首次观察日期 | 脚本该如何处理 |
|---|---|---|---|
| 0 | *（待填：冒烟 A 正常完成）* | — | 成功；仍要校验 A2~A6 |
| *（待填）* | *（待填）* | — | — |

> 填表规则：**只追加**。同一个码在不同场景出现 → 加一行，不要改写既有行。

---

## 5. 判定与后续

- **全绿** → ① 把本文件的「当前状态」改为 ✅ 已验收 + 日期 + 执行人；
  ② ADR-0018 的"生效前提"标注为已满足；③ 章程 §11.9 的状态改为 ✅；
  ④ 才可注册**正式**的每日 23:30 / 02:30 任务；⑤ `LEDGER.md` 追加一行；
  ⑥ `docs/memory/open.md` N4 / N5 追加 `[supersedes]` 结论行。
- **任一项失败** → 不注册正式任务；把失败项与日志尾部记入 `docs/PARKING_LOT.md`，
  并判断是否触发 ADR-0018「重新评估的触发条件」（连续 3 晚失败 → 暂停夜间自动化，回到纯白天推进）。

## 6. 尚未实现的前置物（**执行本清单前必须先有**）

| 物 | 状态 | 归属 |
|---|---|---|
| `scripts/nightly-run.ps1`（包装脚本：锁 → `codex exec` → 日志 → finally 释放锁 → 透传退出码） | ❌ **未创建** | ADR-0018 影响 #2；顶层目录 `scripts/` 由 ADR-0018 授权（gov §5.4「新增顶层目录必须在 ADR 白名单中」） |
| `scripts/` 顶层目录 | ❌ 未创建 | 同上 |
| `docs/nightly/logs/`（**注意**：日志内容含本机路径，是否入库需先裁决） | ❌ 未创建 | `.gitignore` 目前只忽略 `.nightly.lock`；**建议**日志也忽略、只把摘要写进报告 → 待裁决 |
| 正式任务计划（每日 23:30 / 02:30） | ❌ 未注册（**刻意**，见 §5） | 人类操作 |
