# TASK-283　assistant-lease key/error 公共契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：TASK-025 Done　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-025 Done　**预估**：S　**难度**：S
- **write scope**：`crates/lease/tests/lease_contract.rs`、`tasks/TASK-283-lease-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-13-round-9.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/lease/src/key.rs`、`crates/lease/src/error.rs`、`crates/lease/README.md`

**目标**

仅为 `assistant-lease` 增加公开契约测试，重点覆盖 `LeaseOwner` / `LeaseKey` 访问器、标识符边界、
`LeaseError` 全部公开可达变体的稳定 `ErrorCode` 与 `Display` 文案。把 `key.rs` 与 `error.rs`
的行覆盖率提升到可解释的稳定区间，并在不修改生产逻辑、不新增依赖的前提下守住 crate ≥85%。

**背景**

2026-10-13 round 9 开工前实测：`assistant-lease` crate 行覆盖率 89.53%，`key.rs` 79.22%、
`error.rs` 60.24%。既有 `lease_contract.rs` 已覆盖管理器冲突、TTL、preemption 与 deadlock
order，但公开访问器、边界字符串与大多数错误 Display 分支缺少回归证据。候选来自
`docs/automations/2026-10-13-report.md` 的 P2 质量路线。

**步骤**

1. 在既有 `crates/lease/tests/lease_contract.rs` 中追加 tests-only 用例，不改生产源码。
2. 覆盖 `LeaseOwner::as_str` / `Display`、`LeaseKey` 全部访问器、无可选组件 Display 与
   256-byte 边界。
3. 通过公开 `LeaseManager` API 构造输入错误、时钟错误、冲突、缺失、过期、owner mismatch 与
   duplicate key，并断言稳定 `ErrorCode` + Display 子串。
4. 复测 `cargo llvm-cov -p assistant-lease --tests --fail-under-lines 85`，记录 crate / key / error
   行覆盖率。
5. 跑全部全局硬门禁并同步 LEDGER / PLAN / README / plans。

**DoD**

- [ ] 生产逻辑零改动（仅追加测试）
- [ ] 新增测试只调用公开 API
- [ ] `key.rs` 行覆盖率 ≥90%，`error.rs` 行覆盖率 ≥85%（公开 API 可达范围内）
- [ ] `cargo llvm-cov -p assistant-lease --tests --fail-under-lines 85` 与 `cargo test -p assistant-lease` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁与 `cargo deny check` PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-lease --tests --fail-under-lines 85
cargo test -p assistant-lease
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-283 assistant-lease key/error 公共契约覆盖率补齐
【目标】仅追加公开契约测试，提高 `key.rs` / `error.rs` 行覆盖率与稳定性证据
【write scope】仅：`crates/lease/tests/lease_contract.rs` 与本卡列出的状态 / 轮次文件
【铁律】测试不改生产行为；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；启动真实 GUI / 网络 / 凭据
【验收】见「验收命令」→ crate / 模块覆盖率 + 全部硬门禁
【依赖】TASK-025 Done；章程 §2 P2 / §5.3；运行日 2026-10-13 round 9
【疑问】无

### 2. 实际改动文件

- `crates/lease/tests/lease_contract.rs`（新增 6 个公开契约测试）
- `tasks/TASK-283-lease-contract-coverage.md`
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-9.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前实测：`assistant-lease` crate 行覆盖率 89.53%（477/487）；`key.rs` 79.22%、`error.rs` 60.24%。
- `cargo test -p assistant-lease`：25 passed；`cargo llvm-cov -p assistant-lease --tests --fail-under-lines 85`：EXIT 0；
  crate 行覆盖率 **97.54%**（475/487），`key.rs` **94.81%**、`error.rs` **92.77%**。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`：EXIT 0。
- `cargo deny check`：advisories / bans / licenses / sources 全 ok；`cargo test -p assistant-core arch::`：10 passed；
  `cargo run -p xtask -- replay --suite core`：2 step / 5 change / 0 mismatch / PASSED。
- UI：`pnpm lint`、`pnpm typecheck`、`pnpm test` 全绿（78 node tests + 3 vitest tests）。
- 非宿主平台：`x86_64-unknown-linux-gnu` 与 `aarch64-apple-darwin` 的 `assistant-lease` clippy 均 EXIT 0。
- xtask：`hygiene` scanned=410 / 0E / 117W / PASSED，`check-comments` 0E / 67W / PASSED，
  `memory-counts`、`adr-index`、`refscan`、`docscan`、`card-check`、`check-ledger`、`verify-schemas`、
  `codegen --check`、`check-migrations` 全部 PASSED。
- PR #364；pull_request CI = 11/11 SUCCESS；merge `6346352`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；diff 仅 `crates/lease/tests/lease_contract.rs`。
- 新增测试只调用公开 API：满足；访问器、构造与 `LeaseManager` 路径均为公开 API。
- `key.rs` ≥90%、`error.rs` ≥85%：满足，94.81% / 92.77%。
- crate 覆盖率命令与 `cargo test -p assistant-lease`：满足，97.54% / 25 passed。
- 全 workspace 与 xtask 硬门禁、`cargo deny check`：满足。
- 状态同步：满足；`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 已同批同步。

### 5. 偏差

- 本轮优先尝试 P3，但 round 8 后剩余 `hygiene` 告警均为设计性复杂度 / 文件长度，`check-comments` 多数要求公共命名变更；
  后者属契约动作，未动。
- P6 的提示路径为 `MEMORY.md` §5，但 ADR-0021 已把 `MEMORY.md` 降为 L0 索引，无 §5；按章程回退到 P2，未自行改 ADR/spec。
- 首次新增测试使用 `expect_err`，clippy `expect_used` 拒绝；改为复用既有 `expect_error` helper 后通过。
- 首次追加测试使 `lease_contract.rs` 达 605 行，hygiene 新增 file-too-long；压缩测试后回到 599 行，新增告警归零。
- 工具通道偏差：本轮误用 7 次 bash，均未执行到 cargo / 仓库文件；后续严格使用 PowerShell。

### 6. 更合理做法

- 将错误 Display / ErrorCode 契约集中为 tests-only helper 后追加在既有 integration test，
  避免新增 test target，也避免生产源文件的任何改动。

### 7. 遗留问题

- `LeaseError::InternalInvariant` 无公开构造入口，其 Display 分支仍不可由公共 API 覆盖；
  不为了追数字破坏私有边界。
- `crates/lease/src/error.rs` 的既有 `fmt` 圈复杂度 18 仍保留；本轮目标为覆盖加固，不做顺手重构。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；覆盖率事实已写入 `LEDGER.md` 与计划状态。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，未改变生产控制流。
2. 覆盖率提升是否来自真实断言而非重复路径。
3. 错误 Display 文案是否与既有稳定契约一致。
