# TASK-298　replay 提供者公开契约覆盖率补强（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`assistant-replay` 与 `ReplaySession` 已就位）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/replay/tests/provider_contract.rs`、`tasks/TASK-298-replay-provider-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-15-round-1.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/replay/src/provider.rs`、`crates/replay/README.md`（不变量 4 / 6）、`docs/adr/0091-exact-name-and-role-selector.md`、`fixtures/recordings/core/notepad-like-basic.json`

**目标**

仅用公开 API 契约测试补强 `crates/replay/src/provider.rs` 的行覆盖率：
覆盖 ADR-0091 的 `SelectorKind::ExactNameAndRole` 回放匹配（命中 / 空名拒绝 / 取值不匹配 /
0 命中 / 多命中）、`wait_for` 的 `CapabilityMissing`、窗口句柄不匹配的 `TargetNotFound`、
`RoleAndParent` 自引用触发的深度上限、kind/value 不匹配的回退分支与非法
`TargetDescriptor` 的拒绝路径。使 `provider.rs` 行覆盖率从 85.19% 提升到 ≥95%；
生产源码零改动，不新增依赖。

**背景**

运行日 2026-10-15 的 P2 现场扫描显示 `crates/replay/src/provider.rs` 为 421/486 行
（85.19%），其中 `exact_name_and_role_matches()`（provider.rs 402~423）与
`resolve_candidate()` 的 `ExactNameAndRole` 分支（provider.rs 468~470）**零覆盖** ——
ADR-0091 已把 `ExactNameAndRole` 落进 `crates/platform/windows` 的 UIA 搜索与适配包，
而回放侧要求「与 Windows 搜索保持同语义」（provider.rs 模块注释），目前没有任何测试
证明这条同语义契约成立。`cargo llvm-cov -p assistant-replay --tests --show-missing-lines`
证明其余未覆盖行也是公开可达的失败路径（`wait_for` / 窗口句柄不匹配 / 深度上限 /
kind-value 不匹配），适合用 `tests/**` 公开契约测试收口。

**步骤**

1. 在本卡 write scope 内新增 `crates/replay/tests/provider_contract.rs`，只调用公开 API。
2. 覆盖 ADR-0091 `ExactNameAndRole` 回放匹配的 5 条语义（命中 / 空名 / 取值不匹配 / 0 命中 / 多命中）。
3. 覆盖 `wait_for`、窗口句柄不匹配、`RoleAndParent` 深度上限、kind/value 回退、非法描述符。
4. 用真实 `cargo llvm-cov` 对比改动前后行覆盖率，记录完整门禁输出。
5. 同步卡片状态与 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖、测试文件 ≤600 行
- [ ] `crates/replay/src/provider.rs` 行覆盖率从 85.19% 提升到 ≥95%；crate 行覆盖率保持 ≥85%
- [ ] `cargo llvm-cov -p assistant-replay --tests --fail-under-lines 85` 与 `cargo test -p assistant-replay` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-replay --tests --fail-under-lines 85
cargo test -p assistant-replay
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤ -->

## 执行记录

### 1. 约束回执

【任务】TASK-298 replay 提供者公开契约覆盖率补强　【目标】只加公开 API 契约测试，把
`crates/replay/src/provider.rs` 行覆盖率从 85.19% 提升到 ≥95%　【write scope】仅本卡列出的
测试与状态 / 轮次文件　【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】无；【疑问】无

### 2. 实际改动文件

- `crates/replay/tests/provider_contract.rs`（新增公开 API 契约测试）
- `tasks/TASK-298-replay-provider-contract-coverage.md`
- `LEDGER.md`（Review 行）
- `plans/stage-1-pilots.md`（新增 TASK-298 条目行）
- `docs/automations/2026-10-15-round-1.md`
- `PLAN.md` / `README.md`（Done 与 merge hash 由后续 docs-only 回填 PR 同批更新）

### 3. 验收输出摘要

改动前后均用 `cargo llvm-cov` 实测；改动前刻意把新增测试文件临时移出 `crates/replay/tests/`
再测，避免"自己给自己取证"：

```text
# 改动前（provider_contract.rs 临时移出）
provider.rs   456  61  86.62%   56   7  87.50%   486  72  85.19%   0 0 -
TOTAL        1023 128  87.49%  116  16  86.21%   865 110  87.28%   0 0 -

# 改动后
provider.rs   456  25  94.52%   56   4  92.86%   486  23  95.27%   0 0 -
TOTAL        1023  92  91.01%  116  13  88.79%   865  61  92.95%   0 0 -

$ cargo llvm-cov -p assistant-replay --tests --summary-only --fail-under-lines 85
EXIT 0

$ cargo test -p assistant-replay
provider_contract.rs: 11 passed; 0 failed; 0 ignored
replay.rs:            27 passed; 0 failed; 0 ignored
EXIT 0

$ cargo fmt --all --check               -> EXIT 0
$ cargo clippy --all-targets -- -D warnings -> EXIT 0（仅既有 clippy::assert_is_empty unknown-lint 警告）
$ cargo test --workspace                -> EXIT 0；xtask 汇总 469 passed / 0 failed
$ cargo test -p assistant-core arch::   -> EXIT 0
$ cargo run -p xtask -- replay --suite core -> 2 step(s) / 5 change(s) / 0 mismatch / PASSED
$ cargo deny check                      -> advisories / bans / licenses / sources 全 ok；EXIT 0

xtask 硬门禁（全部 verdict=PASSED）：
hygiene 421 files 0E/114W；check-comments 421 files 0E/68W；memory-counts 0E/0W；
adr-index 83 files 0E/0W；refscan 0E/0W；docscan 459 files 0E/264W；
card-check 192 files 0E/34W；check-ledger 0E/0W；verify-schemas 0E；
codegen --check 0 drift / 0E；check-migrations 0E/0W
```

未覆盖残余（公开 API 不可达 / 需伪造记录，不为此改生产源码）：

- `provider.rs:268-271`：`list_windows` 的 `title_regex` 分支 —— `WindowFilter` 为
  `#[non_exhaustive]` 且字段私有，公开 API 无法构造带 `title_regex` 的过滤器。
- `provider.rs:547-569`：`i64 -> i32` 边界转换的 `map_err` 闭包体。
- `provider.rs:601-605`：`node_count` 的 `u32` 溢出兜底（需 > `u32::MAX` 个节点）。

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只调用公开 API、无新依赖、测试文件 297 行（≤600）：**满足**。
- `provider.rs` 85.19% → 95.27%（≥95%）；crate 87.28% → 92.95%（≥85%）：**满足**。
- `cargo llvm-cov ... --fail-under-lines 85` 与 `cargo test -p assistant-replay` 全绿：**满足**。
- `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check`：**满足**。
- `LEDGER.md` / `plans/stage-1-pilots.md` 已写入 Review 留痕；`PLAN.md` / `README.md` 与
  Done 状态、merge hash 由合并后的 docs-only 回填 PR 同批更新（ADR-0039 / ADR-0041）。

### 5. 偏差

- `cargo fmt --all --check` 首跑失败（新增测试 1 行超行宽）；跑 `cargo fmt --all` 后复跑
  EXIT 0，未放宽 `rustfmt.toml`。
- 选靶阶段先评估 `crates/undo/src/rollback.rs`（84.67%），核实其残余未覆盖行是**不可达
  防御分支**后主动改靶到 `crates/replay/src/provider.rs`；依据写入 round 文件「选卡」节。
- 本轮所有 `exec_command` 均带 `shell: "powershell"`，未发生默认 bash 误用。

### 6. 更合理做法

- 先跑 `--show-missing-lines` 再按行判断「公开可达 / 不可达」，比先按百分比排序选靶更省时；
  本轮 `rollback.rs` 的放弃正是这条路径的收益，下轮应默认先做这一步。

### 7. 遗留问题

- 无产品遗留。主 PR 见 round 文件与 `LEDGER.md`；merge hash 由 docs-only 回填 PR 落地。
- 非产品遗留：`crates/replay/src/model.rs`（89.47%）与 `provider.rs` 上述 3 处残余行
  属公开不可达或需伪造记录的分支，建议后续卡不要为数字改生产源码。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；结论均落在本卡与
  `docs/automations/2026-10-15-round-1.md`。

### 9. 给审阅者的关注点

1. `ExactNameAndRole` 的断言是否真的体现「name **与** role 同时相等」——其中
   同名不同 role → `TargetNotFound` 那条是防止回放退化成 `ExactName` 的关键。
2. 11 个用例是否都落在公开契约上（每条都有明确的 `ErrorCode` 或句柄断言），
   以及生产源码是否确实零改动。
