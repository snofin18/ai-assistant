# TASK-311　xtask 文档一致性门禁 IO 层契约覆盖率补强（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`doccheck` 已在 main）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`xtask/src/doccheck.rs`（**只加** `#[cfg(test)] #[path] mod tests;` 声明）、`xtask/src/doccheck_tests.rs`（新建，私有单测；PL-033 的外置约定）、`docs/PARKING_LOT.md`（只追加）、`tasks/TASK-311-xtask-doccheck-io-contract-coverage.md`、`LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处、`plans/stage-1-pilots.md`、`docs/automations/2026-10-16-round-2.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`docs/spec/testing.md` §4.1 / §4.3、`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`、`xtask/Cargo.toml` 不变量 2、`xtask/src/doccheck.rs`

**目标**

只用白盒契约测试补强 `xtask/src/doccheck.rs` —— `memory-counts` 与 `adr-index` 两个
**HARD 门禁 #12b** 的 IO 边界。当前该文件行覆盖率 10.69%（159 行中 142 行未覆盖），
是 workspace（80.85%）里最大的单点缺口；本卡把它提到 ≥85%，规格见
`docs/spec/testing.md` §4.3「输出可注入」。生产逻辑零改动，不新增依赖，不碰网络 / GUI / 凭据。

**背景**

`doccheck.rs` 是「规则判定一律纯函数、IO 集中在边界」（`xtask/Cargo.toml` 不变量 2）里的
**边界**：`memory_counts.rs`（99.68%）与 `adr_index.rs`（100%）的规则已被逐条白盒测试，
而把文件读进来、列出目录、渲染报告、决定退出码的这一层没有测试 —— 一旦退化，
两个门禁会**静默变绿**（不变量 1 的核心风险：「读不到必需文件」被当成空内容）。

**步骤**

1. 单测外置到 `doccheck_tests.rs`（PL-033 的 `#[path]` 约定），`doccheck.rs` 只加声明；
   测试模块按 §4.4 加三项 lint 允许。
2. 用临时仓库根目录（`std::env::temp_dir()` + 进程号，沿用 `main.rs` / `write_channel_tests.rs`
   的既有做法）构造最小 `MEMORY.md` / `docs/memory/` / `docs/adr/` 输入。
3. 覆盖：缺必需文件 → `Err`；0 个记忆文件 → `memory/no-files-scanned` 告警且退出码仍为 0；
   `README.md` 与 `archive/` 排除；Markdown 扩展名大小写无关；发现项排序确定；
   写入失败 → `Err`（输出可注入，不捕获进程 stdout）。
4. 用真实 `cargo llvm-cov` 记录改动前后覆盖率，跑完整门禁并同步状态。

**DoD**

- [ ] 生产逻辑零改动；测试只经 `pub(crate)`/私有入口与 `&mut dyn Write`，无新依赖，文件 ≤600 行
- [ ] `xtask/src/doccheck.rs` 行覆盖率 ≥85%，且相对 10.69% 有真实提升（前后证据进记录区）
- [ ] 覆盖 `run_memory_counts` / `run_adr_index` / `derive_governance_counts` 的成功与失败返回
- [ ] `cargo test -p xtask`、`cargo llvm-cov -p xtask --tests` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / arch / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p xtask --tests
cargo test -p xtask
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-311 xtask 文档一致性门禁（`memory-counts` / `adr-index`）IO 层契约覆盖率补强　
【目标】只加私有白盒测试，把 `xtask/src/doccheck.rs` 行覆盖率 10.69% 提上门槛，生产源码零改动　
【write scope】仅本卡列出的测试文件 + 台账 / 进度文件　
【铁律】1 无静默失败（缺必需文件必须 Err，不得当空内容 PASSED）；9 不得静默扩大范围；
10 契约先行；§12 资源生命周期（临时目录有界、用完显式删除）　
【禁止】改产品逻辑 / 公共接口 / 依赖 / schema / lint / CI；真实 GUI、网络、凭据　
【依赖】无（`doccheck` 已在 main；LEDGER 已核：TASK-310 Done / merge `500d592`）　
【疑问】无

### 2. 实际改动文件

- `xtask/src/doccheck_tests.rs`（**新建**，386 行，16 个私有白盒用例；按 PL-033 约定 `#[path]` 外置）
- `xtask/src/doccheck.rs`（尾部 +9 行：`#[cfg(test)] #[path = "doccheck_tests.rs"] mod tests;` 与 why 注释）
- `docs/PARKING_LOT.md`（只追加 PL-114 一行）
- `docs/automations/2026-10-16-round-2.md`（新建）
- `LEDGER.md`（追加本卡 Review 一行）
- 本卡
- 仓库外日志 `C:\Users\fexfe\.codex\automations-logs\2026-10-16\ai-assistant-task-round-20261017-0320.jsonl`

### 3. 验收输出摘要

```text
改动前（HEAD 首跑）：xtask/src/doccheck.rs 159 行中 142 行未覆盖 = 10.69%
改动后：            xtask/src/doccheck.rs 159 行中  17 行未覆盖 = 89.31%

cargo fmt --all --check -> EXIT 0
cargo clippy --all-targets -- -D warnings -> EXIT 0（仅既有 unknown-lint 提示）
cargo test -p xtask -> 485 passed / 0 failed（本卡新增 16 个用例）
cargo llvm-cov -p xtask --tests -> EXIT 0
cargo test --workspace -> 153 个 test result 全 ok，累加 1630 passed / 0 failed
cargo test -p assistant-core arch:: -> 5 + 5 passed / 0 failed
cargo deny check -> advisories / bans / licenses / sources ok
hygiene 0E/117W ｜ check-comments 0E/68W ｜ memory-counts 0E/1W ｜ adr-index 0E/0W
refscan 0E/0W ｜ docscan 0E/264W ｜ card-check 0E/34W ｜ check-ledger 0E/0W
verify-schemas 0E ｜ codegen --check 0 drift ｜ check-migrations 0E/0W
（以上 11 条 xtask 子命令的 machine-summary 均为 verdict=PASSED；详见 round 2 §4）
```

### 4. DoD 逐条核对

- 生产逻辑零改动（`doccheck.rs` 只加测试声明与注释）、无新依赖、两文件均 ≤600 行：**满足**
  （`doccheck.rs` 265 行、`doccheck_tests.rs` 386 行）。
- `doccheck.rs` 行覆盖率 10.69% → **89.31%**（≥85%）：**满足**。
- 三个入口的成功与失败返回全覆盖：`run_memory_counts`（一致 / 缺 `MEMORY.md` / 计数不符 / 0 文件 /
  排除规则 / 大写扩展名 / 写失败 / 确定性）、`run_adr_index`（缺登记表 / 缺 `decisions.md` / 正常渲染）、
  `derive_governance_counts`（缺事实源 / 本仓库正向）：**满足**。
- `cargo test -p xtask` 与 `cargo llvm-cov -p xtask --tests` 全绿：**满足**。
- `fmt` / `clippy -D warnings` / `test --workspace` / arch / 11 条 xtask 硬门禁 / `cargo deny check`：**满足**。
- 进度同步：`LEDGER.md` Review 行与本卡、round 文件同批落主 PR；`PLAN.md` / `README.md` /
  `plans/stage-1-pilots.md` 与卡面 `- 状态：` 按既有流程在 merge hash 回填 PR 中同批改：**满足**。

### 5. 偏差

- 首版把 16 个用例内联在 `doccheck.rs` 尾部（638 行），`hygiene` 随之多出 1 条
  `hygiene/file-too-long` 告警。按本 crate 既定做法（PL-033）改为 `#[path]` 外置到
  `doccheck_tests.rs`，告警数回到改动前的 117。**未放宽任何阈值、未加 `#[allow]`**。
- `refscan` 首跑 1 Error（样例文本里「`ADR-` 加四位待建编号」的字面量构成裸引用待建号，
  ADR-0032）→ 样例改为不写编号字面量。
- `test_run_adr_index_missing_registry_is_error` 首跑因断言了 `/` 分隔的路径而失败
  （Windows 上 `Path::display()` 用 `\`）→ 改为只断言文件名。
- 首版 write scope 只写了 `doccheck.rs`；因上述外置决定，卡面 write scope 在同一次提交内补上
  `doccheck_tests.rs` 与 `docs/PARKING_LOT.md`（卡在本轮由本 run 新建，非改写他人卡）。
- 未发生范围 / ADR / 契约偏差；未改测试断言来遮掩失败。

### 6. 更合理做法

- 先把「被 IO 边界包住的判定是否已经 100% 覆盖」与「边界本身的失败分支」分开看，
  再决定测什么：本卡因此没有去补 `memory_counts.rs` / `adr_index.rs`（它们已 99.68% / 100%），
  而是集中在没人测的 `doccheck.rs`。
- 一次性把测试外置（`#[path]`），而不是先内联、被 `hygiene` 告警拦下再搬 ——
  本 crate 的文件长度与测试体积早就注定内联会超线。

### 7. 遗留问题

- **PL-114**（新提，见 `docs/PARKING_LOT.md`）：`xtask/src/main.rs` 的「文件长度与测试外置」段
  仍写「四个模块外置」，实测改动前已 12 个、本卡后 13 个；同段引用的行数硬上限口径悬在 PL-033。
  需 Orchestrator / 治理卡改这一句，本卡不越界动 `main.rs`。
- `doccheck.rs` 仍剩 17 行未覆盖，集中在 `list_directory` 的通用错误分支与几处 `?` 错误映射
  （要构造「目录存在但读不了」的权限场景），不在无人值守可稳定复现的范围。
- `docs/memory/apps/paint.md` 709 行仍超 ADR-0021 阈值（`memory-counts` 唯一 warning），
  归档拆分在 TASK-309 的 write scope 内。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED 条目：本卡是纯测试补强，没有产生新的项目认知或否决项。
  （`docs/PARKING_LOT.md` 的 PL-114 属于待处置项，不是长期记忆。）

### 9. 给审阅者的关注点

1. `doccheck_tests.rs` 的 16 条用例是否真的经**注入的边界**（临时仓库根 + `&mut dyn Write`）覆盖了
   「缺必需文件 ⇒ Err，绝不 PASSED」这条不变量；尤其 `memory/no-files-scanned` 那一条同时命中
   `scale-table-unparsable`，是**刻意不美化**的写法。
2. 测试外置（`doccheck_tests.rs`）是否算超出 write scope —— 卡面已在同一次提交里把该文件纳入
   scope，且这是本 crate 的既定约定（PL-033）；如不认可，应先裁决约定而不是让本卡改判据。
3. round 2 §4 对「workspace 469 passed」的口径更正：请确认后续轮次按新口径书写，
   避免继续沿用无法复现的数字。
