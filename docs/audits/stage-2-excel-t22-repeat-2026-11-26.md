# Stage 2 Excel T2.2 repeated write convergence audit

日期：2026-11-26
任务：TASK-409
范围：test-only real Excel COM repeated write, failure injection, ownership journal,
independent watchdog, process/session/handle convergence

## 1. 结论

TASK-409 的 test-only 真实 Excel COM 验收通过：

- 正常路径 **10/10**，每轮使用新的 `ExcelComRunner`、owned Excel process、session 和
  本地临时工作副本；
- 每轮保存后工作副本 digest 改变，shadow restore 后逐字节回到源 digest；
- 10 轮结束后源 fixture digest 不变，owned PID 与全机 `EXCEL.EXE` 均收敛为 0；
- after-write failure injection 以真实失败退出，ownership journal 保留，Drop guard
  回收 owned Excel，最终 `EXCEL.EXE=0`；
- after-open intentional hang 被 20 秒外部 watchdog 捕获；watchdog 只在 PID、creation
  FILETIME、executable path 全匹配时回收 Excel，并回收 cargo/test process tree；
- 所有临时工作副本最终清理为 0。

这不是 production Tool、ToolBus、rollback、assembly 或 production ownership journal /
watchdog 的完成声明。生产 Excel write surface 仍为空。

## 2. 运行环境

- Excel：Microsoft Office Home 2024 Retail x64，`Application.Version=16.0`，
  `Application.Build=20527`。
- 源 fixture：
  `C:\Users\fexfe\AppData\Local\Temp\task409-repeated-write-source.xlsx`。
- 源 fixture SHA-256：
  `75c44d93c9e5c8678d8585bf18347bb026fc7be0a2781a893a047ea79f4a1d12`。
- 证据根：
  `eval/tasks/excel/t22/evidence/2026-11-26-task-409/20261126T063537240Z`。

fixture 只含校准数据：`Sheet1!A1=2`、`B1=3`、`F1` 为 `=A1+B1`。

## 3. 正常 10/10

`normal/normal-summary.json` 记录 `requested_runs=10`、`passed_runs=10`。

首轮 PID `37168`，末轮 PID `37072`；10 轮 PID 均不同。`normal/runs.jsonl` 逐轮记录：

- `process_id`
- creation FILETIME (`start_time_100ns`)
- original digest
- saved working-copy digest
- restored digest

10 轮 saved digest 均与源 digest 不同；10 轮 restored digest 全部等于源 digest。每轮
close 后旧 handle 的 write/recalculate/save/read 均返回 `ToolInvalidArgs`，没有再次进入
backend。

## 4. 失败注入

`EXCEL_T22_REPEAT_FAIL_AFTER=after-write` 在真实 scalar write 成功后触发 panic：

- cargo test exit code：`101`
- marker：`failure-injection/checkpoint-run-01-after-write.ready`
- ownership journal：`failure-injection/ownership-run-01-write.json`
- owned Excel PID：`31132`
- creation FILETIME：`134401487361827885`
- executable path：`C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE`

panic 后 test-level `RepeatAcceptanceGuard` 执行 best-effort shutdown，并在完整身份仍
匹配时回收 owned Excel；若 journal 尚未发布，则先通过持有并复核的 process handle
终止。外部 harness 再次读取 journal 与独立观测记录后确认进程已退出，没有裸 PID kill。

## 5. 独立 watchdog

`EXCEL_T22_REPEAT_HANG_AFTER=after-open` 在 writable open 后故意挂起。外部 PowerShell
watchdog 在 20 秒后超时并执行：

1. 从 `watchdog-injection/ownership-run-01-write.json` 与独立
   `watchdog-observed-excel.json` 读取 owned identity；
2. 校验 process name、PID、FILETIME→UTC ticks 与 executable path；
3. 只对仍匹配的 Excel PID `39000` 持有并复核 process handle 后终止；
4. 通过 Job Object 回收本次 cargo/test child tree；
5. 等待两个 identity 与实际 `EXCEL.EXE` 全部归零；
6. 清理该 run token 对应的两个临时工作文件。

watchdog Excel termination result code 为 `0`；cargo/test child 由 Job Object 终止并
reap；最终 `EXCEL.EXE=0`，临时文件数 0。

## 6. 安全边界

- 启动前与每个场景后均检查全机 `EXCEL.EXE`；
- 只操作显式本地 `.xlsx` fixture 的临时副本；
- 拒绝 UNC 与映射网络盘；
- 不打开用户工作簿、宏工作簿、OneDrive/SharePoint 路径或既有 Excel 实例；
- 不修改 `DisplayAlerts`；
- 不对 production `tools.json`、ToolBus、rollback、assembly、schema、公共 API、
  ErrorCode 或依赖做任何扩展。

## 7. 限制

- 这是 test-only harness，不是 production COM write transaction；
- 10/10 仅证明该 fixture、该 Excel build、该本地会话和当前 runner 的重复收敛；
- watchdog 处理的是外部可观察 hang；机器崩溃、控制台会话丢失和不可验证身份不在保证内；
- production ownership journal、watchdog、ToolBus registration、rollback 与
  end-to-end L1 rollback 仍未实现；
- `.xlsm/.xlsb`、用户工作簿、多工作簿、OneDrive/SharePoint、宏、Protected View 与
  AutoSave=true 未覆盖。

## 8. 证据索引

- `normal/normal-summary.json`
- `normal/runs.jsonl`
- `failure-injection/stderr.txt`
- `failure-injection/ownership-run-01-write.json`
- `watchdog-injection/ownership-run-01-write.json`
- `watchdog-injection/watchdog-observed-excel.json`
- 根 `summary.json`
