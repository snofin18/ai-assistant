# TASK-371 Edge runner failure cleanup

日期：2026-11-16

## 1. 背景

TASK-370 在两次未收到人工决定的预审批运行后发现：

- runner 的 `finally` 会停止 Core、desktop-ui 和 fixture server；
- Core 启动的专用 Edge 进程树不会由 runner 直接回收；
- 两次失败均需操作者手工对 root PID 执行 `taskkill /T /F`。

这违反了 ADR-0063 对失败路径“终止整棵进程树并验证资源收敛”的要求。本卡只修测试 runner，
不改生产 browser launcher。

## 2. 实现

新增 `edge_process_cleanup.ps1`：

- 只匹配命令行包含完整专用 profile 的进程；
- 空 profile、非法进程名或匹配数超过上限时 fail-closed；
- 对匹配进程执行有界 `taskkill /PID <pid> /T /F`；
- `taskkill` 退出码作为诊断，最终以 profile 进程数为 cleanup postcondition；
- 同时限制 taskkill 尝试次数、总等待时间和初始匹配进程数；
- 写入 `edge-cleanup.log`，记录初始数、尝试数、退出码、剩余 PID、耗时和结果。

`run_real_edge_ui_approval.ps1` 在停止 UI/Core 后调用该原语，并在超时/中止/异常的失败 summary
中记录 cleanup 日志、taskkill 尝试数和清理后进程数。

## 3. 离线回归

`test_edge_process_cleanup.ps1` 启动真实 PWsh 父/子进程树，并验证：

- 目标树至少包含 2 个匹配进程；
- `taskkill /T /F` 后目标进程数归零；
- cleanup 日志记录 `result=passed` 与 `remaining_process_count=0`；
- 空 profile 被拒绝；
- 不匹配 profile 的清理不会终止另一棵受保护进程树。

结果：

```text
PASS: profile-scoped process-tree cleanup converged without collateral termination
```

## 4. 真实 runner 失败 smoke

证据目录：

`eval/tasks/edge/t5.2/evidence/2026-11-16-task-371-failure-cleanup/attempt-02-timeout/`

运行参数：`-UiDecisionTimeoutSeconds 30`。人类未在窗口内批准。

结果：

```text
result=failed
failure=No human decision was received within 30 seconds
core_alive=False
ui_alive=False
dedicated_edge_remaining=0
port_8766_listener=False
fixture stopped=True
```

Edge cleanup：

```text
initial_process_count=8
taskkill_attempt_count=8
taskkill_exit_codes=0,128,128,128,128,128,128,128
remaining_process_count=0
elapsed_ms=2515
result=passed
```

root 被回收后，后续子 PID 的 `taskkill=128` 表示进程已不存在；这与 TASK-369 对生产
root reap 后非零 taskkill 的诊断语义一致。最终 0 个专用 profile 进程才是判据。

## 5. 人工批准旁证

attempt-01 在原定的“无人批准”窗口内收到了一次人类 `once` 审批，因此不能作为失败清理证据；
证据独立保存在：

`eval/tasks/edge/t5.2/evidence/2026-11-16-task-371-failure-cleanup/attempt-01-human-approved/`

该次只访问本地 `127.0.0.1` fixture：

- `result=passed`
- `submit_step_count=1`
- `submitted=true`
- `post_url=http://127.0.0.1:8766/t5.2/submitted`
- Core、UI、专用 Edge 与端口均收敛

## 6. 门禁

- Python T5.2 fixture：9/9 passed。
- 进程树清理回归：PASSED。
- `cargo fmt --all --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：exit 0。
- `cargo test --workspace`：510 passed。
- arch、schema/codegen、hygiene、card-check、docscan、refscan、check-ledger、
  check-comments、memory-counts、adr-index、replay：PASSED。
- `cargo deny check`：advisories / bans / licenses / sources 全部 ok。
- desktop-ui lint / typecheck / test：通过。

## 7. 结论

TASK-370 遗留的 runner 失败资源收敛缺口已修复。成功路径仍验证 Core 自然关闭专用 Edge；
失败路径现在也会在有界时间内回收完整 profile 进程树，并以进程数归零作为唯一成功判据。
