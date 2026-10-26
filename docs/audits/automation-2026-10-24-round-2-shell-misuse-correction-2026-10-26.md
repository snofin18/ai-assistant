# 2026-10-24 round 2 shell misuse 证据更正

日期：2026-10-26　关联：TASK-328、TASK-114、2026-10-24 round 2

## 1. 结论

2026-10-24 round 2 的 `shell_misuse` 最终值应为 **36**。

历史文件中出现的 28 与 32 都是运行过程中的中间值，不是 automation 收尾时的最终值。
本次不改写历史 round 报告或既有 LEDGER 行，只追加本审计与新的 LEDGER 更正行。

## 2. 证据

外部 JSONL：

`C:\Users\fexfe\.codex\automations-logs\2026-10-24\ai-assistant-task-round-20261024-2300.jsonl`

关键事件顺序：

1. `gates` 行记录 `shell_misuse=25`。
2. 第一次 `shell_misuse_correction` 记录 `shell_misuse=27`。
3. 合并后第二次 `shell_misuse_correction` 记录 `shell_misuse=32`。
4. 最后一条 `final_state` 记录 `clean=true`、`guard=NONE`、`shell_misuse=36`。

JSONL 以追加顺序为事实；第 4 条是 round 2 外部日志的最终收尾状态，之后没有更高计数或
新的更正事件。

仓库内历史记录：

- `docs/automations/2026-10-24-round-2.md` §7 停留在较早的 `shell_misuse=28`。
- `LEDGER.md` 第 759 行记录 `shell_misuse=32`。

两者都能解释为提交时的中间快照，但不能覆盖外部 JSONL 最后的 `final_state=36`。

## 3. 影响

- 这些误用均为只读命令、锁创建或 git/gh 查询；外部日志明确记录未产生成功仓库写入，
  未伪造验收结果。
- 计数错误不改变 TASK-114 的代码、CI、merge 或功能结论。
- 计数错误影响治理审计的可信度，因此必须追加更正，不能静默保留。

## 4. 处置

- 最终采用 `shell_misuse=36`。
- 不修改历史 round 2 文件。
- `LEDGER.md` 追加一条 `[supersedes:2026-10-24]` 更正行。
- 后续 automation 必须把 PowerShell 零输出、fallback 与每次误用实时写入外部 JSONL，
  收尾时从最终事件而不是中间报告抄数。
