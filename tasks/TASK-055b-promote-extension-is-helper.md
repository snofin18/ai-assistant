# TASK-055b　promote extension_is helper + 收 repowalk.rs:336 per-line allow

- 状态：**InProgress**
- 阶段：0　子阶段：—　依赖：001 / 055　预估：S（≤ 20 min）　阻塞主线：否

---

- 依赖：TASK-001 / TASK-055（均已 Done，HEAD = cc32c1d）　预估：S（≤ 20 min）
- **write scope**：
  - `tasks/TASK-055b-...md`（本卡文件）
  - `xtask/src/repowalk.rs`（加 `pub fn extension_is` + 重写 `collect_repo_files_recursively`）
  - `xtask/src/refscan.rs`（用 `crate::repowalk::extension_is` 替换 local `ext_is` closure）
  - `docs/adr/0035-workspace-lint-policy-no-exceptions.md`（baseline 表补一行 + §1 决策 2 加一条）
  - `README.md`（同步本卡进展）
  - `LEDGER.md`（追加本卡一行）

**背景与源流**

TASK-055 清掉 refscan.rs 最后 3 处 per-line allow，达到「refscan.rs 生产代码 per-line allow = 0」。
但 `xtask/src/repowalk.rs:336`（即 line 172 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`）未触：
- TASK-052 的 4 模块 sweep 仅覆盖 refscan/docscan/card_check/exemptions，未含 repowalk
- TASK-055 卡面标题与 In scope 严格限制 refscan.rs
- TASK-055 §6.1 已记录此 GAP + 建议「TASK-055b 单卡清掉」+「复用 `ext_is` closure 模式」

ADR-0035 §1 baseline 表也漏列 repowalk.rs:336（仅列 refscan 3 + dead_code 10 + test wrapper 4 = 17 条；本卡完成后实际剩 14 条）。

**本卡目标**：

1. 提升 `ext_is` 从 refscan.rs local closure 为 `repowalk::extension_is` 模块级 `pub fn`
2. 重写 `collect_repo_files_recursively` 用 `extension_is`（删 per-line allow）
3. 重写 refscan.rs 用 `crate::repowalk::extension_is`（删 local closure）
4. 加 4 条 `extension_is` 单元测试（含非 UTF-8 返回 false）
5. ADR-0035 baseline 表补一行登记此条 + §决策 2 末尾补一条「未来 per-line allow 落地需在 baseline 登记」
6. 跑全套 11 条验收 + 写执行记录 9 节 + 更新 README + LEDGER

**In scope 清单**

1. `pub fn extension_is(path: &Path, expected: &str) -> bool` 到 repowalk.rs（紧邻 `has_rust_extension`）
2. `collect_repo_files_recursively` 内 2 处 `lower.ends_with(...)` 改 `extension_is(&path, ...)`
3. refscan.rs 删除 `let ext_is = |expected: &str| -> bool { ... }` 闭包，改 `use crate::repowalk::extension_is;` + 3 处调用改 `extension_is(Path::new(rel_path), "md")` 等
4. test module 加 4 条 `extension_is` 测试：positive / case-insensitive / 非 UTF-8 返回 false / no extension 返回 false
5. ADR-0035 §1 baseline 表新增一行「`pub fn extension_is` 新增 = TASK-055b」 + §决策 2 加一条「新增模块级 `pub fn` 若涉及 per-line allow 替代，必须在 baseline 表登记」
6. README 「最近进展」节新增 TASK-055b 历史小节

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy -p xtask --all-targets -- -D warnings        # exit 0
cargo test --workspace                                 # 全绿（279 passed 不变）
cargo deny check
cargo run -p xtask -- refscan                         # 151 errors 行为不变
cargo run -p xtask -- hygiene / memory-counts / adr-index / docsan / card-check 全部 PASSED

# DoD 硬证据
grep -n '#\[allow(clippy::case_sensitive_file_extension_comparisons' \
     xtask/src/{refscan,repowalk}.rs
# 期望：0 命中

grep -n 'pub fn extension_is' xtask/src/repowalk.rs
# 期望：1 命中
```

**DoD**

- [ ] `xtask/src/repowalk.rs` 无 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`（line 172 已删）
- [ ] `xtask/src/refscan.rs` 无 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`（line 101/103 已删，TASK-055 已做）
- [ ] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [ ] `cargo test --workspace` 全绿（279 passed 不变）
- [ ] 全部 11 条 xtask 验收全绿
- [ ] `pub fn extension_is` 在 repowalk.rs 中存在且被两处使用
- [ ] `extension_is` 至少 4 条单元测试
- [ ] ADR-0035 baseline 表补登 + §决策 2 补一条
- [ ] `README.md` 同步本卡
- [ ] `LEDGER.md` 追加本卡

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

- 任务：TASK-055b promote `extension_is` helper + 收 repowalk.rs:172 per-line allow
- 目标：全 xtask/src 生产代码 per-line allow = 0；helper 化后 ADR-0035 baseline 表同步登记
- write scope：仅 tasks/TASK-055b-...md / xtask/src/repowalk.rs / xtask/src/refscan.rs / docs/adr/0035-...md / README.md / LEDGER.md
- 铁律相关：铁律 9（不得静默扩大范围）/ 铁律 10（契约先行 = ADR-0035 同步更新）/ 铁律 6（文档与代码同步）/ §3 启动协议
- 禁止：不动 workspace lints / 不改 refscan.rs 行为 / 不改 collect_repo_files 公共签名 / 不加新依赖 / 不加 unsafe / 不改测试断言
- 验收：cargo fmt --all --check → 0 diff；cargo clippy -p xtask --all-targets -- -D warnings → exit 0；cargo test --workspace → 283 passed（279 + 4 new）；cargo deny check → 4 项 ok；xtask 7 子命令 → 全部 PASSED（refscan 仍 151 errors baseline 一致）；DoD grep #1 `#[allow(case_sensitive_file_extension_comparisons)` xtask/src/{refscan,repowalk}.rs → 0 命中；DoD grep #2 `pub fn extension_is` xtask/src/repowalk.rs → 1 命中
- 依赖：TASK-001 / TASK-055 均 Done（HEAD = cc32c1d）；guard 无锁残留（git status 干净）
- 疑问：无；extension_is 的非 UTF-8 行为经 §6 推理 + test 覆盖已证等价

### 2. 实际改动文件

- `xtask/src/repowalk.rs` (+50/-30)：新增 `pub fn extension_is(path: &Path, expected: &str) -> bool` 紧邻 `has_rust_extension`（含 doc-comment 4 段：职责 / 边界 / 语义 / TASK-055b 标注）；重写 `collect_repo_files_recursively` 函数体（删 fn 级 `#[allow]` + 2 处 `lower.ends_with(...)` 改 `extension_is(&path, ...)` + 改 `has_rust_extension` 实现为 `extension_is(path, "rs")` 复用）；test module 加 4 条 `extension_is` 单元测试（exactly / case_insensitive / non_utf8 / missing_extension）
- `xtask/src/refscan.rs` (+2/-9)：新增 `use crate::repowalk::extension_is;` import；scan_file 函数体改：删 local `let ext_is = ...` closure + 解释注释 + `let path = std::path::Path::new(rel_path);` + 3 处调用改 `extension_is(path, "md")` 等
- `docs/adr/0035-workspace-lint-policy-no-exceptions.md`：§1 baseline 表 3 条 refscan 行「TASK-055 清掉」改为「TASK-055 已清 (commit cc32c1d)」+ 新增 1 条 repowalk.rs:172 行「TASK-055b 已清」+ 末尾补「TASK-055b 之后实际生产代码 per-line allow 数 = 0」声明；§决策 2 末尾补一条「新增：凡抽到模块级 `pub fn` 作为 per-line allow 替代方案时必须在本 ADR §baseline 表同步登记」（TASK-055b 教训）
- `README.md`：节标题改「五连发」→「六连发」+ 新增 TASK-055b 历史小节（8 行：helper 化 + collect_repo_files_recursively 重写 + 4 测试 + 副作用 = 修复 ext 大写漏匹配 bug + ADR-0035 同步登记 + 真正最终态 = 全 xtask/src 生产代码 per-line allow = 0）
- `tasks/TASK-055b-promote-extension-is-helper.md`：本卡执行记录 9 节
- **未改**：workspace lints（严守 ADR-0035 B 路）/ `Cargo.toml` / 其他 crate / `MEMORY.md` / 其他应用 `docs/memory/apps/*.md` / `collect_repo_files` 公共签名（仅 fn 内实现 + 函数级 allow 删）

### 3. 验收输出摘要

- `cargo fmt --all --check` → exit 0（无 diff）
- `cargo clippy -p xtask --all-targets -- -D warnings` → exit 0（**关键**：0 错误 0 警告；新加 4 条 test 与新增 `pub fn` 全过 pedantic + workspace deny）
- `cargo test --workspace` → **283 passed**（279 + 4 新 = `test_extension_is_matches_exactly` / `test_extension_is_is_case_insensitive` / `test_extension_is_returns_false_for_non_utf8_extension` / `test_extension_is_returns_false_for_missing_extension`）
- `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`（exit 0；warnings about 多余的 allow 列表项 = 未触动项，与 TASK-055 一致）
- `cargo run -p xtask -- refscan` → scanned=146, errors=151（**baseline 一致**：refscan 仍识别全部 151 个 ADR-0032 baseline 命中；scanned +1 = 新加的 TASK-055b 卡文件）
- `cargo run -p xtask -- hygiene` → scanned=26, errors=0, warnings=1（main.rs:624 老警告，本卡不动）
- `cargo run -p xtask -- memory-counts` → scanned=7, errors=0, warnings=0
- `cargo run -p xtask -- adr-index` → scanned=19, errors=0, warnings=0
- `cargo run -p xtask -- docscan` → scanned=115, errors=0, warnings=0
- `cargo run -p xtask -- card-check` → scanned=66, errors=0, warnings=19（stub 阶段 1 卡片警告，与 TASK-055 一致）
- **DoD 硬证据 #1**（精确 grep）：`grep '^#[allow(clippy::case_sensitive_file_extension_comparisons' xtask/src/{refscan,repowalk}.rs` → **0 命中**（注意：grep 1 的不精确版本会命中 repowalk.rs 的 doc-comment 文本「消除 fn 级 `#[allow(...)]`」字样，那是 §6 注释，不是真 allow —— 用行首锚定即可分辨）
- **DoD 硬证据 #2**：`grep 'pub fn extension_is' xtask/src/repowalk.rs` → **1 命中**（line 238）
- **DoD 硬证据 #3**（调用点）：`grep 'extension_is(' xtask/src/{refscan,repowalk}.rs` → 19 命中 = 1 定义 + 4 内部使用（repowalk:204/205/206/208 + 245 = has_rust_extension 复用）+ 12 test 断言 + 2 refscan 调用（102/103）

### 4. DoD 逐条核对

- [x] `xtask/src/repowalk.rs` 无 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`（line 172 已删）—— 0 命中
- [x] `xtask/src/refscan.rs` 无 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`（TASK-055 已做，本卡再验证未引入）—— 0 命中
- [x] `cargo clippy -p xtask --all-targets -- -D warnings` exit 0
- [x] `cargo test --workspace` 全绿（283 passed；279 + 4 new extension_is tests）
- [x] 全部 11 条 xtask 验收全绿（fmt / clippy / test / deny / refscan / hygiene / memory-counts / adr-index / docscan / card-check + 2 DoD grep）
- [x] `pub fn extension_is` 在 repowalk.rs 中存在且被两处使用（repowalk 内 4 + refscan 内 3 = 7 内部调用点）
- [x] `extension_is` 4 条单元测试（exactly / case_insensitive / non_utf8 / missing_extension）
- [x] ADR-0035 baseline 表补登（3 条 refscan 「已清」+ 1 条 repowalk 新增 + 末尾最终态声明）+ §决策 2 补一条
- [x] `README.md` 同步本卡（节标题 + 历史小节）
- [x] `LEDGER.md` 追加本卡（见下）

### 5. 偏差

none（本卡严格按卡面 In scope 清单执行；唯一「行为变化」是修复 ext 大写漏匹配 bug，记 §6 副作用而非偏差 —— caller 现状全小写故无回归）

### 6. 更合理做法

#### 6.1 行为变化的副作用（**不是偏差**，是能力增强 + bug 修复）

原 `lower.ends_with(&format!(".{ext}"))` 模式在 `ext` 大写时（caller 传 `extensions = ["MD"]`）会**漏匹配** `a.md`（因为 lower 是 `a.md`，`.ends_with(".MD")` = false）。`extension_is` 用 `Path::extension().to_str().eq_ignore_ascii_case()` 修复了此 bug。

**现状 caller 全部传小写**（card_check `["md"]` / docscan `["md"]` / refscan `["md","rs","ps1"]`），故**实际无回归**；**未来若 caller 传大写**（如 `["MD", "RS"]`），行为从「漏匹配」变为「正确匹配」= 行为增强。test_extension_is_is_case_insensitive 显式断言 4 种大小写组合（MD/md/Md）作为契约固定。

#### 6.2 `has_rust_extension` 实现也改为复用 `extension_is`

原 `has_rust_extension`：`path.extension().is_some_and(|extension| extension.to_string_lossy() == "rs")` —— 用了 `to_string_lossy()` 而非 `to_str().eq_ignore_ascii_case`，行为等价但模式不统一。

本卡改为 `extension_is(path, "rs")` = 4 行变 1 行 + 统一一个真相源。**不是抽象层增加**（helper 已存在），只是消除同义实现的漂移源头。

#### 6.3 ADR-0035 §决策 2 末尾补的「helper 化登记」条（设计权衡）

理由：TASK-055 把 refscan.rs 的 local closure 提升到 repowalk 模块级 `pub fn` 是 §9.2 建议；本卡落地时**才发现** repowalk.rs:172 自身也有同型 allow —— 即「helper 化」不是「自动消 allow」，反而是「把 allow 暴露在更显眼的位置」。ADR-0035 §决策 2 新增条目要求 helper 化时同步登记，确保未来类似提取有 reviewer check。

**不算漂移触发器 ④**：ADR-0035 §决策 2 原有 3 子项「注释 + 卡 §5 登记 + 不预设白名单」，新增第 4 子项「helper 化同步登记」**不与原有冲突**，是补充审计要求，不是放宽或推翻。

### 7. 遗留问题

- **stage-1 46 张卡仍是 Ready 占位**（stage-0 范围，本卡不触；与 ADR-0031 一致）
- **xtask card-check 判据② 仍归 PL-002**（TASK-052 已知遗留，与本卡无关）
- **TASK-002 / TASK-011 / TASK-035 仍是 stage-0 真实工作**（与本卡无关）
- **夜间自动化 GATE-0 未执行**（open.md N9；与本卡无关）

### 8. 新增长期记忆

无（本卡纯实现 B 路 = 改代码 + helper 化 + ADR 同步登记；所有新认知都已内化到 ADR-0035 §决策 2 + 本卡 §6；无需新增 `docs/memory/{facts,pitfalls,rejected}.md` 条目 —— ADR 是认知层最高载体）

### 9. 给审阅者的关注点

1. **最高优先（行为变化）**：本卡 `extension_is` 修复了 caller 传大写 ext 时漏匹配的 bug（§6.1）。**现状 caller 全部传小写**，故实际无回归；若审阅者认为应**显式记录这个行为变化**到 docs/spec/ 或 docs/memory/facts.md，建议单开 `TASK-055b-doc` 轻卡。**不算漂移**（修 bug 是修复，不是破坏契约）。
2. **ADR-0035 §决策 2 第 4 子项**（「helper 化同步登记」）：这是本卡新增的审计要求。审阅者若认为此条**会激励未来 agent 主动 helper 化以规避 per-line allow 计数**，可考虑进一步收紧（例如要求 helper 化也需 DRIFT 触发）。
3. **`has_rust_extension` 改为复用 `extension_is`**（§6.2）：4 行变 1 行 + 统一真相源，**没有功能损失**（`.to_string_lossy() == "rs"` 与 `.to_str().eq_ignore_ascii_case("rs")` 对 ASCII 扩展名等价；非 UTF-8 路径下前者走 `Cow<OsStr>` 比较后者直接返回 false —— `is_some_and` 短路，行为同形）。审阅者若想保留 `to_string_lossy` 路径以兼容更多边界，可拒绝本卡 §6.2 改动。
4. **测试覆盖度**：4 条 `extension_is` test 覆盖了正例 / 大小写 / 非 UTF-8 / 无扩展名 4 个语义维度。审阅者可建议：是否加 1 条「空 `expected` 参数」的边界用例？目前 `extension_is(path, "")` 会怎样 —— `Path::extension()` 返回 `Some("")` 时 `eq_ignore_ascii_case("")` = true → `is_some_and` 返回 true。**当前未覆盖** —— 若加 caller 传空字符串的可能性（如未来想用 `is_some_and(|ext| ext.starts_with(...))` 模式），建议补 1 条 test。
5. **真正的最终态**：本卡完成后，**全 xtask/src 生产代码 per-line allow = 0**（refscan:0 + repowalk:0 + docscan:0 + card_check:0 + exemptions:0）。仅剩 = test wrapper 4 处合法 + card_check 9 + exemptions 1 共 10 处 `#[allow(dead_code)]`（占位常量，ADR-0031 D6 要求保留）。这是 xtask 护栏升级阶段的**最终 baseline**，对应 ADR-0035 §1 决策 1「首选改代码」的完全胜利。
