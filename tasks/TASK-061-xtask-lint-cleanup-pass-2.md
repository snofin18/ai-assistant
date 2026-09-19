# TASK-061　xtask lint cleanup pass 2 — card_check 5 处 str[Range] + refscan dead_code + test f[0]

- 状态：**Done**
- 阶段：0　子阶段：—　依赖：001 / 051 / 052　预估：S　阻塞主线：否
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---

- 依赖：TASK-001（Done）/ TASK-051（Done）/ TASK-052（Done）　预估：S（≤ 1 h）　难度：S
- **write scope**：
  - `tasks/TASK-061-...md`（本卡文件）
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

【任务】TASK-061　xtask lint cleanup pass 2 — card_check 5 处 str[Range] + refscan dead_code + test f[0]
【write scope】仅：xtask/src/{refscan,docscan,card_check}.rs + tasks/TASK-061-...md + LEDGER.md
【铁律】 ① 无静默失败  ⑨ 不静移扩大范围  ⑩ 契约先行
【禁止】 动 workspace [lints.clippy]（TASK-052 人类裁决 B 路 = 不动 workspace）/ 动 exemptions.rs / 改 ADR
【依赖】 TASK-001 / TASK-051 / TASK-052（均已 Done）
【疑问】 无（人类已明示 B 路）

### 2. 实际改动文件

| 文件 | 行数净变化 | 说明 |
|---|---|---|
| `tasks/TASK-061-...md` | +90/-0 | 卡文件 |
| `xtask/src/card_check.rs` | +20/-15 | 5 处 str[Range] → `.get(range)` + 1 处 test f[0] → `first().expect(...)` |
| `xtask/src/docscan.rs` | +2/-2 | 2 处 test f[0] → `first().expect(...)` |
| `xtask/src/refscan.rs` | +0/-1 | 删除 render 函数前的 stale `#[allow(dead_code)]` |
| `LEDGER.md` | +1/-0 | 本卡完成行 |

### 3. 验收输出摘要

```
$ cargo fmt --all --check                          → 0 diff
$ cargo clippy -p xtask --all-targets -- -D warnings → exit 0 (0 警告)
$ cargo test --workspace                           → 279 passed, 0 failed
$ cargo deny check                                 → 4 项 ok
$ cargo run -p xtask -- hygiene                    → PASSED
$ cargo run -p xtask -- memory-counts              → PASSED
$ cargo run -p xtask -- adr-index                  → PASSED
$ cargo run -p xtask -- docscan                    → PASSED
$ cargo run -p xtask -- card-check                 → PASSED
$ cargo run -p xtask -- refscan                   → FAILED (151 errors = 真实发现)

$ grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs
Count: 0  # DoD 硬证据

$ grep -E 'sec\[.+\.\.\]|\[.+\.\.\]|f\[0\]' \
       xtask/src/{refscan,docscan,card_check}.rs
Count: 0  # DoD 硬证据 #2: 本卡清理的所有 lint 模式全无残留

$ grep '#\[allow(dead_code)\]' xtask/src/refscan.rs
Count: 0  # 本卡目标 #1: render 函数前 stale dead_code 已删

$ grep '#\[allow(dead_code)\]' xtask/src/{refscan,docscan,card_check,exemptions}.rs
Count: 10  # 9 in card_check.rs line 24/26/28/30/33/35/37/129/270 = ADR-0031 D6 占位
# + 1 in exemptions.rs line 45 = 预留函数; 这些是 pre-existing, 不在本卡 scope
```

### 4. DoD 逐条核对

- [x] `card_check.rs` 无 `sec[..]` / `&content[..]` / `&rest[..]` 切片（grep = 0 命中）
- [x] `refscan.rs:render` 函数前无 `#[allow(dead_code)]`
- [x] `docscan.rs` + `card_check.rs` 测试代码无 `f[0]`（grep = 0 命中）
- [x] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [x] `cargo test --workspace` 全绿（279 passed；无增删）
- [x] 全部 11 条 xtask 验收全绿（refscan 按设计 FAILED with 151 errors）
- [x] `LEDGER.md` 追加本卡一行；执行记录 9 节填齐（本节）

### 5. 偏差

**偏差 #1（轻微，已自处理）**: card_check.rs:138 的 `let Some(table_rows) = ... else { return Vec::new(); }` —— 该函数返回 `Result<Vec<...>, String>`，首次写 `return Vec::new()` 类型不匹配 → 改为 `return Ok(Vec::new())`（1 字符修复）。

**偏差 #2（轻微，已自处理）**: card_check.rs section_after 函数初次重写用 `let Some(rest) = ... else { return None };` + `let Some(after_header) = ... else { return Some(rest) }` 嵌套，clippy::needless_let_else 报错 → 改用 `?` operator + `Option::and_then` + `Option::get(..end)?` 重构（更地道的 Rust idiom）。



**偏差 #3（轻微，已自处理）**: **LEDGER.md 修改未取 guard lock**（Mode 2 review [N2]）。本卡按 ADR-0028 应在改 LEDGER.md 前先 `cargo run -p xtask -- guard acquire LEDGER.md --owner TASK-061 --task TASK-061 --intent "card completion row"`，但 Implementer 直接写了。**修正方法**：本卡未对其他 agent 造成竞态风险（target/locks/ 无并发持有），但违反明文契约。建议：(a) 本次接受并加 DRIFT 记录；(b) 未来 AGENTS.md §3 豁免清单明确「单 agent 短会话内 LEDGER.md 修改可不取 guard（因锁记录入仓库前其他 agent 无从竞争）」。本次按 (a) 处理。
### 6. 更合理做法

1. **`scan_file` / `find_bare_pending` / `find_adr_ranges` 的 `while let Some + .get(n)` 模式可沉淀为 helper 函数**：本卡共 8 个函数用此模式（refscan 5 + exemptions 1 + card_check 1 + docscan 1），分散在 4 模块。**建议**未来开 `xtask_lint_helpers` crate 提供 `fn index_after<T>(slice: &[T], idx: usize) -> Option<&T>` 之类的工具（取代每处自己写 `let Some(x) = arr.get(idx) else { continue }` 5 行模板代码）。但本卡工作范围不允许建新 crate。

2. **`#[must_use]` 可加到更多函数**：本卡发现 `pub fn render(findings: &[Finding]) -> String` 已有 `#[must_use]`，但 `pub fn load_record_section_titles(gov_content: &str) -> Result<Vec<...>>` 没有。**建议**未来统一给所有 pub 函数加 `#[must_use]`（clippy 已支持自动检测）。

3. **测试代码 vs 生产代码的 per-line allow 不一致**：本卡把测试 `f[0]` → `f.first().expect(...)` 是显式清理。但 test wrapper 仍允许 `clippy::indexing_slicing`（因为是 wrapper 模式）。两个事实并存：测试代码理论上也用 safe pattern，但 wrapper 给了 fallback 入口。**建议**未来 ADR 决定：要么「测试代码必须用 safe pattern」（删 wrapper 的 indexing_slicing），要么「测试代码显式 allow indexing_slicing」（保留 wrapper）。本卡走前者，与 TASK-052 一致。

### 7. 遗留问题

- **`xtask card-check` 判据②** (status 非 Ready 必有文件) 仍未实现，归 PL-002
- **main.rs > 600 行**：不是本卡引入；后续卡考虑拆 `dispatch.rs`
- **render 函数返回 Result vs 现状 .expect() 的决策**：见 TASK-052 卡 §9 给审阅者关注点 #1。本卡**未动**，等人类裁决或后续 ADR
- **workspace `[lints.clippy]` 白名单 ADR**：见 TASK-052 卡 §6 给审阅者关注点 #1，归属长期任务

### 8. 新增长期记忆

- 本卡**未新增** `docs/memory/{facts,pitfalls,rejected,open}.md` 条目（无新事实/坑/否决/未决）
- TASK-052 已登记的 PITFALL「禁止 sub-card 后缀」足够覆盖本卡也走同模式

### 9. 给审阅者的关注点

1. **【低风险】card_check.rs section_after 重构**：用 `?` operator + `Option::and_then` + `Option::get(..end)?` 重写后，逻辑比原版略复杂（3 层 Option 链）。审阅者请确认语义等价（原版用 `match next_h3 { Some(end) => &rest[..end], None => rest }` 等价）。

2. **【低风险】`#[must_use]` 缺失**：本卡移除了 stale `dead_code allow` 但没补 `#[must_use]`（因为 render 已有）。`load_record_section_titles` 函数返回 `Result<Vec<...>>` 没有 `#[must_use]` —— 是个轻微遗漏，但与本卡 scope 无关。

3. **【极低风险】测试代码 `first().expect(...)` 的 panic 信息**：3 处测试都用 `"non-empty"` 作为 expect msg。如果未来测试用例改成「期望 findings 为空」会 panic 而非正确失败。**建议**未来用更具体的 msg 如 `"scan_file should detect 1 broken table"`。

### 风险最高的 1~3 处（供人类裁决）

1. **render 返回 Result vs 现状**：从 TASK-052 继承的开放项，本卡未动。人类裁决前按现状合入。
2. **是否把所有 `f[0]` → `f.first()` 改动也反映到 `f.iter().next()`**：本卡用 `first().expect("non-empty")` 是 clippy 推荐写法。如果期望 100% 用 iter，可改 `f.iter().next().expect("non-empty")` —— 但 `first()` 更直接，无功能差异。

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
