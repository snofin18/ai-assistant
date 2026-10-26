# TASK-328　治理轮次计数、证据更正与真实应用人工验收边界

- 状态：**Done**
- 阶段：1　批次：治理池　依赖：ADR-0089、TASK-306、TASK-307、TASK-309、TASK-114
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上由 Orchestrator 所有；Implementer 只读。

---

**目标**

用可重复的机器命令固定 `N_auto` 的人工重置口径，更正 2026-10-24 round 2 的
`shell_misuse` 最终证据，消除 TASK-079 / TASK-305 的跨文档状态冲突，并明确真实应用验证
只允许在人类在场的会话中进行。

**In scope**

- 新增 `xtask automation-rounds`：从 `docs/automations/*-round-*.md` 解析机制、轮次和
  `N_auto`，输出最近 human reset、当前计数和指定运行日的下一轮次。
- 新增 2026-10-26 human round 记录，使本次人工会话正式重置 `N_auto`。
- 复核 2026-10-24 round 2 外部 JSONL、round 报告与 LEDGER，追加不可递归的更正证据。
- TASK-079：依据 2026-09-23 NO-GO 实测和现行 DoD，将最终状态统一为 Blocked。
- TASK-305：依据后续 TASK-306/307/309 的根因修复与真机通过，按 superseded 收口。
- 为 TASK-056/057/058 记录真实应用人工验收边界；不实现其产品功能，不宣称 Done。
- 同步本卡、计划、PLAN、README、LEDGER 与 xtask 文档。

**Out of scope**

- 不修改任何任务卡正文区，只允许状态行和执行记录区。
- 不实现 Edge 浏览器任务执行装配、表单填写 provider、截图/下载/click 或安全 regression workflow。
- 不执行真实网络、真实登录、凭据、商业应用或 GUI 自动化；只定义并记录人类在场的验收边界。
- 不修改 ADR/spec 已决事项、公共 trait/schema/ErrorCode、第三方依赖或 CI 阈值。
- 不重写历史 round 文件和既有 LEDGER 行；更正一律追加。

**必须遵守**

1. `N_auto` 只统计最近一次显式 `机制=human` 轮次之后的 automation 轮次；缺机制字段按 automation fail-closed。
2. 历史证据不可改写；发现矛盾时新增审计或 LEDGER 行，并标明 supersedes / 证据来源。
3. TASK-056/057/058 的真实应用验证只能由人类在场会话执行；automation 不得用点击、登录或凭据补齐。
4. TASK-079 的最终状态必须与 NO-GO 实测和 DoD 一致；不得以历史 Done 行掩盖未达标。
5. 所有改动必须经过 fmt、clippy、workspace tests 与 xtask 文档门禁。

**验收**

```powershell
cargo test -p xtask automation_rounds
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- automation-rounds --run-date 2026-10-26
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

**DoD**

- [ ] `automation-rounds` 对正常、无 human、缺失机制与下一轮次均有单测。
- [ ] human round 落地后命令输出 `n_auto=0`，且在 origin/main 上可复算。
- [ ] round 2 shell 计数更正证据与最终值一致，且不改历史行。
- [ ] TASK-079 / TASK-305 的卡面状态、LEDGER 与审计结论一致。
- [ ] TASK-056/057/058 明确人工在场验证边界，未伪造完成。
- [ ] 所有验收命令通过。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->

## 执行记录

### 1. 约束回执

见本会话 2026-10-26 的 TASK-328 回执。预授权执行；真实 GUI/凭据与功能实现不进入本卡。

### 2. 实际改动文件

- `xtask/src/automation_rounds.rs`（新增）
- `xtask/src/automation_rounds_tests.rs`（新增）
- `xtask/src/main.rs`
- `xtask/src/cli.rs`
- `xtask/README.md`
- `docs/automations/2026-10-26-round-1.md`（新增 human reset）
- `docs/automations/codex-automations-operations.md`
- `docs/audits/automation-2026-10-24-round-2-shell-misuse-correction-2026-10-26.md`
- `docs/audits/real-app-manual-acceptance-2026-10-26.md`
- `tasks/TASK-056-t5-1-open-site-extract-list-write-csv.md`（仅执行记录区）
- `tasks/TASK-057-t5-2-form-fill-stop-before-submit-diff-origin.md`（仅执行记录区）
- `tasks/TASK-058-t5-3-injection-target-security-ci-audit.md`（仅执行记录区）
- `tasks/TASK-079-b1-7-probe-10-ime.md`（状态行与执行记录区）
- `tasks/TASK-305-canvas-write-postcondition.md`（状态行与执行记录区）
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo run -p xtask -- automation-rounds --run-date 2026-10-26` →
  `latest_human=docs/automations/2026-10-26-round-1.md`、`n_auto=0`、
  `next_round=2`、`verdict=PASSED`。
- `cargo test -p xtask automation_rounds` → 12/12 passed。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅仓内既有
  `clippy::assert_is_empty` unknown-lint warning，未被 `-D warnings` 升级。
- `cargo test --workspace` → exit 0；全部测试与 doctest 通过。
- `cargo test -p assistant-core arch::` → 10/10 passed。
- `verify-schemas` / `codegen --check` → PASSED，0 error / 0 drift。
- `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
  `card-check` / `check-ledger` / `check-comments` / `check-migrations` → 全部 exit 0。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。

### 4. DoD 逐条核对

- [x] `automation-rounds` 对正常、无 human、缺失机制、标题机制与下一轮次均有单测。
- [x] human round 落地后命令输出 `n_auto=0`，且机制/轮次可由仓库文件复算。
- [x] round 2 shell 计数以外部 JSONL 最终 `final_state=36` 更正，历史文件只读。
- [x] TASK-079 = Blocked、TASK-305 = Done（superseded），均追加卡内裁决记录。
- [x] TASK-056/057/058 明确人类在场验证边界，保持 Blocked，未伪造完成。
- [x] 所有验收命令通过。

### 5. 偏差

- 本会话已有 4 条只读命令误填 bash，均在命令查找或解析阶段失败，未写仓库；收尾如实记入 LEDGER。
- 收尾复核又发生 1 条 `Get-Content` 误填 bash，同样在命令查找阶段失败且未写仓库；
  本会话最终 `shell_misuse=5`。该数字与 LEDGER 收尾行一致。

### 6. 更合理做法

- 用机器命令取代 agent 各自手数轮次，避免同一轮在不同报告中采用不同 `N_auto`。
- 用追加式审计解决历史证据冲突，不篡改旧报告。

### 7. 遗留问题

- TASK-056/057/058 的产品执行能力仍需后续独立卡；本卡只固定人类在场验证边界。

### 8. 新增长期记忆

无独立新增。`N_auto` 口径和真实应用人工验收边界分别由
`xtask automation-rounds`、操作手册和两份审计文件作为版本化事实源，避免在 L0/L1
记忆里复制第三份规则。

### 9. 给审阅者的关注点

- `automation-rounds` 必须对机制字段缺失 fail-closed，不能静默跳过。
- TASK-079 / TASK-305 的状态收口必须能追溯到已有真机记录。
