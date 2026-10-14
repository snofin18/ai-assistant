# TASK-289　verify fingerprint 公开契约覆盖率补强（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`assistant-verify` 与 ADR-0074 已就位）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/verify/tests/fingerprint_contract.rs`、`tasks/TASK-289-verify-fingerprint-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-14-round-4.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/verify/src/fingerprint.rs`、`crates/verify/README.md`、ADR-0074

**目标**

仅用 `crates/verify/tests/fingerprint_contract.rs` 的公开 API 契约测试补强 fingerprint 模块覆盖率：
覆盖 `FingerprintField` 的六个稳定规范化名、`ALL` 的规范化顺序，以及
`FingerprintIgnore::excluding(FingerprintField::ALL)` 对每个字段的 ignore 编码；生产源码零改动，
不新增依赖。

**背景**

运行日 2026-10-14 的默认 P2 目标是 `crates/core/src/store.rs`。实测
`cargo llvm-cov -p assistant-core --tests --show-missing-lines` 显示 `store.rs` 61 行中
12 行未覆盖（80.33%），未覆盖行 57 / 61 / 75 / 79 / 110 / 114 全部位于
`Mutex` 中毒错误的三个私有闭包中；公开 API 无法合法制造该私有状态，强行覆盖需要改生产源码。
因此本轮按现场证据改选同 crate 的 `crates/verify/src/fingerprint.rs`，其公开可达缺口为
`FingerprintField::canonical_name` 的六条 match 分支。

**步骤**

1. 新建 `crates/verify/tests/fingerprint_contract.rs`，只调用 `assistant_verify` 公开导出。
2. 表驱动覆盖 `FingerprintField::canonical_name` 六种值与 serde 名称一致性。
3. 覆盖 `FingerprintField::ALL` 顺序和 `FingerprintIgnore::excluding(ALL)` 的 canonical form。
4. 用真实 `cargo llvm-cov` 对比改动前后行覆盖率，记录全门禁输出。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖、测试文件 ≤600 行
- [ ] `crates/verify/src/fingerprint.rs` 行覆盖率从 83.55% 提升到 ≥85%；crate 行覆盖率保持 ≥85%
- [ ] `cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85` 与 `cargo test -p assistant-verify` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85
cargo test -p assistant-verify
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-289 verify fingerprint 公开契约覆盖率补强　【目标】仅追加公开 API 契约测试，把
`fingerprint.rs` 覆盖率提升到 ≥85%　【write scope】仅本卡列出的测试与状态 / 轮次文件
【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】无（crate 与 ADR-0074 已就位）；【疑问】无

### 2. 实际改动文件

- `crates/verify/tests/fingerprint_contract.rs`（新增 3 个公开 API 契约测试，78 行）
- `tasks/TASK-289-verify-fingerprint-contract-coverage.md`

### 3. 验收输出摘要

- 改动前：`crates/verify/src/fingerprint.rs` 127/152 行（83.55%），crate 1494/1645 行（90.82%）。
- 改动后：`crates/verify/src/fingerprint.rs` 147/152 行（**96.71%**），crate 1514/1645 行
  （**92.04%**）；剩余 5 行中的 374-375 / 379-380 是 SHA-256 编码器内部不可达防御，
  公开 API 无法合法制造。
- `cargo test -p assistant-verify --test fingerprint_contract`：3 passed。
- `cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85`：EXIT 0。
- `cargo test -p assistant-verify`：全部通过（含 3 个新增用例与 2 个 doctest）。
- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 `clippy::assert_is_empty`
  unknown-lint 警告）。
- `cargo test --workspace`：EXIT 0；xtask 汇总 469 passed。
- `cargo llvm-cov --workspace --summary-only --fail-under-lines 75`：EXIT 0；workspace
  42718/53118 行（**80.43%**）。
- `cargo run -p xtask -- replay --suite core`：2 steps / 5 changes / 0 mismatch，PASSED。
- `hygiene` 0E/118W、`check-comments` 0E/67W、`memory-counts` 0E/0W、`adr-index`
  0E/0W、`refscan` 0E/0W、`docscan` 0E/264W、`card-check` 0E/34W、`check-ledger`
  0E/0W、`verify-schemas` 0E、`codegen --check` 0 drift / 0E、`check-migrations`
  0E/0W，全部 EXIT 0。
- `cargo deny check`：advisories / bans / licenses / sources 全 ok，EXIT 0。
- 非宿主纯 Rust 交叉 clippy：`x86_64-unknown-linux-gnu` 与 `aarch64-apple-darwin`
  的 `assistant-verify --all-targets -D warnings` 均为 EXIT 0。
- UI：`pnpm lint` / `pnpm typecheck` / `pnpm test` 均为 EXIT 0（node 78 passed +
  vitest 3 passed）。

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只用公开 API、无新依赖、测试文件 78 行：满足。
- `fingerprint.rs` 83.55% → 96.71%（≥85%）；crate 92.04%（≥85%）：满足。
- crate 覆盖率命令、crate 测试、全 workspace 门禁、deny、双平台 clippy：满足。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`：本批同批改
  Done；merge `8d4534a`（PR #384）。

### 5. 偏差

- 默认建议目标 `crates/core/src/store.rs` 实测 80.33%，但 12 个未覆盖行全部是私有
  `Mutex` 中毒错误闭包；公开 API 测试不可达。按“不要追不可达行”改为同一 P2 白名单内
  的 `crates/verify/src/fingerprint.rs`，未改生产源码。
- 本轮有 13 次 `exec_command` 误用默认 bash：多为 `cargo` / `Get-Content` / `gh` /
  `rg` not found，另有行号脚本 shell 语法错误、CLI 输出解析语法错误，以及一次 `git
  switch` 虽意外成功但违反硬要求；均已立即以 `shell: "powershell"` 重跑，没有产生仓库
  垃圾文件或未跑即报通过的命令。
- 另有 1 次 PowerShell 变量插值 ParserError，立即修正；无仓库副作用。

### 6. 更合理做法

- P2 选卡时先跑 `--show-missing-lines` 判断剩余行是否公开可达，再写测试；这样可避免
  把“不可达防御分支”误当成覆盖率任务。

### 7. 遗留问题

- `crates/core/src/store.rs` 的 12 个 Mutex 中毒错误分支仍不可由公开 API 触达；如未来
  要覆盖，应另立卡评估生产重构或 crate 内部测试边界，而不是放宽公开契约。
- `fingerprint.rs` 剩余 374-375 / 379-380 为 SHA-256 编码器不可达防御分支。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；本轮结论均记录在本卡与 round 文件。

### 9. 给审阅者的关注点

1. 新增 3 个用例是否都调用真实公开 API，并覆盖 `canonical_name` / `ALL` / ignore 元数据。
2. `fingerprint.rs` 的 96.71% 是否来自可解释的公开契约断言，而不是与私有实现耦合的白盒
   测试。
