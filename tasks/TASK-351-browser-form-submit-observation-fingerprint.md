# TASK-351　Browser form_submit observation fingerprint

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：344、348、349、350　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

**目标**

解决 `form_submit` 成功输出与 Runtime `EnvelopeObservationCollector` 强制 fingerprint 之间的契约缺口。
`form_submit` 必须在点击前后观察真实页面 identity，并返回可验证的前后 fingerprint；
task-package 对提交步骤使用 `state_changed`，不得用结果哈希、空指纹或放宽通用 observation 规则绕过。

**write scope**

- `docs/adr/0102-browser-form-submit-observation-fingerprint.md`
- `docs/adr/README.md`
- `docs/spec/browser-form-confirmation.md`
- `adapters/browser.edge/tools/tools.json`
- `adapters/browser.chrome/tools/tools.json`
- `adapters/browser.edge/tests/validate_adapter_packs.py`
- `apps/agent-core/src/browser_form_submit.rs`
- `apps/agent-core/src/browser_form_submit_tests.rs`
- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/tests/task_package.rs`
- `docs/memory/facts.md`
- `tasks/TASK-351-browser-form-submit-observation-fingerprint.md`
- `plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`MEMORY.md`：仅状态与进度同步

**In scope**

1. 在提交前 DOM 表达式中捕获 `location.href`、`document.title` 与 `performance.timeOrigin`。
2. 点击后 bounded network-idle 后捕获同一组 identity，继续要求 `timeOrigin` 变化。
3. 使用现有 `fingerprint_value` 生成 `cdp_target` scope 的前后 fingerprint，并输出 `elapsed_ms`。
4. 扩展 Edge/Chrome `form_submit` output schema 与 adapter pack 校验。
5. task-package assertion table 为 `form_submit` 生成 `state_changed`，窗口复用浏览器 step execute timeout。
6. 增加正负测试：缺字段、非法 fingerprint、timeOrigin 未变化、schema 字段缺失与 assertion 形状。

**Out of scope**

- 不改 Runtime `EnvelopeObservationCollector` 的通用 fingerprint 要求。
- 不使用 `submitted=true` / `post_url` 的结果哈希冒充平台状态指纹。
- 不实现 approval/diff、UI 事件、真实表单提交或 TASK-057 完整链路。
- 不增加通用 click、脚本入口、重试、跨源提交、外部站点或真实账号。
- 不改变 `form_submit` 的 write/high/L3/once/非幂等审批合同。

**必须遵守**

- fingerprint 必须来自提交前后实际观察到的页面 identity，不能由模型文本或预期参数生成。
- 点击前校验失败时不得返回 fingerprint 成功结果；点击后未知状态仍为 `UserInteraction`。
- 输出中的 URL/title 不额外回传明文，只进入 canonical fingerprint。
- schema 变更先由 ADR-0102 授权，再同步 spec 与 adapter。
- 不放宽断言、不新增依赖、不改 `PolicyDecision` 或数据库 schema。

**DoD**

- [ ] ADR-0102 登记 Accepted，spec 更新到 0.2。
- [ ] Edge/Chrome `form_submit` output schema 包含 `fingerprint_scope` / `fingerprint` / `previous_fingerprint` / `elapsed_ms`。
- [ ] handler 输出真实前后 fingerprint，`state_changed` 可用。
- [ ] 缺字段、非法 fingerprint、未导航与结果不确定均 fail-closed。
- [ ] task package 为 `form_submit` 生成固定 `state_changed` assertion。
- [ ] form_submit 专项、adapter pack、browser registry/runtime/task、task-package、workspace 全绿。
- [ ] fmt / clippy / schema / codegen / 文档门禁 / deny / replay / coverage 全绿。
- [ ] LEDGER 与长期事实同步。

**验收命令**

```powershell
cargo test -p assistant-agent-core browser_handlers::form_submit
cargo test -p assistant-agent-core browser_registry
cargo test -p assistant-agent-core browser_runtime
cargo test -p assistant-agent-core browser_task
cargo test -p assistant-agent-core --test task_package
python adapters/browser.edge/tests/validate_adapter_packs.py adapters/browser.edge
python adapters/browser.edge/tests/validate_adapter_packs.py adapters/browser.chrome
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
cargo llvm-cov --workspace --fail-under-lines 75
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-351 Browser form_submit observation fingerprint
【目标】用提交前后真实页面 identity 生成 cdp_target fingerprint，使 form_submit 可安全接入 state_changed
【write scope】ADR-0102、spec、Edge/Chrome form_submit schema、adapter validator、agent-core form_submit/task-package 与状态同步文件
【铁律】无静默失败；不可信输入先校验；写操作必须有 postcondition；契约先行；不得静默扩大范围
【禁止】伪造结果哈希、放宽通用 observation、增加通用 click、实现 approval/diff/真实提交、外部站点或新依赖
【验收】专项 + adapter pack + workspace + schema/codegen + 文档门禁 + deny + replay + coverage 全绿
【依赖】TASK-344/348/349/350 已 Done
【疑问】无；采用真实 identity fingerprint 的最小契约扩展
```

### 2. 实际改动文件

- `docs/adr/0102-browser-form-submit-observation-fingerprint.md`
- `docs/adr/README.md`
- `docs/spec/browser-form-confirmation.md`
- `adapters/browser.edge/tools/tools.json`
- `adapters/browser.chrome/tools/tools.json`
- `adapters/browser.edge/tests/validate_adapter_packs.py`
- `apps/agent-core/src/browser_form_submit.rs`
- `apps/agent-core/src/browser_form_submit_tests.rs`
- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/tests/task_package.rs`
- `tasks/TASK-351-browser-form-submit-observation-fingerprint.md`
- `plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`
- `docs/memory/facts.md`、`MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_handlers::form_submit`
  → **6/6 passed**。
- `cargo test -p assistant-agent-core --test task_package browser_form_submit_assertion`
  → **1/1 passed**。
- Edge/Chrome adapter pack 校验 → 均 `PACK OK`。
- browser registry 5/5、browser runtime 5/5、browser task 27/27。
- `cargo test --workspace` → **510 passed / 0 failed**。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `cargo fmt --all --check` → 0 diff。
- `cargo clippy --all-targets -- -D warnings` → exit 0；保留既有
  `clippy::assert_is_empty` unknown-lint warning。
- `verify-schemas` / `codegen --check` → PASSED，0 drift。
- `hygiene` 0E/154W、`memory-counts` 0E/1W、`adr-index` 0E/0W。
- `docscan` 0E/175W、`card-check` 0E/35W、`check-ledger` 0E/0W、
  `check-comments` 0E/75W。
- `cargo deny check` → advisories / bans / licenses / sources 全 ok。
- `replay --suite core` → 2 steps、5 changes、0 mismatch，PASSED。
- `cargo llvm-cov --workspace --fail-under-lines 75` → workspace 行覆盖 **79.73%**。

### 4. DoD 逐条核对

- [x] ADR-0102 登记 Accepted，spec 更新到 0.2。
- [x] Edge/Chrome `form_submit` output schema 包含四个 observation 字段。
- [x] handler 输出真实前后 identity fingerprint，`state_changed` 可用。
- [x] 缺字段、非法 fingerprint、未导航与结果不确定均 fail-closed。
- [x] task package 为 `form_submit` 生成固定 `state_changed` assertion。
- [x] form_submit、adapter pack、browser registry/runtime/task、task-package 与 workspace 全绿。
- [x] fmt / clippy / schema / codegen / 文档门禁 / deny / replay / coverage 全绿。
- [x] LEDGER 与长期事实同步。

### 5. 偏差

```text
无契约偏差。

实现自审修复：首轮 clippy 发现 `validate_post_submit` 残留未使用 lifetime，以及测试使用
`expect()`；已分别删除 lifetime、改为显式 `is_ok()` + `if let`，重跑 clippy exit 0。

过程偏差：本会话有 25 次 exec_command 显式误填 shell="bash"，均为只读源码/文档或门禁
查询，无仓库写入；随后均以 shell="powershell" 重跑或继续。已按证据记录，不以项目 hook
重写作为误填理由。
```

### 6. 更合理做法

- 提交前后使用同一 identity 结构计算 fingerprint，保持 `cdp_target` scope，避免修改
  Runtime 通用 observation 规则。
- title 只参与 canonical fingerprint，不输出明文，兼顾可验证性与页面内容最小暴露。

### 7. 遗留问题

- TASK-057 仍需实现 approval/diff、UI 事件与完整 T5.2 任务链；本卡只解除 task-package
  observation 阻塞。
- TASK-058 注入安全回归与阶段总验收仍未展开。

### 8. 新增长期记忆

- `FACT`：`form_submit` 已输出提交前后真实 identity fingerprint，task-package 使用
  `state_changed`，Runtime 无需放宽 observation。

### 9. 给审阅者的关注点

- 重点核对 fingerprint 来自实际 URL/title/timeOrigin，而不是 `submitted/post_url` 结果哈希。
- 重点核对 title 未回传明文，且 schema `additionalProperties=false`。
- 重点核对点击后未知状态仍为 `UserInteraction`，没有重试路径。
