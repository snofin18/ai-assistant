# TASK-055　清 refscan.rs 最后 3 处 per-line allow

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001 / 051 / 052 / 053 / 054　预估：S　阻塞主线：否

---

- 依赖：TASK-001 / TASK-051 / TASK-052 / TASK-053 / TASK-054（均已 Done）　预估：S（≤ 15 min）
- **write scope**：
  - `tasks/TASK-055-...md`（本卡文件）
  - `xtask/src/refscan.rs`（3 处 per-line allow 消除）
  - `README.md`（同步本卡进展）
  - `LEDGER.md`（追加本卡一行）

**背景与源流**

TASK-052 / TASK-053 清掉了 32 处 indexing_slicing + 5 处 str[Range] + 3 处 test f[0] + 1 处 stale dead_code。
TASK-054 把 render 改 Result 化，消除了 per-line `#[allow(unwrap_used, expect_used)]`。
ADR-0035 把 B 路（workspace 不动 exceptions）显式化。

**剩余 per-line allow**（来自 TASK-052 baseline + 3 轮清理）：
- `refscan.rs:94` `#[allow(clippy::single_char_pattern)]` — `replace("\r", "\n")` workaround
- `refscan.rs:101` `#[allow(clippy::case_sensitive_file_extension_comparisons)]` — `lower.ends_with(".md")` workaround
- `refscan.rs:103` `#[allow(clippy::case_sensitive_file_extension_comparisons)]` — `lower.ends_with(".ps1")` workaround

**本卡目标**：通过改 API 调用消除这 3 处 allow，达到「生产代码 per-line allow = 0」的目标。

**In scope 清单**

1. `replace("\r", "\n")` → 改用 char 字面量（Rust `str::replace` 的 `Pattern` impl 接受 `char`）
2. `lower.ends_with(".md")` → 改用 `std::path::Path::extension()` 链式 `.is_some_and(...)` 模式
3. 删 3 处 per-line allow
4. 跑全套 11 条验收 + 写执行记录 9 节 + 更新 README + LEDGER

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy -p xtask --all-targets -- -D warnings        # exit 0
cargo test --workspace                                 # 全绿
cargo deny check
cargo run -p xtask -- refscan                         # 151 errors 行为不变
cargo run -p xtask -- hygiene / memory-counts / adr-index / docsan / card-check 全部 PASSED

# DoD 硬证据
grep -n '#\[allow(clippy::single_char_pattern|#\[allow(clippy::case_sensitive_file_extension_comparisons' \
     xtask/src/refscan.rs
# 期望：0 命中
```

**DoD**

- [ ] `xtask/src/refscan.rs` 无 `#[allow(clippy::single_char_pattern)]`
- [ ] `xtask/src/refscan.rs` 无 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`
- [ ] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [ ] `cargo test --workspace` 全绿（279 passed）
- [ ] 全部 11 条 xtask 验收全绿
- [ ] `README.md` 同步本卡
- [ ] `LEDGER.md` 追加本卡

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-055 清 refscan.rs 最后 3 处 per-line allow
- 目标：production code per-line allow = 0
- write scope：仅 tasks/TASK-055-clear-last-3-per-line-allows.md / xtask/src/refscan.rs / README.md / LEDGER.md
- 铁律相关：铁律 2（五类不可信输入）/ 7（core 不得调平台 API）/ 9（不得静默扩大范围）/ 10（契约先行）/ 6（文档与代码同步）/ §3 启动协议
- 禁止：不动 workspace / 不改其他文件 / 不加新依赖 / 不加 unsafe / 不放宽 lint / 不加新 `#![allow]` 块 / 不改测试断言
- 验收：cargo fmt --all --check → 0 diff；cargo clippy -p xtask --all-targets -- -D warnings → exit 0；cargo test --workspace → 279 passed；cargo deny check → 4 项 ok；xtask {refscan,hygiene,memory-counts,adr-index,docsan,card-check} → 全 PASSED；grep `single_char_pattern|case_sensitive_file_extension_comparisons` xtask/src/refscan.rs → 0 命中
- 依赖：TASK-001 / 051 / 052 / 053 / 054 均 Done（LEDGER 复核）；guard 无锁残留（git status 干净）
- 疑问：无；refscan.rs 现有测试覆盖 .md / .rs / .ps1 三类扩展名 + 大小写无关，无需新增测试

### 2. 实际改动文件

- `xtask/src/refscan.rs` (+7/-11)：refscan.rs line 94 `replace("\r", "\n")` 改 `replace('\r', "\n")` + 删 line 94 per-line allow；refscan.rs line 96-106 替换为：闭包 `ext_is(expected: &str) -> bool` 用 `Path::extension().and_then(to_str).is_some_and(eq_ignore_ascii_case)` + `is_md_or_rs = ext_is("md") || ext_is("rs")` + `is_ps1 = ext_is("ps1")` + 删除 `let lower = rel_path.to_ascii_lowercase();` 与配套解释注释 + 2 处 per-line allow
- `README.md`：新增 TASK-055 历史小节（7 行）+ 修改「最近进展」节标题为「TASK-051/052/053/054/055 五连发」
- `LEDGER.md`：追加 TASK-055 行（1 条事件）
- `tasks/TASK-055-clear-last-3-per-line-allows.md`：本卡执行记录 9 节（仅本节写入；卡片正文在分界线上保持原样未动）
- **未改**：workspace lints（严守 ADR-0035 B 路）/ `Cargo.toml` / `docs/adr/*` / `MEMORY.md` / 其他 crate / 其他应用 `docs/memory/apps/*.md`

### 3. 验收输出摘要

- `cargo fmt --all --check` → exit 0（无 diff）
- `cargo clippy -p xtask --all-targets -- -D warnings` → exit 0（**关键**：0 错误 0 警告，包含 pedantic 全套 + workspace deny 全部规则）
- `cargo test --workspace` → **279 passed**（与 TASK-054 baseline 一致）
- `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`（exit 0；warnings about 多余的 allow 列表项 = 未触动项，与 TASK-054 一致）
- `cargo run -p xtask -- refscan` → scanned=145, errors=151（**baseline 一致**：refscan 行为不变，仍能识别全部 151 个 ADR-0032 baseline 命中）
- `cargo run -p xtask -- hygiene` → scanned=26, errors=0, warnings=1（main.rs:624 文件 624 行超建议上限 600 = 老警告，本卡不动）
- `cargo run -p xtask -- memory-counts` → scanned=7, errors=0, warnings=0
- `cargo run -p xtask -- adr-index` → scanned=19, errors=0, warnings=0
- `cargo run -p xtask -- docscan` → scanned=114, errors=0, warnings=0
- `cargo run -p xtask -- card-check` → scanned=65, errors=0, warnings=19（均为 stub 阶段 1 卡片的「记录区未填」警告，本卡不动）
- **DoD 硬证据**（卡面明确指定的 grep）：`grep 'single_char_pattern|case_sensitive_file_extension_comparisons' xtask/src/refscan.rs` → **0 命中**
- **复核扩展**（外推到全 xtask/src）：`grep 'single_char_pattern|case_sensitive_file_extension_comparisons' xtask/src/**/*.rs` → 1 命中 = **xtask/src/repowalk.rs:336**（卡 scope 外，见 §6/§7/§9）

### 4. DoD 逐条核对

- [x] `xtask/src/refscan.rs` 无 `#[allow(clippy::single_char_pattern)]` —— 0 命中
- [x] `xtask/src/refscan.rs` 无 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` —— 0 命中
- [x] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [x] `cargo test --workspace` 全绿（279 passed）
- [x] 全部 11 条 xtask 验收全绿（fmt / clippy / test / deny / refscan / hygiene / memory-counts / adr-index / docscan / card-check + DoD grep）
- [x] `README.md` 同步本卡（新增 TASK-055 历史小节 + 修改最近进展标题）
- [x] `LEDGER.md` 追加本卡

### 5. 偏差

none（本卡严格按卡面 In scope 清单执行，未越界、未加新依赖、未放宽 lint、未加 unsafe、未改公共接口、未引入新抽象层、未删改 ADR）

### 6. 更合理做法

#### 6.1 已记录的「卡外发现」与处理（**不算漂移**，仅作 §7/§9 提醒）

`xtask/src/repowalk.rs:336` `#[allow(clippy::case_sensitive_file_extension_comparisons)]` 覆盖 fn `collect_repo_files_recursively`，其内部 image-extension check 用 `lower.ends_with(".png")` 等 4 处模式。**卡面标题「最后 3 处 per-line allow」与 In scope 都严格限制 refscan.rs**，故本卡未触。两条更合理路径（任一非漂移触发器 ④）：

- **路径 A（推荐）**：开 `TASK-055b` 单卡清掉，**复用本卡的 `ext_is` closure 模式**，照 `has_rust_extension`（line 210，同模块现成 `Path::extension` 模板）做一遍 image-extension helper。预计 S 级（≤15 min）。
- **路径 B**：本卡不动，给 ADR-0035 baseline 表补一行登记「repowalk.rs:336 1 处」，把「清点后生产代码 per-line allow 目标 = 0」明确为「refscan.rs 内 = 0；repowalk.rs 内 = 1 待处理」。

**两条路径都不动 ADR 已决事项**（ADR-0035 §1 仅写「清点后生产代码 per-line allow 目标 = 0」，**仅含 refscan 3 条 + dead_code 10 条 + test wrapper 4 处 = 17 条**；本卡清 3 条 = 14 条；repowalk.rs:336 不在其表内也未在任何历史 baseline 出现 = **与 baseline 矛盾** = 应在路径 B 中修正文字即可；不算 drift ④ 触发）。

#### 6.2 不应做但被卡面漏列的更优路径

未发现。卡面给的两种 fix（char 字面量 + `Path::extension` + `is_some_and`）已经是 ADR-0035 §决策 1「首选：改代码/改 API 签名」的标准路径。

### 7. 遗留问题

- **repowalk.rs:336 per-line allow**（§6.1 路径 A/B 待决策）
- **ADR-0035 baseline 表漏列 repowalk.rs:336**（同上）
- **stage-1 46 张卡仍是 Ready 占位**（stage-0 范围，本卡不触；与 ADR-0031 一致）
- **xtask card-check 判据② 仍归 PL-002**（TASK-052 已知遗留，与本卡无关）
- **PL-NEW / TASK-052 已知偏差 #1（render 函数 Result 决策）**已由 TASK-054 关闭（render 现在返回 `Result<String, std::fmt::Error>`，本卡 §3 已验证不变）

### 8. 新增长期记忆

无（本卡纯实现 B 路 = 改代码不改 workspace；所有新认知都已内化到 ADR-0035 的 baseline 表 + 本卡 §6/§7；无需新增 `docs/memory/{facts,pitfalls,rejected}.md` 条目）

### 9. 给审阅者的关注点

1. **repowalk.rs:336 GAP（最高优先）**：本卡严格按卡面 scope 限制只动 refscan.rs，未触 repowalk.rs 的同型 lint（覆盖 line 192/196 的 4 处 `lower.ends_with(".png"/.jpg/.gif/.bmp/.webp")` 等 image extension 检查）。**这条 allow 在 ADR-0035 baseline 表里未登记**——如果审阅者认为 ADR-0035 应同时是「全仓 baseline」，则本卡连同 ADR-0035 一并需要回填登记。处理路径见 §6.1。
2. **ext_is closure 复用性**：本卡用局部闭包 `ext_is(expected: &str) -> bool` 把「Path::extension + case-insensitive comparison」封装一次。同一 closure 模板可直接复用到 §6.1 路径 A（repowalk.rs image extensions）。审阅者可建议：是否要把它提到 `repowalk` 模块级 helper（与 `has_rust_extension` 并列）以便两处共用？
3. **非 UTF-8 扩展名的语义等价性论证**：原 `lower.ends_with(".md")` 在 `lower: &str` 上要求 UTF-8 合法；新 `Path::extension().and_then(to_str).is_some_and(...)` 在 `to_str()` 失败时返回 `false`。**两端在非 UTF-8 路径上行为同形**（都 = false）。审阅者若想加 1 条单元测试覆盖「非 UTF-8 路径名返回 false」可作为下一卡的 test 增量。
4. **render Result 化的可逆性**：本卡未触动 render 函数（TASK-054 决策已完成）。但若 §6.1 路径 A 推进且 repowalk.rs 也走 Result 化，refscan.rs + repowalk.rs 共有 2 个 `render → Result` 模式，**未来是否考虑在 `xtask::report` 模块里集中处理**（目前分散在 refscan.rs：297 + repowalk.rs：417 两个不相关函数）。
