# TASK-251　测试 lint 允许清单：ADR-0027 转 Accepted + `docs/spec/testing.md` §4.4 落地

- 状态：**Done**
- 阶段：1　子阶段：**治理**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`docs/adr/0027-*.md`（Draft → Accepted 改名）、`docs/adr/README.md`、`docs/spec/testing.md`、`docs/automation-charter.md`（仅 §13 W3 勾选）、`tasks/TASK-251-*.md`、`LEDGER.md`、`docs/PARKING_LOT.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`docs/automations/2026-10-07-round-4.md`
- **关联**：ADR-0027、ADR-0035、章程 §13 W3、`PL-098` / `DRIFT-W3-1`

**目标**

把 ADR-0027《测试代码的 lint 允许清单》从 Draft 转为 Accepted，并在 `docs/spec/testing.md`
落地允许清单条款，闭环 `PL-098` / `DRIFT-W3-1` 并勾选章程 §13 W3。

**背景**

`docs/spec/testing.md` 已由 TASK-072 / TASK-200 建立并保持 **Draft**，但缺 `clippy::unwrap_used`
/ `clippy::expect_used` / `clippy::panic` 的测试允许清单（章程 §13 W3 第 ③ 项）。2026-10-01
round-5 记 `PL-098` / `DRIFT-W3-1`：自动化不得直接修改既有 spec，须先走 ADR。ADR-0027 已由
W4 落成 **Draft**，其 D4 明确要求「批准本 ADR 时同步补 `docs/spec/testing.md`」。ADR-0035
baseline 已把测试模块 wrapper 列为「保留（合法）」，故两者不冲突。

**步骤**

1. ADR-0027 去 `.draft` 后缀、状态行转 Accepted、更新落地状态块。
2. `docs/adr/README.md` §1 更新 0027 行（Draft → Accepted、文件名），§2 脚注与「不连续」说明同步。
3. `docs/spec/testing.md` 新增 §4.4、版本 0.1 → 0.2、演进记录补一行。
4. 章程 §13 W3 勾选；`docs/PARKING_LOT.md` 追加 `PL-098` / `DRIFT-W3-1` 闭环行。
5. 状态同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`；新增 FACT 追加 `docs/memory/facts.md`。

**DoD**

- [x] ADR-0027 文件名无 `.draft` 后缀且状态 = **Accepted**
- [x] `docs/spec/testing.md` 含 §4.4 与三项 lint 名 + 产品代码禁令
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check / check-migrations` 全部 PASSED
- [x] `LEDGER.md` 追加一行；新增 FACT 追加 `docs/memory/facts.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
Select-String docs/spec/testing.md -Pattern 'unwrap_used','expect_used','panic'
Test-Path docs/adr/0027-test-only-lint-allowlist.md; Test-Path docs/adr/0027-test-only-lint-allowlist.draft.md
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-251 测试 lint 允许清单
【目标】ADR-0027 转 Accepted + `docs/spec/testing.md` §4.4 落地，闭环 `PL-098` / `DRIFT-W3-1`
【write scope】仅：本卡正文所列文件
【铁律】①无静默失败（D4 必须真正落进 spec）⑨不得静默扩大范围 ⑩契约先行（ADR 先于 spec）热点文件写前取锁
【禁止】不改 `[workspace.lints]` / `Cargo.toml` / `.github/workflows/**`；不新增依赖 / crate / 顶层目录；不操作真实 GUI；不改 ADR-0035 正文
【验收】见「验收命令」→ fmt / clippy / test + xtask 十一项门禁全绿 + spec 含三项 lint 名
【依赖】无前置卡；ADR-0027 已存在（Draft），与 ADR-0035（Accepted）一致，无 ADR 冲突
【疑问】无

### 2. 实际改动文件

- `docs/adr/0027-test-only-lint-allowlist.draft.md` → `docs/adr/0027-test-only-lint-allowlist.md`（Draft → Accepted）
- `docs/adr/README.md`（§1 0027 行 + §2 两处说明）
- `docs/spec/testing.md`（新增 §4.4、版本 0.2、演进记录）
- `docs/automation-charter.md`（§13 W3 `[ ]`→`[x]`）
- `docs/PARKING_LOT.md`（`PL-098` / `DRIFT-W3-1` 闭环行）
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`
- `tasks/TASK-251-test-only-lint-allowlist-spec.md`、`docs/automations/2026-10-07-round-4.md`

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0
- `cargo clippy --all-targets -- -D warnings` → EXIT 0
- `cargo test --workspace` → EXIT 0
- `xtask` 十一项门禁 → 全 EXIT 0（`hygiene` 0 error / 108 warning，`adr-index` 0 Error，`refscan` 0E/0W）
- `Select-String docs/spec/testing.md -Pattern 'unwrap_used','expect_used','panic'` → 命中
- `Test-Path docs/adr/0027-test-only-lint-allowlist.md` = True；`.draft.md` = False

### 4. DoD 逐条核对

见正文 DoD 复选框，全部 `[x]`；证据见 §3。

### 5. 偏差

- 章程 §13 W3 原 write scope 写「新建 `docs/spec/testing.md`」，而该文件已由 TASK-072 / TASK-200
  建立；本轮按 ADR-0027 D4 的「批准时同步补」口径落地 §4.4，未新建重复文件。属口径收窄，非放宽。
- 无其他偏差：未改 `Cargo.toml` / lint 配置 / CI workflow / 产品代码；未新增依赖 / crate / 顶层目录。

### 6. 更合理做法

章程 §13 W3 若当初写成「在 `docs/spec/testing.md` 补测试允许清单」并允许修改该 Draft，
就不必绕经 `DRIFT-W3-1`；本卡已把该缺口一次性收口。

### 7. 遗留问题

无。`docs/spec/testing.md` 整体仍为 Draft，其余条款的 ADR 批准不在本卡范围。

### 8. 新增长期记忆

- `docs/memory/facts.md`：测试 lint 允许清单（三项 lint 仅 `#[cfg(test)] mod tests`）已由
  ADR-0027 Accepted + `docs/spec/testing.md` §4.4 冻结为单一事实源。

### 9. 给审阅者的关注点

1. ADR-0027 与 ADR-0035 的作用域边界（测试模块 vs 产品代码 per-line allow）是否被准确表述。
2. `docs/spec/testing.md` 的「Draft」总状态与 §4.4「已由 ADR-0027 批准」的表述是否会被误读为整份 spec 已批准。
