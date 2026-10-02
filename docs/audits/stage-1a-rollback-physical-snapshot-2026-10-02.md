# 阶段 1a 回滚物理快照与执行（TASK-218）— 2026-10-02

> 类型：阶段 1a 回滚链运行证据
> 上位：`tasks/TASK-218-notepad-rollback-execution-verification.md`、ADR-0062
> 结论：物理快照捕获与 L0/L1 回滚执行已落地；完整 L1 恢复在真实 UIA 上通过；
> L0 未到锚点 → L1 fallback 与三类负向用例已在 fake-platform 生产入口取得证据。

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

## 4. Fallback 与负向证据（fake-platform 生产入口）

`cargo test -p assistant-agent-core --test production_root`（13 passed）覆盖：

| 场景 | 测试 | 断言 |
|---|---|---|
| L0 未到锚点 → L1 fallback | `test_production_rollback_uses_l1_fallback_when_l0_misses` | `used_fallback=true`，编辑区与磁盘恢复为 anchor |
| 快照缺失 | `test_production_rollback_without_a_captured_anchor_is_refused` | 观察与回滚都显式报 `no captured anchor` |
| 显式整锚点恢复覆盖后发生的文件改动 | `test_production_rollback_whole_anchor_overwrites_later_file_change` | `RestoreOverall` 下文件恢复为捕获字节并复核 digest |
| 用户改动阻断 fail-closed 路径 | `test_production_rollback_user_change_is_an_incident` | 编辑器路径返回 incident/错误而非成功 |

补充：`prepare_anchors` 现在在任务声明 L1 但缺目标文件路径时显式 `VerifyFailed`，不静默降级成“只锚定编辑器”。

## 5. 未完成项

- 无新增功能缺口；TASK-218 的 DoD 已全部取得证据。
- 阶段 1a 仍为 NO-GO：Notepad 之外子阶段与 1a 集成验收尚未完成。

## 6. 全套验收

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | EXIT 0 |
| `cargo test --workspace` | EXIT 0 |
| `cargo test -p assistant-agent-core --test production_root` | 13 passed（含 fallback 与三类负向） |
| `cargo test -p assistant-agent-core --test production_root_uia -- --ignored` | T1.1 / T1.2（含回滚）/ T1.3 全通过 |
| `python eval/tasks/notepad/t1.2/validate.py` | PASSED |
| `cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json` | PASSED |
| `cargo run -p xtask -- check-ledger / docscan / card-check / adr-index / memory-counts` | 全 PASSED |
