# TASK-356 Browser form human approval loopback chain

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-350、TASK-354、TASK-355
- 本文件 = **卡片正文 + 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

## 目标

把 TASK-355 的 `step_approval_requested` / `PendingApprovals` 与 TASK-333
的 once `ApprovalGrants` 接成人类在场的真实本地 loopback 任务链：browser
runner 在 `form_submit` 的 `AwaitingApproval` 处暂停，显示脱敏审批卡，人类
输入一次批准后在同一 task/step 上恢复，Policy 消费 once grant 并只调用一次
`form_submit`。

本卡的人类在场入口是私有 CLI 终端审批；它不冒充真实 UI session，也不替代
TASK-057 / TASK-058 的正式派单。

## In scope

- `apps/agent-core/src/ui_control.rs`：抽取 UI handler 与私有 runner 共用的
  pending approval approve/reject 校验，不改变 UI 线上契约。
- `apps/agent-core/src/browser_task_runtime.rs`：共享 pending registry /
  approval grants；在 `AwaitingApproval` 暂停、显示事件、接受一次批准、恢复
  同一 Prechecking step。
- `apps/agent-core/src/browser_task_support.rs`：新增私有 `--human-approval`
  选项；允许预批准 navigate 等非提交前置步骤，但禁止预批准 `form_submit`。
- `apps/agent-core/src/browser_task_runtime_tests.rs`、`apps/agent-core/tests/ui_ipc.rs`：
  覆盖批准、拒绝、重复恢复、非 once、过期、无 grant 不执行和 once 消费。
- `adapters/browser.edge/tasks/t5.2.profile-human-approval.json`：navigate →
  form_read → form_fill → form_submit 的本地 synthetic profile package。
- 经人类批准扩展：`apps/agent-core/src/task_package_render.rs` 与
  `apps/agent-core/tests/task_package.rs`：补 `form_submit` 的
  `point_of_no_return=true` 映射与契约断言。
- 经人类批准扩展：`apps/agent-core/src/browser_task_approval.rs`：补
  browser-specific high/L3/once confirmation 规则与负向测试。
- `adapters/browser.edge/tasks/README.md`、`eval/tasks/edge/t5.2/README.md`：
  记录人类在场命令与安全边界。
- `eval/tasks/edge/t5.2/run_real_edge_human_approval.ps1`：启动 loopback
  fixture、运行真实 Edge、保留审批输出/JSON、校验结果并回收进程。
- 本卡、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、
  `docs/memory/facts.md`：状态与进展同步。

## Out of scope

- 不新增真实 UI session 组装、NamedPipe UI 验收或 App 页面布局。
- 不新增通用点击、脚本执行、Tool schema、DB schema、ErrorCode 或依赖。
- 不自动批准、不预授权 `form_submit`、不允许 `this_task` / `session` /
  `persistent` scope 进入 T5.2。
- 不访问外部站点、真实账号、密码、文件上传、支付或验证码。
- 不修改 TASK-057 / TASK-058 正文或状态。

## 必须遵守

1. 只有 `form_submit` 的 `AllowWithConfirmation` 可触发人类审批暂停；其他
   `AwaitingApproval` 不得被终端批准绕过。
2. 审批事件必须来自 TASK-355 已登记的同一 `request_id`，且 task/step/action
   与当前 step 完全一致；过期、重复或 scope 非 `once` 一律拒绝。
3. 人类拒绝时 `form_submit` 调用次数必须为 0；批准后 grant 只能消费一次，
   第二次恢复不得再次点击提交。
4. 终端只显示已脱敏 diff 和最小 form evidence；不得打印原始字段值、DOM、
   Cookie、密码或文件内容。
5. runner 失败、拒绝、超时、Ctrl+C 或浏览器错误都必须关闭 ToolBus、浏览器
   进程树并释放 pending/approval 状态。
6. 真实 Edge 只访问 `127.0.0.1:8766` fixture；外部站点不在本卡范围。

## 验收命令

```powershell
cargo test -p assistant-agent-core browser_task_runtime
cargo test -p assistant-agent-core --test ui_ipc
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
cargo run -p xtask -- replay --suite core
python -m unittest discover -s eval/tasks/edge/t5.2 -p "test_*.py" -v
```

人类在场验收：

```powershell
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_real_edge_human_approval.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/<date>-task-356-human-approval `
  -EdgeExecutable "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
```

期望：终端出现 high/L3/once 审批卡；输入 `once` 后提交恰好一次；JSON
`status=completed`，`submitted=true`，`post_url=http://127.0.0.1:8766/t5.2/submitted`，
成功页含 `submitted-marker`；跑后 Edge/8766 收敛。

## DoD

- [ ] `--human-approval` 在 `form_submit` 前暂停；允许非提交步骤预批准，禁止预批准 `form_submit`。
- [ ] 人类输入 `once` 后经共享 pending/grants 路径恢复；拒绝时提交调用为 0。
- [ ] once grant 只消费一次，重复恢复、过期、非 once 和不匹配 request 全部 fail-closed。
- [ ] 真实 Edge 本地 loopback 人类在场 1/1 通过，结果 JSON 与进程回收证据齐全。
- [ ] 全部门禁通过；TASK-057 / TASK-058 状态与正文未被本卡改写。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤，
     并会被 xtask card-check 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写，9 节骨架见 gov §3.4 / ADR-0031 D4）
### 1. 约束回执

```text
【任务】TASK-356 Browser form human approval loopback chain
【目标】把既有 step approval event、pending registry 与 once grant 接成真实本地 form_submit 链，并在人类输入 once 后只提交一次。
【write scope】仅：本卡列出的 agent-core browser approval/runtime/support/UI 共享校验、task-package renderer、T5.2 profile/eval 脚本、本卡与状态/记忆同步文件；经人类批准追加 browser_form_submit 输出脱敏与对应测试。
【铁律】无静默失败；不可信输入先校验；Policy 是唯一放行点；L3 不可逆仅 once 人工确认；不得打印原始字段值；失败必须回收进程与临时状态。
【禁止】不新增通用 click/script/shell 工具；不预批准 form_submit；不允许 this_task/session/persistent；不访问外部站点、真实账号、文件、支付或验证码；不改 TASK-057/058。
【验收】agent-core browser approval/runtime/UI IPC tests、workspace/fmt/clippy、UI lint/typecheck/test、schema/codegen/hygiene/memory/adr/refscan/docscan/card/ledger/comments、deny/replay、Python T5.2 9/9、真实 Edge 本地人类在场 1/1。
【依赖】TASK-350 / TASK-354 / TASK-355 已 Done；TASK-057 / TASK-058 仍为 Blocked/占位，不在本卡改写。
【疑问】无；DRIFT-356-1/2/3 均经人类批准后按最小范围修复。
```

### 2. 实际改动文件

- `apps/agent-core/src/ui_control.rs`：抽取 pending approval approve 校验，供 UI handler 与私有 runner 共用。
- `apps/agent-core/src/browser_task_runtime.rs`：新增 `--human-approval` 暂停/恢复、同一 task/step 事件匹配、精确 `once` 输入、once grant 与 fail-closed 恢复。
- `apps/agent-core/src/browser_task_runtime_tests.rs`：覆盖人类批准、拒绝、过期、重复、不匹配与 form_submit 预批准拒绝。
- `apps/agent-core/src/browser_task_human_approval_tests.rs`：把人类审批专项测试从 900 行硬上限的主测试文件拆出，保持脚本化 ToolBus 边界。
- `apps/agent-core/src/browser_task_support.rs`：解析 `--human-approval`，并映射拒绝/过期/提示失败 ErrorCode。
- `apps/agent-core/src/browser_task_approval.rs`、`browser_task_approval_tests.rs`：新增 browser-specific high/L3/once confirmation 规则，保留 unattended/tainted deny floor。
- `apps/agent-core/src/task_package_render.rs`、`apps/agent-core/tests/task_package.rs`：把 `edge|chrome.cdp.form_submit` 映射为 `point_of_no_return=true` 并补契约断言。
- `apps/agent-core/src/browser_form_submit.rs`、`browser_form_submit_tests.rs`：移除模型可见 `post_url` 的 query/fragment；路径含预期字段值时替换为 `%5Bredacted%5D`，并补两类脱敏测试。
- `adapters/browser.edge/tasks/t5.2.profile-human-approval.json`：四步本地 loopback 人类审批任务包。
- `adapters/browser.edge/tasks/README.md`、`eval/tasks/edge/t5.2/README.md`：记录人类在场边界、命令与脱敏口径。
- `eval/tasks/edge/t5.2/run_real_edge_human_approval.ps1`：真实 Edge 驱动、精确 query-free post_url 校验、证据与进程/端口回收。
- `eval/tasks/edge/t5.2/evidence/2026-11-02-task-356-human-approval*`：保留 v1~v5 定位失败证据与 v7 安全成功证据；v6 含原始 GET 字段值，违反 D4，已删除且不入库。
- `tasks/TASK-356-browser-form-human-approval-loopback.md`、`LEDGER.md`、`docs/memory/facts.md`：执行记录与状态同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_runtime` → 43 passed / 0 failed。
- `cargo test -p assistant-agent-core browser_task_approval` → 12 passed / 0 failed。
- `cargo test -p assistant-agent-core --test ui_ipc` → 18 passed / 0 failed。
- `cargo test --workspace` → 510 passed / 0 failed，doctests 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings` → exit 0；仓库既有 `unknown lint: clippy::assert_is_empty` 提示仍在。
- UI `lint` / `typecheck` / `test` → 全绿；model 85/85、DOM 3/3。
- Python T5.2 fixture `-m unittest ... -v` → 9/9 passed。
- 真实 Edge v7 → `status=completed`、4/4 `committed`、`submitted=true`、`post_url=http://127.0.0.1:8766/t5.2/submitted`、成功页含 `submitted-marker`；专用 profile 进程 0、8766 listener false。
- v7 证据全文检索无 `Ada` / email / note 原值；审批卡仅显示 `[redacted:3]` / `[redacted:16]` / `[redacted:23]`。

### 4. DoD 逐条核对

- [x] `--human-approval` 在 `form_submit` 前暂停；允许 non-submit 预批准，`form_submit` 预批准被拒绝。
- [x] 人类输入 `once` 后经 pending/grants 恢复；拒绝路径不执行提交，approval/runtime/UI tests 覆盖 fail-closed。
- [x] once grant 只消费一次；过期、重复、非 once、task/step/request 不匹配均拒绝。
- [x] 真实 Edge 本地 loopback 人类在场 1/1 通过，结果 JSON、脱敏、进程与端口回收证据齐全。
- [x] 自动化门禁全绿；TASK-057 / TASK-058 正文与状态未被本卡改写。

### 5. 偏差

```text
DRIFT-356-1
触发器：#5（超出 write scope）+ #8（spec/代码映射缺失）。
现象：真实 Edge 运行在审批前失败："L3Irreversible step submit_form must set point_of_no_return"。
影响：TASK-356 的 human approval loopback 无法进入 AwaitingApproval。
根因：task-package renderer 的 point_of_no_return 映射未包含
edge.cdp.form_submit / chrome.cdp.form_submit；ADR-0101 D7 要求该工具为
point-of-no-return。
建议：批准将 apps/agent-core/src/task_package_render.rs 纳入 TASK-356
write scope，并补一个 form_submit → point_of_no_return 的契约测试。
已停止工作：未启动第二次浏览器；未修改 task-package renderer 或断言。
```

```text
DRIFT-356-2
触发器：#5（超出 write scope）+ #8（spec/代码策略映射缺失）。
现象：真实 Edge 已完成 navigate/read/fill，但 form_submit 被 PolicyDenied；
未进入 AwaitingApproval。
影响：human approval 无法生成 once grant，真实 loopback 链继续阻塞。
根因：RuleSet::example_v0() 只有 medium write confirmation 规则，没有
high + L3Irreversible 的 AllowWithConfirmation { once } 规则；ADR-0101 D7
要求 high/L3 form_submit 经 once 人工审批，而不是直接 Deny。
建议：批准将 apps/agent-core/src/browser_task_approval.rs 纳入 TASK-356
write scope，增加 browser-specific high/L3/once confirmation 规则及负向测试；
不放宽 unattended 或 tainted 的 deny floor。
已停止工作：未修改 policy、未重跑浏览器、未用预批准绕过 form_submit。
```

```text
DRIFT-356-3
触发器：#5（超出 write scope）+ #8（ADR D4 与 spec post_url 字段交叠）。
现象：v6 真实成功结果的 GET post_url 携带 display-name/email/note 原始 query 值，违反 ADR-0101 D4「原始字段值不得进入模型、日志、审计、DB、任务输出或 replay」。
影响：即使审批事件已脱敏，成功 ToolOutput 与证据仍可能泄露字段值。
根因：spec 要求返回 observed post_url，但 GET 提交会把字段编码进 query；输出层未做 D4 脱敏。
裁决：人类批准把 browser_form_submit.rs 与其测试纳入本卡，沿用 D4：移除 query/fragment；若路径出现预期字段值则整段路径替换为 %5Bredacted%5D。
已处理：新增两类负向测试；重跑 v7 真实 Edge；删除含原值的 v6 证据，不提交。
```

Shell misuse：同一 user turn 机器统计 `shell="bash"` **16**、`shell="cmd"` **0**；15 次为只读检索、文件读取或目录列举，1 次是在误填 shell 下执行 evidence 分支切换；未通过误填 shell 写入产品文件。

### 6. 更合理做法

- 人类审批复用既有 `PendingApprovals` / `ApprovalGrants`，不新增审批协议。
- `form_submit` 的 point-of-no-return、high/L3 policy 与 output DLP 同时收口，避免只修“能跑”而留下契约缺口。
- 真实证据先做敏感值检索再入库；不安全成功证据宁可不保留。

### 7. 遗留问题

- 当前 human-approval 入口是私有 CLI，不是 TASK-057 的正式 UI session。
- 终端审批完成后由同一 runner 恢复，不自动重试；正式 UI 路径仍由 TASK-057 后续承接。
- `check-comments` 仍有仓库既有 75 条 warning，本次 0 error。

### 8. 新增长期记忆

- `FACT`：browser form human approval loopback 已能把同一 high/L3/once 事件暂停到 `AwaitingApproval`，人类精确输入 `once` 后消费一次 grant 并仅提交一次。
- `FACT`：`form_submit` 模型可见 `post_url` 必须移除 query/fragment；预期字段值若出现在路径中，路径整体脱敏为 `%5Bredacted%5D`。

### 9. 给审阅者的关注点

- 重点审查 `browser_task_runtime.rs` 的事件来源、request/task/step 绑定与拒绝路径。
- 重点审查 `browser_task_approval.rs` 的规则是否只覆盖 high/L3 write，未放宽 unattended/tainted deny floor。
- 重点审查 `browser_form_submit.rs` 的 post_url DLP 与 fingerprint 仍基于真实 observed identity，二者没有混淆。

---

**2026-11-02 merge evidence**

- PR #685 checks **22**（21 success + 1 skipped，pending 0），Mergeable / Clean，base `main`；merge `315bd53`。
- post-merge main CI `41385329263` = **success**。
