# TASK-312　xtask 模块注释与 README 派生值指针化（PL-114 闭环）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（PL-114 由 TASK-311 提出并已合并）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`xtask/src/main.rs`（仅模块注释）、`xtask/README.md`（仅现行说明）、`docs/PARKING_LOT.md`（只追加 PL-114 闭环）、`tasks/TASK-312-xtask-module-comment-derived-value-pointer.md`、`LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处、`plans/stage-1-pilots.md`、`docs/automations/2026-10-17-round-1.md`
- **关联**：`docs/adr/0031-one-card-one-file-and-write-scope.md`、`docs/adr/0033-file-size-gates-two-tier.md`、`docs/adr/0072-derived-values-point-to-ssot.md`、`xtask/README.md`、`xtask/src/main.rs`、`docs/PARKING_LOT.md` PL-033 / PL-114

**目标**

闭环 PL-114：删除 `xtask/src/main.rs` 与 `xtask/README.md` 中会持续漂移的“四个 / 三个模块”清单，
把测试外置的现行清单指向 `xtask/src/**` 里的 `#[path = "*_tests.rs"]` 声明这一事实源；
同时停止在注释里复制行数阈值，改指 ADR-0033。

**背景**

TASK-311 新增 `doccheck_tests.rs` 后，实测 `xtask/src` 已有 13 处 `#[path]` 测试外置声明，
而 `main.rs` 仍写“四个模块”、`xtask/README.md` 仍写“三个模块”。两处都把派生值手抄成正文，
违反 ADR-0072，并会误导下一张 xtask 卡的测试放置方式。PL-033 已由 ADR-0033 裁决为
“写作规范与 CI 门禁双层阈值”，但旧注释仍把阈值与测试外置混写。

**步骤**

1. 把 `main.rs` 的“文件长度与测试外置”段改为：阈值以 ADR-0033 为准，外置清单以 `#[path]` 声明为准，不手抄数量 / 名单。
2. 把 `xtask/README.md` 的模块表行与“测试外置”段同样改成指针式说明。
3. 用 `rg` 证明固定名单消失、`#[path]` 声明清单可独立取得。
4. 跑完整门禁，追加 PL-114 闭环、LEDGER / PLAN / README / plans 与轮次留痕。

**DoD**

- [ ] `main.rs` / `xtask/README.md` 不再手抄外置模块数量或固定名单，均指向机器可查事实源
- [ ] 注释与文档明确“写作规范 / CI 门禁阈值以 ADR-0033 为准”，不再声称 PL-033 未决
- [ ] 生产逻辑零改动；不新增依赖、不改公共接口、schema、lint、CI
- [ ] `rg -n '^#\[path = ".*_tests\.rs"\]' xtask/src` 输出真实清单，固定名单扫描归零
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `PARKING_LOT.md` 追加 PL-114 闭环节点；`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
rg -n -e '四个模块' -e '三个模块' -e 'adr_index` / `guard` / `guard_runner' xtask/src/main.rs xtask/README.md
rg -n '^#\[path = ".*_tests\.rs"\]' xtask/src
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

【任务】TASK-312 xtask 模块注释派生值指针化　　【目标】闭环 PL-114，让外置模块清单与行数阈值各指向事实源　
【write scope】仅卡面列出的 `xtask/src/main.rs` / `xtask/README.md` / PL / 台账 / 进度 / 轮次文件　
【铁律】9 不静默扩大范围；10 契约先行；ADR-0072 派生值指针化；ADR-0033 双层阈值口径　
【禁止】改行为、公共接口、依赖、schema、lint、CI；不碰真实 GUI　
【验收】固定名单 `rg` 归零；`#[path]` 声明清单可独立取得；完整门禁全绿　
【依赖】TASK-311 Done（PR #442 / merge `601c76c`；PL-114 为其遗留）　　【疑问】无

### 2. 实际改动文件

- `xtask/src/main.rs`：模块头「文件长度与测试外置」段改为阈值指向 ADR-0033、外置清单指向 `#[path]` 声明。
- `xtask/README.md`：模块表与「测试外置」段移除固定名单 / 阈值副本，改为同一事实源。
- `docs/PARKING_LOT.md`：只追加 PL-114 闭环节点。
- `LEDGER.md`：追加本卡 Review 行。
- `docs/automations/2026-10-17-round-1.md`：新建本轮轮次产物。
- 本卡：状态与执行记录。

### 3. 验收输出摘要

```text
固定名单扫描：
rg -n -e '四个模块' -e '三个模块' -e 'adr_index` / `guard` / `guard_runner' xtask/src/main.rs xtask/README.md
EXIT=1（0 命中）

外置声明清单：
rg -n '^#\[path = ".*_tests\.rs"\]' xtask/src
13 处：adr_index / adr_registry / doccheck / comments / guard_runner / guard / duplicate_code /
hygiene / replay / repowalk / top_level_dirs / write_channel / rustscan

cargo fmt --all --check -> EXIT 0
cargo clippy --all-targets -- -D warnings -> EXIT 0（仅既有 unknown-lint 提示）
cargo test --workspace -> EXIT 0
cargo test -p assistant-core arch:: -> EXIT 0（arch_dependencies 5 passed / arch_layering 5 passed）
cargo deny check -> EXIT 0（advisories / bans / licenses / sources ok）

hygiene         0E/117W scanned=428
check-comments  0E/68W  scanned=428
memory-counts   0E/1W   scanned=9
adr-index       0E/0W   scanned=83
refscan         0E/0W   scanned_files=933
docscan         0E/264W scanned_files=482
card-check      0E/34W  scanned_files=206
check-ledger    0E/0W
verify-schemas  0E      scanned_schemas=5
codegen --check 0 drift / 0 error
check-migrations 0E/0W  scanned_migration_files=5
```

### 4. DoD 逐条核对

- `main.rs` / `xtask/README.md` 不再手抄外置模块数量或固定名单，均指向机器可查事实源：**满足**。
- 注释与文档明确阈值以 ADR-0033 为准，不再声称 PL-033 未决：**满足**。
- 生产逻辑零改动，无新依赖 / 公共接口 / schema / lint / CI 变更：**满足**。
- `#[path]` 声明清单可独立取得，固定名单扫描归零：**满足**（13 处 / 0 命中）。
- `fmt` / `clippy` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿：**满足**。
- `PARKING_LOT.md` 追加 PL-114 闭环节点：**满足**；`PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与 merge hash 将在合并后的回填 PR 同批同步。

### 5. 偏差

- 无产品行为 / 契约 / 范围偏差。
- 自动化执行中共发生 3 次误用默认 bash 的调用；其中 guard acquire 一次未获得任何锁、未改文件，随后均在 PowerShell 重试成功。记录为操作偏差，不影响产物。

### 6. 更合理做法

- 首次修复就删除固定数量与固定名单，而不是把“四个”改成“13 个”；后者会在下一张 xtask 卡再次漂移。
- `xtask/README.md` 与 `main.rs` 同属该事实源的现行说明，应同卡同步，避免只修一个入口。

### 7. 遗留问题

- 无本卡遗留。
- 已知既有缺口：`docs/memory/apps/paint.md` 仍超 ADR-0021 归档阈值，归 TASK-309 / 后续治理；`xtask/README.md` 其他历史限制段落未在本卡审查。

### 8. 新增长期记忆

无。

### 9. 给审阅者的关注点

1. 是否认可“外置清单以 `#[path]` 声明为唯一事实源、注释不复制数量 / 名单”符合 ADR-0072。
2. `xtask/README.md` 的同根因修正是本卡范围内的共同入口，不是 drive-by；请按 diff 核对。
