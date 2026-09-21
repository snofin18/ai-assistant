# TASK-076　B1.5 probe-08 失败注入 PoC（4 种）

- 状态：**InProgress**
- 阶段：0　子任务：TASK-002 B1.5　依赖：TASK-002 B1.1 + B1.2 + B1.4（Done）　预估：M　阻塞主线：否
- write scope：spikes/spike-a-notepad/probe-08-failure-injection.ps1（新建）/ spikes/spike-a-notepad/README.md（追加）/ docs/spike-reports/SPIKE-A.md（§11 追加）/ D:\csart\eol-probe\RESULT-08.txt（probe 产出）/ 本卡执行记录 / LEDGER.md（追加）

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（B1.5 启动填）

```text
【任务】TASK-076 B1.5 probe-08 失败注入 PoC   【目标】关闭 stage-0 DoD carry-over #5 的剩余部分 = 验证失败注入 4 种下 Adapter 恢复能力
【write scope】仅：spikes/spike-a-notepad/probe-08-failure-injection.ps1（新建 ASCII 探针）/ spikes/spike-a-notepad/README.md（追加）/ docs/spike-reports/SPIKE-A.md（§11）/ D:\csart\eol-probe\RESULT-08.txt / 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1/D4 / ADR-0028（写公共热点前取锁）
【禁止】不写 crates/platform/windows / 不改公共热点（plans/MEMORY/AGENTS/docs/adr/*）/ 不引入新依赖
【验收】cargo test / xtask hygiene / docscan / memory-counts / adr-index 全 PASSED；probe-08 0 non-ASCII；RESULT-08.txt 含 4 场景 × 10 iter 数据；SPIKE-A §11 填入；TASK-076 §1-9 填入；LEDGER +1
【依赖】TASK-002 B1.1 + B1.2 + B1.4（Done, 已核 LEDGER）+ TASK-001 Done
【疑问】无
```

### 2-9 将在 B1.5 实施后填入

<!-- ══ 9 节执行记录填写完毕（B1.5 = 2026-09-21） ══ -->
