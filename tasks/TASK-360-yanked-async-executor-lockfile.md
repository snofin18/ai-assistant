# TASK-360 修复 yanked async-executor 锁文件

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：依赖安全修复
- 依赖：TASK-358、TASK-359
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

- **write scope**：
  `Cargo.lock`、
  `tasks/TASK-360-yanked-async-executor-lockfile.md`、
  `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`

## 目标

将传递依赖 `async-executor` 从已被 crates.io yanked 的 `1.14.0` 升级到
`1.14.1`，只更新锁文件，解除 `cargo deny` 的 `error[yanked]`，使受影响的
automation evidence PR 可以重新过门禁。

## In scope

- `cargo update -p async-executor --precise 1.14.1`
- 核对 `Cargo.lock` 只发生该包版本与 checksum 更新
- 验证 `async-executor` 的传递依赖链仍由 `zbus` / `secret-service` /
  `keyring` / `assistant-secrets` 使用
- 验证 `cargo deny check` 四类检查全绿
- 同步任务卡、阶段进度与 LEDGER 证据

## Out of scope

- 不修改任何 `Cargo.toml`
- 不增加依赖、不放宽 `cargo deny`、不添加 advisory waiver
- 不修改产品代码、测试断言、Tool/DB/UI IPC schema、ErrorCode 或平台 trait
- 不处理 TASK-057 / TASK-058 的占位正文
- 不把 automation evidence PR #701 / #702 的内容混入本卡实现提交

## 必须遵守

1. 只允许锁文件版本修复；若 `--precise 1.14.1` 改动其他包，停止并记录 DRIFT。
2. 禁止 `cargo update` 全量刷新，禁止删除或重建 `Cargo.lock`。
3. `cargo deny` 失败时必须查真实 advisory，不得用 ignore、allow 或降低门禁绕过。
4. 所有 `exec_command` 必须显式使用 `shell: "powershell"`。
5. 合并门禁固定为 base=main、checks>=11、pending=0、failed=0、
   `MERGEABLE`、`CLEAN`，且 post-merge main CI 成功。

## 验收命令

```powershell
cargo update -p async-executor --precise 1.14.1
cargo tree --target all -i async-executor
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

期望：`async-executor v1.14.1`；`Cargo.lock` 仅该包版本/checksum变化；
`cargo deny check` advisories / bans / licenses / sources 全部 ok；全部门禁通过。

## DoD

- [ ] `async-executor` 锁版本为 `1.14.1`，传递依赖链未意外改变。
- [ ] `Cargo.lock` 之外没有依赖声明变化，未放宽 deny 判据。
- [ ] `cargo deny check` 全绿，yanked advisory 消失。
- [ ] fmt、clippy、workspace tests、arch、schema/codegen、文档门禁和 replay 全绿。
- [ ] PR 合并并记录 merge / checks / post-merge main CI；任务卡与进度文件同步 Done。

<!-- 分界线：以上为卡片正文，Orchestrator 所有；以下由 Implementer 填写。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-360 修复 yanked async-executor 锁文件
【目标】只更新 Cargo.lock 到 async-executor 1.14.1，解除 cargo deny yanked 失败
【write scope】Cargo.lock、本卡、PLAN.md、plans/stage-1-pilots.md、README.md、LEDGER.md
【铁律】无静默失败；契约先行；不放宽门禁；依赖变更必须最小化并可审计
【禁止】不改 Cargo.toml；不加依赖/waiver；不改产品代码、schema/API、workflow 或 automation PR 内容
【验收】cargo tree/deny + fmt/clippy/workspace/arch/schema/codegen/文档门禁/replay → 全绿
【依赖】TASK-358、TASK-359 Done；origin/main=9422190
【疑问】无；版本 1.14.1 已由 cargo info 确认为可下载的 crate
```

### 2. 实际改动文件

- `Cargo.lock`：`async-executor` 由 `1.14.0` 更新到 `1.14.1`，仅版本与 checksum 变化。
- `tasks/TASK-360-yanked-async-executor-lockfile.md`：记录修复范围、验收入口与证据。

### 3. 验收输出摘要

- `cargo update -p async-executor --precise 1.14.1`：`Updating async-executor v1.14.0 -> v1.14.1`，39 个其他依赖未变。
- `git diff -- Cargo.lock`：仅 `async-executor` 的 version 与 checksum 两行变化。
- `cargo tree --target all -i async-executor`：`async-executor v1.14.1`，链路仍为 `zbus -> secret-service -> zbus-secret-service-keyring-store -> keyring -> assistant-secrets`。
- `cargo deny check`：`advisories ok, bans ok, licenses ok, sources ok`。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`：exit 0；保留既有 `unknown lint: clippy::assert_is_empty` 提示。
- `cargo test --workspace`：exit 0；agent-core 98 + 61、xtask 510，全部 doctest 通过。
- `cargo test -p assistant-core arch::`：10 passed。
- `verify-schemas`、`codegen --check`、`hygiene`、`memory-counts`、`adr-index`、`refscan`、`docscan`、`check-comments`、`replay --suite core`：均 PASSED。

### 4. DoD 逐条核对

- [x] `async-executor` 锁版本为 `1.14.1`，传递依赖链未改变。
- [x] `Cargo.lock` 之外没有依赖声明变化，未放宽 deny 判据。
- [x] `cargo deny check` 全绿，yanked advisory 消失。
- [x] fmt、clippy、workspace tests、arch、schema/codegen、文档门禁和 replay 全绿。
- [ ] PR 合并并记录 merge / checks / post-merge main CI；任务卡与进度文件同步 Done。

### 5. 偏差

- 本会话有一次只读 `git switch main` 误填 `shell="bash"`；ADR-0100 hook 在启动前拒绝，未在 WSL/bash 执行，也未写入仓库。

### 6. 实施中发现的更合理做法

- 使用 `cargo update -p async-executor --precise 1.14.1` 而不是全量刷新锁文件，可把依赖变化限制到单一包。

### 7. 遗留问题

- 依赖修复合并后，需要更新并重跑 automation evidence PR #701 / #702。

### 8. 新增长期记忆

- 无。依赖版本与 advisory 处置已落任务卡和后续 LEDGER 行。

### 9. 给审阅者的关注点

- 审查 `Cargo.lock` diff 是否只有 `async-executor` 版本与 checksum。
- 审查 `cargo deny check` 是否保持四类检查全开，没有新增 ignore/waiver。
