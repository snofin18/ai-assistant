# TASK-417 Stabilize Excel worker reap deadline test

- 状态：**Review**
- 阶段：跨阶段 platform test hardening

---

## write scope

- `tasks/TASK-417-stabilize-excel-worker-reap-test.md`
- `crates/platform/windows/src/excel_com_runner_journal_tests.rs`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

消除 macOS 上 `test_worker_reap_deadline_is_bounded` 的线程调度竞态。worker 必须在
测试确认 1 ms join deadline 已失败后才允许退出，同时保留原有“deadline 必须失败”的断言。

## In scope

- 只修改 test-only 的 worker 生命周期控制：增加 release/done 通道，等待 deadline 断言完成后再
  放行 worker。
- 验证 worker 最终退出，避免留下后台线程。
- 同步 `PLAN.md`、`README.md`、`LEDGER.md` 与 PR/CI 证据。

## Out of scope

- 不修改 `join_worker_bounded` 的实现、超时时长或 `ErrorCode`。
- 不改生产作业调度、COM runner、watchdog、journal、schema、公共 API 或依赖。
- 不放宽测试断言，不通过删除测试或 `#[ignore]` 规避问题。
- 不启动真实 Excel/COM/GUI。

## 必须遵守

1. 失败原因必须是测试调度竞态，不是将产品缺陷改绿。
2. worker 在 deadline 失败前必须保持存活；deadline 失败后再确定性退出。
3. 测试应验证 worker 已完成，不能只 sleep 后假定回收。
4. 只改本卡 write scope 内文件。

## 验收

```powershell
cargo test -p assistant-platform-windows excel_com_runner::tests::journal_lifecycle::test_worker_reap_deadline_is_bounded --lib -- --exact
1..50 | ForEach-Object { cargo test -p assistant-platform-windows excel_com_runner::tests::journal_lifecycle::test_worker_reap_deadline_is_bounded --lib -- --exact --quiet }
cargo fmt --all --check
cargo test -p assistant-platform-windows
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo run -p xtask -- check-ledger
git diff --check
```

预期：定向测试和 50 次循环全过；worker 无残留；PR CI 的 macOS job 通过。

## DoD

- [ ] worker 在 deadline 断言前不会自然退出。
- [ ] deadline 仍返回 `Fatal`，断言未被弱化。
- [ ] 放行后 worker 确定性完成并被观察。
- [ ] 定向测试循环 50/50 通过。
- [ ] workspace/CI 全绿，PR/merge/main CI 证据已记录。

---

## 执行记录

### 1. 约束回执

```text
【任务】TASK-417 Stabilize Excel worker reap deadline test
【目标】用 release/done 同步消除 macOS worker 调度竞态
【write scope】仅本卡、excel_com_runner_journal_tests.rs、PLAN/README/LEDGER
【铁律】不弱化断言；测试确定性；worker 必须回收；不改产品语义
【禁止】不改 join_worker_bounded；不放宽测试；不改依赖/schema/API；不启动 Excel/GUI
【验收】定向测试 50 次 + platform crate + workspace + clippy + CI
【依赖】TASK-416 #890 merge `a68cd30`；macOS failure log 已确认
【疑问】无
```

### 2. 实际修改

- `crates/platform/windows/src/excel_com_runner_journal_tests.rs` 的 worker 现在通过
  release/done channel 受测试控制：先确认 1 ms deadline 返回 `Fatal`，再放行 worker，
  并等待 done channel 证明线程已完成。
- 未修改 `join_worker_bounded`、生产 runner、watchdog、schema、API、依赖或真实应用。

### 3. 验收记录

- full-path 定向测试：1 passed。
- 同一 full-path 定向测试循环 50 次：**50/50 passed**。
- 首次使用短名称加 `--exact` 命中 0 tests，未计入验收；已改为完整路径并重新执行。
- `cargo test -p assistant-platform-windows --lib`：196 passed / 8 ignored。
- `cargo test --workspace`：PASSED，包含 doctest。
- `cargo fmt --all --check`：PASSED。
- `cargo clippy --all-targets -- -D warnings`：PASSED；仅有既有
  `clippy::assert_is_empty` unknown-lint warning。
- `git diff --check`：待提交前再跑。

### 4. DoD 核对

- [x] worker 在 deadline 断言前不会自然退出。
- [x] deadline 仍返回 `Fatal`，断言未被弱化。
- [x] 放行后 worker 确定性完成并被观察。
- [x] 定向测试循环 50/50 通过。
- [ ] PR/CI/merge/main CI 证据记录。

### 5. 偏差

无。

### 6. 新增长期记忆

无。

### 7. 遗留问题与下一步

- 本卡只修测试竞态；不改变 Excel runner 的生产 deadline 语义。
- 完成后下一张回到 Stage 2 私有 production save caller / task-end 集成边界。

### 8. 外部证据

- 待提交后回填 implementation PR / CI / merge / main CI。

### 9. 给审阅者的关注点

- 核对 worker 在 deadline 失败前是否确实被通道阻塞。
- 核对断言仍是 `expect_err`，没有改成只检查返回值。
- 核对放行后的完成等待不是固定 sleep。
