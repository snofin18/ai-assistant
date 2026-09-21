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

### 2-9 在 probe-10 实施后填入
