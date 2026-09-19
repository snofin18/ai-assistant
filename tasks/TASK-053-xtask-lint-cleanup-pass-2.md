# TASK-053　xtask lint cleanup pass 2 — card_check 5 处 str[Range] + refscan dead_code + test f[0]

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001 / 051 / 052　预估：S　阻塞主线：否
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---

- 依赖：TASK-001（Done）/ TASK-051（Done）/ TASK-052（Done）　预估：S（≤ 1 h）　难度：S
- **write scope**：
  - `tasks/TASK-053-...md`（本卡文件）
  - `xtask/src/{refscan,docscan,card_check}.rs`（3 模块的清理）
  - `LEDGER.md`（追加本卡一行）
- **Out of scope**：
  - workspace `[lints.clippy]` 配置**不动**（TASK-052 人类裁决 B 路 = 纯重构 = 不动 workspace）
  - `exemptions.rs`（本卡无新改动）
  - `xtask` 新功能 / 新子命令 / 公共接口改动
  - 其他 crate / AGENTS.md / PLAN.md / 其他 ADR
  - 「render 函数返回 Result vs `.expect()`」决策（属后续 ADR / 卡）

**背景与源流**

- TASK-052 收尾时遗留 3 类问题（Mode 2 sub-agent [N5][N6][N7]），本卡集中处理：
  1. **`card_check.rs` 5 处 `str[Range]` 切片**：line 116/132/164/165/168。clippy 1.98 未报错（usize 来自 `.find()` 跨函数追踪 + 半开区间），但技术上与 TASK-052 已修的 32 处 indexing_slicing 同源（都是 `arr[range]` 触发 `indexing_slicing` lint 的可能路径）。本卡显式清理，避免误导读者「全部已替换」。
  2. **`refscan.rs:290` 的 stale `#[allow(dead_code)]`**：`render` 在 line 75 被 `run()` 调用（`output.write_all(render(&findings).as_bytes())`），dead_code lint 不会触发，注释本身是 dead 元注释。
  3. **测试代码 `f[0]` 不一致**：TASK-052 只 refactor 了 refscan.rs 测试的 `sorted[0..2]`，docscan.rs:248/263、card_check.rs:385 仍用 `f[0]`。test wrapper 已 allow，但对外表述应是「测试也走 safe pattern」。

- 严格遵循 TASK-052 路线 B（纯重构，不动 workspace `[lints.clippy]`）。

**In scope 清单**

1. `card_check.rs` 5 处 `str[Range]` → 全替换为 `let Some(slice) = sec.get(...) else { ... };`
2. `refscan.rs` `render` 函数前的 `#[allow(dead_code)]` 删除
3. `docscan.rs` 测试代码 `f[0]` → `f.first().expect("non-empty")`
4. `card_check.rs` 测试代码 `f[0]` → `f.first().expect("non-empty")`
5. 跑全套 11 条验收 + 写执行记录 9 节 + 更新 LEDGER

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy -p xtask --all-targets -- -D warnings        # exit 0
cargo test --workspace                                 # 全绿
cargo deny check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo llvm-cov --workspace --fail-under-lines 75
grep -nE 'sec\[.+\.\.\]|\[.+\.\.\]|f\[0\]|\#\[allow\(dead_code\)\]' \
     xtask/src/{refscan,docscan,card_check,exemptions}.rs
# 期望：仅剩 test wrapper 的 #[allow(clippy::*)] 与 per-line case_sensitive / single_char_pattern
```

**DoD**

- [ ] `card_check.rs` 无 `sec[..]` / `&content[..]` / `&rest[..]` 切片（grep 硬证据）
- [ ] `refscan.rs:render` 函数前无 `#[allow(dead_code)]`
- [ ] `docscan.rs` + `card_check.rs` 测试代码无 `f[0]`
- [ ] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [ ] `cargo test --workspace` 全绿（基线 279 + 本卡 0 增删）
- [ ] 全部 11 条 xtask 验收全绿（refscan 仍按设计 FAILED with 151 errors）
- [ ] `LEDGER.md` 追加本卡一行；执行记录 9 节填齐

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
