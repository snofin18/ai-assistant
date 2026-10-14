# TASK-288　verify 视觉公开契约覆盖率补强（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`assistant-verify` 与 ADR-0074 已就位）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/verify/tests/visual_contract.rs`、`tasks/TASK-288-verify-visual-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-14-round-3.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/verify/src/visual/assert.rs`、`crates/verify/src/visual/image.rs`、`crates/verify/src/visual/hash.rs`、`crates/verify/README.md`、ADR-0074

**目标**

仅用 `crates/verify/tests/visual_contract.rs` 的公开 API 契约测试补强视觉模块覆盖率：覆盖
`VisualTolerance::op` 的三类算子、`parse_visual_assert` 的 kind / 字节范围失败分支、
changed-ratio 满意路径、Hamming 反证路径、`VisualVerdict::to_assertion_outcome` 的
`Falsified` 投影，以及 `GrayImage::pixel_at` 的两种行地址溢出错误；生产源码零改动，不新增依赖。

**背景**

运行日 2026-10-14 的 P2 建议目标为 `crates/verify` 视觉模块。实测基线：
`visual/image.rs` 103/132 行（78.03%）、`visual/hash.rs` 87/109 行（79.82%）、
`visual/assert.rs` 329/404 行（81.44%），crate 总计 1456/1645 行（88.51%）。
`hash.rs` 与 `image.rs` 的剩余缺口主要是私有函数内部不变量、不可达溢出 / 越界兜底；
不改生产逻辑时只能在公开可达路径上补测试，并把不可达剩余显式记录。

**步骤**

1. 新建 `crates/verify/tests/visual_contract.rs`，只调用 `assistant_verify` 公开导出。
2. 覆盖解析器三类算子与 fail-closed 边界，覆盖 changed-ratio / Hamming / 投影三种结论。
3. 覆盖公开可达的 `pixel_at` 溢出错误，不触碰私有不变量。
4. 用真实 `cargo llvm-cov` 对比改动前后行覆盖率，记录不可达剩余与全门禁输出。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖、测试文件 ≤600 行
- [ ] `visual/assert.rs` 行覆盖率从 81.44% 提升到 ≥85%；crate 行覆盖率保持 ≥85%
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

【任务】TASK-288 verify 视觉公开契约覆盖率补强　【目标】仅追加公开 API 契约测试，把
`visual/assert.rs` 覆盖率提升到 ≥85%　【write scope】仅本卡列出的测试与状态 / 轮次文件
【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】无（crate 与 ADR-0074 已就位）；【疑问】无

### 2. 实际改动文件

- `crates/verify/tests/visual_contract.rs`（新增 9 个公开 API 契约测试，263 行）
- `tasks/TASK-288-verify-visual-contract-coverage.md`

### 3. 验收输出摘要

- 改动前：`visual/assert.rs` 329/404 行（81.44%）、`visual/image.rs` 103/132 行（78.03%）、
  `visual/hash.rs` 87/109 行（79.82%），crate 总计 1456/1645 行（88.51%）。
- 改动后：`visual/assert.rs` 361/404 行（**89.36%**）、`visual/image.rs` 109/132 行
  （**82.58%**）、`visual/hash.rs` 87/109 行（79.82%，无可由公开 API 触达的新增行），
  crate 总计 1494/1645 行（**90.82%**）。
- `cargo test -p assistant-verify --test visual_contract`：9 passed。
- `cargo llvm-cov -p assistant-verify --tests --fail-under-lines 85`：EXIT 0。
- `cargo test -p assistant-verify`：全部通过（含 9 个新增用例与 2 个 doctest）。
- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 `clippy::assert_is_empty`
  unknown-lint 警告）。
- `cargo test --workspace`：EXIT 0；xtask 汇总 469 passed。
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

- 生产逻辑零改动、测试只用公开 API、无新依赖、测试文件 263 行：满足。
- `visual/assert.rs` 81.44% → 89.36%（≥85%）；crate 90.82%（≥85%）：满足。
- crate 覆盖率命令、crate 测试、全 workspace 门禁、deny、双平台 clippy：满足。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`：待本批状态同步；
  PR #382；pull_request CI = 11/11 SUCCESS；merge `d45c1eb`。

### 5. 偏差

- 本轮有 7 次 `exec_command` 误用默认 bash，立即以 `shell: "powershell"` 重跑；没有产生
  仓库垃圾文件或未跑即报通过的命令。已按环境硬要求记录。
- `visual/hash.rs` 剩余 22 行与 `visual/image.rs` 部分剩余行属于私有函数内部不变量、
  不可能触发的溢出 / 越界兜底；公开 API 无法合法触发，未改生产源码去追求虚假覆盖率。
- 单卡 diff 实际 496 insertions / 8 deletions（新增测试 263 行 + 任务卡 133 行 + 轮次报告
  88 行 + 状态同步 12 行），超过 400 行 96 行；按覆盖率卡既有惯例保留完整卡并在此显式
  记录。

### 6. 更合理做法

- 把视觉断言的外部契约集中到一个 integration test，直接覆盖 parser、verdict 投影和
  `pixel_at` 错误语义；不复制 crate 内已有私有单测，也不为不可达分支新增生产钩子。

### 7. 遗留问题

- `visual/hash.rs` 79.82%：剩余 22 行全部为 `VisualError::Inconsistent` 内部兜底
  （DCT 槽位 / median / grid sample / cosine table / `u8` 转换），公开 API 无法触达。
- `visual/image.rs` 剩余 23 行中，`downsample` 零目标维度与若干“内部不可能发生”
  分支都不是公开路径；若未来需要 >85%，应单独立卡评估是否重构，而不是放宽测试断言。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；本轮结论均记录在本卡与 round 文件。

### 9. 给审阅者的关注点

1. 新增 9 个用例是否都调用真实公开 API，并覆盖 parser 与 verdict 的 fail-closed 语义。
2. `visual/assert.rs` 的 89.36% 是否来自可解释的边界断言，而不是与生产实现耦合的白盒
   测试。
