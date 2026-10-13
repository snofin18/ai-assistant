# 自动化任务审计：2026-10-10 至 2026-10-13 的 26 个一次性任务

状态：**Audit**（2026-10-13，按用户要求复核 26 个一次性 automation）　范围：
`docs/automations/2026-10-10-round-*.md`、`docs/automations/2026-10-11-round-*.md`、
`docs/automations/2026-10-12-round-*.md`、`LEDGER.md` 与 PR #291~#342。

## 1. 总体结果

- 26 个 automation 均有对应轮次产物；没有 `-skipped.md`。
- 26 个 automation 均已从 `~/.codex/automations/` 自删；当前只剩 `.run-jitter-salt`。
- 仓库级 `.automation.lock` 当前不存在；`xtask guard status` 为 NONE。
- 26 轮关联的 PR #291~#342 全部已合并，无 open PR。
- `main` 工作区干净，HEAD = `263d0d2`。

## 2. 已确认的偏差

### 2.1 `ai-assistant-task-round-20261011-0330` 运行日归错

该任务本地触发时刻为 `2026-10-11 03:30`，按 `docs/automation-charter.md` §11.3 应归属
`2026-10-10`，但产物实际写成了 `docs/automations/2026-10-11-round-1.md`。该偏差已在
`docs/automations/2026-10-10-round-7.md` 与 `docs/automations/2026-10-10-report.md` 中记录。

用户 2026-10-13 裁决：**忽略历史归日错误**；以后建立 automation 时必须在 prompt 中
明确写出运行日规则，并要求 agent 先计算运行日再决定轮次文件路径。

### 2.2 `N > 3` 后仍写代码

章程 §11.3 要求：运行日开始时刻在本地 00:00~11:59 时归属前一天；当运行日
`N > 3` 时，本次运行不写代码，只补/校阶段报告后结束。

实际执行：

- 2026-10-10 运行日：round 5（TASK-261）和 round 6（TASK-263）仍新增了测试代码；
- 2026-10-11 运行日：round 4（TASK-266）到 round 10（TASK-272）仍新增/修改了代码；
- 2026-10-12 运行日：round 3 起正确转为 docs-only。

结论：已合并的测试本身有效，**不回滚**；但“N > 3 后继续写代码”属于流程漂移。
以后 automation prompt 必须复述该停止条件，agent 在计算到 `N > 3` 时只能补报告。

## 3. 补测安排

这些轮次直接新增的覆盖率测试已完成并通过当时门禁，不需要重做。若还需要补测，应按
**新的运行日**重新排期：

- 2026-10-13 本地 12:00 前，运行日仍归属 2026-10-12，`N > 3`，不得写代码；
- 2026-10-13 本地 12:00 后，运行日切为 2026-10-13，可从 `N = 1` 开始正常安排测试卡；
- 优先候选仍以 `docs/automations/2026-10-12-report.md` §③ 的未覆盖模块为准，按
  N=1 / N=2 / N=3 顺序执行，超过 N=3 只收口报告。

## 4. 本地持久日志

Codex 的 automation 条目必须自删，但调度器删除 automation 后，SQLite 中的
`automations` 行与常见 run 记录也会消失；因此本地持久日志必须放在仓库外：

`%USERPROFILE%\.codex\automations-logs\<run-date>\<automation-id>.jsonl`

每轮至少记录：`start`、`gates`、`work`、`pr`、`ci`、`merge`、`lock_release`、
`self_delete`。日志文件不提交仓库，也不随 automation 删除。

历史 26 轮的一次性索引已回填到：

`%USERPROFILE%\.codex\automations-logs\2026-10\index.md`

## 5. 结论

需要修正的是流程记录和未来 prompt，不需要回滚已合并的有效测试。0330 归日错误按
用户裁决不追溯改名；未来 automation prompt 必须显式包含运行日、`N > 3` 停止条件和
本地持久日志要求。
