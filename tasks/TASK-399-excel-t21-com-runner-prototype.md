# TASK-399 Excel T2.1 private COM runner prototype

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：4e　依赖：TASK-391、TASK-395、TASK-398、ADR-0108、ADR-0110

---

## write scope

- `tasks/TASK-399-excel-t21-com-runner-prototype.md`
- `docs/adr/0110-excel-t21-implementation-order.md`
- `docs/adr/README.md`
- `docs/spec/excel-t21-readonly-workflow.md`
- `docs/memory/decisions.md`
- `MEMORY.md`
- `crates/platform/windows/src/lib.rs`
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在不创建 Excel、不读取真实工作簿、不暴露生产 Tool 的前提下，交付 Excel T2.1
private COM runner 原型：dedicated worker thread 内持有 COM 状态，请求按 FIFO
串行化，队列有界，fake backend 覆盖成功、失败、关闭和线程归属；Windows 后端编译
真实 IDispatch 创建、`Application.Hwnd` 读取和 `Quit` 调用，但测试不执行它。

同时用 ADR-0110 修正 spec §11 的顺序冲突：ADR-0108 D10 优先，生产 `tools.json`
必须继续保持空。

## In scope

- 新增 test-only crate-private `excel_com_runner` 模块，仅由同模块 fake tests 调用。
- `ExcelComBackend` 私有 trait：创建 Application、读取 owned PID、显式 Quit。
- `ExcelComRunner<B>`：dedicated thread、`sync_channel(1)` 有界请求、逐请求 response、
  严格 FIFO、显式 `shutdown()` 和线程 join。
- fake backend 记录创建/读取/Quit 顺序、执行线程和每个阶段只执行一次的契约。
- Windows 真实后端：`CLSIDFromProgID("Excel.Application")`、
  `CoCreateInstance::<_, IDispatch>`、`Application.Hwnd` property get、
  `GetWindowThreadProcessId`、`Application.Quit` method invoke。
- 真实后端只做 construction/empty-state 编译测试，不调用 Create，不启动 Excel。
- ADR-0110、spec §11 修正、平台 README、Stage 2/PLAN/README/LEDGER 同步。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口。

## Out of scope

- 不修改 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipes 或 ToolBus 注册。
- 不新增或修改 `assistant-platform-api` trait、protocol schema、ErrorCode、依赖、CI。
- 不修改 `apps/agent-core` 的 test-only Excel handlers/wrappers 或 production assembly。
- 不创建 Excel COM 实例，不打开/读取/保存 `.xlsx`，不执行真实 GUI。
- 不实现 ownership journal 文件、独立 watchdog 进程或真实 process-tree kill。
- 不把 test-only runner prototype 写成生产可用或四 Tool 已生效。

## 必须遵守

1. COM 对象只在 worker thread 内创建、调用和释放；不得进入 `Send + Sync` 公共类型。
2. 请求通道容量固定为 1；full、disconnected、thread panic、backend failure 均显式失败。
3. FIFO 顺序必须由 fake tests 证明；不许并发调用 backend。
4. `shutdown()` 必须停止接收新请求、完成当前请求、join worker；重复 shutdown 显式失败。
5. Windows 后端必须在 `with_com_apartment` 内运行整个 worker loop，不能把 guard 跨线程。
6. `Hwnd` 只经 `GetWindowThreadProcessId` 映射 PID；不得把 PID/HWND 当 opaque session。
7. 真实后端测试不得调用 Create/Quit；只验证空状态与类型边界。
8. 生产 `tools.json`、rollback、production assembly 与 ToolBus 保持零改动。
9. 无生产 caller 的 runner 只能 test-only 编译；不得加 `#[allow(dead_code)]`。

## 验收命令

```powershell
cargo test -p assistant-platform-windows excel_com_runner
cargo test -p assistant-platform-windows com::tests
cargo test -p assistant-platform-windows
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo deny check
git diff --check
```

预期：runner fake 矩阵与 apartment 回归全绿；workspace/门禁全绿；Windows 真实后端
仅编译不执行；Excel `tools.json` 仍为 `tools: []`，无 Adapter/rollback/公共 API 差异。

## DoD

- [ ] runner 在 dedicated thread 内串行处理请求，FIFO 和有界队列均有负向测试。
- [ ] fake 成功路径证明 Create → ProcessId → Quit 顺序与单次执行。
- [ ] backend Create、ProcessId、Quit 失败均原样返回 `ErrorCode`，不降级。
- [ ] disconnected/full/panic 路径显式失败；shutdown 恰好一次并完成 join。
- [ ] Windows 后端真实 IDispatch 代码编译，但测试不启动 Excel。
- [ ] ADR-0110 与 spec §11 消除顺序冲突，生产 `tools.json` 仍为空。
- [ ] 无公共 API、依赖、Adapter、rollback、production assembly、ToolBus 差异。
- [ ] 独立 review 无未关闭 P0/P1/P2；PR/CI/merge/post-merge main CI 回填。

<!-- ───────── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────────
     以下由 Implementer 填写。状态行仅按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-399 Excel T2.1 private COM runner prototype
【目标】交付 test-only 私有 COM runner 的线程/队列/顺序/关闭与真实 IDispatch 编译边界，并修正 spec 顺序
【write scope】仅：本卡、ADR-0110/索引/spec/decisions、platform-windows runner 与 README、Stage 2/PLAN/README/LEDGER
【铁律】COM 不跨线程；资源有界并 join；无静默失败；契约先行；不扩公共接口；不把原型冒充生产
【禁止】不创建 Excel；不碰 tools.json/rollback/ToolBus/assembly/protocol/公共 API/依赖；不读真实 .xlsx
【验收】runner fake 矩阵、apartment、platform/workspace、fmt/clippy/schema/codegen/xtask/deny/diff-check 全绿
【依赖】TASK-391/395/398 Done，ADR-0108/0109/0110；按 ADR-0108 D10 优先
【疑问】无
```

### 2. 实际改动文件

- `docs/adr/0110-excel-t21-implementation-order.md` / `docs/adr/README.md` /
  `docs/memory/decisions.md` / `MEMORY.md`：记录 ADR-0110 与派生计数。
- `docs/spec/excel-t21-readonly-workflow.md`：加入 ADR-0110 normative correction，
  明确生产 Tool surface 最后开放。
- `crates/platform/windows/src/lib.rs` / `excel_com_runner.rs` /
  `excel_com_runner_tests.rs`：test-only private runner、Windows IDispatch backend、
  capacity-1 FIFO、failure/panic/disconnect/cleanup/shutdown 回归。
- `plans/stage-2-excel.md` / `PLAN.md` / `README.md` / `LEDGER.md`：Review 状态和
  ADR-0110 下一步顺序同步。
- `adapters/**`、`protocol/**`、`apps/agent-core/**`、production assembly、ToolBus、
  rollback 与依赖零改动。

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows excel_com_runner -- --nocapture` →
  **8 passed / 0 failed**。
- `cargo test -p assistant-platform-windows --test handle_discipline` →
  **7 passed / 0 failed**。
- `cargo test --workspace` → PASSED；agent-core lib 128、main 61、xtask 587，
  Windows platform 152 passed + 5 ignored。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo fmt --all --check` / `git diff --check` → PASSED。
- `verify-schemas` 7/7、`codegen --check` 0 drift、arch 10/10、`cargo deny check`
  advisories/bans/licenses/sources ok。
- `adr-index` / `memory-counts` / `card-check` / `docscan` / `refscan` /
  `check-ledger` / `hygiene` / `check-comments` → 全部 PASSED；本卡无新增
  hygiene/comment warning。
- 独立 review 三轮：首轮 2 P1 + 3 P2；第二轮确认主要 P1 关闭后剩 1 P1 + 2 P2；
  第三轮 **no findings**。
- Implementation PR #823 checks **22**（21 SUCCESS + 1 SKIPPED），pending 0 / failed 0，
  base=main，`MERGEABLE` / `CLEAN`；squash merge
  `b27ff85fb73182f51d6ffc7db997fc6ee5523fa1`；post-merge main CI
  `45338066396` success。
- 首轮 PR CI 在 Ubuntu/macOS 暴露 Rust 1.100 `redundant_clone`；已删除三个无效
  clone 后重跑，最终两套 PR run 22 checks 全绿。

### 4. DoD 逐条核对

- [x] runner 在 dedicated thread 内串行处理请求；capacity-1 queue-full 经
  `dispatch()` 断言 `TargetUnresponsive`。
- [x] fake 成功路径证明 Create → ProcessId → Quit 顺序与 worker thread identity。
- [x] backend Create/ProcessId/Quit 失败均原样返回 ErrorCode。
- [x] disconnect/panic/重复 shutdown 显式失败；成功 shutdown 完成 join，response
  timeout 不执行无界 join。
- [x] Windows 真实 IDispatch 代码编译；测试只验证 idle backend，不创建 Excel。
- [x] ADR-0110/spec 与 PLAN/Stage/README 顺序一致，生产 `tools.json` 仍为空。
- [x] 无公共 API、依赖、Adapter、rollback、production assembly、ToolBus 差异。
- [x] 独立 review 三轮最终 no findings；PR #823 / CI / merge / post-merge main CI
  已回填。

### 5. 偏差

- 独立 review 首轮：shutdown response 超时后仍无条件 join，可能永久阻塞；已改为
  response 成功才 join，超时返回 `Fatal` 并显式 detach。
- 独立 review 首轮：请求方 response receiver 丢失时只释放 COM 引用而不 Quit；已让
  worker 对已创建 Application 执行 best-effort Quit cleanup，并补专项回归。
- 独立 review 首轮：PLAN/Stage/README 仍指向直接 production assembly；已同步为
  ADR-0110 顺序。
- 独立 review 首轮：queue-full 测试绕过 `dispatch()`；已改为通过 `dispatch()` 验证
  `TargetUnresponsive`。
- `PROCESS-TASK399-SHELL-MISUSE`：本轮存在只读/验证命令误填 `shell="bash"`，均由
  项目 hook 实际改写为 PowerShell 执行；无仓库越界写入或外部副作用。机器重算：
  主会话 **95** 次、独立 review **18** 次，合计 **113** 次。

### 6. 更合理做法

- runner 作为 `#[cfg(test)]` prototype 落地，避免无生产 caller 时用
  `#[allow(dead_code)]` 掩盖死代码，同时真实 Windows backend 仍参与 Windows test
  编译。
- shutdown request 是唯一允许同步 join 的路径；其他 timeout 只返回明确失败，
  不把线程无法收敛伪装成成功。

### 7. 遗留问题

- 该 runner 尚无生产 caller；下一步若进入真实 COM/人类在场验收，必须先裁决
  injection boundary 或 assembly 变更。
- ownership journal、独立 watchdog、真实 process-tree kill、真实 `.xlsx` 与重复
  收敛仍未实现。
- readiness timeout 时 worker 只能显式 detach；真正生产版必须把该情形接入进程级
  回收或独立 watchdog。

### 8. 新增长期记忆

- ADR-0110 已落盘；未新增额外 FACT/PITFALL。ADR 决策由 decisions + ADR 索引承载。

### 9. 给审阅者的关注点

- shutdown 成功 join 与 timeout detach 的边界是否清晰，错误是否始终显式。
- response receiver 丢失后的 best-effort Quit cleanup 是否足够，且不会误碰未创建
  Application。
- Windows IDispatch/VARIANT 清理是否仍在 worker thread 内完成，未泄漏或跨线程。
- `tools.json`、rollback、production assembly、ToolBus、公共 API 与依赖是否保持零改动。
