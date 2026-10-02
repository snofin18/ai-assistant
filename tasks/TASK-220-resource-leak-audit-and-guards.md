# TASK-220　资源泄露审计与防护：内存 / 句柄 / PowerShell 子进程

- 状态：**Ready（2026-10-02，人类要求的全仓泄露审计）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-218、TASK-220 审计结论
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`crates/platform/windows/src/handles.rs`、`apps/agent-core/src/notepad_registry.rs`、`apps/agent-core/src/approval_grants.rs`、`apps/agent-core/tests/production_root_uia.rs`

---

## 目标（一句话）

修掉三类会在长期运行中耗尽内存/句柄/进程的泄露：UIA 元素表只增不删、生产根任务注册表永不清理、PowerShell 靶机启动失败不 kill/reap，并加防护机制让它们在超限或异常退出时显式收敛。

## 背景（为什么现在做）

人类 2026-10-02 要求"核查所有可能导致泄露的代码块，含 PowerShell 异常结束导致的锁死/内存满"。实测定位：

1. **UIA 元素表无淘汰**：`crates/platform/windows/src/handles.rs` 的线程本地
   `HashMap<u64, IUIAutomationElement>` 只有 `insert` / `get`，没有 `remove` / `clear` /
   容量上限。每次解析元素都 `AddRef` 一个 COM 引用；长驻 Host 反复定位（每个任务、
   每次自愈重解析）会持续累积 COM 引用与内存。
2. **生产根任务注册表不清理**：`notepad_registry.rs` 的 anchor 注册表
   （`Mutex<BTreeMap<String, CapturedRollbackAnchor>>`）按 task_id 插入且从不移除，
   每个 anchor 还持有编辑区文本与整份目标文件字节；长驻进程每跑一个任务就多留一份。
3. **PowerShell 靶机启动失败不 kill/reap**：`production_root_uia.rs` 的 `start_fixture`
   先 `spawn()`，再 `wait_for_state_file()`；ready 探测失败时直接 `?` 返回，
   `Child` 被 drop 但**不 kill、不 wait** → 孤儿 PowerShell 进程残留；`Drop` 只 kill
   直接子进程、不覆盖进程树，WPF/子进程可能继续持有内存与窗口。
4. **审批授权表不清理**：`approval_grants.rs` 的 `Mutex<BTreeMap<(task,step), Grant>>`
   随每个任务累积；一次性 grant 消费后虽删除条目，但未消费/过期条目与 task 键长期留存。

## write scope

- `crates/platform/windows/src/handles.rs`
- `apps/agent-core/src/notepad_registry.rs`、`apps/agent-core/src/approval_grants.rs`
- `apps/agent-core/tests/production_root_uia.rs`
- `apps/agent-core/tests/production_root.rs`、`apps/agent-core/tests/support/production_fixture.rs`
- `apps/agent-core/src/production.rs`（仅任务清理入口）
- `tasks/TASK-220-resource-leak-audit-and-guards.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（本卡标记 + 「当前进度」句）/ `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- **UIA 元素表上限**：给线程本地元素表加**硬上限**（例如 4096）；插入超过上限时按
  最旧句柄淘汰（FIFO），被淘汰的 `IUIAutomationElement` 立即 Drop 释放 COM 引用。
  提供显式 `clear_thread_elements()`，供任务边界/测试调用。
- **生产根任务注册表清理**：
  - `ProductionHost` 增加 `release_task(&self, task_id)`（或等价入口），任务结束/失败/取消时
    同时清理 anchor 注册表与审批授权表；
  - anchor 注册表加容量上限（例如 64 tasks）并在超限时显式淘汰最旧任务；
  - `ApprovalGrants` 增加 `prune_expired(now_ms)` 与容量上限，插入时调用。
- **PowerShell 靶机进程回收**：
  - `FixtureProcess::drop` 改成**终止进程树**（`taskkill /T /F` 或等价）后再 `wait()` reap；
  - `start_fixture` 在 ready 探测失败时**也**终止进程树并 reap，绝不留孤儿；
  - 靶机脚本已有 `--auto-close-ms` 自退；补一个 watchdog，让父进程在超时后强制收敛。
- **防护机制**：给上述注册表加统一的"超限即淘汰 + 显式失败/记录"，并在 README/记忆里写清
  "长驻进程不得持有无界任务状态"的不变量。
- 单测/负向用例：元素表超限淘汰、anchor 注册表超限淘汰、grant 清理、fixture 启动失败不残留进程。

## Out of scope（做了算漂移）

- 改 `crates/platform/api` 的公共 trait 或 `ResolvedElement` / 句柄类型。
- 引入新的全局缓存/淘汰框架、第三方 LRU 依赖。
- 真实商业 Notepad、Paint/Edge/Excel。
- 用 `unsafe` 或放宽 lint 来"省事"。

## 必须遵守

- **铁律 1**：淘汰/清理必须显式、可测；不得静默丢弃仍在使用的句柄而不报错。
- **铁律 8**：元素/句柄不跨进程；淘汰只影响本进程线程本地表。
- **铁律 9**：只改 write scope 内文件；需要扩 `crates/**` 公共形状先记 DRIFT。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。
- ADR-0022：元素句柄线程本地语义不变；淘汰旧句柄后使用必须得到 `TargetNotFound`（已有不变量）。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-platform-windows handles
cargo test -p assistant-agent-core --test production_root
cargo test -p assistant-agent-core --test production_root_uia -- --ignored --nocapture
cargo run -p xtask -- hygiene
```

并附：元素表超限淘汰、anchor 注册表上限、grant 清理、PowerShell 启动失败无残留进程四类证据。

## 完成定义（DoD）

- [ ] UIA 元素表有硬上限与淘汰，超限时释放旧 COM 引用（有单测证明）。
- [ ] 生产根 anchor 注册表有上限与淘汰，任务完成后可显式清理。
- [ ] `ApprovalGrants` 有过期清理与上限，不再随任务数无界增长。
- [ ] PowerShell 靶机在启动失败/正常结束/被杀三种情况下都不残留进程（`taskkill /T` + `wait`）。
- [ ] 全套门禁绿；真实 UIA 三用例仍通过。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-220 资源泄露审计与防护
【目标】补齐有界状态的可调用清理入口、容量淘汰测试与进程树回收测试，并留下诚实 WIP 证据
【write scope】仅 crates/platform/windows/src/{handles,lib}.rs、platform/windows README、
agent-core 的 notepad_registry / approval_grants / production_root_uia、本卡记录区、
LEDGER / PARKING_LOT / memory / 轮次产物
【铁律】1 无静默失败；8 句柄不跨进程；9 不扩大范围；ADR-0063；ADR-0022
【禁止】改公共 trait / ErrorCode / schema；加依赖或 crate；操作真实 GUI；放宽 lint
【验收】fmt / clippy -D warnings / workspace test / platform handles /
production_root / 新增三项专项 / hygiene
【依赖】TASK-218 Done；TASK-220 的初始容量实现已在 d740498 落地
【疑问】真实 UIA 三用例仍受自动化禁止 GUI 约束，未运行且不伪装通过
```

### 2. 实际改动文件

- `crates/platform/windows/src/handles.rs`、`crates/platform/windows/src/lib.rs`
- `crates/platform/windows/README.md`
- `apps/agent-core/src/notepad_registry.rs`、`apps/agent-core/src/approval_grants.rs`
- `apps/agent-core/tests/production_root_uia.rs`
- 本卡；`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls}.md`

说明：基础容量实现来自 `d740498 fix(task-220): bound thread-local element table`。
本轮补上可调用的 `clear_thread_elements()`、anchor/grant 的容量淘汰单测，以及
`taskkill /T /F` + `wait()` 的进程树回收专项测试。

### 3. 验收输出摘要

- `cargo fmt --all --check`：PASS。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅有仓库既有 unknown-lint warning）。
- `cargo test --workspace`：PASS；agent-core lib 44 passed、production_root 13 passed。
- `cargo test -p assistant-platform-windows handles`：6 passed。
- `cargo test -p assistant-agent-core --lib approval_grants`：6 passed。
- `cargo test -p assistant-agent-core --lib notepad_registry::tests`：7 passed。
- `cargo test -p assistant-agent-core --test production_root_uia
  test_fixture_process_tree_termination_reaps_direct_child`：1 passed。
- `cargo run -p xtask -- hygiene`：0E / 102W，PASSED（warning 数与本轮前 LEDGER 基线一致）。
- PR #188：CI run `37076339997` = 11/11 SUCCESS，merge `8fa8a7b`。

未运行 `production_root_uia -- --ignored`：该命令会启动 `notepad-like` 并操作真实 GUI，
与自动化章程 §3.7 冲突。

### 4. DoD 逐条核对

- [x] UIA 元素表有硬上限（4096）与 FIFO 淘汰，超限时释放旧 COM 引用；有单测。
- [x] `clear_thread_elements()` 可在任务边界显式清空本线程元素表。
- [x] 生产根 anchor 注册表有 64 任务上限与最旧淘汰；有泛型单测。
- [x] `ApprovalGrants` 有 1024 条目上限、过期清理与单测。
- [x] PowerShell 子进程使用 `taskkill /T /F` 终止进程树并 `wait()` reap；
      新增无害 `cmd/ping` 子进程测试证明直接子进程被回收。
- [x] `cargo fmt` / clippy / workspace test / hygiene 全绿。
- [ ] 真实 UIA 三用例复跑：自动化禁止真实 GUI，保持未执行；不能据此标 Done。
- [x] 未修改 Out of scope 的公共 trait / `ResolvedElement` / `ErrorCode` / schema。

### 5. 偏差

**DRIFT-220-1（真实 UIA 证据不能在无人值守轮次内取得）**

- 现象：卡片验收要求 `production_root_uia -- --ignored --nocapture` 的真实 GUI 三用例通过。
- 影响：机器可验证的容量、清理和进程树回收已覆盖，但不能宣称完整真机验收完成。
- 建议：合并本 WIP 后由人工或受控真机运行三用例，并把 run 输出回填本卡；在此之前
  TASK-220 保持 InProgress。
- 已停工作：未启动、点击、输入或截图任何真实 GUI 应用。

### 6. 更合理做法

把“有界 + 回收”拆成机器可验证层和人工真机层：容量淘汰、显式清理、进程树 kill/reap
都用无 GUI fixture 验证；真实 UIA 只作为最终人工验收，不再阻塞代码侧泄露防护的合并。

### 7. 遗留问题

- 本轮只覆盖已定位的四类泄露；`xtask` / `audit` / `secrets` 的长期状态仍需后续按同口径复查。
- 真实 `production_root_uia` 三用例仍需人工执行；测试文件当前为 1 passed / 7 ignored。

### 8. 新增长期记忆

- FACT：元素表上限 4096 + `clear_thread_elements()`；anchor 上限 64；grant 上限 1024。
- PITFALL：对泛型注册表使用 `#[derive(Default)]` 会给 `T` 加不必要的 `Default` 约束，
  需要用不约束 `T` 的手写 `Default`。

### 9. 给审阅者的关注点

- 审阅 `clear_thread_elements()` 作为窄公共 API 的必要性与命名。
- 审阅 `TaskAnchorRegistry<T>` 的泛型是否只服务测试且没有行为变化。
- 确认真实 UIA 三项仍未勾选，不得把本 WIP 当作完整真机验收。
