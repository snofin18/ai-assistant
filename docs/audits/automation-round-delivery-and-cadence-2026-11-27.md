# Automation round delivery and cadence audit

日期：2026-11-27

## 1. 背景

2026-11-26 五个 automation 槽位均完成业务任务，但仓库没有对应的五个
`docs/automations/2026-11-26-round-*.md`。外部 JSONL 与 GitHub PR/CI 仍可证明任务
实际执行，但 `automation-rounds --run-date 2026-11-26` 返回：

```text
existing_rounds_for_run_date=0
next_round=1
n_auto=0
missing_before_latest_human=61
```

本次不重建历史文件，只把“实际产出 round 文件”升级为每个新 automation prompt 的硬交付物。

## 2. 近期槽位完成时间

数据源：

- `C:\Users\fexfe\.codex\automations-logs\2026-11-25\*.jsonl`
- `C:\Users\fexfe\.codex\automations-logs\2026-11-26\*.jsonl`

统计规则：

- `runtime_min` 优先取首个有效事件时间到 final/最后有效事件时间。
- 缺少开始或 final 时间戳时，使用该 JSONL 文件最后写入时间作为最终完成时刻。
- `wall_min` 从计划槽位时刻算到最终完成时刻。

| Automation ID | 计划 | 首个事件 | 最终完成 | runtime_min | wall_min |
|---|---:|---:|---:|---:|---:|
| `ai-assistant-task-round-20261125-1700` | 11-25 17:00 | 11-25 18:31 | 11-25 20:22 | 111.6 | 202.7 |
| `ai-assistant-task-round-20261125-1930` | 11-25 19:30 | 11-25 19:52 | 11-25 20:21 | 29.9 | 51.9 |
| `ai-assistant-task-round-20261125-2200` | 11-25 22:00 | 11-25 22:00 | 11-26 00:50 | 169.8 | 170.7 |
| `ai-assistant-task-round-20261126-0030` | 11-26 00:30 | n/a | 11-26 02:11 | n/a | 101.8 |
| `ai-assistant-task-round-20261126-0300` | 11-26 03:00 | 11-26 03:26 | 11-26 04:23 | 56.6 | 83.0 |
| `ai-assistant-task-round-20261126-1700` | 11-26 17:00 | 11-26 17:49 | 11-26 18:48 | 59.1 | 108.5 |
| `ai-assistant-task-round-20261126-1930` | 11-26 19:30 | n/a | 11-26 21:37 | n/a | 128.0 |
| `ai-assistant-task-round-20261126-2200` | 11-26 22:00 | n/a | 11-26 23:58 | n/a | 118.0 |
| `ai-assistant-task-round-20261127-0030` | 11-27 00:30 | 11-27 00:58 | 11-27 03:09 | 131.5 | 159.5 |
| `ai-assistant-task-round-20261127-0300` | 11-27 03:00 | 11-27 03:08 | 11-27 04:13 | 64.9 | 73.3 |

可计算 runtime 的 7 个槽位中：

- 最大 runtime：169.8 分钟（约 2 小时 50 分）
- 中位 runtime：64.9 分钟
- 最大 planned-to-final：202.7 分钟（约 3 小时 23 分）

## 3. 间隔建议

2.5 小时间隔小于近期最大 runtime，已经造成 00:30 与 03:00 槽位重叠。

建议新建五槽批次时默认使用 **3 小时 30 分间隔**，并把第一槽设为 **17:15**：

```text
17:15 -> 20:45 -> 00:15 -> 03:45 -> 07:15
```

理由：

- 相对最大 runtime 仍有约 40 分钟缓冲。
- `00:15` 避开 00:00 运行日边界。
- `07:15` 距 08:00 reset 边界仍有 45 分钟。
- `.automation.lock` 仍是硬互斥判据，间隔只能降低重叠概率。

该建议不改变 ADR-0046 的“不同 automation 间隔至少 2 小时”已决下限。若某轮再次出现
超过 3 小时的 planned-to-final，下一批应改用 4 小时间隔或减少槽位数量。

## 4. Round 文件硬门禁

后续 automation prompt 必须逐字包含下列交付要求：

```text
【本轮硬交付物：round 文件】
1. 按 ADR-0088 计算 run-date，并运行
   cargo run -p xtask -- automation-rounds --run-date <run-date>。
2. 必须创建 docs/automations/<run-date>-round-<N>.md；路径占用则顺延，禁止覆盖。
3. round 文件必须通过 PR 合并到 origin/main，并在 final 审计中记录：
   round_file / round_pr / round_merge / round_main_ci。
4. 只有 origin/main 已包含 round 文件及合并证据后才允许执行 self_delete。
5. 若无法落地 round 文件，必须记录 skipped/failed 原因并保留 automation，禁止静默自删。
```

建任务后必须读取 `~/.codex/automations/<id>/automation.toml`，确认 prompt 含上述字段；
缺失时必须 update 或删除重建，不得保留缺少硬门禁的 `ACTIVE` automation。

## 5. 外部目录清理

清理根：

```text
C:\Users\fexfe\.codex\automations
```

删除的孤儿目录：

- `ai-assistant-task-round-20261023-2350`
- `ai-assistant-task-round-20261123-0030`

两个目录均只有 `memory.md`，没有 `automation.toml`。删除前已解析绝对路径并确认位于清理根内。

保留：

- `.run-jitter-salt`
- `human-session-reset.v1`

## 6. 结论

- 历史 round 缺失无法追溯修复，保持事实记录。
- 新 automation 的 round 文件要求已提升为 prompt 硬门禁和自删前置条件。
- 近期数据支持把五槽默认间隔从 2.5 小时提高到 3.5 小时。
- 外部孤儿 automation 目录已清理，运行时控制文件保留。
