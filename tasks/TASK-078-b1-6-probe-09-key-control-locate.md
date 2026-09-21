# TASK-078　B1.6 probe-09 关键控件定位率 PoC

- 状态：**InProgress**
- 阶段：0　子任务：TASK-002 B1.6　依赖：TASK-002 B1.1+B1.2+B1.3+B1.4(B1.4 v3) Done　预估：S（~30 min）　阻塞主线：否
- write scope：spikes/spike-a-notepad/probe-09-key-control-locate.ps1（新建）/ D:\csart\eol-probe\RESULT-09.txt / docs/spike-reports/SPIKE-A.md（§13）/ 本卡 / LEDGER.md

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（B1.6 启动填）

```text
【任务】TASK-078 B1.6 probe-09 关键控件定位 PoC  【目标】关闭 stage-0 DoD carry-over = 关键控件定位 ≥ 90%
【write scope】仅：spikes/spike-a-notepad/probe-09-key-control-locate.ps1（新建）/ D:\csart\eol-probe\RESULT-09.txt / docs/spike-reports/SPIKE-A.md（§13）/ 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1/D4 / ADR-0028
【禁止】不写 crates/platform/windows / 不改公共热点 / 不引入新依赖
【验收】probe-09 0 non-ASCII / cargo test / xtask hygiene / docscan / memory-counts / adr-index 全 PASSED；RESULT-09.txt 含 6 控件 × 10 iter 数据；SPIKE-A §13 / TASK-078 §1-9 / LEDGER +1
【依赖】TASK-002 B1.1-B1.4 Done（已核 LEDGER）+ TASK-001
【疑问】无
```

### 2-9 在 probe-09 实施后填入
