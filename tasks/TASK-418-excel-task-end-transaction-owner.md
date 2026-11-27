# TASK-418 Excel T2.2 private task-end transaction owner

- 状态：**Done**
- 阶段：Stage 2 Excel T2.2 / private task-end integration

---

## write scope

- `tasks/TASK-418-excel-task-end-transaction-owner.md`
- `apps/agent-core/src/excel_t21_l1_write_port.rs`
- `apps/agent-core/src/excel_t21_l1_transaction_tests.rs`
- `apps/agent-core/README.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 test-only 边界内把 TASK-415 的 scalar file transaction receipt、anchor 和文件影子副本
收敛到一个显式 task-end owner：running 时只允许一笔 active transaction；closing 后拒绝新写入；
成功提交或回滚后通过 owned cleanup 进入 terminal；失败时保留可审计证据。

## In scope

- 在既有 `excel_t21_l1_write_port` 文件中增加 test-only owner，不新增模块。
- owner 提供：
  - `execute`：running 且无 active transaction 时执行一次 scalar file transaction；
  - `close_committed`：释放成功事务 anchor 并清理文件影子副本；
  - `close_rolled_back`：按调用方提供的 saved-file digest 回滚，再清理证据；
  - closing/terminal 后拒绝新事务。
- 在既有 transaction tests 中覆盖提交清理、回滚清理、无 active transaction 的 close 和
  terminal 后拒绝新写入。
- 同步 agent-core README、PLAN、README 与 LEDGER。

## Out of scope

- 不实现 production in-place save caller、真实 COM 写盘、原子替换、断电恢复或 Host
  helper process。
- 不注册 `excel.*` production Tool，不填写 `tools.json`、rollback recipe 或 assembly。
- 不修改 schema、公共 API、ErrorCode、Windows runner、依赖或真实 Excel/GUI。
- 不把 test-only owner 宣称为 production task cleanup。

## 必须遵守

1. 一个 owner 同时最多持有一个 active scalar file transaction。
2. closing/terminal 后 execute 必须显式失败，不得继续写文件。
3. commit/rollback 成功后才进入 terminal；cleanup 失败必须返回错误并保留证据。
4. 所有路径继续使用仓库外 owned temporary root 和 fake port。
5. 不新增模块、不扩大 production surface。

## 验收

```powershell
cargo test -p assistant-agent-core excel_t21_l1_task_end_owner
cargo test -p assistant-agent-core excel_t21
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo run -p xtask -- check-ledger
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
git diff --check
```

预期：owner 专项全绿；不启动真实 Excel/COM/GUI；production Tool surface 仍为空。

## DoD

- [x] owner 在 running 时只允许一笔 active transaction。
- [x] closing/terminal 后拒绝新事务。
- [x] committed close 释放 anchor 并清理文件影子副本。
- [x] rollback close 恢复原文件 digest 后释放 anchor 并清理证据。
- [x] 无 active transaction 的 close 和 cleanup failure 显式失败。
- [x] production Tool/ToolBus/tools.json/rollback/assembly/schema/API/依赖零改动。
- [x] 文档门禁、PR/CI/merge/main CI 证据完成。

---

## 执行记录

### 1. 约束回执

```text
【任务】TASK-418 Excel T2.2 private task-end transaction owner
【目标】将 scalar file transaction receipt、anchor 与文件影子副本收敛到 test-only task-end owner
【write scope】仅本卡、excel_t21_l1_write_port.rs、excel_t21_l1_transaction_tests.rs、agent-core README、PLAN/README/LEDGER
【铁律】写操作有 postcondition；失败保留证据；资源有界并清理；不伪造 production 能力
【禁止】不改 production ToolBus/tools.json/rollback/assembly/schema/API/依赖；不启动真实 Excel/COM/GUI；不新增模块
【验收】owner 专项 + Excel 域 + workspace/doctest + fmt/clippy + 文档门禁
【依赖】TASK-415 Done；TASK-416/417 Done；ADR-0062/0107/0108/0110/0111 已核对
【疑问】无
```

### 2. 实际修改

- 在既有 `excel_t21_l1_transaction_tests.rs` 内增加 test-only
  `ExcelL1TaskTransactionOwner`：
  `running -> closing -> terminal`、单 active transaction、committed/rollback close、
  owned shadow cleanup；不新增模块，不扩大 production surface。
- 在既有 transaction tests 增加 owner committed、rollback、empty-close 三个专项测试。

### 3. 验收记录

- `cargo test -p assistant-agent-core l1_task_end_owner`：3 passed。
- `cargo test -p assistant-agent-core excel_t21`：113 passed / 1 ignored。
- `cargo test --workspace`：PASSED，doctest PASSED。
- `cargo fmt --all --check`：待最后复核。
- `cargo clippy --all-targets -- -D warnings`：PASSED；仅有既有
  `clippy::assert_is_empty` unknown-lint warning。
- 文档门禁与 `git diff --check`：待 PR 前复核。

### 4. DoD 核对

待完成后回填。

### 5. 偏差

无。

### 6. 新增长期记忆

无。

### 7. 遗留问题与下一步

- production in-place save caller、真实 COM 写盘、原子替换、断电恢复与 Host 崩溃
  helper process 仍未实现；本卡只证明 private task-end owner 的资源收敛。

### 8. 外部证据

- implementation PR：`https://github.com/snofin18/ai-assistant/pull/894`
- PR checks：21 success + 1 skipped，0 failed / 0 pending
- merge：`0aa7494c396b3aedb00f79293084c5084883bedb`
- post-merge main CI：`45936224728` success

### 9. 给审阅者的关注点

- 核对 owner 的 closing/terminal 是否在文件写入前拒绝新事务。
- 核对 committed/rollback 清理是否释放 anchor 并删除 owned shadow 文件。
- 核对失败路径没有把证据误删或伪装成 terminal 成功。
