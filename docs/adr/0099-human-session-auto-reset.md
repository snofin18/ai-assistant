# ADR-0099　人类会话自动重置 N_auto

状态：Accepted　日期：2026-10-29　Supersedes：—
Amends：ADR-0089 D4 / D5
关联：ADR-0088（运行日边界）、ADR-0089（无人值守轮次上限）、
`docs/automation-charter.md` §11.3、`docs/automations/codex-automations-operations.md` §4.1.c

## 背景

ADR-0089 规定 `N_auto` 从最近一次人类会话重置。当前 `xtask automation-rounds` 只把
`docs/automations/<run-date>-round-<N>.md` 中显式写有 `机制：human` 的文件当作 reset，
因此人类开新会话本身不会归零；人类必须在仓库里手工补一份 human round 才能重置。

这与“每次人类会话都应该重置”的语义不符：人类已经介入并承担了即时审阅，reset 不应再依赖
人类额外通知或手工文件。

## 决策

1. **自动识别当前 Codex 会话来源。** `automation-rounds` 从 `CODEX_SESSION_ID` 定位当前
   rollout 的 `session_meta`，只把 `thread_source=automation` 视为 automation；其他明确来源
   视为人类会话。缺失来源或无法解析时 fail-closed，不猜测为 human。
2. **人类会话自动记录 reset 水位。** 人类会话运行 `automation-rounds` 时，工具把当前
   round 顺序的最大水位写入单文件
   `$CODEX_HOME/automations/human-session-reset.v1`。该文件位于仓库外，不污染工作树，
   只保留最新一个水位，不无限累积。
3. **automation 从 reset 水位起算。** automation 会话读取该水位，`N_auto` 只统计水位之后
   的 automation / Missing round。历史 `机制=human` 轮次继续兼容；若它晚于会话水位，
   使用更晚的边界。
4. **不再要求人工 human round。** 新人类会话由工具的启动 preflight 自动 reset；
   历史 human round 文件只作为兼容证据保留，不要求继续手工创建。
5. **重置本身不写仓库。** reset 水位是 Codex 本地会话状态；仓库中的 round 报告仍记录
   实际工作与证据，但不再承担“证明用户点过重置”的职责。

## 考虑过的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 继续要求手工 human round | 否决 | 用户必须额外通知，且会话活跃但未落文件时 `N_auto` 明显算错 |
| 每次人类消息都重置 | 否决 | “消息”不是稳定可审计边界，且同一会话中的自动工具消息容易混淆 |
| 读取所有 session JSONL 的逐条消息并推断最新人类 turn | 否决 | 扫描范围大、实现脆弱，且需要长期维护宿主私有格式 |
| **会话启动时写外部 reset 水位** | 采纳 | 单次 O(1) 写、单文件有界、人类无需通知；automation 可稳定复算 |

## 影响

- `xtask/src/automation_session*.rs`：新增会话来源探测与单文件 reset 水位。
- `xtask/src/automation_rounds*.rs`：`N_auto` 计算加入会话 reset 边界；输出
  `session_source` / `reset_source`。
- `xtask/README.md`、`AGENTS.md` §3：所有人类会话启动时执行
  `automation-rounds --run-date <YYYY-MM-DD>`，无需人工创建 human round。
- `docs/automation-charter.md`、`docs/automations/codex-automations-operations.md`：
  human reset 的权威口径改为会话自动水位。
- `docs/adr/README.md`、`docs/memory/decisions.md`：登记本 ADR。
- 历史 round 文件、ADR-0089 正文与既有 LEDGER 行不改写。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 会话文件尚未落盘导致无法识别 | 调用失败并明确报错，不静默降级为 human |
| 旧 automation 子线程被误判为 human | 只把明确 `thread_source=automation` 归为 automation，其余显式来源按 human；缺少来源时 fail-closed |
| reset 文件损坏或丢失 | 损坏时显式失败；丢失时回退到最近历史 human round，并在输出中标明来源 |
| 外部状态在换机后不可用 | 本地文件只保存最新边界；换机后的首次人类会话会重新建立水位 |

## 验证

1. 当前人类会话运行 `cargo run -p xtask -- automation-rounds --run-date 2026-10-28`
   → `session_source=human`、`reset_source=human-session`、`n_auto=0`。
2. 单测覆盖 human / automation / 未知来源、水位 round-trip、损坏文件、后置 automation
   计数与历史 human round 兼容。
3. `cargo test -p xtask automation_rounds` 与全量门禁全绿。
