# Automation round evidence audit — 2026-11-16

## 1. 结论

2026-11-16 的五个 automation 槽位均真实执行并留下永久 JSONL，但仓库当时只生成了
`docs/automations/2026-11-16-report.md`，没有生成 ADR-0054 要求的
`docs/automations/2026-11-16-round-{1..5}.md`。

因此回填前 `automation-rounds` 的 `0 / 1 / 0` 是**按当前契约输入得到的正确结果**：

- `existing_rounds_for_run_date=0`
- `next_round=1`
- `n_auto=0`

缺陷位于 automation 收口执行：轮次文件没有先进入 `main`。本次新增五份基于永久 JSONL
的事实文件后，按真实 automation rollout 来源派生为：

- `existing_rounds_for_run_date=5`
- `next_round=6`
- `n_auto=5`

当前检查本身发生在人类会话，ADR-0099 会把 `N_auto` 自动重置为 0。因此实际门禁应同时
展示 `existing_rounds=5 / next_round=6` 与 `n_auto=0 / reset_source=human-session`；
用本轮真实 automation rollout 构造只读临时 session home 后，应得到
`session_source=automation / n_auto=5`。

## 2. 一手证据

永久日志目录：

`C:\Users\fexfe\.codex\automations-logs\2026-11-16\`

| 槽位 | automation id | 永久 JSONL | 当前 automation 目录 |
|---|---|---|---|
| 1/5 | `ai-assistant-task-round-20261116-1700` | 存在 | 不存在（已自删） |
| 2/5 | `ai-assistant-task-round-20261116-1930` | 存在 | 不存在（已自删） |
| 3/5 | `ai-assistant-task-round-20261116-2200` | 存在 | 不存在（已自删） |
| 4/5 | `ai-assistant-task-round-20261117-0030` | 存在 | 不存在（已自删） |
| 5/5 | `ai-assistant-task-round-20261117-0300` | 存在 | 不存在（已自删） |

运行前 `~/.codex/automations/` 只余 `.run-jitter-salt`、一个更早的残留目录和
`human-session-reset.v1`；不包含本轮五个 automation。仓库启动时 `.automation.lock`
不存在，主工作树 `main...origin/main` 干净。

## 3. 槽位事实

| 槽位 | 任务/结果 | 主 PR / merge | closeout / correction | 主要偏差 |
|---|---|---|---|---|
| 1 | TASK-373 Done | #736 / `2fab59f` | #737 / `bd52a9d` | bash 误填 35；PowerShell 与 ADR-0100 guard |
| 2 | TASK-374 Done | #738 / `7e5805c` | #739 / `bf1b231` | bash 误填 12（parent 11 + subagent 1） |
| 3 | TASK-375 Done | #740 / `c5cb52f` | #741 / `d4b8add` | bash 误填 30；PowerShell 零输出后回退 cmd |
| 4 | TASK-376 Blocked | #742 / `5ebbf9c` | #743 / `bdf6787` | 首次 main CI macOS 队列超时；bash 误填 3 |
| 5 | TASK-377 Done | #744 / `9b2b9d8` | #745 / `01cf0b5`；#746 / `a9f37ea` | bash 误填 3；PowerShell 零输出后回退 cmd |

所有实现 PR 与 closeout/correction PR 均记录为 11/11 checks success、pending 0、failed 0，
base=`main`。Slot 4 与 Slot 5 的首次 main CI 各出现一次 macOS runner 排队取消；failed-job
重跑或 correction 后的最终 main CI 均为 success。

## 4. 根因

1. 五轮执行都把业务/治理工作、PR 与 CI 证据写进了永久 JSONL。
2. 五轮都按“先合并、再自删”完成了自删，但没有把本轮 `round-N.md` 放进任一主 PR 或
   docs-only 收口 PR。
3. `automation-rounds` 只读取 `main` 上的轮次文件，不读取仓库外 JSONL；这是 ADR-0054 D3
   为避免工作区未合并文件撞号而设定的正确契约。
4. 因而外部执行事实与仓库派生计数分叉：工作确实完成，`N_auto` 与 `next_round` 却一直回到
   初始值。运行日报告只能并列披露两种事实，不能自行改写机器输出。

另发现一个独立的实现偏差：`xtask/src/automation_rounds.rs` 当前通过
`scan_round_files(&repo_root.join("docs/automations"))` 读取工作区，而 ADR-0054 D3
明确要求只按 `origin/main` / `main` 上已合并的 `round-*.md` 计数。该偏差会让本轮尚未
提交的轮次文件立即把 `existing_rounds` 从 0 变成 5，也可能让并发会话把未合并文件当作
已占用轮次。它不是本次五个 JSONL 漏写的原因，但属于必须另卡修复的计数来源缺陷。

## 5. 处置

- 新建 `docs/automations/2026-11-16-round-1.md` 至 `round-5.md`。
- 每份文件只使用对应永久 JSONL、GitHub PR/CI/merge 元数据和运行日报告可证实的字段。
- 对日志未记录的 base commit 或 CI run id 明确写“未记录”，不补造。
- 在 `docs/automations/2026-11-16-report.md` 末尾追加回填说明，不改写既有段落。
- 不改 `automation-rounds`、ADR、spec、`protocol/**`、`adapters/**` 或产品代码。

## 6. 复发防护方案

建议后续独立治理卡实现以下三道修复，并先走 ADR/spec 或现有治理文档授权的范围：

1. **automation 最终审计硬前置**：任何 `机制=automation` 的槽位在请求自删前，必须证明
   本轮 `docs/automations/<run-date>-round-<N>.md` 已在其主 PR 或 closeout PR 中，并在
   JSONL 记录 round path、PR 与 merge commit；不存在时保持 automation 存活并报告失败。
2. **运行日一致性检查**：报告生成时比较永久 JSONL 数量与 `main` 上的 automation round
   数量；差异必须作为失败性审计项输出，而不是只写说明。是否把该检查接入 `xtask` 硬门禁，
   需另卡评估其只读边界、外部路径可移植性与 CI 可运行性。
3. **修正计数来源**：将 `automation-rounds` 的轮次枚举改为只读取已解析的
   `origin/main` 树，或先 fetch 后读取 `main`；工作区未跟踪、已修改、仅本地提交的
   round 文件不得参与计数。至少增加 untracked、staged、branch-only、deleted-unmerged
   与正常 merged 五类负向/正向测试，并保留 ADR-0054 D3 的 fail-closed 行为。

不得用放宽 ADR-0054、让工作区轮次文件参与计数、或默认猜测缺失轮次的方式修复。

## 7. TASK-376 待裁决建议（不构成 Accepted 契约）

建议人类一次性批准以下六项，随后 TASK-376 才可转为 Ready：

1. `adapter.toml` 六个 `[bundle]` 路径均必填，且解析后的路径必须留在 bundle root 内。
2. Rich App Map 是源，core App Map v1 是确定性生成投影，不得人工分叉维护。
3. 接受两个显式 selector model：`uia_candidate_chain_v1` 与 `shell_target_v1`；DOM selector
   不进入这两类文件。
4. 只为 read tool 增加 `none_readonly`；Adapter tool envelope 必须携带有效 capabilities
   与至少一个 postcondition。
5. 每个 write 必须恰好有一行 rollback coverage；L3 必须显式 `no_recovery` 并保留人工批准。
6. interrupt 动作采用闭集：`cancel`、`dismiss_explicitly`、`cancel_unless_declared_by_tool`、
   `delegate_to_tool`、`ask_human`；禁止危险默认值与历史 task-id delegation。

以上仅为建议。没有人类明确批准前，TASK-376 保持 Blocked。

## 8. 验收证据

- 回填前 `cargo run -p xtask -- automation-rounds --run-date 2026-11-16`：
  `existing_rounds_for_run_date=0`、`next_round=1`、`n_auto=0`。
- 回填后当前人类会话：
  `existing_rounds_for_run_date=5`、`next_round=6`、`n_auto=0`、
  `reset_source=human-session`。
- 回填后使用真实 Slot 1 automation rollout 的只读临时 session home：
  `existing_rounds_for_run_date=5`、`next_round=6`、`n_auto=5`、
  `session_source=automation`。
- 全部文档门禁结果由 TASK-378 执行记录回填。
