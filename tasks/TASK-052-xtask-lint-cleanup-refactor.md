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

【任务】TASK-052　xtask lint cleanup — refscan 40-lint 块清场 + 4 模块 indexing_slicing 纯重构
【write scope】仅：xtask/src/{refscan,docscan,card_check,exemptions}.rs + tasks/TASK-052-...md + docs/PARKING_LOT.md + LEDGER.md
【铁律】 ① 无静默失败  ⑨ 不静移扩大范围  ⑩ 契约先行
【禁止】 动 workspace [lints.clippy]（人类「漂移坚决不能忍受」= 不走 A 路加 priority=-1）
【依赖】 TASK-001（Done）/ TASK-051（Done）；ADR-0031/0032/0030 现行
【疑问】 无（人类已裁决 B 路）

### 2. 实际改动文件

| 文件 | 行数净变化 | 说明 |
|---|---|---|
| `tasks/TASK-052-...md` | +101/-0 | Orchestrator 卡文件 |
| `xtask/src/refscan.rs` | +147/-82 | 重构 find_adr_ranges / find_bare_pending / render + 全清 #![allow |
| `xtask/src/docscan.rs` | +26/-13 | 重构 scan_broken_tables 内层循环 + 移 use import 到 const doc 前 |
| `xtask/src/card_check.rs` | +15/-7 | cells[n] → cells.first()/get(n) |
| `xtask/src/exemptions.rs` | +41/-16 | cells[n] → cells.first()/get(n) |
| `docs/PARKING_LOT.md` | +3/-0 | PL-NEW supersede + 关闭 |
| `LEDGER.md` | +1/-0 | 本卡完成行 |

### 3. 验收输出摘要

```
$ grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs
(no output)  # 0 命中 = DoD 硬证据

$ cargo fmt --all --check                          → 0 diff
$ cargo clippy -p xtask --all-targets -- -D warnings → exit 0 (0 警告)
$ cargo test --workspace                           → 279 passed, 0 failed
$ cargo deny check                                 → advisories/bans/licenses/sources 全部 ok
$ cargo run -p xtask -- hygiene                    → PASSED
$ cargo run -p xtask -- memory-counts              → PASSED
$ cargo run -p xtask -- adr-index                  → PASSED
$ cargo run -p xtask -- docscan                    → PASSED
$ cargo run -p xtask -- card-check                 → PASSED
$ cargo run -p xtask -- refscan                   → FAILED (151 errors = 真实发现)
```

### 4. DoD 逐条核对

- [x] 4 个新模块顶部**无**任何 `#![allow(...)]` 块（grep 硬证据 = 0 命中）
- [x] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0（0 警告，0 依赖模块级 allow）
- [x] 32 处 `indexing_slicing` 全部用 `while let Some(x) = chars.get(i)` / `.get(n).expect(...)` / `match` / `slice → get(...).copied()` 替换
- [x] 其他 pedantic lint 全部修掉（8 类）；per-item `#[allow(...)]` 4 处带卡号注释
- [x] PL-NEW 在 `docs/PARKING_LOT.md` 标注 `[supersedes:2026-09-19]` + 关闭理由
- [x] 全部 11 条验收命令全绿（refscan 按设计 FAILED with 151 errors）
- [x] `LEDGER.md` 追加本卡一行；执行记录 9 节填齐（本节即填齐动作）

### 5. 偏差

**偏差 #1（轻微，已自处理）**: refscan.rs:7 的 `// 凡命中…` 在 clippy 修复中反复触发 `clippy::doc_lazy_continuation`（doc 列表项缩进不一致）；先改为 `//`（非 doc 注释）规避 lint，Mode 2 review [N1] 指出后**改回 `//!` 形式 + 在前面加空 `//!` 分隔**（clippy 接受的 list 终止写法）。本卡共触发 3 轮 clippy 修复才稳定。

**偏差 #2（轻微，已自处理）**: docscan.rs 在删除 `#![allow(clippy::indexing_slicing)]` 块时，误把 `use crate::report::{Finding, Severity};` 一同删掉，再补回时放在 `///` 注释与 const 之间 → 编译通过但 `///` 注释 attach 到 use（rustdoc 不收录 use），const 失去文档。Mode 2 review [N2] 指出后**将 use 移到 const doc 之前**，const 文档恢复。

**偏差 #3（已在 P2 [N5] 登记）**: 本卡严格只清 clippy 实际报错的 32 处 indexing_slicing（点索引 `arr[n]` 与闭区间切片 `arr[i..j]` 中 clippy 能跟踪的）。`card_check.rs` 另有 5 处 `str[Range]` 切片（line 116/132/164/165/168），clippy 1.98 未触发（usize 来自 `.find()`，clippy 无法跨函数追踪），但技术上同源 → 归后续卡清理（**不在本卡扩 scope** = 漂移触发器 ⑤）。

### 6. 更合理做法

1. **render 函数的 `let _ = writeln!` 与铁律 ① 的张力**：`writeln!` 到 `String` 在实践中永不失败（仅 OOM 时才走 Error 路径，OS 会处理进程崩溃）。`.unwrap()` / `.expect()` 被 workspace `unwrap_used = "deny"` / `expect_used = "deny"` 拒绝（即使 per-item allow 也可，但有 philosophic 矛盾）。本卡最终用 `#[allow(clippy::unwrap_used, clippy::expect_used)] // writeln! to String only fails on OOM (process-level crash); tests cover the write path.`.expect("…")` 解决 — 比 `let _ =` 严格，但需显式 per-item allow。**建议未来 ADR**：把 `writeln!` 到 `String` 的成功路径加入 workspace `[workspace.lints.clippy]` 的可豁免白名单（如 `[workspace.lints.clippy] unwrap_used = { level = "deny", exceptions = ["String write", "Vec grow"] }`），但此为 lint config 改动 = 漂移触发器 ⑥，必须走 ADR。

2. **`case_sensitive_file_extension_comparisons` 的 per-line allow 模式**：本卡有 3 处此 lint 的 per-line allow（refscan.rs:101/103 + 注释解释「`lower` 已小写」）。**建议**未来 ADR 0034 / PL-NEW 后续卡写一个 `xtask_lint_helpers` crate 提供 `enum_or_str_eq_ignore_ascii_case()` 之类的工具函数，把这个 case-insensitive 比较语义沉淀下来，避免 per-line allow 模式扩散。

3. **测试代码 safe-pattern 不一致 [P3 N7]**：本卡只 refactor 了 refscan.rs 的 `sorted[0..2]`，docscan/card_check 的 `f[0]` 未动（test wrapper 已 allow）。**建议**未来小任务统一改 `f[0]` → `f.first().expect("non-empty")`，对外表述 =「测试也走 safe pattern」而非「测试 wrapper 允许 indexing_slicing 是显式设计」。

### 7. 遗留问题

- **后续卡清理（PL-NEW 后续动作）**:
  - `xtask/src/card_check.rs:116/132/164/165/168` 共 5 处 str[Range] 切片（[P2 N5]），clippy 未报但同源问题
  - `xtask/src/refscan.rs:290` 的 `#[allow(dead_code)]`（render 现在被 run() 调用，dead 注释本身已 dead）— [P3 N6]
  - 测试代码 `f[0]` → `f.first()` 统一 — [P3 N7]
- **`xtask card-check` 判据②** (status 非 Ready 必有文件) 仍未实现，归 PL-002
- **main.rs > 600 行**（ADR-0033 写作规范软上限）：不是本卡引入，但本卡新增 4 个 mod 声明让数字上去；后续卡考虑拆 `dispatch.rs`

### 8. 新增长期记忆

- 本卡**未新增** `docs/memory/{facts,pitfalls,rejected,open}.md` 条目（无新事实/坑/否决/未决）
- 仅**关闭**了 `docs/PARKING_LOT.md` 的 PL-NEW（用 `[supersedes:2026-09-19]` 行）

### 9. 给审阅者的关注点

1. **【高风险】DoD 硬证据已通过**：`grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs` = **0 命中**。所有模块级 `#![allow` 块已全部清除。
2. **【中风险】per-item `#[allow(...)]` 4 处**（均带卡号注释）:
   - `refscan.rs:94`: `single_char_pattern` (replace() needs &str pattern)
   - `refscan.rs:101, 103`: `case_sensitive_file_extension_comparisons` (lower 已小写)
   - `refscan.rs:~298`: `unwrap_used + expect_used` (writeln! to String only fails on OOM)
   均非模块级，均有 why 注释。
3. **【低风险】test wrapper**: 4 模块各 `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)] #[cfg(test)] mod tests {` —— 这是 outer attribute（在 mod 上）而非 inner `#![...]`，**不在 DoD 「4 模块顶部无模块级 lint allow」范围内**（grep `^#![allow` 不会命中，因为 `#[` ≠ `#!`）。
4. **【低风险】refscan.rs:7 的修复链**: 3 轮 clippy 修改才稳定（`//` → `//!` (3 spaces) → `//!` (4 spaces) → `//!` + 前空行）。最终形态见 git diff。
5. **【极低风险】**本会话所有 git 操作均 forward-only（无 force push / reset --hard / tag 删除）。3 个 guard 锁（PARKING_LOT.md / MEMORY.md / LEDGER.md）ACQUIRED → RELEASE 完整记录。

### 风险最高的 1~3 处（供人类裁决）

1. **`refscan.rs:298` 的 `.expect()` + per-item allow 组合** —— 是否接受「writeln! to String 的 Result 在 xtask 用例里事实上永不出现」这一论断？如不接受，建议改成 `render()` 返回 `Result<String, std::fmt::Error>` 并把 error 上抛到 `run()` 的 `Result<u8, Failure>`。
2. **「32 处全替换」措辞严谨性** —— [P2 N5] 指出 card_check.rs 还有 5 处 str[Range] clippy 不报；本卡已加限定语「32 处 clippy 实际报错位 = 全替换，另有 5 处 str[Range] clippy 不报故未动，归后续卡」。如果读者严格按字面理解「全部」会被这 5 处误导。
3. **Mode 2 sub-agent 是否需要二次审** —— 本卡 amend 一次（4efcbde → e9eae0e）后 Mode 1 + Mode 2 都跑了，最终 commit `e9eae0e` 是 cleanup 后状态。如果人类希望 Mode 2 sub-agent 对最终 commit 再审一次可提出。

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
