# 阶段 1a 运行验收（TASK-105）— 2026-10-02

> 类型：阶段 1a 运行验收证据
> 上位：`tasks/TASK-105-notepad-t1-runtime-validation.md`
> 结论：T1.1 / T1.2 / T1.3 主路径各 10/10 通过；TASK-105 仍为 InProgress，
> 因为真实 L0/L1 撤销与回滚链尚未在运行层执行取证。

---

## 1. 环境与入口

- 日期：2026-10-02
- 平台：Windows 11 25H2，交互式桌面
- 靶机：`fixtures/apps/notepad-like`（WPF + UIA + 跨进程 Save As dialog）
- 生产入口：`assistant_agent_core::assemble_production_host` + `RuntimeExecutor`
- 真实平台：`assistant_platform_windows::WindowsPlatform`
- 编排脚本：`eval/tasks/notepad/run-real-uia.ps1`
- 结果文件：`docs/audits/stage-1a-runtime-validation-2026-10-02-runs.json`

运行命令模板：

```powershell
cargo test -p assistant-agent-core --test production_root_uia <test> -- --ignored --nocapture
```

## 2. 3 × 10 次真实运行结果

| 任务 | 测试 | 运行数 | 通过 | 失败 | 平均耗时 | 最低 90% 门槛 |
|---|---|---:|---:|---:|---:|---|
| T1.1 | `test_production_t1_1_dry_run_over_real_uia` | 10 | 10 | 0 | 1128 ms | PASS |
| T1.2 | `test_production_t1_2_dry_run_over_real_uia` | 10 | 10 | 0 | 1342 ms | PASS |
| T1.3 | `test_production_t1_3_dry_run_over_real_uia` | 10 | 10 | 0 | 2595 ms | PASS |

汇总：

```text
T1.1: 10/10 PASS
T1.2: 10/10 PASS
T1.3: 10/10 PASS
silent_failure_rate = 0
```

每轮退出码、耗时和日志路径见结果 JSON；日志位于 `target/t1-real-uia-logs/`。

## 3. 覆盖内容

### T1.1

- fixture 真实启动并写出 ready state。
- UIA 定位主窗口和 `EditorTextBox`。
- 经真实 ToolBus 调用 `notepad.file.read_text`。
- VerifyReceipt 生成并由 task-engine 提交。

### T1.2

- fixture 使用 `--document` 打开真实临时文件。
- `notepad.file.read_text` 读取前序文本。
- pure replacement/diff 产生审批数据。
- `notepad.file.replace_text` 写回并回读。
- 两次 bounded approval 各消费一次。
- `notepad.file.save` 经 `Ctrl+S` 写回真实文件。
- 运行结束后磁盘内容为 `报告 报告`。

### T1.3

- `validate_t1_3_inputs` 校验绝对路径。
- `inspect_target_path` 确认目标不存在。
- `notepad.tab.new` 读取 `TabCountText` 并确认 tab count 增加。
- `set_editor_value` 写入，`notepad.file.read_text` 回读。
- Save As 进入 `hitl` 一次审批。
- `notepad.file.save_as` 驱动跨进程 Save As dialog。
- 运行结束后目标文件存在且内容匹配输入文本。

## 4. 审批与失败边界证据

真实 3 × 10 运行使用 bounded `once` grants；审批控制流另由以下测试覆盖：

| 命令 | 结果 |
|---|---|
| `cargo test -p assistant-agent-core --test production_root test_production_t1_2_pauses_and_resumes_with_ui_approval` | 1 passed |
| `cargo test -p assistant-agent-core --test runtime_toolbus` | 5 passed |
| `fixtures/apps/notepad-like/test-notepad-like.ps1` | 13 checks passed |

覆盖点：

- 无授权时停在 `AwaitingApproval`，批准后从同一快照恢复。
- 恢复不重放已提交步骤。
- `request_approval` 永不自动批准。
- 批准授权只消费一次。
- `prepare_anchors` 拒绝 `l3_irreversible`。
- fixture 的 `none / disappear / timeout / ambiguous / dialog / busy` 六种故障形状通过真实 UIA 检查。

## 5. 为通过真实入口做的修复

1. `fixtures/apps/notepad-like/notepad-like.ps1`
   - 处理 `Ctrl+S`，与 Save 按钮共用写盘逻辑。
   - Save/AddTab 的 enabled 状态反映 dirty / tab 状态，使 UIA 指纹真实反映状态变化。
2. `fixtures/apps/notepad-like/MainWindow.xaml`
   - `TabCountText` 改为只读 `TextBox`，让 UIA 暴露稳定的 ValuePattern。
3. `apps/agent-core/src/notepad_handlers.rs`
   - Save As 点击后按既有超时有界轮询目标文件，避免跨进程对话框异步完成造成误报。
4. `apps/agent-core/tests/production_root_uia.rs`
   - 增加真实 T1.2 / T1.3 ignored dry-run。
5. `eval/tasks/notepad/run-real-uia.ps1`
   - 3 × 10 次真实运行编排与 JSON 证据汇总。

上述第 3 / 4 项超出原任务卡 write scope；因人工已授权按最优方案处理，记入
`DRIFT-105-4`，不作为 TASK-105 Done 的静默扩权。

## 6. 未完成项

- T1.2 的物理 L0 undo 与 L1 disk snapshot 尚未通过真实运行链执行回滚并复核。
- `prepare_rollback_anchors` 当前只验证 anchor 方案合法性，不创建物理 snapshot。
- 因此 TASK-105 不标 Done；阶段 1a 仍为 NO-GO。

## 7. 全套验收

| 命令 | 结果 |
|---|---|
| `cargo test --workspace` | EXIT 0 |
| `cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json` | PASSED |
| `python eval/tasks/notepad/t1.1/validate.py` | PASSED |
| `python eval/tasks/notepad/t1.2/validate.py` | PASSED |
| `python eval/tasks/notepad/t1.3/validate.py` | PASSED |
| `cargo run -p xtask -- docscan` | 0 error / 352 warning，PASSED |
