# ADR-0099　人类 prompt 与每日边界自动重置 N_auto

状态：Accepted　日期：2026-10-29　Supersedes：—
Amends：ADR-0089 D4 / D5
关联：ADR-0088（运行日边界）、ADR-0089（无人值守轮次上限）、
`docs/automation-charter.md` §11.3、`docs/automations/codex-automations-operations.md` §4.1.c

## 背景

ADR-0089 规定 `N_auto` 从最近一次人类会话重置。当前 `xtask automation-rounds` 只把
`docs/automations/<run-date>-round-<N>.md` 中显式写有 `机制：human` 的文件当作 reset，
因此人类开新会话本身不会归零；人类必须在仓库里手工补一份 human round 才能重置。

这与用户最终确认的两点语义不符：① 每天早上 08:00 自动重置；② 只要和用户有对话就自动重置，
不仅限于新开会话。人类已经介入并承担即时审阅时，reset 不应依赖额外通知或手工文件。

## 决策

1. **自动识别当前 Codex 会话来源。** `automation-rounds` 从 `CODEX_SESSION_ID` 定位当前
   rollout 的 `session_meta`，只把 `thread_source=automation` 视为 automation；其他明确来源
   视为人类会话。缺失来源或无法解析时 fail-closed，不猜测为 human。
2. **每次人类 prompt 自动记录 reset 水位。** 项目级 `UserPromptSubmit` hook 在每条人类
   prompt 提交时运行 `cargo run --quiet -p xtask -- automation-reset`。该命令把当前 round
   顺序的最大水位写入单文件
   `$CODEX_HOME/automations/human-session-reset.v1`。该文件位于仓库外，不污染工作树，
   只保留最新一个水位，不无限累积。本 ADR 同时授权新增 Git 跟踪的 `.codex/` 顶层目录，
   用于项目级 hooks 配置；该目录不得保存凭据、会话正文或运行时状态。
3. **人类会话启动保留幂等 preflight。** `automation-rounds` 在人类会话中仍会把水位更新到
   当前 round 顺序末尾，作为 hook 尚未完成持久信任或启动竞态时的第二道保障。
4. **每日本地 08:00 是硬边界。** 运行日规则已规定本地 08:00 切换到新 `run-date`；
   当日 `round 0` 作为独立 reset 边界，早于该运行日的 automation 一律不计入 `N_auto`。
   实际边界取每日 08:00、最新人类 prompt 水位与历史 human round 三者的最晚值。
5. **automation 从最近 reset 边界起算。** automation 会话读取该水位，`N_auto` 只统计边界
   之后的 automation / Missing round。历史 `机制=human` 轮次继续兼容，但不再要求人工创建。
6. **重置本身不写仓库。** reset 水位是 Codex 本地会话状态；仓库中的 round 报告仍记录
   实际工作与证据，但不再承担“证明用户点过重置”的职责。

## 考虑过的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 继续要求手工 human round | 否决 | 用户必须额外通知，且会话活跃但未落文件时 `N_auto` 明显算错 |
| 只在人类会话启动时重置 | 否决 | 不满足“和用户有对话就重置”；长会话中的新人类消息仍会继承旧计数 |
| **每次人类 prompt 由 `UserPromptSubmit` hook 重置** | 采纳 | 宿主提供稳定事件边界；单文件 O(1) 写，不扫描会话正文，也不依赖手工文件 |
| **本地每日 08:00 无事件派生边界** | 采纳 | 与 ADR-0088 运行日一致，不依赖调度器触发；即使一天没有人类 prompt 也自动归零 |
| 读取所有 session JSONL 的逐条消息并推断最新人类 turn | 否决 | 扫描范围大、实现脆弱，且需要长期维护宿主私有格式 |

## 影响

- `xtask/src/automation_session*.rs`：新增会话来源探测与单文件 reset 水位。
- `xtask/src/automation_rounds*.rs`：`N_auto` 计算加入每日与 prompt reset 边界；输出
  `session_source` / `reset_source`。
- `.codex/hooks.json`、`.codex/hooks/user-prompt-reset.ps1`：每条人类 prompt 调用
  `automation-reset`；automation 会话调用时 no-op。
- `docs/adr/top-level-directories.md`：登记 `.codex/` 顶层目录及其 ADR-0099 授权边界。
- `xtask/README.md`、`AGENTS.md` §3：说明每 prompt hook、启动 preflight 与每日 08:00
  边界，无需人工创建 human round。
- `docs/automation-charter.md`、`docs/automations/codex-automations-operations.md`：
  human reset 的权威口径改为“每 prompt hook + 每日 08:00 + 历史兼容”的最晚边界。
- `docs/adr/README.md`、`docs/memory/decisions.md`：登记本 ADR。
- 历史 round 文件、ADR-0089 正文与既有 LEDGER 行不改写。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 会话文件尚未落盘导致无法识别 | 调用失败并明确报错，不静默降级为 human |
| 旧 automation 子线程被误判为 human | 只把明确 `thread_source=automation` 归为 automation；其余显式来源按 human；缺少来源时 fail-closed |
| reset 文件损坏或丢失 | 损坏时显式失败；丢失时回退到最近历史 human round，并在输出中标明来源 |
| 项目 hook 未获 Codex 持久信任 | 首次使用项目 hook 时完成一次 trust；e2e 以 `--dangerously-bypass-hook-trust` 验证 hook 本身，未信任时启动 preflight 仍提供 session 级兜底 |
| hook 命令失败或超时 | hook 使用有界 30 秒超时并传播非零退出；失败不得伪装成 reset 成功 |
| 外部状态在换机后不可用 | 本地文件只保存最新边界；换机后的首次人类会话会重新建立水位 |

## 验证

1. 2026-10-29 运行 `cargo run -p xtask -- automation-rounds --run-date 2026-10-29`
   → `reset_source=daily-0800`、`n_auto=0`，旧运行日轮次只计入历史警告。
2. 同一运行日内，最新人类 prompt 水位优先于 08:00；前一天的 human 水位不覆盖当日
   08:00。
3. `codex exec --dangerously-bypass-hook-trust` 端到端触发 `UserPromptSubmit`，确认
   reset 文件 mtime 更新；automation 会话调用 `automation-reset` 不更新水位。
4. 单测覆盖 human / automation / 未知来源、水位 round-trip、损坏文件、每日边界、
   后置 automation 计数与历史 human round 兼容。
5. `cargo test -p xtask automation_rounds`、`cargo test -p xtask automation_session`
   与全量门禁全绿。
