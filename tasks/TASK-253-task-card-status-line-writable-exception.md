# TASK-253　任务卡状态行：正文区唯一可写例外（PL-073）

- 状态：**Done**
- 阶段：1　子阶段：**治理**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`docs/adr/0083-task-card-status-line-writable-exception.md`、`docs/adr/README.md`、`docs/adr/0032-doc-rule-exemption-registry.md`（仅同步被本次 gov 行移影响的豁免行号）、`AGENTS.md`、`docs/governance-ai-agent-execution.md`、`xtask/src/card_check.rs`、`xtask/src/cli.rs`（后两者仅文档与 CLI 说明）、`docs/memory/decisions.md`、`docs/PARKING_LOT.md`、`tasks/TASK-253-task-card-status-line-writable-exception.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md`、`docs/automations/2026-10-07-round-5.md`
- **关联**：ADR-0031、ADR-0041、ADR-0083、`PL-073`

**目标**

用 Accepted ADR 明确任务卡 `- 状态：` 行是正文区的唯一可写例外，并同步规则文本，闭环 `PL-073`。

**背景**

`PL-073` 记录：状态唯一落点在卡片正文区，而正文区对 Implementer 只读；Implementer 完成卡后
更新状态会按字面违反 write scope。ADR-0041 已解决计划文件同类问题，但没有定义任务卡状态行。

**步骤**

1. 新建 ADR-0083 并转 Accepted，冻结“单行状态例外 + 同批更新 + 其余正文只读”规则。
2. 更新 ADR 登记表与 `docs/memory/decisions.md`。
3. 在 `AGENTS.md` §1/§8、gov §3.2~§3.4、`card-check` 说明中同步引用 ADR-0083。
4. `docs/PARKING_LOT.md` 追加 `PL-073` 闭环行。
5. 状态同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `MEMORY.md` 规模表。

**DoD**

- [ ] `docs/adr/0083-task-card-status-line-writable-exception.md` 存在且状态 = **Accepted**
- [ ] ADR-0083、`AGENTS.md`、gov、`card-check` 均明确状态行是正文区唯一可写例外
- [ ] `docs/PARKING_LOT.md` 追加 `PL-073` 闭环行且原行不改
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check / check-migrations` 全部 PASSED
- [ ] `LEDGER.md` 追加一行；新增 DECISION 追加 `docs/memory/decisions.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
rg -n "ADR-0083" AGENTS.md docs/governance-ai-agent-execution.md xtask/src/card_check.rs docs/adr/README.md
rg -n "唯一可写例外|状态行" AGENTS.md docs/governance-ai-agent-execution.md xtask/src/card_check.rs
Select-String docs/PARKING_LOT.md -Pattern 'PL-073.*已闭环'
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-253 任务卡状态行正文例外
【目标】ADR-0083 Accepted + 规则文本同步，闭环 PL-073
【write scope】仅：本卡正文所列文件
【铁律】①无静默失败 ⑨不得静默扩大范围 ⑩契约先行；热点文件写前取锁
【禁止】不改产品契约 / 公共 API / 依赖 / crate / GUI；不改既有 Accepted ADR 正文；不操作 PR #275
【验收】见「验收命令」→ fmt / clippy / test + xtask 十一项门禁全绿 + ADR-0083 引用可检索
【依赖】ADR-0031 / ADR-0041 已 Accepted；PL-073 已复核开放
【疑问】无

### 2. 实际改动文件

- `docs/adr/0083-task-card-status-line-writable-exception.md`（新增，Accepted）
- `docs/adr/README.md`（新增 0083 行，下一可用号改 0084）
- `docs/adr/0032-doc-rule-exemption-registry.md`（仅同步 gov 行移后的 E-004/E-005/E-006 行号）
- `AGENTS.md` §1/§8、`docs/governance-ai-agent-execution.md` §3.2~§3.4
- `xtask/src/card_check.rs`、`xtask/src/cli.rs`（仅规则说明与帮助文本）
- `docs/memory/decisions.md`、`docs/PARKING_LOT.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`LEDGER.md`
- `tasks/TASK-253-task-card-status-line-writable-exception.md`
- `docs/automations/2026-10-07-round-5.md`

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0
- `cargo clippy --all-targets -- -D warnings` → EXIT 0，含 xtask Linux/macOS 交叉 clippy
- `cargo test --workspace` → EXIT 0；`cargo test -p assistant-core arch::` → EXIT 0
- `cargo deny check`、`cargo llvm-cov --fail-under-lines 75` → EXIT 0
- `pnpm --dir apps/desktop-ui lint/typecheck/test` → EXIT 0（78 Node tests + 3 Vitest tests）
- `xtask hygiene` → 0E/109W；`memory-counts` → 0E/0W；`adr-index` → 0E/0W
- `refscan` → 0E/0W；`docscan` → 0E/273W；`card-check` → 0E/34W
- `check-ledger` → 0E/0W；`check-comments` → 0E/71W；`verify-schemas`、`codegen --check`、`check-migrations` → PASSED
- `rg "ADR-0083"` 命中 ADR / AGENTS / gov / card-check / decisions；`PL-073` 闭环行已追加

### 4. DoD 逐条核对

- ADR-0083 存在且 Accepted：满足（`adr-index` PASSED）
- AGENTS / gov / card-check 均明确状态行唯一例外：满足（`rg` 命中）
- `PL-073` 闭环行已追加且原行不改：满足（PARKING_LOT 追加行位于文件尾部）
- Rust / UI / 覆盖率 / 安全性门禁全绿：满足（§3）
- `LEDGER.md` 与 `decisions.md` 已更新：满足
- 正文仅状态行变化：满足（本卡其余正文未改；行号豁免同步是 gov 编辑的直接机械后果）

### 5. 偏差

none。

### 6. 更合理做法

gov 插入规则文本后，ADR-0032 豁免登记表按精确行号引用的三条豁免发生机械失配；本轮直接修正
E-004/E-005/E-006 到新行号，未放宽 `refscan` 规则，也未新增豁免。

### 7. 遗留问题

无。`card-check` 的 git diff 实现在未来落地时必须按 ADR-0083 只排除唯一状态行并补正负测试；
该约束已写入 ADR 验证方式。

### 8. 新增长期记忆

`docs/memory/decisions.md`：任务卡 `- 状态：` 行是正文区唯一可写例外，必须与产生状态的提交同批；
其他正文行仍只读。

### 9. 给审阅者的关注点

1. ADR-0083 是否把例外严格限制在唯一状态行，没有顺带开放其他 metadata。
2. ADR-0032 的 E-004/E-005/E-006 行号修正是否只反映 gov 行移，不改变豁免语义。
