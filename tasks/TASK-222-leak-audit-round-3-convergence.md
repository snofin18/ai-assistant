# TASK-222　第三轮泄露审计：真机长跑收敛测量

- 状态：**Done（2026-10-02，首轮收敛测量）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-220、TASK-221、ADR-0063
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`eval/tasks/notepad/measure-resource-convergence.ps1`、`docs/audits/leak-audit-round-3-convergence.json`、ADR-0063

---

## 目标（一句话）

按 ADR-0063 的真实运行要求，连续跑 N 次真实 UIA 任务并采样进程数/句柄/工作集，验证泄漏防护真的收敛。

## write scope

- `eval/tasks/notepad/measure-resource-convergence.ps1`
- `docs/audits/leak-audit-round-3-convergence.json`
- `tasks/TASK-222-leak-audit-round-3-convergence.md`（本文件）
- `LEDGER.md` / `plans/stage-1-pilots.md`（本卡标记 + 「当前进度」句）/ `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- 真机脚本：连续运行指定 T1.x 真实 UIA 用例 N 次，每轮采样 powershell/notepad 进程数、harness 句柄数与工作集、系统可用内存。
- 收敛判定：后半段进程数不高于基线、工作集不继续增长。
- 记录测量口径与已知局限。

## Out of scope（做了算漂移）

- 改运行时/平台实现（本轮只测量）。
- 引入第三方监控/压测依赖。

## 必须遵守

- **铁律 1**：测量结果必须如实记录，增长未落平就写增长，不得用阈值掩盖。
- ADR-0063：这是"新卡必须附资源收敛证据"的首个执行实例。

## 验收命令

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File eval/tasks/notepad/measure-resource-convergence.ps1 -Task t1.1 -RepeatRuns 30
python -c "import json; json.load(open('docs/audits/leak-audit-round-3-convergence.json', encoding='utf-8'))"
```

## 完成定义（DoD）

- [x] 连续 30 次真实 UIA 全部成功并落采样 JSON。
- [x] 进程数收敛（powershell / notepad 均回到基线）。
- [x] JSON 无 BOM、可被 Python 解析。
- [x] 已知局限写入本卡：当前采样对象是**父 PowerShell harness**，不是 agent 进程本身。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-222 第三轮泄露真机收敛测量
【目标】连续真实 UIA 运行并验证资源收敛
【write scope】收敛脚本 + 审计 JSON + 本卡与状态同步文件
【铁律】1 如实记录
【禁止】改良测量以掩盖增长；改运行时实现
【验收】30 次真实 UIA + JSON 可解析
【依赖】TASK-220 / 221 已合并；ADR-0063 已 Accepted
【疑问】无

### 2. 实际改动文件

- `eval/tasks/notepad/measure-resource-convergence.ps1`（新增）
- `docs/audits/leak-audit-round-3-convergence.json`（新增，30 次采样）

### 3. 验收输出摘要

```text
t1.1 x30 全部成功
verdict=converged powershell=1->1 notepad=0->0 working_set_mb=76->103
发现：
  powershell 进程数 1 -> 1（收敛，无 t1.1 fixture 需启动 PS；始终只有本会话）
  notepad 进程数   0 -> 0（收敛，fixture 每轮任务结束无残留 notepad）
  harness 句柄 min/max 623/744，无单调增长
  harness 工作集 76 -> 103 MB；前半均值 95，后半均值 99（+4MB，未完全落平）
```

### 4. DoD 逐条核对

- [x] 30 次真实 UIA 全部成功。
- [x] 进程数收敛（PS / notepad 均回到基线）。
- [x] JSON 无 BOM、Python 可解析、`docscan` 通过。
- [x] 已知局限已记录。

### 5. 偏差

**DRIFT-222-1（测量口径）**：采样对象是**父 PowerShell harness 进程**（脚本自身），
不是被 `cargo test` 启动的 agent 测试进程。因此 76→103 MB 的增长**不能直接归因于
agent**：父进程在 30 轮里持续 `spawn` cargo 并累积自身的 .NET/管道状态。
进程数与句柄数仍有意义（它们反映整机残留），但工作集必须改测目标进程才算数。

### 6. 更合理做法

下一轮应测**单个长驻 agent 进程**：在同一次 `assistant-agent-core` 运行里连续执行 N 个任务，
在进程内用 `GetProcessMemoryInfo` / `GetProcessHandleCount` 采样自身，或者由父进程采样
子进程 PID 而**不采样自己**。

### 7. 遗留问题

- **agent 进程内测量已完成（后半段）**：`test_production_resource_convergence_over_real_uia` 连续跑 20 次真实 UIA，
  采样自身工作集与句柄：
  - **工作集 6→30 MB 后落平**（warmup 后稳定在 29–30 MB），无增长；
  - **句柄线性增长 +4/次**（125→326，20 次无拐点）——**真信号**，不是 warmup。
- **对照实验已定位到 fixture 子进程路径**（`test_production_resource_convergence_control_no_fixture`）：
  不启动任何 fixture、只重复采样自身时，句柄 **恒为 125（20 次零增长）**。
  → 泄漏来自「启动/回收 fixture 子进程」这条路径（每次 `Command::spawn` + `taskkill` + `wait` 多留 4 个句柄），
  不在 `assistant-agent-core` 本身的惰性初始化里。
  下一次会话需枚举具体句柄类型（进程 / 管道 / 控制台）并修复该路径；候选点在
  `apps/agent-core/tests/production_root_uia.rs` 的 `start_fixture` / `terminate_process_tree`。
- `KeyringSecretStore` 的 OS 句柄释放仍未测。

### 8. 新增长期记忆

- **FACT**：30 次真实 UIA t1.1 后 powershell/notepad 进程数回到基线；harness 工作集 76→103 MB 但后半段趋于平缓，需换测目标进程才能定论。
- **FACT**：在 agent 测试进程内连续跑 20 次真实 UIA，工作集 warmup 后稳定在 29–30 MB；句柄 +4/次线性增长，且对照实验（不启动 fixture）句柄恒为 125 → 泄漏定位在 fixture 子进程启动/回收路径。

### 9. 给审阅者的关注点

1. 本轮只证明"进程数收敛"，**没有证明 agent 自身内存收敛**；不要把它读成内存无泄漏。
2. 收敛脚本的采样对象是父 harness，这是下一步要修的口径，不是实现缺陷。
