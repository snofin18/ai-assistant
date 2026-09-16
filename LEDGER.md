# LEDGER.md — 事件台账（只追加，不改写）

> **用途**：回答"哪张卡、在哪个 commit、验收结果如何、有没有偏差"。
> **与 `MEMORY.md` 的分工**：这里记**事件**（发生过什么），`MEMORY.md` 记**认知**（因此我们知道了什么）。
> **写法**：只追加。一行一事件。已写入的行**不得修改或删除**；写错了就追加一行更正并在备注里指向被更正的行。
> 模板见 `docs/governance-ai-agent-execution.md` §9.2。

## 状态取值

| 状态 | 含义 |
|---|---|
| `Done` | 卡内 DoD 全部满足，验收命令全绿，已提交（等待或已完成人类合并） |
| `Review` | 已提交，等待独立 review agent / 人类审阅 |
| `Blocked` | 被前置条件或 DRIFT 卡住，未提交 |
| `Abandoned` | 决定不做（必须在备注写明理由与批准人） |

## 台账

| 日期 | 卡号 | 状态 | commit | 验收命令结果 | 偏差 | 备注 |
|---|---|---|---|---|---|---|
| 2026-09-16 | TASK-001 | Review | 待填（见本卡执行记录） | 5/6 通过（`cargo deny check` 未执行：cargo-deny 未安装） | DRIFT-001-1、DRIFT-001-2、DRIFT-001-3 | 仓库骨架 + xtask 护栏 v0；详见 `tasks/TASK-001-repo-skeleton.md` |