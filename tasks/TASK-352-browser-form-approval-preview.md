# TASK-352 Browser form approval preview and attribution

- 状态：**Review**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：344、346、347、348、349、351　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

**目标**

在 `form_read` 的 before 状态与 `form_fill` 的 readback after 状态之间，生成可由审批层消费的字段 diff，并携带经过校验的 `InstructionAttribution`；不接 UI 事件、不消费审批、不启动真实浏览器。

**write scope**

- `apps/agent-core/src/browser_task_approval.rs`
- `apps/agent-core/src/browser_task_approval_tests.rs`
- `apps/agent-core/src/browser_form_read.rs`
- `apps/agent-core/src/browser_form_fill.rs`
- `apps/agent-core/src/browser_task_runtime.rs`
- `apps/agent-core/src/browser_task_runtime_tests.rs`
- `docs/memory/facts.md`
- `tasks/TASK-352-browser-form-approval-preview.md`
- `plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`MEMORY.md`：仅状态与进度同步

**In scope**

1. 新增内部 `FormApprovalPreview`，只接受同一 form 的 `form_read` before 与 `form_fill` readback after。
2. 校验 origin、action、字段 selector 顺序、字段数量与值边界；不一致时 fail-closed。
3. 使用 before/after 的已校验值生成 `hitl::DiffPreview::Fields`，不读取模型摘要或页面可见文本。
4. 将 `assistant_core::InstructionAttribution` 投影到预览；`app_content` 保持高风险标记，默认不可直接批准。
5. 增加正负测试：字段差异、同值、origin/action 漂移、selector 顺序漂移、超限、非法归因。

**Out of scope**

- 不新增 UI/IPC 事件、不实现 `step_approval_requested`、不修改审批 UI。
- 不改变 `ApprovalRequest`、`PolicyDecision`、Tool schema、数据库或 ErrorCode。
- 不消费 once grant、不执行 `form_submit`、不启动真实浏览器。
- 不把 `app_content` 默认升格为 `user_request`，不新增通用点击或脚本入口。

**必须遵守**

- diff 的 after 只能来自 `form_fill` readback；不得使用模型参数或页面摘要补值。
- 所有字符串和字段数量沿用现有边界；超限、未知来源、缺少 source ref 一律失败。
- 预览层只准备证据，不自行授权；Policy / HITL 仍是唯一放行点。
- 不增加依赖、不放宽断言、不改测试以掩盖失败。

**DoD**

- [ ] `FormApprovalPreview` 只由同一 form 的 before/readback 构造。
- [ ] origin/action/selector 顺序/字段数量漂移均 fail-closed。
- [ ] diff 精确反映 before → after，且不泄露未允许的字段值。
- [ ] `app_content` 归因保持高风险，未知或缺失归因被拒绝。
- [ ] 专项、workspace、fmt、clippy、schema/codegen、文档门禁、deny、replay 全绿。
- [ ] LEDGER、事实与状态文件同步。

**验收命令**

```powershell
cargo test -p assistant-agent-core browser_task_approval
cargo test -p assistant-agent-core browser_task
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
cargo run -p xtask -- replay --suite core
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-352 Browser form approval preview and attribution
【目标】从 form_read before 与 form_fill readback after 生成审批字段 diff 和已校验来源归因
【write scope】browser task approval/runtime、form read/fill 与状态同步文件
【铁律】不可信输入先校验；写操作必须有 postcondition；Policy/HITL 唯一放行；不得静默扩大范围
【禁止】UI/IPC、once 消费、真实提交、真实浏览器、公共 schema/API、依赖或通用点击
【验收】专项 + workspace + schema/codegen + 文档门禁 + deny + replay 全绿
【依赖】TASK-344/346/347/348/349/351 已 Done
【疑问】无；只实现审批前预览层
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_form_fill.rs`
- `apps/agent-core/src/browser_task_approval.rs`
- `apps/agent-core/src/browser_task_approval_tests.rs`
- `apps/agent-core/src/browser_task_runtime.rs`
- `apps/agent-core/src/browser_task_runtime_tests.rs`
- `apps/agent-core/src/browser_registry_tests.rs`（`DRIFT-352-2`：仅更新既有 scripted runtime 的 payload 并断言 evidence 不进入 data）
- `docs/memory/facts.md`
- 本卡与 `plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`MEMORY.md` 的状态/进度同步

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_approval` → **8 passed / 0 failed**
- `cargo test -p assistant-agent-core browser_task` → **36 passed / 0 failed**
- `cargo test -p assistant-agent-core` → **91 + 47 passed / 0 failed**（集成套件全绿，既有 ignored 保持）
- `cargo test --workspace` → **exit 0**
- `cargo fmt --all --check` → **PASSED**
- `cargo clippy --all-targets -- -D warnings` → **exit 0**；仅有仓库既有未知 lint `clippy::assert_is_empty` 提示
- `verify-schemas` / `codegen --check` / `hygiene` / `memory-counts` / `adr-index` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `cargo deny check` / `replay --suite core` → 全部 **PASSED**

### 4. DoD 逐条核对

- [x] `FormApprovalPreview` 只由同一 form 的 before/readback 构造。
- [x] origin/action/selector 顺序/字段数量漂移均 fail-closed。
- [x] diff 精确反映 before → after；非空值只以 `[redacted:<utf8-bytes>]` 进入预览。
- [x] `app_content` 归因保持高风险；未知/畸形 evidence 与空 diff 被拒绝。
- [x] 专项、workspace、fmt、clippy、schema/codegen、文档门禁、deny、replay 全绿。
- [x] LEDGER、事实与状态文件同批同步。

### 5. 偏差

```text
DRIFT-352-1（已由 ADR-0103 / TASK-353 关闭）
实现按 ADR-0103 从 form_fill evidence.extra 一次性消费 readback；公共 data/schema 未变。

DRIFT-352-2（最小卡外测试夹具更新，自决落地）
现象：既有 browser_registry_tests.rs 的 FakeRuntime 仍返回旧 form_fill payload，
新增同 form/origin/action 校验后该真实 ToolBus 测试按契约失败。
影响：不更新夹具就只能放宽产品校验或让全量测试保持失败，两者均不可接受。
处理：只给 scripted runtime 补 form_origin/form_action，并新增 evidence 存在性及
data 不含 evidence 的断言；未改产品行为、公共 schema/API、依赖或断言松紧。

shell_misuse
本轮 rollout 自 TASK-352 接续段起机器统计到 36 次显式 shell="bash" 误填；均为只读、
门禁或 guard 锁命令，无产品文件写入。后续命令全部显式使用 shell="powershell"。
```

### 6. 更合理做法

- `form_fill` 在同一 DOM 表达式内先校验同 form、HTTP(S) origin/action 与所有字段，
  再写入并 readback；成功后才附加 evidence，失败路径不带证据。
- Runtime 在 ToolBus 结果进入 verify/commit 前消费 evidence，evidence 缺失或漂移会
  直接终止该 step；原始 before/after 不进入 `BrowserTaskResult.outputs`。

### 7. 遗留问题

- UI `step_approval_requested`、once grant 消费和 loopback 任务链仍由后续卡承接。
- 当前 preview 由 runtime 内部返回后由现有 CLI wrapper 丢弃；下一张 UI 卡需要接入
  `step_approval_requested`，不得把 evidence 或原始字段值序列化出去。

### 8. 新增长期记忆

- FACT：`form_fill` 审批 readback 已由固定 evidence、同 form 匹配与全值脱敏 preview
  实现；原始字段值不进入任务输出。

### 9. 给审阅者的关注点

- 核对 runtime 消费发生在 commit 前，evidence 缺失/重复/漂移不会生成可批准 preview。
- 核对 `FieldDiff` 即使 before/after 脱敏后同形，仍只在原值确实变化时进入 diff。
- 核对 `browser_registry_tests.rs` 的两处夹具更新只补真实 payload 形状与不可见边界。
