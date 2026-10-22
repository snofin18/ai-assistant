# TASK-107　Browser task package assertion mapping

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c support**
- 依赖：TASK-048 / TASK-055 Done；TASK-056 离线 contract WIP
- 关联：ADR-0058 D2；`adapters/browser.edge/tasks/t5.1.books-list-to-csv.json`

---

## 目标

让 `TaskPackageProvider` 能确定性渲染 browser CDP 的导航、DOM 读取和 network-idle
工具，而不是在加载 task package 时以 `MissingAssertion` fail-closed。此卡只补
task-package assertion 映射与其契约测试，不启动浏览器、不访问网络、不修改工具 schema。

## In scope

- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/tests/task_package.rs`
- 本卡执行记录区与 `LEDGER.md`

## Out of scope

- 不新增或修改 browser tool schema。
- 不修改 `browser.edge` / `browser.chrome` adapter pack。
- 不启动 Edge/Chrome、不连接 CDP、不访问 `books.toscrape.com`。
- 不实现 form fill、download、screenshot 或 shell read 的 assertion。
- 不改变 `verify` 支持的后置断言集合。

## 必须遵守

- 只映射已注册且已在 TASK-056 contract 中使用的工具。
- 每个渲染出的 postcondition 必须能被 `assistant_verify::parse_postconditions` 接受。
- 未知 browser tool 继续 fail-closed，禁默认成功。
- 不修改测试断言以绕过失败；若现有契约冲突，记录 DRIFT 并停止。

## 步骤

1. 为 Edge/Chrome 的 `cdp.navigate` 映射
   `state_changed`（`cdp_target`，3000 ms）。
2. 为 Edge/Chrome 的 `cdp.dom.read` 映射 `state_unchanged`。
3. 为 Edge/Chrome 的 `cdp.wait_network_idle` 映射 `state_unchanged`。
4. 用 synthetic package 覆盖 6 个 Edge/Chrome 工具断言形状。
5. 从文件加载 TASK-056 的 T5.1 package，断言 6 个 executable steps 可渲染且
   所有 postcondition 被 verify 解析。
6. 运行本卡专项测试、workspace 测试和文档门禁。

## DoD

- [ ] Edge/Chrome navigate / dom.read / wait_network_idle 均可被
  `TaskPackageProvider` 渲染。
- [ ] TASK-056 的 T5.1 package 文件可被加载，不再 `MissingAssertion`。
- [ ] 未知 browser tool 仍报 `MissingAssertion`。
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace` 全部 exit 0。
- [ ] `hygiene / memory-counts / adr-index / refscan / docscan / card-check /
  check-ledger / check-comments` 全部 0 error。
- [ ] LEDGER 追加 Done 事件；不改写历史行。

## 验收命令

```powershell
cargo test -p assistant-agent-core --test task_package
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
cargo run -p xtask -- check-comments
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-107 Browser task package assertion mapping
【目标】让 TaskPackageProvider 渲染 browser navigate/dom.read/network-idle，并解除 TASK-056 的 MissingAssertion 阻塞
【write scope】仅：apps/agent-core/src/task_package_render.rs、apps/agent-core/tests/task_package.rs、本卡执行记录、LEDGER.md
【铁律】无静默失败；不可信输入先校验；不得放宽断言；契约先行；不扩大公共接口
【禁止】不启动浏览器/不访问网络；不改 browser tool schema、adapter pack、verify 断言集合或其他 runtime 行为
【验收】专项测试；fmt/clippy/workspace test；全部 xtask 文档门禁 → 全绿
【依赖】TASK-048 / TASK-055 Done；TASK-056 离线 contract WIP，已核对 LEDGER
【疑问】无；按预授权直接实现
```

### 2. 实际改动文件

- `apps/agent-core/src/task_package_render.rs`：新增 Edge/Chrome browser assertion 映射。
- `apps/agent-core/tests/task_package.rs`：新增 3 个 browser mapping 契约测试。
- `tasks/TASK-107-browser-task-package-assertion-mapping.md`：本卡记录。
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`：Done 状态同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --test task_package` → **23 passed / 0 failed**。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 `clippy::assert_is_empty` unknown lint warning。
- `cargo test --workspace` → exit 0。
- `hygiene` 0E/137W、`memory-counts` 0E/1W、`adr-index` 0E/0W、`refscan` 0E/0W、`docscan` 0E/184W、`card-check` 0E、`check-ledger` 0E、`check-comments` 0E。
- TASK-056 的 `t5.1.books-list-to-csv.json` 已由测试真实加载并渲染 6 个 executable steps，不再 `MissingAssertion`。

### 4. DoD 逐条核对

- [x] Edge/Chrome navigate / dom.read / wait_network_idle 均可被 `TaskPackageProvider` 渲染。
- [x] TASK-056 的 T5.1 package 文件可加载，不再 `MissingAssertion`。
- [x] 未知 browser tool `cdp.form.fill` 仍 fail-closed 为 `MissingAssertion`。
- [x] fmt / clippy / workspace test 全部 exit 0。
- [x] 全部 xtask 文档门禁 0 error。
- [x] LEDGER 追加 Done 事件；不改写历史行。

### 5. 偏差

- 本会话有 **2** 条只读命令误用默认 bash：目录枚举与 `Get-Content` 均因 PowerShell cmdlet 不存在而失败，未写仓库；随后均以显式 PowerShell 重跑。按用户最新规则，后续所有命令继续强制 PowerShell 优先。

### 6. 更合理做法

- 使用 `browser.edge.` / `browser.chrome.` 公共前缀解析 operation，避免为两个浏览器复制断言表；未映射 operation 继续返回 `None`，保持 fail-closed。

### 7. 遗留问题

- TASK-056 的 adapter README、任务卡 `DRIFT-056-2` 与状态需要在独立 docs-only 收口提交中同步为“mapping 已解锁、真实 GUI 待执行”。

### 8. 新增长期记忆

- 无。TASK-056 已登记“工具注册不等于 task package 可执行”的 PITFALL；本卡只解除该阻塞。

### 9. 给审阅者的关注点

- navigate 使用 `state_changed` + `cdp_target` + 3000 ms；dom.read / network-idle 使用 `state_unchanged`。
- 测试同时覆盖 Edge 与 Chrome，并确认未映射的 `form.fill` 仍失败。
- 本卡没有执行真实浏览器；真实 Edge/CDP 验收仍归 TASK-056。
