# TASK-341　人类 prompt 与每日边界自动重置 N_auto

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：治理修复
- 依赖：ADR-0089、TASK-328、ADR-0099
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`AGENTS.md`、`docs/adr/0099-human-session-auto-reset.md`、
  `docs/adr/README.md`、`docs/adr/top-level-directories.md`、`docs/memory/decisions.md`、
  `docs/memory/facts.md`、
  `docs/memory/pitfalls.md`、`.codex/hooks.json`、`.codex/hooks/user-prompt-reset.ps1`、
  `xtask/Cargo.toml`、`xtask/src/cli.rs`、`xtask/src/main.rs`、
  `xtask/src/automation_rounds.rs`、
  `xtask/src/automation_rounds_tests.rs`、`xtask/src/automation_session.rs`、
  `xtask/src/automation_session_tests.rs`、`xtask/README.md`、
  `docs/automation-charter.md`、`docs/automations/codex-automations-operations.md`、
  `tasks/TASK-341-human-session-auto-reset.md`、`PLAN.md`、`plans/stage-1-pilots.md`、
  `README.md`、`MEMORY.md`、`LEDGER.md`

## 目标

每天早上 08:00、以及每次和用户有对话时自动重置 `N_auto`；automation 从最近 reset 边界
重新累计，不再要求用户手工通知或额外创建 human round 文件。

## 必须遵守

1. 从 `CODEX_SESSION_ID` 对应的 rollout `session_meta` 自动识别来源。
2. 只有明确 `thread_source=automation` 才按 automation；缺失或无法解析时 fail-closed。
3. 每日本地 08:00 切换运行日时自动清零更早轮次。
4. 项目 `UserPromptSubmit` hook 在每次人类 prompt 提交时更新 reset 水位，不限新会话。
5. 人类会话写仓库外单文件 reset 水位；不无限累积，不写仓库。
6. automation session 只统计最近 reset 边界后的 automation / Missing round。
7. 历史 `机制=human` round 文件继续兼容。
8. 不改 ADR-0089 历史正文，不新增依赖。

## Out of scope

- 不修改 Codex 应用的调度器或数据库。
- 不追溯改写历史轮次与 LEDGER。
- 不创建后续 automation。

## 验收命令

```powershell
cargo test -p xtask automation_rounds
cargo test -p xtask automation_session
cargo run -p xtask -- automation-rounds --run-date 2026-10-29
cargo run -p xtask -- automation-reset
codex exec --dangerously-bypass-hook-trust -C . "只回复 OK，不调用工具"
codex exec resume --dangerously-bypass-hook-trust --json <same-thread-id> "再次回复 OK"
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- check-migrations
cargo deny check
```

## DoD

- [ ] 当前运行日无新 automation 时输出 `reset_source=daily-0800`、`n_auto=0`。
- [ ] 每次人类 prompt 的 `UserPromptSubmit` hook 实测更新 reset 水位，且不限新会话。
- [ ] automation 会话从 reset 水位后开始计数。
- [ ] 未知或损坏的会话证据显式失败，不默认 human。
- [ ] 历史 human round 仍可用。
- [ ] 全部验收命令全绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-341 人类 prompt 与每日边界自动重置 N_auto
【目标】每日 08:00 与每次人类 prompt 自动 reset，automation 从最近边界计数
【write scope】见卡片正文
【铁律】1 无静默失败 / 8 外部状态边界 / 9 不扩范围 / 10 契约先行
【禁止】不依赖手工 human round 或手工通知；不新增依赖；不改 ADR-0089 历史正文
【验收】专项正负测试 + 每日边界 + 每次 prompt hook + 全量门禁 → 全绿
【依赖】ADR-0089、TASK-328、ADR-0099
【疑问】无
```

### 2. 实际改动文件

- `AGENTS.md`
- `docs/adr/0099-human-session-auto-reset.md`
- `docs/adr/README.md`
- `docs/adr/top-level-directories.md`
- `docs/memory/decisions.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `.codex/hooks.json`
- `.codex/hooks/user-prompt-reset.ps1`
- `xtask/Cargo.toml`
- `xtask/src/cli.rs`
- `xtask/src/main.rs`
- `xtask/src/automation_rounds.rs`
- `xtask/src/automation_rounds_tests.rs`
- `xtask/src/automation_session.rs`
- `xtask/src/automation_session_tests.rs`
- `xtask/README.md`
- `docs/automation-charter.md`
- `docs/automations/codex-automations-operations.md`
- `tasks/TASK-341-human-session-auto-reset.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `MEMORY.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo test -p xtask automation_rounds` → **18/18 passed**。
- `cargo test -p xtask automation_session` → **7/7 passed**。
- `cargo test -p xtask` → **510/510 passed**。
- `cargo test --workspace` → **0 failed**，全部 doctest 通过。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` → exit 0；
  仅保留仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- Linux / macOS 双目标 `xtask` clippy → exit 0；仅同一既有 unknown-lint warning。
- `verify-schemas` → 5/5；`codegen --check` → 0 drift / 0 error。
- `hygiene` 0E（151W）、`memory-counts` 0E、`adr-index` 0E、`refscan` 0E、`docscan` 0E、
  `card-check` 0E、`check-comments` 0E、`check-migrations` 0E。
- `check-ledger` 0E；`cargo deny check` → advisories / bans / licenses / sources ok；
  `replay --suite core` → 2 steps / 5 changes / 0 mismatch。
- 2026-10-29 运行 `automation-rounds --run-date 2026-10-29` →
  `session_source=human`、`reset_source=daily-0800`、`n_auto=0`。
- `automation-reset` 在当前人类会话更新水位；切换真实 automation rollout session 后
  mtime 保持不变，确认 no-op。
- `codex exec --dangerously-bypass-hook-trust` 端到端输出
  `hook: UserPromptSubmit Completed`。随后在同一 thread `01a18575-d9ff-7551-8918-35e5ffa0f25a`
  连续执行两次 prompt，reset mtime 两次都更新（`02:20:57` → `02:21:23`），证明不是只在新会话触发。

### 4. DoD 逐条核对

- [x] 当前运行日输出 `session_source=human`、`reset_source=daily-0800`、`n_auto=0`。
- [x] 每次人类 prompt 的 `UserPromptSubmit` hook 实测更新 reset 水位；同一 thread 第二次 prompt 也更新，且不限新会话。
- [x] automation 会话从 reset 水位后开始计数。
- [x] 未知、重复或损坏的会话证据显式失败，不默认 human。
- [x] 历史 human round 仍可用。
- [x] 全部验收命令全绿。

### 5. 偏差

- 本人类 turn 的 `exec_command` 共 31 次误填 `shell: "bash"`，其中多数只读查询因
  PowerShell 语法 / 命令不存在而失败，另有少量 shell-neutral 的只读 `git` 查询成功；
  没有通过 Bash 写仓库。发现后均以 PowerShell 重跑，完整证据见本轮 rollout。
- 未新增第三方依赖；`xtask` 仍为零第三方依赖。

### 6. 更合理做法

- 用每日运行日派生边界与宿主 `UserPromptSubmit` 事件共同建立 reset，而不是扫描会话正文；
  水位保存在仓库外单文件，既避免工作树污染，也不引入无界历史。

### 7. 遗留问题

- 当前改动处于 Review，待 PR / CI 与人类合并。
- Codex rollout 首次落盘前的极早调用会显式失败；按 fail-closed 处理，不猜测 human。
- 项目 hook 首次使用需在 Codex 完成一次持久 trust；本卡只以 bypass 证明 hook 本身可执行，
  不代替人类信任该项目配置。

### 8. 新增长期记忆

- FACT：`CODEX_SESSION_ID` + rollout 首行 `thread_source` 可稳定区分本机 human / automation。
- FACT：08:00 运行日 `round 0` 与 `UserPromptSubmit` 水位取最晚值；同一 thread 连续两次 prompt 均更新水位。
- PITFALL：只认手工 human round 或只在新会话启动时 reset，都会把后续人类介入误算成旧 automation 批次。

### 9. 给审阅者的关注点

- 确认 automation 子线程不会被误认为 human reset；当前规则只把明确
  `thread_source=automation` 归为 automation。
- 确认外部 reset 水位始终只有一个文件，且损坏时 fail-closed。
- 确认每日 08:00、同运行日 prompt 水位与历史 human round 的“最晚边界”不会漏算或双算。
- 确认 `.codex/` 只含可审查 hook 配置，不含凭据、会话正文或运行时状态。
