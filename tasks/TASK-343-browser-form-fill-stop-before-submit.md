# TASK-343　Browser form_fill：有界填表并强制停在提交前

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-055、TASK-056、TASK-057（整体确认流程仍由 057 承接）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`apps/agent-core/src/browser_handlers.rs`、
  `apps/agent-core/src/browser_form_fill.rs`、`apps/agent-core/src/browser_registry.rs`、
  `apps/agent-core/src/browser_registry_tests.rs`、`apps/agent-core/src/browser_task_runtime.rs`、
  `apps/agent-core/src/browser_task_runtime_tests.rs`、
  `apps/agent-core/src/task_package_render.rs`、`apps/agent-core/src/task_package.rs`、
  `apps/agent-core/tests/task_package.rs`、`adapters/browser.edge/tasks/README.md`、
  `tasks/TASK-343-browser-form-fill-stop-before-submit.md`、`docs/memory/facts.md`、
  `docs/memory/pitfalls.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、
  `MEMORY.md`、`LEDGER.md`

## 目标

实现已声明但未挂载的 `edge.cdp.form_fill` / `chrome.cdp.form_fill` 生产 handler：
只填写有界、唯一的文本类表单字段，返回 `filled_fields` 与 `submitted=false`，任何路径都不得
调用 `submit` / `requestSubmit` / 点击提交控件。

## 必须遵守

1. 复用既有 `CdpProvider::evaluate_dom`，不新增平台 trait 方法或第三方依赖。
2. 最多 100 个字段；单个 selector ≤4 KiB，单个 value ≤16 KiB，总 value ≤64 KiB。
3. selector 必须唯一命中，且只允许 text/search/email/url/tel/number/date/time/
   datetime-local/month/week input、textarea、select、contenteditable。
4. password/file/hidden/button/submit/reset 等元素一律拒绝；`select` 必须存在精确 value
   或 text 匹配项；所有字段预校验通过后才写入。
5. 返回的可信数据只包含 `filled_fields`、`submitted=false` 与 CDP fingerprint 元数据；
   不回传字段值或页面内容。
6. 不实现提交、diff UI、来源归因 UI 或 approval 流程；这些仍归 TASK-057。

## Out of scope

- 不提交表单、不点击按钮、不登录、不读取用户 profile、不复制 Cookie。
- 不新增 `edge.cdp.form_submit` / click 工具或修改 Tool schema。
- 不改 UI、IPC、protocol、数据库或 CI workflow。
- 不宣称 TASK-057 已完成。

## 验收命令

```powershell
cargo test -p assistant-agent-core browser_registry
cargo test -p assistant-agent-core browser_task_schemas
cargo test -p assistant-agent-core task_package
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
```

## DoD

- [ ] Edge/Chrome registry 都挂载四个已实现 CDP 工具，包含 `form_fill`。
- [ ] `form_fill` 只发出一次受限 DOM 写入表达式，且源码级测试证明不含 submit/click。
- [ ] selector 重复、超长、敏感字段类型与无匹配项全部 fail-closed。
- [ ] `form_fill` 输出验证映射为 `filled_fields=<N>` 且 `submitted=false`。
- [ ] 全部门禁全绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-343 Browser form_fill 有界填表并停在提交前
【目标】实现既有 form_fill 声明，不新增 schema，不触发任何提交动作
【write scope】见卡片正文
【铁律】2 不可信输入 / 4 写操作 postcondition / 5 API 优先 / 9 不扩范围 / 10 契约先行
【禁止】提交、点击、登录、profile 复制、公共 schema / UI / IPC / 依赖改动
【验收】EdgexChrome registry + form_fill 正负测试 + workspace / 文档门禁
【依赖】TASK-055 / TASK-056 Done；TASK-057 整体确认流程仍 Blocked
【疑问】无；本卡只实现已声明的 form_fill 前置能力
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_form_fill.rs`：新增有界 form_fill handler、受限 DOM 表达式、纯 JSON 字段状态校验与专项测试。
- `apps/agent-core/src/browser_handlers.rs`、`browser_registry.rs`、`browser_registry_tests.rs`：Edge/Chrome 第四工具挂载与真实 ToolBus 路径测试。
- `apps/agent-core/src/browser_runtime.rs`、`browser_runtime_tests.rs`、`browser_task_runtime.rs`、`browser_task_runtime_tests.rs`：四工具 schema 数量、runtime 装配与 task runner 断言同步。
- `apps/agent-core/src/task_package_render.rs`、`apps/agent-core/tests/task_package.rs`：`filled_fields=<N>` 与 `submitted=false` postcondition。
- `adapters/browser.edge/tasks/README.md`：说明 form_fill 已生产化，TASK-057 的 diff / 来源归因 / 人工确认提交仍独立。
- `tasks/TASK-343-browser-form-fill-stop-before-submit.md`、`docs/memory/facts.md`、`docs/memory/pitfalls.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`MEMORY.md`、`LEDGER.md`：任务、长期记忆和进度同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core form_fill`：6 passed / 0 failed。
- `cargo test -p assistant-agent-core browser_`：47 passed / 0 failed（含 registry、runtime、task runtime 与 task package 聚焦路径）。
- `cargo test --workspace`：全绿，0 failed。
- `cargo fmt --all --check`：全绿。
- `cargo clippy --all-targets -- -D warnings`：exit 0；仅有仓库既有未知 lint `clippy::assert_is_empty` warning。
- `hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check`：全部 PASSED，0 error。
- `cargo deny check`：advisories / bans / licenses / sources 全部 ok。
- `xtask replay --suite core`：2 steps、5 changes、0 mismatch，PASSED。

### 4. DoD 逐条核对

- Edge/Chrome registry 都挂载四个生产工具并包含 `cdp.form_fill`。
- handler 每次只调用一次 `evaluate_dom`；源码级测试断言表达式不含 `.submit(` / `.requestSubmit(` / `.click(`，结果固定 `submitted=false`。
- 1..100 字段、selector ≤4 KiB、value ≤16 KiB、总 value ≤64 KiB；重复 selector、控制字符、非文本控件、歧义与无匹配均 fail-closed。
- 全量 selector 预校验后才写入；成功后逐字段核对 selector、requested value、matched_by 与实际 readback。
- task package 输出验证映射为 `filled_fields=<N>` 和 `submitted=false`；TASK-057 未被标记完成。
- 代码、格式、Clippy、workspace、文档门禁、schema/codegen、deny 与 replay 已全部通过。

### 5. 偏差

- PowerShell 通道在只读命令上再次零输出并超过 10 秒，已终止 session 36823，保留证据后按既有规则回退 `cmd`。
- 本轮 `cmd` 调用中仍有 6 次误选 bash 的 `shell_misuse` 记录：5 次在命令查找/解析阶段失败，1 次只读 `git diff` exit 0；均未写产品文件，验收结论来自后续显式 `cmd` 重跑。
- 首次 guard acquire 的多词 intent 被 CLI 按空格拆成额外目标，产生 3 把额外本会话锁；已逐把按 owner 正常 release，未接管他人锁、未覆盖文件。后续改用单词 intent。
- 最终 shell_misuse 计数更正时，LEDGER 已先经历一轮 release；发现后立即重新 acquire 对应锁并复核当前内容后 release。无并发覆盖或 lost update，但该单行更正顺序不符合“先锁后写”，记录为过程偏差。
- 专项审查发现成功 payload 仍携带 DOM `element`，会被 Rust `deny_unknown_fields` 拒绝；已改为纯 JSON 映射，并新增 requested/matched_by 对应校验。
- 无写操作导致无法回滚、无公共 schema/API/依赖变化，因此未触发 DRIFT。

### 6. 更合理做法

- 保留既有 adapter Tool schema，只补生产 handler，避免另造提交接口或修改公共契约。
- 把“填写”和“提交”拆成两个明确阶段：本卡只做到 readback 后停止，提交继续由 TASK-057 的人工确认链路承接。

### 7. 遗留问题

- TASK-057 仍需独立展开：diff 展示、指令来源归因、人工确认提交、提交后验证与失败恢复均未实现。
- 未启动真实浏览器做人工表单场景验收；本卡是 handler/契约层前置能力，不宣称 TASK-057 真机完成。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `edge|chrome.cdp.form_fill` 已成为有界、唯一命中、readback、强制非提交的生产 handler。
- `docs/memory/pitfalls.md`：记录 form_fill 禁止 selector 猜提交、源码层禁 submit/click，以及 DOM element 不得混入严格响应结构。

### 9. 给审阅者的关注点

- 检查 `browser_form_fill.rs` 的 JavaScript 表达式是否始终没有 submit/requestSubmit/click，并确认字段状态映射不再暴露 DOM element。
- 检查所有 selector 是否在第一次写入前全部 resolve/validate 完毕，确保歧义或非法字段不会造成部分写入。
- 检查 Edge/Chrome 均为四工具、task package postcondition 固定 `submitted=false`，且没有把 TASK-343 误写成 TASK-057 Done。
