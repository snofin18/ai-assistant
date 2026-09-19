# TASK-052　xtask lint cleanup — refscan 40-lint 块清场 + 4 模块 indexing_slicing 纯重构

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001 / 051　预估：M　阻塞主线：否（但 PL-NEW 挂着 = 决策待执行）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---

- 依赖：TASK-001（Done）/ TASK-051（Done）　预估：M　难度：M
- **write scope**：
  - `tasks/TASK-052-...md`（本卡文件）
  - `xtask/src/{refscan,docscan,card_check,exemptions}.rs`（4 个新模块的 lint 清理 + 32 处 indexing 重构）
  - `docs/PARKING_LOT.md`（追加 `[CLOSED]` 行关闭 PL-NEW）
  - `LEDGER.md`（追加本卡一行）
  - `MEMORY.md`（若 §1 表格需更新；本卡预计不动）
- **Out of scope**：
  - workspace `[lints.clippy]` 配置**不动**（人类已裁决 B 路 = 纯重构，禁止放宽 lint = 漂移触发器 ⑥）
  - 任何 ADR 文件、AGENTS.md、PLAN.md、其他任务卡
  - 其他 crate（spikes/crates/*/apps/*）
  - 「状态行唯一 / 分界线唯一」实施（Mode 2 review [N4] 提及，归后续卡）
  - `card_check.rs` 的 `load_record_section_titles()` 接入（Mode 2 review [N2] 提及，归后续卡）

**背景与源流**

- TASK-051 收尾时（commit `26afc43`）发现 workspace `[lints.clippy] indexing_slicing = "deny"` 无 `priority = -1`，per-item `#[allow]` 无法 override（clippy 1.98 实测）。
- TASK-051 的已知偏差 #1：refscan.rs 保留原 10f78db 自带的 40-lint `#![allow(...)]` 块；其他 3 模块已收窄为 `#![allow(clippy::indexing_slicing)]`（1 条）。两态都违反 DoD「无模块级 lint allow」。
- 人类于 TASK-051 完成报告后裁决：**B 路 = 纯重构**（禁止走 A 路「改 workspace 加 priority = -1」= 漂移触发器 ⑥ 不可忍受）。
- 登记于 `docs/PARKING_LOT.md` 的 **PL-NEW** 是本卡的入口。本卡完成时关闭 PL-NEW。

**In scope 清单**

1. **移除** 4 个新模块顶部的 `#![allow(...)]` 块（refscan 40-lint + 其他 3 模块 1-lint `indexing_slicing` 收窄块）
2. **重构 32 处 indexing_slicing**（全部用 `.first()` / `.get(n)` / iterators 替换 `arr[n]`）—— `.expect()` 由现有 `mod tests` 包装允许
3. **修其他 pedantic lint**（refscan.rs 砍掉 40-lint 块后会暴露）：`single_char_pattern` / `case_sensitive_file_extension_comparisons` / `uninlined_format_args` / `redundant_closure` / `format_push_string` / `collapsible_if` / `single_match` 等
4. **关闭 PL-NEW**（PARKING_LOT.md 追加 `[supersedes]` 行）
5. 填执行记录 9 节 + 更新 LEDGER
6. 复核两次（机械 + 语义）

**验收命令**

```powershell
# 11 条全套（gov §5.1 的 6 硬 + 关键软门禁）
cargo fmt --all --check                           # 0 diff
cargo clippy -p xtask --all-targets -- -D warnings # exit 0（**关键**：无任何模块级 allow 块）
cargo test --workspace                            # 全绿（基线 279 + 本卡新增/改动 0 test 增减）
cargo deny check                                  # 4 项 ok
cargo run -p xtask -- hygiene                     # PASSED
cargo run -p xtask -- memory-counts               # PASSED
cargo run -p xtask -- adr-index                   # PASSED
cargo run -p xtask -- refscan                     # 行为不变（仍是 150+ 真实发现）
cargo run -p xtask -- docscan                     # PASSED
cargo run -p xtask -- card-check                  # PASSED（19 warnings 不变）
cargo llvm-cov --workspace --fail-under-lines 75 # ≥ 75%
grep -n '#!\[allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs  # 0 命中（DoD 硬性证据）
```

**DoD**

- [ ] 4 个新模块顶部**无**任何 `#![allow(...)]` 块（硬性证据见 grep 命令）
- [ ] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0（0 警告，0 allow 块依赖）
- [ ] 32 处 `indexing_slicing` 全部用 `.first()` / `.get(n).expect(...)` / iterators 替换
- [ ] 其他 pedantic lint 全部修掉或加 per-item `#[allow(...)]` 带注释（每个允许项执行记录 §5 登记）
- [ ] PL-NEW 在 `docs/PARKING_LOT.md` 标注 `[supersedes:2026-09-19]` + 关闭理由
- [ ] 全部 11 条验收命令全绿
- [ ] `LEDGER.md` 追加本卡一行；执行记录 9 节填齐

**约束回执（前置提示，等 Implementer 复核后填 §1）**

```text
【任务】TASK-052 xtask lint cleanup — refscan 40-lint 块清场 + 4 模块 indexing_slicing 纯重构
【write scope】仅：xtask/src/{refscan,docscan,card_check,exemptions}.rs + tasks/TASK-052-...md + docs/PARKING_LOT.md + LEDGER.md + MEMORY.md（按需）
【铁律】 ① 无静默失败  ⑨ 不静默扩大范围  ⑩ 契约先行
【禁止】 动 workspace [lints.clippy]（漂移 ⑥ 不可忍受）/ 动其他 crate / 改 ADR / 改 AGENTS.md
【依赖】 TASK-001 / TASK-051（已核对 LEDGER：Done）
【疑问】 无（人类已裁决 B 路；本卡是 B 路的实施）
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
