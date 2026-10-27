# TASK-333　Browser task runner 显式逐 step 人工批准

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-329、TASK-330、TASK-332、`DRIFT-056-5`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。

---

- **write scope**：`apps/agent-core/src/browser_task_runtime.rs`、`apps/agent-core/src/browser_task_approval.rs`、
  `apps/agent-core/src/browser_task_approval_tests.rs`、`apps/agent-core/src/browser_task_runtime_tests.rs`、
  `apps/agent-core/src/browser_task_support.rs`、`apps/agent-core/src/main.rs`、`apps/agent-core/README.md`

## 目标

为私有 `--browser-task-run` 增加可重复的 `--approve-step <step-id>`。每个批准只允许对应 Plan 中一个
当前会被 Policy 要求确认的写步骤，并由现有有界、一次性的 `ApprovalGrants` 消费；Plan 批准不得替代
step 批准。

## 必须遵守

1. 只接受 Plan 中真实存在、`effect=write` 且无批准时 Policy 返回 `AllowWithConfirmation` 的步骤；
   未知步骤、只读步骤、无需确认的步骤、重复批准与超过 Plan 步骤数的批准全部 fail-closed。
2. 每个 `--approve-step` 只授予一个 step，使用 `ApprovalScope::Once`、正 TTL、单次使用；不得转换成
   任务级或会话级授权。
3. 未批准的 medium write 仍由 `RuntimeExecutor` 返回 `AwaitingApproval`，runner 必须显式失败，不能
   自动批准。
4. 测试必须从 `adapters/browser.edge/tools/tools.json` 读取权威 schema，至少覆盖
   `edge.cdp.navigate = write/medium/l2_compensation`，不能继续用全 read fake schema 掩盖缺口。
5. 不改公共 trait、Tool schema、ErrorCode、Policy 规则、依赖、TASK-056 断言或 CSV 契约。

## Out of scope

- 不执行 TASK-056 的真实 Edge 10/10、外部 `books.toscrape.com` 3/3 或最终人工验收。
- 不实现新的审批 UI、交互恢复、任务级授权或持久化批准；本卡只补私有一次运行 CLI 的显式 step 批准。
- 不修改 `CatalogStepPolicy` / `ApprovalWindows` 的库内可见性，也不为 runner 扩大公共 API。

## 验收命令

```powershell
cargo test -p assistant-agent-core --bin assistant-agent-core browser_task_runtime
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
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
cargo run -p xtask -- check-migrations
cargo deny check
```

## DoD

- [ ] T5.1 的权威 write/medium navigate schema 在无 step 批准时显式得到 `AwaitingApproval`。
- [ ] 三个 navigate step 均显式批准后，六步全部 `Committed`，三页输出与既有 CSV 契约不变。
- [ ] 未知、只读、无需确认、重复与超量批准均有 fail-closed 测试。
- [ ] 单次批准被消费后不能复用，runner 不产生任务级或持久授权。
- [ ] 上述验收命令全绿；TASK-056 仍保持 Blocked，真实 Edge 验收另批执行。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-333 Browser task runner 显式逐 step 人工批准
【目标】增加一次性 --approve-step，解除 DRIFT-056-5，不把 Plan 批准冒充 step 批准
【write scope】仅：apps/agent-core 私有 runner 三文件、main 帮助、crate README、本卡与状态同步文件
【铁律】1 无静默失败 / 2 不可信输入先校验 / 3 Policy 唯一放行点 / 4 写操作有 postcondition / 9 不扩范围 / 12 有界
【禁止】不改 Policy/schema/公共 API/依赖；不放宽断言；不在本卡执行真实 Edge 或外部 smoke
【验收】runner 权威 write/medium 专项 + 全量门禁 → 全绿
【依赖】TASK-329 / TASK-330 / TASK-332 Done，DRIFT-056-5 已核对
【疑问】无
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_task_runtime.rs`
- `apps/agent-core/src/browser_task_approval.rs`（新增私有 step 批准边界）
- `apps/agent-core/src/browser_task_approval_tests.rs`（新增）
- `apps/agent-core/src/browser_task_runtime_tests.rs`
- `apps/agent-core/src/browser_task_support.rs`
- `apps/agent-core/src/main.rs`
- `apps/agent-core/README.md`
- `tasks/TASK-333-browser-task-step-approval.md`
- `plans/stage-1-pilots.md` / `PLAN.md` / `README.md` / `LEDGER.md`
- `docs/memory/facts.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --bin assistant-agent-core browser_task_runtime`
  → **22 passed / 0 failed**。权威 `edge.cdp.navigate = write/medium/L2` 无批准时在
  `navigate_page_1` 返回 `AwaitingApproval`；显式批准三个 navigate 后 T5.1 六步全部
  `Committed`。
- `cargo test --workspace` → exit 0；`cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅有既有 unknown-lint warning。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `verify-schemas` / `codegen --check` → PASSED，0 error / 0 drift。
- `hygiene` → PASSED，0 error / **149 warning**；本卡未新增 file-length、complexity 或
  function-length warning。
- `memory-counts` → PASSED，0 error / 1 warning（Paint 既有归档 Warning）。
- `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-comments` / `check-migrations` → 全部 exit 0。

### 4. DoD 逐条核对

- [x] T5.1 的权威 write/medium navigate schema 在无 step 批准时显式得到 `AwaitingApproval`。
- [x] 三个 navigate step 均显式批准后，六步全部 `Committed`，三页输出与既有 CSV 契约不变。
- [x] 未知、只读、无需确认、重复与超量批准均有 fail-closed 测试。
- [x] 单次批准被消费后不能复用，runner 不产生任务级或持久授权。
- [x] 上述验收命令通过；TASK-056 仍保持 Blocked，真实 Edge 验收另批执行。

### 5. 偏差

- `DRIFT-056-5` 已在 TASK-333 范围内解除，无新 DRIFT。
- 为满足单文件 600 行硬线，新增两个私有文件：`browser_task_approval.rs` 与
  `browser_task_approval_tests.rs`。未新增 crate、顶层目录、公共 API 或依赖。
- 对当前 rollout JSONL 做最终机器审计：误填 bash **52** 次；此前记录 35 少计 17。
  52 次中 1 次 `git switch` 明确 exit 0，45 次 exit 127，4 次 exit 2，2 次未从
  session 输出解析出退出码（1 个只读状态会话、1 个审计时的当前调用）；未通过 bash
  写入产品文件内容。LEDGER 追加最终更正行，不改旧行。

### 6. 更合理做法

- 不把库内私有 `ApprovalWindows` 提升成公共 API；runner 复用现有公开
  `ApprovalGrants` 的有界、单次、TTL 授权表。Policy 先求值，只有
  `AllowWithConfirmation` 才消费一个 once grant，Deny 与 read-only 均不可被批准覆盖。
- 批准先绑定生成的 Plan 与权威 catalog，再启动 task engine，参数错误在工具执行前失败。

### 7. 遗留问题

- TASK-056 仍需人类在场的真实 Edge loopback 10/10、每轮 JSON/CSV validator 和外部
  `books.toscrape.com` 3/3 smoke。

### 8. 新增长期记忆

- FACT：私有 browser runner 已把 Plan 批准与逐 step 人工批准分开；权威 write/medium
  navigate 在无批准时保持 `AwaitingApproval`，显式 once grant 只消费一次。

### 9. 给审阅者的关注点

- 重点检查 `validate_step_approvals` 的 fail-closed 顺序：Plan 存在性、write effect、
  authoritative schema、Policy confirmation 四层都在写授权前。
- 重点检查 `BrowserTaskPolicy::decide` 先做 Policy 求值、只有
  `AllowWithConfirmation` 才消费批准，确认批准没有越过 Deny。
- 本卡没有真实 Edge 证据；不要把脚本化六步提交或 CLI 参数测试冒充 TASK-056 真机结果。
