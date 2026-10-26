# TASK-329　Browser task-package 执行器与步骤输出导出

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：治理/基础设施　依赖：TASK-107、TASK-112、TASK-114、TASK-328
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文；以下为执行记录。

---

**目标**

在 `assistant-agent-core` 的私有 CLI 层增加一次显式 browser task-package 运行：
用专用 profile 启动 Edge/Chrome，连接 launcher 发布的 loopback CDP，启动真实 in-process
`ToolBus`，按 task-package 渲染出的 Plan 逐步执行；全部步骤提交后，把每个已声明步骤输出
写到调用方指定的新 JSON 文件，并读回验证。

本卡是 TASK-056 产品执行路径的中间层，不把 runner 自检当成 TASK-056 真机完成。

**In scope**

- `--browser-task-run` 私有 CLI 模式与参数验证。
- 复用 `TaskPackageProvider`、`Planner`、`TaskEngine`、`RuntimeExecutor`、`BindingInvoker`
  和 `ToolBusInvoker`，不建立第二套执行状态机。
- Browser navigate / dom_read 返回稳定 CDP fingerprint、previous fingerprint 与 scope，
  使现有 `state_changed` / `state_unchanged` 后置条件可真实求值。
- Binding 发布 browser 输出的显式别名：`*_ready` → `ready`，`*_dom` → `dom`。
- 输出文件必须 `create_new`，写后读回逐字节核对；失败、拒绝或未完成的步骤不得留下
  “成功”输出文件。
- 自动化测试只使用脚本化 CDP runtime 与本地内存 ToolBus，不启动浏览器。

**Out of scope**

- 不实现 CSV 解析/规范化；CSV 提取仍由 TASK-056 的后续实现与 eval harness 收口。
- 不执行 TASK-056 的真实 Edge 10/10、外部 `books.toscrape.com` 3/3 或人工登录。
- 不实现 form fill、submit、click、download、screenshot、browser close provider 行为。
- 不修改公共 trait、Tool schema、ErrorCode、IPC、DB schema、ADR 或 spec。
- 不新增第三方依赖，不启动真实浏览器、不访问外网、不使用凭据。

**必须遵守**

1. 浏览器只用调用方给出的绝对专用 profile 根；不得读取或复制用户日常 profile。
2. 只挂载 registry 中实际存在的三个 browser tools；未知工具继续 fail-closed。
3. 页面 DOM 是不可信 `app_content`；输出文件只保存有界工具结果，不执行页面指令。
4. 任一步非 `Committed` 都立即终止；不得把 PolicyDenied、ToolFailed、NeedsHuman 或
   VerificationFailed 继续当作成功。
5. 成功和所有失败路径都必须 shutdown `ToolBus` 并 close/reap 浏览器进程树。
6. 输出路径已存在时拒绝覆盖（`ToolInvalidArgs`），写后读回不一致时显式失败。

**验收命令**

```powershell
cargo test -p assistant-agent-core --bin assistant-agent-core browser_task_runtime
cargo test -p assistant-agent-core --lib browser_registry
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

**DoD**

- [ ] six-step T5.1 browser package 在脚本化 CDP runtime 上全部 `Committed`。
- [ ] navigate 使用 `cdp_target` scope，带前后 fingerprint 与实际 elapsed；dom_read 的
  `state_unchanged` 可求值。
- [ ] binding 捕获 `page_1_ready` / `page_1_dom` 等声明输出，输出 JSON 包含三页 DOM。
- [ ] 输出文件新建、读回一致；已存在路径、缺输出、验证失败和 ToolBus 失败均显式退出。
- [ ] ToolBus 与浏览器生命周期在成功/失败路径均回收。
- [ ] 全量验收命令通过；TASK-056 仍保持 Blocked。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->

## 执行记录

### 1. 约束回执

见本会话约束回执。预授权在本卡范围内执行；真实 GUI/外网/凭据与 CSV 规范化不在本卡。

### 2. 实际改动文件

- `apps/agent-core/src/browser_task_runtime.rs`（新增）
- `apps/agent-core/src/browser_task_runtime_tests.rs`（新增）
- `apps/agent-core/src/browser_task_support.rs`（新增）
- `apps/agent-core/src/main.rs`
- `apps/agent-core/src/browser_runtime.rs`
- `apps/agent-core/src/browser_handlers.rs`
- `apps/agent-core/src/browser_registry_tests.rs`
- `apps/agent-core/src/runtime.rs`
- `apps/agent-core/src/runtime_binding.rs`
- `apps/agent-core/README.md`
- `tasks/TASK-329-browser-task-package-execution-runner.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --bin assistant-agent-core browser_task_runtime`
  → 5/5 passed；其中六步 T5.1 package 在脚本化 ToolBus 上 6/6 `Committed`，
  `page_1_dom` 到 `page_3_dom` 均导出，输出文件 create-new 与读回一致。
- `cargo test -p assistant-agent-core --lib browser_registry` → 5/5 passed；
  navigate 输出 `cdp_target` scope、变化的前后 fingerprint；dom_read 的前后 fingerprint 相同。
- `cargo test -p assistant-agent-core` → 全部通过；真机 ignored 项保持 ignored。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅仓内既有 unknown-lint warning。
- `cargo test --workspace` → exit 0。
- `cargo test -p assistant-core arch::` → 10/10 passed。
- `verify-schemas` / `codegen --check` → PASSED，0 error / 0 drift。
- `hygiene` → PASSED，0 error / 151 warning；新增 runner 拆分后无 file-too-long。
- `memory-counts` → PASSED（facts 271/218、pitfalls 294/183；paint 既有归档 Warning）。
- `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-comments` / `check-migrations` → 全部 exit 0。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。

### 4. DoD 逐条核对

- [x] six-step T5.1 browser package 在脚本化 CDP runtime 上全部 `Committed`。
- [x] navigate 使用 `cdp_target` scope、真实前后 fingerprint 与 elapsed；dom_read
  `state_unchanged` 可求值。
- [x] binding 捕获 `page_1_ready` / `page_1_dom` 等声明输出，结果包含三页 DOM。
- [x] 输出文件新建并读回；已存在路径、缺输出、验证失败与 ToolBus 失败均显式退出。
- [x] 成功和失败路径都会尝试 `ToolBus` shutdown 与 browser close；启动/连接后失败由
  `LaunchedBrowser` 回收进程树。
- [x] 全量验收通过；TASK-056 仍保持 Blocked。

### 5. 偏差

- 本会话探查阶段已有 7 条只读命令误填 bash，均在命令查找/解析阶段失败，未写仓库。
- `BrowserRegistryBuild::tool_schemas()` 还包含 11 个 reserved runtime schemas；runner
  已按当前 family 的 `edge.cdp.` / `chrome.cdp.` 过滤为恰好三个后才启动 ToolBus。
- 本卡只导出步骤输出 JSON，不生成 TASK-056 的最终 CSV；未把中间 runner 冒充真机结果。

### 6. 更合理做法

- 复用现有 `RuntimeExecutor` / `BindingInvoker` / `ToolBusInvoker`，runner 只负责私有 CLI
  参数、bounded browser 生命周期与最终输出文件，不另建执行状态机。
- browser handler 自己发布 fingerprint 与 scope，通用 observation 只消费显式字段；
  这样没有为 browser 单独分叉 verify 引擎。

### 7. 遗留问题

- TASK-056 仍缺 CSV 规范化与人类在场的真实 Edge / 外部 smoke。
- TASK-057 的 form_fill / submit-stop 与 TASK-058 的安全 CI 仍未实现。

### 8. 新增长期记忆

- FACT：browser runner 已在脚本化六步 ToolBus 上完成提交与步骤输出导出。
- PITFALL：Planner 的 retained runtime schemas 不能拿来作为 ToolBus 挂载名单。

### 9. 给审阅者的关注点

- 不得把脚本化 runtime 的六步提交当成真实 Edge 验收。
- 重点检查 fingerprint scope、失败清理与输出文件 create-new 后置条件。
