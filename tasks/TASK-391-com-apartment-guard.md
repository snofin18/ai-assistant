# TASK-391 Windows COM apartment ownership guard

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 COM lifecycle　批次：2
- 依赖：TASK-372、TASK-375、ADR-0063

---

## write scope

- `tasks/TASK-391-com-apartment-guard.md`
- `crates/platform/windows/src/com.rs`
- `crates/platform/windows/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把现有 UIA 专用的 `CoInitializeEx` / `CoUninitialize` 责任收敛为可复用的线程内
STA scope，证明正常返回、错误返回与 panic 路径都确定性配平释放，为后续 Excel
COM runner 提供不泄漏、不可跨线程的生命周期原语。

## In scope

- 抽取 crate-private COM apartment guard，统一 STA 初始化和配对释放。
- 增加 `with_com_apartment` crate-private scope API，只执行同步闭包。
- 保持 UIA `with_automation` 行为不变，并复用它作为唯一 apartment 入口。
- 覆盖正常返回、闭包错误、panic、线程已按 MTA 初始化四类路径。
- README 记录 COM 对象不得跨线程、apartment 必须同线程释放和当前非公共 API 边界。

## Out of scope

- 不新增公共 `platform/api` trait、类型、Excel 专用接口或 crate。
- 不创建真实 Excel COM 对象，不打开/读取/保存工作簿，不启动真实 GUI。
- 不改 protocol、Adapter、Policy、ToolBus、依赖或 CI。
- 不实现进程 ownership journal、watchdog 或 Excel Adapter runner。

## 必须遵守

1. `CoInitializeEx` 的 `S_OK` 与 `S_FALSE` 都必须恰好配平一次 `CoUninitialize`。
2. scope 返回错误或 panic 时仍必须释放 apartment。
3. 线程若已按相反 apartment model 初始化，继续显式失败。
4. guard/闭包不得跨线程移动；本卡不暴露任何 `Send + Sync` COM 句柄。
5. 不改 UIA 的线程本地实例策略，也不放宽其错误映射。

## 验收命令

```powershell
cargo test -p assistant-platform-windows com::tests -- --nocapture
cargo test -p assistant-platform-windows
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：四类 apartment 路径全部显式通过；UIA 与 workspace 无回归；无公共 API 或依赖变化。

## DoD

- [ ] 正常返回后 apartment 已释放，可立即按另一模型重新初始化。
- [ ] 闭包错误和 panic 路径都执行配对释放。
- [ ] 已按 MTA 初始化的线程调用 STA scope 显式失败。
- [ ] UIA `with_automation` 行为与线程本地实例隔离不回归。
- [ ] 不新增公共 API、依赖、Excel Adapter 或真实 GUI 行为。
- [ ] PR checks、merge 与 post-merge main CI 回填。

<!-- ─── 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ───
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤。
-->

## 执行记录

### 1. 约束回执

【任务】TASK-391 Windows COM apartment ownership guard
【目标】把 UIA 专用 STA 初始化/释放责任收敛为线程内 RAII scope，并证明所有退出路径配平。
【write scope】仅：`tasks/TASK-391-com-apartment-guard.md`、`crates/platform/windows/src/com.rs`、
`crates/platform/windows/README.md`、`plans/stage-2-excel.md`、`PLAN.md`、`README.md`、`LEDGER.md`。
【铁律】1 无静默失败；8 COM 对象不跨进程/本卡不跨线程；9 不静默扩围；10 契约先行；
ADR-0063 子进程/线程本地资源必须有界且释放。
【禁止】不新增公共 `platform/api` API、依赖、Excel Adapter、真实 GUI、工作簿 I/O 或 ownership journal。
【验收】com 单测 5 类路径全过；平台与 workspace 无回归；fmt/clippy/xtask 文档门禁全绿。
【依赖】TASK-372、TASK-375、ADR-0063，均已在 `LEDGER.md` 核对。
【疑问】无；按用户“继续下一步”直接实施。

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-391-com-apartment-guard.md`：Review 状态与执行记录，in scope。
- `crates/platform/windows/src/com.rs`：抽取 `ComApartmentGuard`；`ComApartment<T>` 将闭包结果
  与 guard 绑定，UIA 使用同一 wrapper；加线程绑定 marker 与 5 条 apartment 测试，in scope。
- `crates/platform/windows/README.md`：COM scope 不变量、测试覆盖与暂不公开边界，in scope。
- `plans/stage-2-excel.md`：新增 TASK-391 批次行并同步 Review / Done 状态，in scope。
- `LEDGER.md`：追加 TASK-391 Review / Done 事件，in scope。
- `PLAN.md` / `README.md`：merge 后同步当前状态、当前阶段和最近进展，in scope。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `cargo test -p assistant-platform-windows com::tests -- --nocapture` → **5 passed / 0 failed**：
  `S_OK`、嵌套 `S_FALSE`、闭包错误、panic、MTA 反模型五条路径全部通过。
- `cargo test -p assistant-platform-windows` → **145 passed / 0 failed / 5 ignored**；
  集成与 doctest 均通过，ignored 均为既有真机验收。
- `cargo test --workspace --no-fail-fast` → **exit 0 / 0 failed**；xtask 自身 **584 passed**。
- `cargo fmt --all --check` → PASSED。
- `cargo clippy --all-targets -- -D warnings` → **exit 0**；仅仓库既有
  `clippy::assert_is_empty` unknown-lint warning（`unknown_lints` 不受 `-D warnings` 提升）。
- 修复独立 review 的 P2/P3 后，重新执行 com 5/5、platform、workspace、fmt 与 Clippy；
  最终结果以 PR 新 commit 的 CI 为准。
- `cargo run -p xtask -- hygiene` → PASSED（0 error；195 条既有 warning）。
- `cargo run -p xtask -- card-check` → PASSED（0 error；其余为既有 warning）。
- `cargo run -p xtask -- docscan` → PASSED（0 error；175 条既有 warning）。
- `cargo run -p xtask -- refscan` → PASSED（0 error / 0 warning）。
- `cargo run -p xtask -- memory-counts` → PASSED（0 error；1 条既有 paint 归档 warning）。
- `cargo run -p xtask -- check-ledger` → PASSED。
- `git diff --check` → PASSED。
- PR #797 → **11/11 success**，pending 0 / failed 0，base=main，`MERGEABLE` / `CLEAN`；
  merge `092c79a568cc4863c19270366b62c3cb1254fcf9`。
- post-merge main CI `44695688650` → **success**。

### 4. DoD 逐条核对

- [x] 正常返回后可重新按 MTA 初始化，证明 STA 已释放。
- [x] 错误与 panic 都经 RAII 配平；另有嵌套 `S_FALSE` 专门用例。
- [x] MTA 预初始化线程调用 STA scope 明确失败，错误为 changed apartment model。
- [x] `with_automation` 仍复用线程本地 `IUIAutomation`，平台 145 条可用测试与 workspace 无回归。
- [x] `pub` 仅位于私有 `mod com`，crate 外不可达；未加依赖、Excel Adapter 或真实 GUI。
- [x] PR #797 checks 11/11 success、`MERGEABLE/CLEAN`；merge `092c79a`，
  post-merge main CI `44695688650` success。

### 5. 偏差

- `PROCESS-TASK391-SHELL-MISUSE`：本轮及交接前累计 6 次 shell 误用：5 次显式
  `shell:"bash"`（含 1 次独立 review agent），1 次在 `cmd` 回退中使用 PowerShell 的
  `Select-Object`。均未造成仓库写入；
  已统一改用 PowerShell-first、通道零输出后 `cmd` 回退的显式命令。
- `PROCESS-TASK391-PLAN-LOCK`：`plans/stage-2-excel.md` 初次加行早于 guard acquire。
  收口时在最终状态写入前补取该热点文件锁并立即释放；无并发写入或内容冲突。
- `PROCESS-TASK391-GUARD-ARGS`：`cmd` 下带空格的 guard intent 被拆成 7 个词级锁；
  未写热点文件前已全部释放，并用无空格 intent 重新正确获取 `plans/stage-2-excel.md` 与
  `LEDGER.md` 两把锁。
- `PROCESS-TASK391-GIT-MSG`：首次 commit 的带空格消息被 `cmd` 拆成 pathspec，提交未发生、
  文件未变化；改用无空格单 token message 重提。
- `PROCESS-TASK391-CLOSEOUT-BRANCH`：closeout commit 首次落在本地 `main`，尚未 push；
  已保留到 `codex/task-391-closeout`，并把本地 `main` 指回 `origin/main`。
- `PROCESS-TASK391-CLOSEOUT-REVIEW-1`：独立 closeout review 发现任务卡正文 DoD 被直接勾选、
  Stage 2 当前进度与 PLAN 当前状态块未完整同步；均在本 closeout PR 内修正，正文除状态行外
  恢复只读。
- `PROCESS-TASK391-CLOSEOUT-REVIEW-2`：最终复核又发现 PLAN 的“更新日期”和“下一步动作”
  仍停留在旧任务；已同步 TASK-391 并把下一卡改为 Excel Adapter pack。
- 无功能/契约 DRIFT。

### 6. 更合理做法（实施中发现，非漂移）

- 自审发现空 guard 会因 auto-trait 自动成为 `Send + Sync`，改为
  `PhantomData<Rc<()>>`，由编译器阻止 COM guard 跨线程移动。
- 增加同线程嵌套 STA 的 `S_FALSE` 测试，避免只覆盖新线程 `S_OK` 而漏掉 MSDN 要求的另一种成功返回。
- 独立 review 的 **P2**：原 `with_com_apartment` 允许 guard 随 `T` 单独逃逸，与释放承诺冲突。
  现将闭包结果包装为 `ComApartment<T>`，由同一对象持有 value + guard；调用方无法拆散二者，
  UIA 线程本地实例也直接复用该 wrapper。
- 独立 review 的 **P3**：修正 `ApartmentGuard::drop` 旧名与“每线程至多一个”的过期注释；
  README 如实记录测试模块只为 panic/unwind 断言放开三类 Clippy lint。

### 7. 遗留问题

- Excel COM runner、跨线程调度、进程 ownership journal/watchdog 仍属后续卡，本卡未实现。
- `!Send` 目前由 `PhantomData<Rc<()>>` 编译期保证，但没有 trybuild/compile-fail 回归；
  后续若引入负向编译测试框架，应补“删除 marker 会失败”的机器证据。

### 8. 新增长期记忆

- 无（本卡事实已记录在平台 README；未达到项目级长期记忆门槛）。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

1. `ComApartmentGuard` 的 `PhantomData<Rc<()>>` 是否足以固定“同线程构造、同线程释放”的边界。
2. `with_com_apartment` 接管闭包返回值，正常/错误/panic 三条 unwind 路径是否都只释放一次。
3. `with_automation` 把 guard 与线程本地 `IUIAutomation` 绑定后，是否完全保持原错误映射和隔离行为。
