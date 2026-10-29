# TASK-341　人类会话自动重置 N_auto

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：治理修复
- 依赖：ADR-0089、TASK-328、ADR-0099
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`AGENTS.md`、`docs/adr/0099-human-session-auto-reset.md`、
  `docs/adr/README.md`、`docs/memory/decisions.md`、`docs/memory/facts.md`、
  `xtask/Cargo.toml`、`xtask/src/main.rs`、`xtask/src/automation_rounds.rs`、
  `xtask/src/automation_rounds_tests.rs`、`xtask/src/automation_session.rs`、
  `xtask/src/automation_session_tests.rs`、`xtask/README.md`、
  `docs/automation-charter.md`、`docs/automations/codex-automations-operations.md`、
  `tasks/TASK-341-human-session-auto-reset.md`、`PLAN.md`、`plans/stage-1-pilots.md`、
  `README.md`、`LEDGER.md`

## 目标

让每次人类 Codex 会话自动重置 `N_auto`，automation 从该 reset 水位重新累计；不再要求
用户手工通知或额外创建 human round 文件。

## 必须遵守

1. 从 `CODEX_SESSION_ID` 对应的 rollout `session_meta` 自动识别来源。
2. 只有明确 `thread_source=automation` 才按 automation；缺失或无法解析时 fail-closed。
3. 人类会话写仓库外单文件 reset 水位；不无限累积，不写仓库。
4. automation session 读取水位，只统计其后的 automation / Missing round。
5. 历史 `机制=human` round 文件继续兼容。
6. 不改 ADR-0089 历史正文，不新增依赖。

## Out of scope

- 不修改 Codex 应用的调度器或数据库。
- 不追溯改写历史轮次与 LEDGER。
- 不创建后续 automation。

## 验收命令

```powershell
cargo test -p xtask automation_rounds
cargo run -p xtask -- automation-rounds --run-date 2026-10-28
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

- [ ] 当前人类会话输出 `session_source=human`、`reset_source=human-session`、`n_auto=0`。
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
【任务】TASK-341 人类会话自动重置 N_auto
【目标】会话元数据自动识别 human / automation，并用外部单文件水位 reset
【write scope】见卡片正文
【铁律】1 无静默失败 / 8 外部状态边界 / 9 不扩范围 / 10 契约先行
【禁止】不依赖手工 human round；不新增依赖；不改 ADR-0089 历史正文
【验收】专项正负测试 + 当前会话 n_auto=0 + 全量门禁 → 全绿
【依赖】ADR-0089、TASK-328、ADR-0099
【疑问】无
```

### 2. 实际改动文件

- `AGENTS.md`
- `docs/adr/0099-human-session-auto-reset.md`
- `docs/adr/README.md`
- `docs/memory/decisions.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `xtask/Cargo.toml`
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

- `cargo test -p xtask automation_rounds` → **16/16 passed**。
- `cargo test -p xtask automation_session` → **7/7 passed**。
- `cargo test -p xtask` → **508/508 passed**。
- `cargo test --workspace` → **0 failed**，全部 doctest 通过。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` → exit 0；
  仅保留仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- `verify-schemas` → 5/5；`codegen --check` → 0 drift / 0 error。
- `hygiene` 0E、`memory-counts` 0E、`adr-index` 0E、`refscan` 0E、`docscan` 0E、
  `card-check` 0E、`check-comments` 0E、`check-migrations` 0E。
- `cargo deny check` → advisories / bans / licenses / sources ok。
- 当前人类会话运行 `automation-rounds --run-date 2026-10-28` →
  `session_source=human`、`reset_source=human-session`、`n_auto=0`。
- 使用真实 automation rollout 模拟运行同一命令 →
  `session_source=automation`、`reset_source=human-session`、`n_auto=0`。

### 4. DoD 逐条核对

- [x] 当前人类会话输出 `session_source=human`、`reset_source=human-session`、`n_auto=0`。
- [x] automation 会话从 reset 水位后开始计数。
- [x] 未知、重复或损坏的会话证据显式失败，不默认 human。
- [x] 历史 human round 仍可用。
- [x] 全部验收命令全绿。

### 5. 偏差

- 本人类会话有多次只读命令误填 Bash，均在命令查找或参数解析阶段失败，未写仓库；
  发现后均以 PowerShell 重跑。
- 未新增第三方依赖；`xtask` 仍为零第三方依赖。

### 6. 更合理做法

- 用 Codex 会话来源自动建立 reset 边界，而不是要求用户在仓库中手工创建 human round；
  水位保存在仓库外单文件，既避免工作树污染，也不引入无界历史。

### 7. 遗留问题

- 当前改动处于 Review，待 PR / CI 与人类合并。
- Codex rollout 首次落盘前的极早调用会显式失败；按 fail-closed 处理，不猜测 human。

### 8. 新增长期记忆

- FACT：`CODEX_SESSION_ID` + rollout 首行 `thread_source` 可稳定区分本机 human / automation。
- PITFALL：只认手工 human round 会让已开始的人类会话继续累计旧 automation。

### 9. 给审阅者的关注点

- 确认 automation 子线程不会被误认为 human reset；当前规则只把明确
  `thread_source=automation` 归为 automation。
- 确认外部 reset 水位始终只有一个文件，且损坏时 fail-closed。
- 确认历史 human round 与新会话水位取较晚边界，不产生双重计数。
