# TASK-352 Browser form approval preview and attribution

- 状态：**Ready**
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

- 仅本卡执行记录与状态同步文件；未改产品代码。

### 3. 验收输出摘要

- 只读核对 `browser_form_read.rs`、`browser_form_fill.rs`、adapter `form_fill` schema 与
  `browser_task_approval.rs`：确认当前 ToolBus 输出无法携带 preview 所需的逐字段 readback。
- 未运行产品验收命令；发现契约阻塞后已停止编码。

### 4. DoD 逐条核对

- [ ] 待实现。

### 5. 偏差

```text
DRIFT-352-1
现象：#8（spec / 代码矛盾）+ #3（需要扩展内部/公共接口）。
当前 `form_read` 输出含 origin/action/fields，但 `form_fill` ToolBus 输出仅含
`filled_fields` / `submitted`（以及 fingerprint 元数据）；逐字段 before/after readback
只在 handler 内部用于计算 fingerprint，adapter output schema 还设置了
`additionalProperties=false`。因此 Runtime 侧无法拿到“由 form_fill readback 产生的 after”
来构造 `hitl::DiffPreview::Fields`，也无法比较 after origin/action。
影响：按卡内约束只能三选一：扩展 form_fill 公共输出/schema；增加跨 crate 内部通道
或公开 API；或在 fill 后再做一次 form_read（但这违反“after 只来自 form_fill readback”）。
三者都超出 TASK-352 write scope，不能静默选一条继续。
建议：人类裁决优先选择“扩展 form_fill 输出契约”，由新 ADR/spec 明确哪些 readback 字段
可以进入内部审批层、哪些不得进入模型可见 ToolOutput；随后重开实现卡。
已停工作：未修改 handler、runtime、schema、UI、测试断言或产品代码；未启动浏览器。
需要人类裁决。
```

### 6. 更合理做法

- 将 diff 与来源归因限制在内部预览层，避免在 UI 卡之前改动公共审批接口。

### 7. 遗留问题

- `DRIFT-352-1` 已由 ADR-0103 / TASK-353 关闭；实现时从 `form_fill` evidence.extra 消费 readback。
- UI `step_approval_requested`、once grant 消费和 loopback 任务链仍由后续卡承接。

### 8. 新增长期记忆

- 无（尚未验证新事实）。

### 9. 给审阅者的关注点

- 核对 after 值只来自 readback，且 origin/action/字段状态漂移不会生成可批准预览。
