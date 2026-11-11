# TASK-362 修复 rmcp / SQLite 新 advisory

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：依赖安全修复
- 依赖：TASK-020、TASK-012
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

- **write scope**：
  `Cargo.toml`、
  `Cargo.lock`、
  `clippy.toml`、
  `apps/agent-core/src/main.rs`、
  `crates/tool-bus/Cargo.toml`、
  `crates/storage/Cargo.toml`、
  `crates/audit/Cargo.toml`、
  `docs/DEPENDENCIES.md`、
  `docs/memory/facts.md`、
  `MEMORY.md`、
  `tasks/TASK-362-rustsec-rmcp-sqlite-security-update.md`、
  `PLAN.md`、
  `plans/stage-1-pilots.md`、
  `README.md`、
  `LEDGER.md`

## 目标

修复新 advisory 阻断：

- `rmcp 3.4.1` / RUSTSEC-2026-0457、0461、0462，最低安全版本 3.9.0。
- `libsqlite3-sys 0.35.0` / RUSTSEC-2026-0459，最低安全版本 0.39.0。

`rmcp` 提升到 3.9；SQLite 通过 `rusqlite 0.41.1` 引入
`libsqlite3-sys 0.39.1`。`rusqlite 0.41.1` 的 MSRV 为 Rust 1.89，因此根
workspace MSRV 同步提升到 1.89。

## In scope

- `crates/tool-bus` 的 `rmcp` 声明提升到 `3.9`，保持原有 feature 白名单。
- `crates/storage` / `crates/audit` 的 `rusqlite` 提升到 `0.41.1`，显式保留
  `bundled` 与 `cache`，关闭新版默认 wasm feature。
- 根 workspace `rust-version` 从 1.88 提升到 1.89。
- `clippy.toml` MSRV 同步为 1.89。
- `apps/agent-core/src/main.rs` 对升级后超过大小阈值的 production future
  使用 `Box::pin`，不改变行为。
- 更新 `Cargo.lock`，核对依赖链只剩安全版本。
- 更新依赖登记、FACT、进度文件。

## Out of scope

- 不修改公开 Rust API、Tool/UI/DB schema、ErrorCode 或工具语义。
- 不新增第三方依赖或 feature；不放宽 `cargo deny`。
- 不修改 SQLite 迁移、SQL 或业务行为。
- 不处理 TASK-057/058 占位正文。

## 必须遵守

1. 只做 advisory 所需的最小版本升级，禁止全量刷新其他依赖。
2. `rmcp` 必须保持 `default-features=false` + `client/server`。
3. `rusqlite` 必须保留 bundled 与 cache 语义，不依赖系统 SQLite。
4. 若升级需要扩大产品源码改动或公共 API，停止并记录 DRIFT；仅允许上述已知
   production future 装箱。
5. `cargo deny check` 必须四类全绿，不允许 ignore/waiver。

## 验收命令

```powershell
cargo update -p rmcp --precise 3.9.0
cargo update -p rusqlite --precise 0.41.1
cargo update -p libsqlite3-sys --precise 0.39.0
cargo tree -i rmcp --target all
cargo tree -i libsqlite3-sys --target all
cargo deny check
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- replay --suite core
git diff --check
```

期望：`rmcp 3.9.0`、`rusqlite 0.41.1`、`libsqlite3-sys 0.39.1`；
依赖链无其他意外变化；`cargo deny` 四类全绿；全部门禁通过。

## DoD

- [ ] `rmcp 3.9.0`，feature 白名单不变。
- [ ] `rusqlite 0.41.1` → `libsqlite3-sys 0.39.1`，bundled/cache 语义不变。
- [ ] 根 workspace MSRV 明确为 1.89，依赖登记与文档同步。
- [ ] 四个 RUSTSEC advisory 消失，`cargo deny` 四类全绿。
- [ ] 全部门禁通过；PR 合并并记录 checks / merge / post-merge main CI。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-362 修复 rmcp / SQLite 新 advisory
【目标】最小升级 rmcp 3.9.0、rusqlite 0.41.1/libsqlite3-sys 0.39.0，关闭四个 RUSTSEC 阻断
【write scope】根/成员 Cargo.toml、Cargo.lock、依赖登记、FACT/MEMORY、本卡与进度文件
【铁律】无静默失败；不放宽 deny；不改产品 API/schema；依赖变化最小且可审计
【禁止】不加依赖/feature、不改 SQL/迁移/产品代码、不使用 ignore/waiver、不改 TASK-057/058
【验收】deny、tree、fmt/clippy/workspace/arch/schema/codegen/文档门禁/replay
【依赖】TASK-020 / TASK-012 为两处依赖来源；TASK-361 暂为 Review 等待本卡
【疑问】无；rusqlite 0.41.1 最低要求 Rust 1.89，需同步根 workspace MSRV
```

### 2. 实际改动文件

- `Cargo.toml` / `clippy.toml`：workspace MSRV 与 clippy MSRV 同步到 1.89。
- `crates/tool-bus/Cargo.toml`：`rmcp` 从 caret 3.4 提升到 caret 3.9，
  feature 仍只开 `client` + `server`。
- `crates/storage/Cargo.toml` / `crates/audit/Cargo.toml`：
  `rusqlite 0.41.1`，`default-features=false` + `bundled/cache`。
- `Cargo.lock`：`rmcp 3.9.0`、`rusqlite 0.41.1`、
  `libsqlite3-sys 0.39.1` 及新版依赖所需最小传递更新。
- `apps/agent-core/src/main.rs`：production future 使用 `Box::pin`，
  保持行为不变并使 clippy `large_futures` 通过。
- `docs/DEPENDENCIES.md`、`docs/memory/facts.md`、`MEMORY.md`：依赖登记与
  FACT / 规模同步。
- 本卡、`PLAN.md`、`plans/stage-1-pilots.md`、`LEDGER.md`：进度同步。

### 3. 验收输出摘要

- `cargo deny check`：advisories / bans / licenses / sources 全部 ok；
  RUSTSEC-2026-0457 / 0459 / 0461 / 0462 均消失。
- `cargo tree`：`rmcp 3.9.0`；`libsqlite3-sys 0.39.1 <- rusqlite 0.41.1`
  仍由 audit/storage 使用。
- `cargo fmt --all --check`、all-targets clippy、`cargo test --workspace`：通过。
- `cargo test -p assistant-core arch::`：10 passed。
- verify-schemas、codegen、hygiene、memory-counts、adr-index、refscan、
  docscan、card-check、check-ledger、check-comments、check-migrations：通过。
- `replay --suite core`：2 steps / 5 changes / 0 mismatch，通过。

### 4. DoD 逐条核对

- [x] `rmcp 3.9.0`，feature 白名单不变。
- [x] `rusqlite 0.41.1` → `libsqlite3-sys 0.39.1`，bundled/cache 语义不变。
- [x] 根 workspace MSRV 明确为 1.89，依赖登记与文档同步。
- [x] 四个 RUSTSEC advisory 消失，`cargo deny` 四类全绿。
- [ ] PR 合并并记录 checks / merge / post-merge main CI。

### 5. 偏差

- `rmcp 3.9` 使 production future 超过 clippy `large_futures` 阈值；在
  `apps/agent-core/src/main.rs` 调用点使用 `Box::pin`，不改变行为。
- 当前 user turn 机器统计 `shell_misuse=bash=44` / `cmd=0`；均被 ADR-0100
  hook 重写为 PowerShell，未产生非预期仓库写入。

### 6. 实施中发现的更合理做法

- 保持 `rmcp` 现有 feature 白名单，并为 `rusqlite` 显式恢复旧默认语义
  `bundled + cache`，避免新版默认 feature 扩张。
- `rmcp 3.9` 增大 production future；只在调用点 `Box::pin`，避免为了
  clippy 阈值拆分既有 runtime 装配。

### 7. 遗留问题

- TASK-362 完成后，TASK-361 需要 rebase 并重跑全门禁。

### 8. 新增长期记忆

- FACT 已记录：RUSTSEC-2026-0457/0459/0461/0462 的安全下限、最终版本链、
  MSRV 1.89 与 production future 装箱原因。

### 9. 给审阅者的关注点

- 审查 `Cargo.lock` 是否只发生 advisory 所需升级。
- 审查 `rusqlite` feature 是否仍为 bundled/cache，且无系统 SQLite 回退。
