# 2026-10-20 automation 自删顺序偏差审计

- 审计日期：2026-10-21
- 任务：TASK-326
- 范围：2026-10-20 17:00 / 19:30 / 22:00 / 2026-10-21 00:30 / 03:00 五个 automation
- 结论：五轮均执行并自删，但 **2 轮违反 ADR-0054「先落地，再自删」**

## 1. 证据来源

- `C:\Users\fexfe\.codex\automations-logs\2026-10-20\ai-assistant-task-round-*.jsonl`
- `main` 上的 `docs/automations/2026-10-20-round-1..5.md`
- `main` 上的 `docs/automations/2026-10-20-report.md`
- TASK-321~325 卡与 LEDGER
- GitHub PR #500~#511 的 merge 元数据

所有外部 JSONL 均可解析。5 个 automation 目录均已删除，当前：

| 检查项 | 结果 |
|---|---|
| automation 条目 | 0（只剩 `.run-jitter-salt`） |
| `.automation.lock` | 不存在 |
| open PR | 0 |
| main | `b8e4eb730f920c7057ce249618bfaae50b9319e4` |
| 工作树 | clean |

## 2. 五轮执行结果

| 槽位 | 任务 | 主 PR / merge | 结果 |
|---|---|---|---|
| 17:00 | TASK-321 | #500 / `5e9f453` | docs-only 状态一致性；另有 #501 / #502 回填与更正 |
| 19:30 | TASK-322 | #503 / `247d9c0` | round 1 证据回填；另有 #504 |
| 22:00 | TASK-323 | #505 / `ab34987` | TASK-313 历史对账；另有 #506 |
| 00:30 | TASK-324 | #507 / `863ef36` | 7 张治理卡状态对账；另有 #508 / #509 |
| 03:00 | TASK-325 | #510 / `77c0814` | 五轮审计 + 运行日报告；另有 #511 |

## 3. 两次顺序偏差

### DRIFT-AUTO-20261020-1：17:00 轮

时间线：

- `2026-10-20T17:51:36+08:00`：释放 `.automation.lock`
- `2026-10-20T17:51:44+08:00`：自删 automation
- `2026-10-20T18:08:26+08:00`：PR #502 合并，merge
  `9e990f6cda5977ff22cd03a81910fd7a00add50e`

PR #502 的内容是把 shell 偏差计数从 3 更正为 7，属于本轮仓库留痕。它晚于
self-delete 落地，违反 ADR-0054 D1「先落地，再自删」。

外部日志已经显式记录：

```text
sequence_deviation: evidence correction PR #502 merged after self-delete
```

### DRIFT-AUTO-20261020-2：00:30 轮

时间线：

- `2026-10-21T01:09:10+08:00`：释放 `.automation.lock`
- `2026-10-21T01:10:00+08:00`：自删 automation
- `2026-10-21T01:23:15+08:00`：PR #509 合并，merge
  `ffa1061bf6148928f05900a4ccaae28113faf799`

PR #509 把 TASK-324 / round 4 的 shell 偏差从 18 更正为最终 21 条，属于本轮仓库留痕。
同样晚于 self-delete，构成第二次顺序偏差。

## 4. 其余偏差与平台异常

- 五轮 shell 偏差累计 **61 条**：分别 7 / 4 / 14 / 21 / 15。
- 其中绝大多数为只读查询；round 4 有一次已授权 `git push` 误用 bash，最终分支内容仍在预期范围。
- round 5 的 macOS runner 两次因 runner 分配超时被平台取消，单 job 重跑后通过；
  没有绕过检查或放宽门禁。
- PowerShell / Git TLS 在早期轮次出现握手失败，均以重试或一次性 OpenSSL backend 恢复；
  未改写提交历史。

## 5. 结论与处置

- 5 个任务全部执行并自删，没有残留 automation、锁或 open PR。
- 两次自删顺序偏差是流程事实，不得被“最终 main 含全部证据”覆盖。
- 运行日报告 §4 原先只列 shell 偏差与 TLS 异常，未列出这两条顺序偏差，因此
  本次在报告末尾追加更正，不改写历史 §4。
- 后续 automation 的收尾门必须固定为：所有主 PR / 回填 PR / 更正 PR 已 merged
  → final verification → release lock → self-delete。self-delete 后不得再产生仓库留痕。
