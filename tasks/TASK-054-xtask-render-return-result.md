# TASK-054　xtask render() 返回 Result — 消除 per-line unwrap_used 允许

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001 / 051 / 052 / 053　预估：S　阻塞主线：否
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- 依赖：TASK-001 / TASK-051 / TASK-052 / TASK-053（均已 Done）　预估：S（≤ 30 min）　难度：S
- **write scope**：
  - `tasks/TASK-054-...md`（本卡文件）
  - `xtask/src/refscan.rs`（改 `render` 签名 + `run` 适配）
  - `README.md`（同步本卡进展）
  - `LEDGER.md`（追加本卡一行）
  - `docs/PARKING_LOT.md`（关闭 PL-NEW 跟进项）
- **Out of scope**：
  - workspace `[lints.clippy]` 配置**不动**（TASK-052 人类裁决 B 路 = 不动 workspace）
  - 其他 xtask 子命令的 `render` / `write` 模式（refscan 独有模式）
  - 新建 ADR（TASK-052 §9 #1 决策已在本卡实施，无需 ADR 路径）

**背景与源流**

- TASK-052 / TASK-053 都遗留一个开放项：render 函数（refscan.rs:292）用 `.expect("...")` 处理 `writeln!` 到 `String` 的 Result，因 `writeln!` 到 `String` 永不失败（仅 OOM），`expect` 等同于 panic。`#[allow(clippy::unwrap_used, clippy::expect_used)]` per-line allow 被加上作 bypass。
- 人类在 TASK-051 / TASK-052 / TASK-053 三轮 review 中反复提出这一处应改成 `render() -> Result<String, std::fmt::Error>`，把错误上抛到 `run() -> Result<u8, String>`。
- 本卡实施 B 路径（人类推荐）：改签名 + 删 per-line allow + 错误上抛。
- 严格遵循 TASK-052 路线 B（纯重构 = 不动 workspace lints = 漂移触发器 ⑥ 0 命中）。

**In scope 清单**

1. `pub fn render(findings: &[Finding]) -> String` → `pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error>`
2. render 内部 `writeln!(...).expect("...")` → `writeln!(..)?`
3. 删除 render 前的 `#[allow(clippy::unwrap_used, clippy::expect_used)]`
4. `run()` 中 `render(&findings).as_bytes()` → `render(&findings)?.as_bytes()`（用 `?` operator）
5. README.md 同步本卡落地
6. 跑全套 11 条验收 + 写执行记录 9 节 + 更新 LEDGER

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy -p xtask --all-targets -- -D warnings        # exit 0（per-line allow 移除后仍无新警告）
cargo test --workspace                                 # 279 passed（无增删）
cargo deny check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan                            # 行为不变（仍报 151 errors）
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
grep -nE '#!\[allow\(clippy::unwrap_used|#!\[allow\(clippy::expect_used' \
     xtask/src/refscan.rs
# 期望：0 命中（per-line allow 已删）
grep -nE '\.expect\(' xtask/src/refscan.rs
# 期望：仅在 #[cfg(test)] 测试代码里有 .expect()（如有）
```

**DoD**

- [ ] `pub fn render` 返回 `Result<String, std::fmt::Error>`
- [ ] render 内部无 `#[allow(clippy::unwrap_used, clippy::expect_used)]`
- [ ] render 内部无 `.expect("...")` 处理 writeln! Result
- [ ] `run()` 用 `?` operator 上抛 render 错误
- [ ] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [ ] `cargo test --workspace` 全绿（279 passed）
- [ ] 全部 11 条 xtask 验收全绿
- [ ] `README.md` 同步本卡落地（新增 sub-bullet）
- [ ] `LEDGER.md` 追加本卡一行

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-054　xtask render() 返回 Result — 消除 per-line unwrap_used 允许
【write scope】仅：xtask/src/refscan.rs + tasks/TASK-054-...md + README.md + LEDGER.md + docs/PARKING_LOT.md
【铁律】 ① 无静默失败  ⑨ 不静移扩大范围  ⑩ 契约先行
【禁止】 动 workspace [lints.clippy]（TASK-052 人类裁决 B 路 = 不动 workspace）/ 其他 xtask 子命令 / 改 ADR
【依赖】 TASK-001 / TASK-051 / TASK-052 / TASK-053（均已 Done）
【疑问】 无（人类已在 TASK-051 报告后明示「做 B，纯重构」+ TASK-053 报告后选 b = render 返回 Result）

### 2. 实际改动文件

| 文件 | 行数净变化 | 说明 |
|---|---|---|
| `tasks/TASK-054-...md` | +90/-0 | 卡文件 |
| `xtask/src/refscan.rs` | +3/-6 | render 签名 String→Result + 删 `#[must_use]`（Result 自带）+ 删 per-line allow + run() 加 `.map_err(\|e\| e.to_string())?` |
| `README.md` | +9/-0 | 「最近进展」节补 TASK-054 条目（按用户「README 漂移 = bug」反馈） |
| `LEDGER.md` | +1/-0 | 本卡完成行 |
| `docs/PARKING_LOT.md` | +0/-0 | 本卡无新条目（PL-NEW 已在 TASK-052 关闭） |

### 3. 验收输出摘要

```
$ cargo fmt --all --check                          → 0 diff
$ cargo clippy -p xtask --all-targets -- -D warnings → exit 0
$ cargo test --workspace                           → 279 passed, 0 failed
$ cargo deny check                                 → 4 项 ok
$ cargo run -p xtask -- hygiene                    → PASSED
$ cargo run -p xtask -- memory-counts              → PASSED
$ cargo run -p xtask -- adr-index                  → PASSED
$ cargo run -p xtask -- docscan                    → PASSED
$ cargo run -p xtask -- card-check                 → PASSED
$ cargo run -p xtask -- refscan                   → FAILED (151 errors = 真实发现, 行为不变)

$ grep '#\[allow(clippy::unwrap_used, clippy::expect_used)\]' xtask/src/refscan.rs
Count: 0  # DoD 硬证据: per-line allow 已删
```

### 4. DoD 逐条核对

- [x] `pub fn render` 返回 `Result<String, std::fmt::Error>`
- [x] render 内部无 `#[allow(clippy::unwrap_used, clippy::expect_used)]`
- [x] render 内部无 `.expect("...")` 处理 writeln! Result
- [x] `run()` 用 `?` operator 上抛 render 错误（`.map_err(|e| e.to_string())?` 适配到 `Result<u8, String>`）
- [x] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0（修了 doc_markdown backticks + double_must_use）
- [x] `cargo test --workspace` 全绿（279 passed）
- [x] 全部 11 条 xtask 验收全绿（refscan 仍报 151 真实发现）
- [x] `README.md` 同步本卡落地（按用户「README 漂移 = bug」反馈建立习惯）
- [x] `LEDGER.md` 追加本卡一行

### 5. 偏差

**偏差 #1（轻微，已自处理）**: clippy 修复中遇到 2 个连续 lint:
- a) `clippy::doc_markdown`（缺 backticks on `Result<u8, String>`）— 加 backticks
- b) `clippy::double_must_use`（render 返回 `Result<_, _>` 已自带 `must_use` 语义，重复声明）— 删除 `#[must_use]`

### 6. 更合理做法

1. **render 返回 Result 模板**：本卡建立的 `pub fn render(...) -> Result<String, std::fmt::Error>` + `writeln!(..)?` + `.map_err(|e| e.to_string())?` 模式可推广到其他 xtask 子命令（如 docscan 的输出路径）。**建议**未来 card-check 输出函数也走 Result 路径。

2. **`#[must_use]` 在 Result 返回类型上的冗余**：本卡发现 `Result<T, E>` 自带 must_use 语义，重复声明 `#[must_use]` 触发 clippy::double_must_use。**建议**未来不再给返回 `Result<_, _>` 的函数加 `#[must_use]`（如不确定，留给 clippy 检查）。

### 7. 遗留问题

- （从 TASK-052 / TASK-053 继承）`xtask card-check` 判据② 仍未实现，归 PL-002
- （从 TASK-052 / TASK-053 继承）main.rs > 600 行 ADR-0033 软上限
- （从 TASK-053 继承）`xtask/src/card_check.rs` 9 处 + `exemptions.rs` 1 处 pre-existing `#[allow(dead_code)]` 保留（ADR-0031 D6 占位 + 预留函数）
- （从 TASK-053 继承）防御性 else 分支可加注释解释为何不可达 [N3]
- （从 TASK-053 继承）任务卡结构二次 `### 1-9` 空模板冗余 [N6]
- （从 TASK-053 继承）`fn section_after` 缺 doc comment [N8]
- （本卡新增）`run()` 的 `render(&findings).map_err(|e| e.to_string())?` 适配路径：未来若有多个 `Result`-返回子函数串联，可抽成 helper

### 8. 新增长期记忆

- 本卡**未新增** `docs/memory/{facts,pitfalls,rejected,open}.md` 条目（实施是 render 签名修改，无新事实/坑）

### 9. 给审阅者的关注点

1. **【极低风险】行为不变**：render 的输出格式未改（仍 `writeln!(out, "{} {}:{} {}", ...)`），仅 Result 化表面。`refscan` 仍报 151 真实发现。

2. **【低风险】render 错误处理路径**：`run()` 用 `.map_err(|e| e.to_string())?` 把 `std::fmt::Error` 转成 `String` 后上抛。`Failure::Io` 已存在但本路径不经过 Failure 枚举——错误直接成 `Result::Err(String)`。审阅者请确认这个映射语义符合预期（= 与已有的 `output.write_all(...).map_err(|e| e.to_string())` 一致）。

3. **【建立习惯】README 同步**：按用户反馈「README 漂移 = bug」，本卡每次完成都同步 README。本卡新增了 TASK-054 条目到「最近进展」节。建议以后每张卡完成都补 3-9 行 progress 条目（不重写整个 README，避免 diff 噪音）。

### 风险最高的 1~3 处（供人类裁决）

1. **render 错误的字符串映射**：`std::fmt::Error` 的 `to_string()` 输出是 `"an error occurred when formatting an argument"`（impl Display 是固定的），可能不够具体。**建议**：未来可定义 `enum XtaskError { Io(io::Error), Fmt(std::fmt::Error), Parse(...) }` 替代裸 String，但本卡 scope 不允许扩。

2. **README 同步粒度**：当前每次完成卡就补 1 个 sub-bullet。**建议**：每 2-3 张卡集中做一次 README 更新，避免 diff 噪音；或者保持每次更新（= 当前做法），让 README 永远 ≤ 24 小时滞后。

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
