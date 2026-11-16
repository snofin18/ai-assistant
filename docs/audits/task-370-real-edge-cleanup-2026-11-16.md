# TASK-370 Real Edge cleanup regression

日期：2026-11-16

## 1. 背景

TASK-367 `run-04` 在真实 Edge 表单提交后遇到：

```text
TargetUnresponsive: taskkill returned exit code: 255, but the browser root was reaped
```

提交已成功，但 cleanup 非零状态覆盖了已经达成的 root reap 后置条件。TASK-369
将“root 已 reap 后，已返回的 `taskkill` 非零状态”降为诊断信息，同时保留
`taskkill` 启动失败与 root 未 reap 的 fail-closed 分支。

本卡不重写 TASK-367 的 9/10 证据，只增加一次新的单轮真实 Edge 回归。

## 2. 方法

- 使用正式 desktop-ui 二进制和 authenticated Core UI pipe。
- 使用专用 Edge profile，不读取默认用户 profile。
- 人类在正式审批卡中亲自完成一次授权。
- 只提交一次本地 `127.0.0.1:8766` fixture，不访问外部站点。
- 使用独立证据目录：
  `eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression-attempt-03/`。

## 3. 真实运行结果

- Edge：`157.0.4341.60`
- Core PID：`8652`
- desktop-ui PID：`39620`
- profile id：`task364-ui-human-74328712`
- phase：`passed`
- result：`passed`
- `submit_step_count=1`
- `submitted=true`
- `post_url=http://127.0.0.1:8766/t5.2/submitted`
- `submitted_marker=submitted-marker`
- `pre_approval_result_exists=false`
- `token_logged=false`

`human/result.json` 包含 4 个 `committed` step，其中 `submit_form` 恰好一次。

## 4. Cleanup 证据

- Core 正常退出并返回 0。
- `core_alive=false`
- `ui_alive=false`
- 专用 Edge 进程 `edge_remaining=0`
- fixture listener `port_8766_listener=false`
- `server/shutdown.log`：`stopped=True`

因此 TASK-367 `run-04` 对应的 post-submit cleanup 路径已在真实 Edge 上完成一次
成功重放，没有再出现 TASK-369 修复前的 cleanup 误失败。

## 5. 预审批失败保留

前两次尝试均未收到人工决定，单独保留且不计人工轮：

- attempt-01：300 秒超时，`No human decision was received within 300 seconds`
- attempt-02：900 秒窗口内审批卡可见，但未输入 `once` / 点击 Approve，操作者中止

两次都没有提交、没有 `human/result.json`。失败后现有 runner 没有自动清理 Core
启动的专用 Edge 进程树，均已手工 `taskkill /T /F` 并确认收敛。

## 6. 结论与边界

- 单轮真实 Edge cleanup 回归：**PASS**
- 该结果不改变 TASK-367 十轮历史：仍是 **9/10**
- 该结果不把 Stage 1 矩阵扩写为新的十轮成功率

遗留：`run_real_edge_ui_approval.ps1` 的失败 `finally` 仍缺少专用 Edge 清理，
建议另立窄卡修复 runner 的失败资源收敛。
