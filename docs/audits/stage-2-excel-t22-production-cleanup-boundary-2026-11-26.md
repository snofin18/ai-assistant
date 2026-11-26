# Stage 2 Excel T2.2 production cleanup boundary audit

日期：2026-11-26
关联：TASK-410
范围：ADR-0108 D8、production `ExcelComRunner` 生命周期、TASK-409 test-only
ownership journal / watchdog evidence

## 1. 结论

ADR-0108 D8 已经把 Excel task-end cleanup 的目标契约冻结得足够具体，但 current
production 只实现到“同一 STA worker 内 best-effort Close/Quit + join”；durable
ownership journal、独立 watchdog、verified process-tree termination 与可观测收敛结果
都尚未进入 production。

因此：

- **GO**：下一卡可以开始实现 platform-private ownership journal；watchdog contract
  留到后续独立卡。
- **NO-GO**：不得据此填写生产 `tools.json`、注册 ToolBus、补齐 rollback recipe 或
  production assembly。
- **ADR clarification**：ADR-0108 的“独立线程”只能覆盖 STA 阻塞，不能覆盖 Host
  进程整体崩溃；若要求后一种恢复能力，必须明确独立 helper process / 外部 watchdog
  的启动、身份、生命周期和错误映射。

## 2. 审计来源

- `docs/adr/0108-excel-t21-readonly-tool-contract.md` D8 / D9 / D10。
- `docs/spec/excel-t21-readonly-workflow.md` §9 Cleanup。
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_windows.rs`
- `crates/platform/windows/src/excel_process.rs`
- `apps/agent-core/src/excel_t21_platform_runtime.rs`
- `crates/platform/windows/src/excel_com_runner_repeat_tests.rs`
- `eval/tasks/excel/t22/test_excel_t22_repeated_write.ps1`
- `docs/audits/stage-2-excel-t22-repeat-2026-11-26.md`

## 3. Findings

### P1：production 没有 durable ownership journal

`RunnerState::create_application` 会在创建后捕获完整 owned identity，但 identity 只保存在
内存中的 `owned_process`。Open、Close、Quit 和 worker timeout 之前都没有把
process id、creation FILETIME、executable path、workbook path、task/session 关联写入
仓库外 durable journal。

影响：Host/worker 崩溃或被强制终止后，下一次启动没有可信的 owned identity 可用于
核对是否应清理遗留 Excel；当前只能返回“requires process-level cleanup”文本，无法执行
或证明清理。

### P1：production 没有独立 watchdog

`ExcelComRunner::shutdown()` 在 worker 30 秒无响应后返回 `Fatal` 并 detach worker，
但没有另一条独立执行路径读取 owned identity 并回收进程。`Drop` 仍尝试在同一 worker
发送 Shutdown，只忽略 join 结果，无法覆盖 STA 阻塞、receiver 消失或进程退出竞态。

影响：危险点正好发生在 Close/Quit 阻塞时，当前 shutdown 能报告失败但不能保证 owned
Excel 收敛。

### P1：收到 shutdown 响应后的 worker join 仍是无界的

30 秒超时只覆盖主线程等待 worker 响应；一旦收到响应，
`ExcelComRunner::shutdown()` 调用 `join_worker()`，其 `worker.join()` 没有第二层
deadline。worker 若在发送响应后卡住，task runtime 仍会无限期阻塞；`Drop` 也有同类
同步等待风险。

影响：当前并不是完整的 bounded wait + reap。后续实现必须分别约束“请求响应”和
“最终 join/reap”，不能把 30 秒 response timeout 当成整个 cleanup 的有界证明。

### P1：cleanup 错误码与 ADR-0108 D9 冲突

`RunnerState::shutdown()` 在 `Workbook.Close(false)` 失败时无条件包装为 `Fatal`，
即使原始错误是 `UserInteraction`；`Application.Quit()` 失败则在内层原样返回。
`RPC_E_DISCONNECTED` 内层会先映射为 `TargetUnresponsive`，但公共
`ExcelComRunner::shutdown()` 的 response error 分支会再把该错误改写为 `Fatal`；
`E_ACCESSDENIED` 仍会以 `PlatformPermission` 透出。

影响：违反 D9 的“cleanup 结构失败为 `Fatal`，应用对话框或人工拒绝为
`UserInteraction`”口径。后续实现必须明确内层/公共边界的最终错误，并至少修正
`UserInteraction` 被包装为 `Fatal`、`E_ACCESSDENIED` 以 `PlatformPermission` 透出的
冲突；负向测试必须断言公共 `shutdown()` 结果，不能只验证它返回 `Err`。

### P1：timeout 后没有 verified process-tree termination

`excel_process.rs` 已具备 Toolhelp 枚举、PID/start time/path 捕获与 identity match，
但没有 terminate API、Job Object 绑定、整棵进程树终止或有界 reap。现有
`terminate_verified_process` 只存在于 TASK-409 test module，不是 production。

影响：即使 watchdog 将来读到 journal，也缺少 production 级“三项身份复核后终止”实现。

### P1：production task-end caller 仍不存在

`PlatformExcelT21Runtime` 位于 agent-core test-only adapter 中，文件头明确不实现
ownership journal、watchdog 或 process-tree kill；`shutdown()` 只把
`ExcelComRunner::shutdown()` 的错误转成领域错误并清空 workbook map。

影响：当前没有真实 task runtime 在成功、失败、取消、TTL 到期四条路径统一触发 cleanup；
更不存在全局 8 active session / task 结束立即失效的生产装配。

### P2：test-only runtime 的 closing 顺序和失败收敛不符合 D8

`PlatformExcelT21Runtime::shutdown()` 先持有 mutex 调用 `runner.shutdown()`，返回后
才清空 workbook map 并设置 `is_terminal=true`。这意味着 cleanup 期间的并发调用会先等待
长 cleanup，而不是在开始时立即拒绝；cleanup 返回错误时仍无条件删除 active session，
没有满足 D8 的“确认进程树收敛后再删除 journal / active session”。

影响：当前 adapter 只能作为 private mapping 参考，不能直接晋升为 production task
runtime。后续生产状态机必须先进入 closing 状态、拒绝新调用，再在 cleanup 结果确认后
决定 session 是否删除及 incident 如何发布。

### P2：Drop 是 best-effort 且不可观测

`Drop for ExcelComRunner` 在发送 Shutdown 成功后才 join，并通过 `let _join_result`
丢弃 join 结果，也不验证 Excel PID 是否退出。

影响：显式 `shutdown()` 以外的析构路径不能提供 postcondition，也无法在 incident 中
记录 cleanup 失败。生产实现需要把不可报告 Drop 与可报告 task-end cleanup 明确分层。

### P2：完整 identity 已公开在 workbook handle，但 application PID API 只暴露 PID

runner 在 create 后捕获完整 identity，workbook handle 也保留完整 identity；
`ExcelWorkbookHandle::owned_process()` 已按 ADR-0112 公开完整 PID、creation FILETIME
和 executable path。`application_process_id()` 只返回 PID，但不是唯一 ownership surface；
真正的缺口是生产 journal 尚未定义，因此不存在
“journal before risky Open/Close/Quit”的落盘点。

影响：不能把 `application_process_id()` 当作 ownership evidence；后续 journal 应复用
现有完整 identity，不得为此重新扩公共接口，也不得新增裸 PID kill 路径。

### P2：Excel ownership 的建立仍存在检查/创建竞态

`ensure_no_existing_excel_process()` 是 `CoCreateInstance` 前的全机快照检查，之后没有
process creation handle、Job 绑定或其他原子证据证明新 Excel 由本次 runner 创建。用户
Excel 可能在检查与创建之间启动，COM 也可能复用既有实例。

同时 `process_identity()` 把任意 `OpenProcess` 失败都折叠为 `Ok(None)`；访问拒绝会被
误报为“进程已消失”，而不是显式权限/身份校验失败。

影响：在原子 ownership 证据建立前，即使 journal、watchdog 和 terminate 都正确，也无法
证明 kill 的目标一定是本 runner 新建的 Excel。后续实现必须先解决 creation race 与
OpenProcess 错误分类。

### P2：TASK-409 harness 不可直接移植

TASK-409 的外部 PowerShell watchdog、Job Object、`.ready` checkpoint、`.jsonl`
evidence 与 cargo/test child 绑定都是验收工具语义：

- wrapper 进入 Job 后才 spawn cargo/test；
- 只清理 journal 身份匹配或仍属于 Job 的 Excel；
- watchdog 读取 journal 后通过已打开并复核的 process handle 终止；
- Job 成员有界轮询到空后再关闭。

这些语义可用于 production 设计参考，但 PowerShell 脚本、测试 checkpoint 和 cargo
子进程结构不得成为产品运行时依赖。

### P3：cleanup 失败只有文本增强，没有结构化收敛报告

当前 shutdown 失败通过组合 `PlatformError` 消息表达 Close/Quit/join 失败，没有结构化
记录“session closing、journal 是否保留、owned identity 是否匹配、进程树是否收敛、
临时文件是否清理”。

影响：后续卡若需要 incident / audit event，应先在 platform-private 结构中定义报告，
再决定是否扩展公共 API；不得本卡直接改 `platform/api`。

### P3：同一路径的重复原子 publish 不能直接执行

TASK-409 的 `.tmp + sync_all + rename` 依赖每次 journal 目标路径唯一。Windows
`std::fs::rename` 在目标已存在时通常失败，因此若 production 在同一路径重复发布
`opening/closing/cleanup` 状态，必须先定义唯一文件名或显式 replace/remove 语义；
不能把测试里“每个 phase 一个唯一文件”的做法模糊成通用原子覆盖。

## 4. ADR-0108 D8 覆盖矩阵

| D8 步骤 | Current production | TASK-409 harness | 结论 |
|---|---|---|---|
| 1. session mark closing，拒绝新调用 | shutdown 前不先进入 closing；无 production task assembly | test guard 在异常后停止 runner | production gap |
| 2. Open/Close/Quit 前写 durable journal | 缺失 | `.tmp + sync_all + rename` | production gap |
| 3. 独立 watchdog，不共享 STA 阻塞路径 | 缺失 | 外部 PowerShell watchdog | production gap |
| 4. STA 释放 Range/Worksheet 子对象 | backend 通过 typed Drop/局部对象释放 | 真实 COM 10/10 间接验证 | 部分 |
| 5. 同 STA `Workbook.Close(false)` | 已实现 | 已验证 | 已实现 |
| 6. 同 STA `Application.Quit()` | 已实现 | 已验证 | 已实现 |
| 7. 最后释放 Workbook/Application 引用 | backend `Option = None`，无显式 `FinalReleaseComObject` | 重复运行最终进程归零 | 部分 |
| 8. watchdog/runtime bounded wait + reap | 仅 response timeout；最终 join 无界；无 process tree reap | Job Object + handle terminate + poll | production gap |
| 9. 收敛后删 journal / active session | 无 journal | journal complete / 临时文件 0 | production gap |
| 10. Open/Close/Quit 阻塞、owned-only kill、不增长 | 部分 fake tests | after-write、after-open 20s、10/10 | test-only only |

## 5. ADR-0108 D9 cleanup 错误映射矩阵

| cleanup 条件 | 当前行为 | D9 期望 | 结论 |
|---|---|---|---|
| workbook close 返回 `UserInteraction` | 包装为 `Fatal` | `UserInteraction` | 冲突 |
| workbook close 返回结构/协议失败 | 包装为 `Fatal` | `Fatal` | 表象一致，但丢失原始原因 |
| application quit 返回 `RPC_E_DISCONNECTED` | 内层 `TargetUnresponsive`，公共 response error 分支改写为 `Fatal` | cleanup 结构失败应为 `Fatal` | 公共码一致，但原因链与测试口径需显式 |
| application quit 返回 `E_ACCESSDENIED` | 公共 `PlatformPermission` | cleanup 结构失败应为 `Fatal` | 冲突 |
| 收到 shutdown 响应后 worker 不再退出 | `worker.join()` 无界等待 | bounded wait + reap | 缺口 |
| 未收到 shutdown 响应 | 30 秒后 `Fatal` 并 detach | `Fatal` + verified cleanup | 部分 |

## 6. 风险排序

1. **P1 durability**：无 journal 时任何进程崩溃都失去可信清理身份。
2. **P1 liveness**：无独立 watchdog 时 Close/Quit 阻塞只能报错，不能收敛。
3. **P1 boundedness**：response timeout 后仍可能被无界 join 卡住。
4. **P1 error semantics**：cleanup 错误码会把结构失败伪装成可重试目标错误。
5. **P1 termination safety**：无 production verified termination，不能安全清理。
6. **P1 lifecycle integration**：没有真实 task-end caller，所有 cleanup 实现都没有
   生产触发点。
7. **P2 ownership proof**：check-then-create 竞态与 `OpenProcess` 错误折叠会破坏
   “只清理 owned Excel”的前提。
8. **P2 state machine**：test-only registry 未先 closing，失败时提前删 active session。
9. **P2 observability**：Drop 静默、缺少结构化 cleanup report。
10. **P2 portability**：测试 harness 依赖 PowerShell/cargo/Job 结构，不可直接复用。

## 7. 决策

### GO

下一卡允许在 `assistant-platform-windows` 内实现 **platform-private** durable
ownership journal 与 bounded journal schema，仍然不注册 Tool，不修改公共
`ExcelComRunner` 方法面，除非独立 review 证明必须的写入点无法在现有边界内完成。

### NO-GO

以下动作继续禁止：

- 填写 `adapters/com.microsoft.excel/tools/tools.json`；
- 注册 `excel.workbook.open/read` 或其他 production ToolBus Tool；
- 把 TASK-409 PowerShell watchdog 直接打包为产品依赖；
- 以裸 PID 调用 `taskkill` 或终止未复核身份的 Excel；
- 把“test-only 10/10”宣称 production task cleanup 完成。

### ADR clarification

后续 ADR/spec 必须回答：

1. “独立线程 watchdog”是否仅用于 STA hang；
2. 是否要求覆盖 Host 进程崩溃；若要求，独立 helper process 如何启动、发现、认证与回收；
3. journal 仓库外根目录、权限、原子写、残留清理与多 session 隔离；
4. cleanup 报告是否只进 platform-private 结构，还是需要公共 API/审计事件扩展。

## 8. 后续实现顺序

### TASK-411：platform-private ownership journal

- 固定 schema version 1 与硬长度上限；
- 复用完整 `ExcelOwnedProcessIdentity`，禁止裸 PID；
- `.tmp + create_new + write_all + sync_all + rename` 原子发布，且每个 runner/session/
  phase 使用唯一目标文件名；不得把 Windows rename 当作通用 replace；
- 在 create、Open、Close、Quit 前后选择精确持久化点；
- fake tests 覆盖 io failure、目标已存在、stale identity、bounded cleanup 和文件命名冲突；
- 补 cleanup 错误映射矩阵与负向测试，区分 `UserInteraction`、`Fatal` 与原始原因保留；
- 不新增依赖、不改 ToolBus/tools.json。

### TASK-412：watchdog contract + verified termination

- 定义不共享 STA 阻塞路径的 watchdog contract；
- 解决 `CoCreateInstance` 前检查与 owned process 建立之间的竞态，并区分
  `OpenProcess` 的“不存在”和“访问拒绝”；
- 完整身份复核后才终止；
- response wait、join/reap、termination polling 三层都设置独立 deadline；
- bounded wait + reap，返回结构化 cleanup report；
- 若必须跨 Host 进程存活，先完成 ADR clarification；
- 只做 process termination primitives，不接 production Tool registration。

### 后续集成卡

- 将 journal/watchdog 接入真实 task runtime 的成功、失败、取消、TTL 到期；
- 人类在场执行 Open/Close/Quit hang 与 Host/child 崩溃注入；
- 证明 session、workbook、Excel process、journal、临时文件均收敛；
- 全部通过后，另行派发 production `tools.json` / rollback / assembly / ToolBus 卡。

## 9. 非目标

- 本审计不启动 Excel、不修改文件系统 journal 根目录、不验证真实 watchdog。
- 本审计不新增依赖、公共 API、schema、ErrorCode、Tool、Job Object 或 helper process。
- 本审计不判断 T2.2 production write Tool 的最终 approval/rollback JSON。

## 10. 验证

```powershell
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
git diff --check
```

预期：全部 PASSED；仅有仓库既有 warning；产品代码与依赖零差异。
