# TASK-223　L1 文件通道 `read_utf8_prefix` 作为 binary 层保留 host_service

- 状态：**InProgress（2026-10-03 WIP；完整大文件 Plan 执行被 DRIFT-223-1 阻断）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-219、**ADR-0064 Accepted**
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0061-runtime-task-dataflow-and-resumable-approval.md` D5、`docs/adr/0064-*`、`adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`

---

## 目标（一句话）

让 T1.1 的**大文件路径**真正可用：为 `l1_file / read_utf8_prefix` 提供一个 binary 层保留的
`host_service` 工具（读 UTF-8 前缀并标 `truncated`），使 1 MB 文件走文件通道降级而不是被静默跳过。

## 背景（为什么现在做）

TASK-219 让 `pure` 步骤真实执行后，T1.1 仍只提交 `read_text` + `analyze` 两步；
`read_file_channel` 是 `kind = "l1_file"`，按 **ADR-0061 D5** 属"不由运行时执行"的
文件通道，当前被 `render_step` 静默跳过（只记 `declared_not_executed`）。

**这里必须纠正一个先前的口头建议**：我曾建议"把 L1 文件通道接入运行时"，但
ADR-0061 D5 明确把 `l1_file` 归给"文件通道 / 工具化"，**不属于**运行时步骤种类。
正确做法不是把 `l1_file` 塞进运行时，而是**另立一个 binary 层保留 `host_service` 工具**
（与 `inspect_target_path` / `set_editor_value` 同型），让任务包用它表达文件通道降级。
按铁律 10，先有 ADR-0064，再实现。

## write scope

- `docs/adr/0064-*.md`、`docs/adr/README.md`、`docs/memory/decisions.md`
- `apps/agent-core/src/runtime_tools.rs`、`apps/agent-core/src/reserved_invoker.rs`、`apps/agent-core/src/reserved_host.rs`、`apps/agent-core/src/notepad_registry.rs`、`apps/agent-core/src/runtime_host_ops.rs`
- `apps/agent-core/tests/task_package.rs`、`apps/agent-core/tests/production_root.rs`
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`（`read_file_channel` 改声明）
- `tasks/TASK-223-l1-file-channel-read-utf8-prefix.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（本卡标记 + 「当前进度」句）/ `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- ADR-0064：授权"文件通道以 binary 层保留 `host_service` 工具表达"，**不改 `l1_file` 的语义**
  （它仍是任务前置/通道标记，不是运行时步骤种类），并说明与 ADR-0061 D5 的关系。
- 新增保留工具 `assistant.runtime.host_read_utf8_prefix`：输入绝对路径 + `max_text_bytes`，
  按 UTF-8 边界读前缀（不得在码点中间截断），输出 `text` / `truncated` / `bytes_read` /
  `bytes_total` / `fingerprint`；路径必须是绝对路径且不含 `..`；文件缺失 → `TargetNotFound`。
- 任务包 `read_file_channel` 改声明为 `kind = "host_service"` + `operation = read_utf8_prefix`，
  保留 `when: file_size_bytes > max_text_bytes` 与 `outputs: ["text", "truncated"]`。
- 负向用例：相对路径/含 `..` 拒绝；文件缺失 `TargetNotFound`；UTF-8 多字节边界不截断；
  `max_text_bytes` 非正数拒绝。

## Out of scope（做了算漂移）

- 改 `crates/**` 公共接口、`PlanStep` / `Plan` / `Planner` 形状。
- 把 `l1_file` 定义成运行时步骤种类（那会违反 ADR-0061 D5）。
- 真实 LLM / 商业应用 / 网络。

## 必须遵守

- **铁律 1**：读失败、非绝对路径、非正预算一律显式失败。
- **铁律 10**：ADR-0064 Accepted 前不写实现。
- **铁律 9**：只改 write scope 内文件。
- ADR-0063：读文件不得引入无界缓冲（只读前缀，不整文件入内存）。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test task_package
cargo test -p assistant-agent-core --test production_root
cargo run -p xtask -- docscan
```

并附大文件走文件通道、UTF-8 边界不截断、三类负向四条证据。

## 完成定义（DoD）

- [ ] ADR-0064 落档并 Accepted（人类裁决）。
- [ ] `assistant.runtime.host_read_utf8_prefix` 实现并通过负向用例。
- [ ] T1.1 大文件用例在 Plan 中走 `host_service` 文件通道分支。
- [ ] 读取只取前缀，`truncated` 显式，不把整文件读入内存。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-223 L1 文件通道 read_utf8_prefix
【目标】以 binary 层保留 host_service 表达文件通道，支持有界 UTF-8 前缀读取并显式 truncated
【write scope】仅：card 列出的 ADR / agent-core / adapter / 状态与记忆文件
【铁律】1 无静默失败；9 不静默扩大范围；10 ADR-0064 Accepted 后实现；ADR-0061 D5；ADR-0063
【禁止】改 crates/**、PlanStep/Plan/Planner 形状、把 l1_file 变成运行时步骤、整文件读入内存
【验收】fmt / clippy / workspace tests / task_package / production_root / docscan
【依赖】TASK-219 Done；ADR-0064 已按用户授权 Accepted
【疑问】完整大文件执行需要改 runtime_binding 条件跳过语义，超出卡面 write scope；按规则记 DRIFT-223-1，不越权修改
```

### 2. 实际改动文件

- `docs/adr/0064-l1-file-channel-as-reserved-host-service.md`（新增）
- `docs/adr/README.md`、`docs/memory/decisions.md`（登记 ADR-0064）
- `apps/agent-core/src/runtime_tools.rs`、`reserved_invoker.rs`、`reserved_host.rs`
- `apps/agent-core/src/runtime_host_ops.rs`、`notepad_registry.rs`
- `apps/agent-core/tests/task_package.rs`、`production_root.rs`
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`
- 本卡；`LEDGER.md` / `docs/PARKING_LOT.md` / `docs/memory/{facts,pitfalls}.md`

### 3. 验收输出摘要

- `cargo fmt --all --check`：PASS。
- `cargo clippy --all-targets -- -D warnings`：PASS（仅既有 unknown-lint warning）。
- `cargo test --workspace`：PASS（agent-core 42 unit + 13 production_root + 15 task_package 等全绿）。
- `cargo test -p assistant-agent-core notepad_registry::tests`：PASS（6/6，含 1 MiB 前缀读取）。
- `cargo run -p xtask -- docscan`：PASS（0 error / 354 warning）。

### 4. DoD 逐条核对

- [x] ADR-0064 落档并 Accepted（用户授权自动化代为裁决）。
- [x] `assistant.runtime.host_read_utf8_prefix` 实现并通过负向用例。
- [ ] T1.1 大文件用例完整执行走 `host_service` 文件通道分支：Plan 已进入 host_service
      分支，但完整执行被 DRIFT-223-1 阻断。
- [x] 读取只取前缀，`truncated` 显式，不把整文件读入内存。
- [x] 未修改 Out of scope 的 `crates/**`、`PlanStep` / `Plan` / `Planner`。

### 5. 偏差

**DRIFT-223-1（完整大文件 Plan 执行被运行时条件语义阻断，超出 write scope）**

- 现象：T1.1 的第一个可执行步骤是 `read_text`，其 `when` 在大文件输入下为假；
  `BindingInvoker` 在“条件为假且尚无 fingerprint”时返回 `VerifyFailed`，因此不会继续到
  `read_file_channel`。反向把文件通道步骤放前面也会让小文件分支撞同一限制。
- 附带现象：被条件跳过的保留步骤仍会走 verify/commit，`production_root` 的 T1.1
  快照数从 2 变成 3；`production_root_uia.rs` 的 ignored 真机测试仍写死 2。
- 影响：本轮能验证保留工具、有界读取、UTF-8 边界、Plan 映射与负向用例；不能声称
  “1 MB 文件完整 Plan 执行成功”。
- 建议：另立 ADR/卡定义“条件为假的第一个步骤”是否允许无 fingerprint 跳过，以及
  跳过步骤是否提交；随后同步 ignored UIA 断言。具体写入 **PL-100**。
- 已停工作：未修改 `runtime_binding.rs`、`production_root_uia.rs`、`docs/spec/**`。

### 6. 更合理做法

用 `host_service` 表达文件通道，而不是扩展运行时步骤种类——前者只动 binary 层，
后者要改 `PlanStep`/`planner` 公共形状并违反 ADR-0061 D5。

### 7. 遗留问题

- 本卡起因是纠正"把 L1 通道接入运行时"这一与 ADR-0061 D5 冲突的口头建议；正式落点以 ADR-0064 为准。
- **PL-100**：补齐条件为假的第一个步骤 / 跳过步骤提交语义，然后完成 T1.1 大文件 Plan 真
  执行与 ignored UIA 证据更新。

### 8. 新增长期记忆

- FACT：ADR-0064 保留工具与 1 MiB 有界前缀读取已实现并通过单测。
- PITFALL：第一个可执行步骤若条件为假且无 fingerprint，binding 会 fail-closed；
  跳过步骤当前仍提交，改变快照数。

### 9. 给审阅者的关注点

- 关注 `host_read_utf8_prefix` 的 UTF-8 边界回退和 16 MiB 硬上限是否满足安全要求。
- 关注 T1.1 Plan 现在包含 3 个可执行步骤（read_text、文件通道、analyze）这一可见变化。
- 关注 DRIFT-223-1：完整大文件执行仍未闭环，不应把本 WIP 标为 Done。
