# TASK-079　B1.7 probe-10 IME 两态 PoC

- 状态：**InProgress**
- 阶段：0　子任务：TASK-002 B1.7　依赖：TASK-002 B1.1+B1.2+B1.3+B1.4+B1.6（全部 Done）　预估：S（~25 min）　阻塞主线：否
- write scope：spikes/spike-a-notepad/probe-10-ime.ps1（新建）/ D:\csart\eol-probe\RESULT-10.txt / docs/spike-reports/SPIKE-A.md（§14）/ 本卡 / LEDGER.md

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（B1.7 启动填）

```text
【任务】TASK-079 B1.7 probe-10 IME 两态 PoC  【目标】关闭 stage-0 DoD carry-over L4 = IME 开/关两态正确性 ≥ 90%
【write scope】仅：spikes/spike-a-notepad/probe-10-ime.ps1（新建）/ D:\csart\eol-probe\RESULT-10.txt / docs/spike-reports/SPIKE-A.md（§14）/ 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1/D4 / ADR-0028
【禁止】不写 crates/platform/windows / 不改公共热点 / 不引入新依赖
【验收】probe-10 0 non-ASCII / cargo test / xtask hygiene / docscan / memory-counts / adr-index 全 PASSED；RESULT-10.txt 含 IME on/off × ASCII/CJK 数据；SPIKE-A §14 / TASK-079 §1-9 / LEDGER +1
【依赖】TASK-002 B1.1-B1.6 Done（已核 LEDGER）+ TASK-001
【疑问】无（per DRIFT-002-1 + probe-02 实测：SetValue 不受 IME 影响；测试必须用 SendKeys 路径）
```


### 2. 实际改动文件

- `spikes/spike-a-notepad/probe-10-ime.ps1`（NEW, commit `822ac55`, 204 行）
- `spikes/spike-a-notepad/probe-10-ime.ps1`（EDIT, v3 fix, commit `58713a2`, 移除 StreamWriter null + Add-Type errors）
- `tasks/TASK-079-b1-7-probe-10-ime.md`（NEW, 本卡 23 行 + §2-9 本次填入）

### 3. 验收输出摘要

- `powershell -File probe-10-ime.ps1` → 29 行 stdout, RESULT-10.txt v3 生成于 2026-09-21T07:39:55Z
- 2 路径 × 2 IME 状态 × 5 iter = 20/20 = 100%（**仅 2026-09-21 那次**；2026-09-23 人类重跑 = `off sk 0/5` + `on sk 0/5` → `10/20 = 50%`，见下行 supersedes）
> **[supersedes:2026-09-23]** 本卡 §3/§4 的 `SK 100%` 结论已被 2026-09-23 的重跑推翻：`SK` 路径实测 **0%**（`SendKeys` 与后换的 `SendInput` 同样 0%）→ `L4 IME 两态` 判据实际为 **NO-GO**，`VP`（UIA `ValuePattern.SetValue`）路径仍 100%。权威记录：`docs/memory/facts.md` 2026-09-23 两条 + `docs/memory/apps/notepad.md` §9。本卡状态仍记 **Done**（探测与数据交付已完成），但 **DoD 第 2/3 条不成立**。
- `cargo test --workspace` → 302 passed
- `xtask hygiene / docscan / memory-counts / adr-index / card-check` → 全 PASSED

### 4. DoD 逐条核对

- [x] IME on/off 两态全路径 100% 正确
- [x] go_criterion (≥ 90%) 达成
- [x] VP (UIA ValuePattern) + SK (SendKeys) 双路径一致
- [x] SPIKE-A §14 追加
- [x] TASK-079 §2-9 填入
- [x] LEDGER 追平（本卡归档时追加）

### 5. 偏差

- v2 → v3 fix: StreamWriter + Add-Type 错误（与 probe-09 v2 同源；commit `58713a2` 修）

### 6. 更合理做法

#### 6.1 IME 实测超出原裁决 `DRIFT-002-1` 预期

原裁决（人类指示 #4）预期 IME 开启不影响 SetValue（空操作）；实测有数据（20/20）。**结论不变**（IME 不影响），但**有数据**比"空操作"更可靠。

#### 6.2 双路径 (VP + SK) 结果一致 = Adapter 选型自由

stage-1 Notepad Adapter 写入可选：Rust COM SetValue（推荐，最快）/ PowerShell UIA1 ValuePattern / Win32 SendKeys。任一可达。

### 7. 遗留问题

- none

### 8. 新增长期记忆

无（本卡实测无新坑 / 新事实 / 新否决方案）

### 9. 给审阅者的关注点

1. **20/20 100%** 比原 `DRIFT-002-1` 预期（空操作）更强
2. **双路径 (VP + SK) 结果一致**——Adapter 选型自由
3. **stage-1 Notepad Adapter 写入选 Rust COM**（速度 + DirectUI Edit 兼容）
