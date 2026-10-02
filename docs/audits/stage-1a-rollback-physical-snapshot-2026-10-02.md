# 阶段 1a 回滚物理快照与执行（TASK-218）— 2026-10-02

> 类型：阶段 1a 回滚链运行证据
> 上位：`tasks/TASK-218-notepad-rollback-execution-verification.md`、ADR-0062
> 结论：物理快照捕获与 L0/L1 回滚执行已落地；完整 L1 恢复在真实 UIA 上通过。
> TASK-218 仍为 InProgress，因为「L0 不可用 → L1 fallback」与三类负向 incident 尚未取得真实证据。

---

## 1. 环境与入口

- 日期：2026-10-02
- 平台：Windows 11 25H2
- 靶机：`fixtures/apps/notepad-like`（WPF + UIA）
- 生产入口：`assistant_agent_core::assemble_production_host`
- 测试：`cargo test -p assistant-agent-core --test production_root_uia test_production_t1_2_dry_run_over_real_uia -- --ignored --nocapture`

## 2. 实现内容

| 部件 | 位置 | 行为 |
|---|---|---|
| 快照捕获 | `apps/agent-core/src/notepad_rollback.rs` | `prepare_anchors` 时读取编辑区规范化文本与目标文件原始字节，记录 SHA-256（复用 `assistant_storage::BlobId`），存进程内注册表 |
| 回滚执行器 | 同上 | 实现 `assistant_undo::RollbackExecutor`：`UndoStack` → 编辑器聚焦后的 `Ctrl+Z`；`RestoreContentSnapshot`/`RestoreShadowCopy` → 编辑区恢复 + 文件字节写回 |
| 回滚后复核 | 同上 | 回滚后同时复核编辑区文本等于 anchor、目标文件字节 digest 等于捕获值；任一不符即 `VerifyFailed` |
| 冲突语义 | 同上 | L1 完整恢复走 `RestoreOverall`（显式整锚点恢复）；编辑器单路径保持 `FailClosed` |
| 装配接线 | `apps/agent-core/src/notepad_registry.rs`、`runtime_host_ops.rs`、`reserved_invoker.rs`、`production.rs` | `prepare_anchors` 真实调用捕获；`ProductionHost::execute_rollback` / `observe_rollback_state` 暴露执行与观察入口 |

补充：`apps/agent-core/Cargo.toml` 新增 workspace 依赖 `assistant-undo`（非第三方，符合 ADR-0053 D3：黑名单针对 `crates/core`，装配点可链接）。

## 3. 真实运行结果

T1.2 完整流程（读文本 → 准备 anchor → 替换 → 两次审批 → 保存）在真靶机跑到 `Completed`，随后执行回滚：

```text
test test_production_t1_2_dry_run_over_real_uia ... ok
```

回滚断言（全部通过）：

- 回滚前：编辑区文本已变更为「报告 报告」，`editor_matches_anchor = false`。
- `execute_rollback(restore_file = true)`：
  - 编辑区文本恢复为「报表 报表」；
  - 目标文件字节恢复为捕获快照；
  - 回滚结果 `editor_matches_anchor = true`、`file_matches_snapshot = true`。
- 回滚后再次观察：`editor_matches_anchor = true`、`file_matches_snapshot = true`。
- 重复回滚：状态仍停留在 anchor（`already_at_anchor` 或幂等 `restored`）。

## 4. 未完成项

- `undo_l1_when_l0_unavailable`（L0 不可用 → L1 fallback，`used_fallback=true`）尚无真实证据。
- 快照缺失 / digest 不匹配 / 回滚期间用户改动三类负向 incident 尚无真实入口证据（`crates/undo` 契约测试已覆盖冲突与 incident 判定）。
- 因此 TASK-218 不标 Done；阶段 1a 仍 NO-GO。

## 5. 全套验收

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | EXIT 0 |
| `cargo test --workspace` | EXIT 0 |
| `cargo test -p assistant-agent-core --test production_root_uia -- --ignored` | T1.1 / T1.2（含回滚）/ T1.3 全通过 |
| `python eval/tasks/notepad/t1.2/validate.py` | PASSED |
| `cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json` | PASSED |
| `cargo run -p xtask -- check-ledger / docscan / card-check / adr-index / memory-counts` | 全 PASSED |
