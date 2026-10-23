# TASK-112　Browser CDP production handlers 与 ToolBus 注册

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：TASK-107、TASK-109、TASK-110　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-107 Done、TASK-109 Done、TASK-110 Done
- **write scope**：`apps/agent-core/src/browser_handlers.rs`、`apps/agent-core/src/browser_registry.rs`、`apps/agent-core/src/browser_registry_tests.rs`、`apps/agent-core/src/task_package_render.rs`、`apps/agent-core/tests/task_package.rs`、`apps/agent-core/src/lib.rs`、`adapters/browser.edge/**`、`adapters/browser.chrome/**`、`docs/spec/naming.md`、本卡与状态同步文件
- **关联**：`PL-120`、`adapters/browser.edge/tools/tools.json`、`adapters/browser.chrome/tools/tools.json`、`docs/spec/tool-schema.md`

**目标**

在 binary assembly 层实现 `navigate`、`dom.read`、`wait_network_idle` 三个 production handler，并提供可从 Edge/Chrome `tools.json` 注册到真实 `ToolRegistry` 的 builder。`ToolBus` 调用路径用本地 fake CDP runtime 验证，不启动浏览器。

**In scope**

- `navigate`：校验绝对 HTTP(S) URL 与 load condition，调用既有 `CdpProvider::navigate`。
- `dom.read`：把 selector / `max_nodes` 转成受限 DOM 截取表达式，调用既有 `evaluate_dom`，解析并再次校验 `{dom,node_count}`。
- `wait_network_idle`：调用 `RuntimeCdpProvider::wait_network_idle`，限制 1..=10000 ms。
- Edge / Chrome 工具名前缀映射与 `DeclaredToolSelection::Subset` 注册；声明中未实现的 form fill / download / screenshot 不挂载。
- `ToolRegistry` 与完整 `ToolBus::call_tool` 测试；DOM 输出标记为 `app_content` 不可信来源。
- adapter task README 移除“RuntimeCdpProvider 仍返回 synthetic”的过期说明。

**Out of scope**

- 不启动真实 Edge/Chrome、不连接外部网络、不登录、不读取用户 profile。
- 不实现 form fill、download、screenshot、click、close provider 方法。
- 不修改 `CdpProvider` trait、Tool schema、ErrorCode、数据 schema、ADR、CI 或第三方依赖。
- 不把新 registry builder 自动接入 CLI `ProductionConfig`；专用浏览器生产装配与人工验收另卡。

**DoD**

- [ ] 三个 handler 的成功、非法参数、provider 错误和 malformed DOM 负向测试覆盖。
- [ ] Edge / Chrome registry 都只挂载三个已实现工具，未实现工具保持 unmounted。
- [ ] 完整 `ToolBus::call_tool` 路径可执行 navigate / dom.read / wait_network_idle。
- [ ] DOM 输出为 `untrusted=true` 且来源为 adapter 的 `app_content`。
- [ ] fmt / clippy / workspace test、8 项 xtask 与 `cargo deny check` 全绿。
- [ ] LEDGER、`PL-120` 部分闭环、FACT / PITFALL 与规模表同步。

**验收命令**

```powershell
cargo test -p assistant-agent-core --lib browser_registry
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
cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-112 Browser CDP production handlers
【目标】navigate / dom.read / wait_network_idle handler + Edge/Chrome ToolBus registry builder
【write scope】仅：browser handlers / registry / tests / lib.rs、Edge task README、本卡与状态同步文件
【铁律】输入先校验；provider 错误不吞；DOM 标记不可信；未实现工具 fail-closed
【禁止】真实浏览器 / 外部网络 / 凭据 / form-fill / download / screenshot / trait 或 schema 变更
【验收】专项与完整 ToolBus 测试、workspace fmt/clippy/test、8 项 xtask、cargo deny
【依赖】TASK-107、TASK-109、TASK-110 已 Done
【疑问】无；按离线 fake runtime 验证
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_handlers.rs`（新增）
- `apps/agent-core/src/browser_registry.rs`（新增）
- `apps/agent-core/src/browser_registry_tests.rs`（新增）
- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/tests/task_package.rs`
- `apps/agent-core/src/lib.rs`
- `adapters/browser.edge/tools/tools.json`
- `adapters/browser.edge/app_map.json`
- `adapters/browser.edge/tasks/t5.1.books-list-to-csv.json`
- `adapters/browser.edge/tasks/README.md`
- `adapters/browser.edge/tests/validate_adapter_packs.py`
- `adapters/browser.chrome/tools/tools.json`
- `adapters/browser.chrome/app_map.json`
- `docs/spec/naming.md`
- `tasks/TASK-112-browser-production-handlers.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/PARKING_LOT.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --lib browser_registry` → 5/5 passed。
- `cargo test -p assistant-agent-core --test task_package` → 23/23 passed。
- `python adapters/browser.edge/tests/validate_adapter_packs.py` → `PACK OK browser.edge` / `PACK OK browser.chrome`。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint 警告。
- `cargo test --workspace` → 0 failed。
- `cargo deny check` → advisories / bans / licenses / sources ok。
- 8 项 xtask 文档门禁 → 全绿（最终计数见 LEDGER）。

### 4. DoD 逐条核对

- [x] 三个 handler 的成功、非法参数、provider 错误映射和 malformed DOM 负向测试覆盖。
- [x] Edge / Chrome registry 都只挂载三个已实现工具，未实现工具保持 unmounted。
- [x] 完整 `ToolBus::call_tool` 路径执行 navigate / dom.read / wait_network_idle。
- [x] DOM 输出为 `untrusted=true`，来源为 adapter 的 `app_content`。
- [x] fmt / clippy / workspace test、8 项 xtask 与 `cargo deny check` 全绿。
- [x] LEDGER、`PL-120`、FACT / PITFALL 与规模表同步。

### 5. 偏差

`DRIFT-112-1`

- 现象：adapter `tools.json` 与 browser task package 使用四段名 `browser.edge.cdp.navigate` / `browser.chrome.cdp.navigate`，但 `ToolRegistry::register` 按 `docs/spec/naming.md` 只接受三段 `<app>.<domain>.<action>`，真实注册返回 `InvalidToolName`。
- 影响：不裁决命名契约就无法把 browser handler 挂进 `ToolBus`；继续改测试或放宽注册规则会违反“契约先行”和“不得为通过测试改断言”。
- 建议：默认建议把 browser CDP 工具名统一为三段（如 `edge.cdp.navigate` / `chrome.cdp.navigate`），同步 adapter tools、task package、task-package assertion mapping 与测试；若必须保留 `browser.*`，则先以 ADR 扩展/豁免命名契约并同步 spec。
- 已停工作：停止 registry 集成与后续代码编辑，未启动真实浏览器，未修改 `Cargo.toml`、trait/schema/ErrorCode/CI。

**裁决（2026-10-23，人类）**：采用三段 `edge.cdp.*` / `chrome.cdp.*` 与 `edge.shell.*_read` / `chrome.shell.*_read`，保留 `browser` 仅作 tags / adapter 分类，不占用 `<app>` 段。已同步两个 adapter、task package、assertion mapping 与 `docs/spec/naming.md` 示例；DRIFT 解除，任务恢复 InProgress。

本轮启动、迁移与验收共有 **18** 条只读命令误填 `shell: "bash"`；均在执行前失败、未写仓库，随后以显式 PowerShell 重跑。

### 6. 更合理做法

- 使用内部同步 `BrowserCdpRuntime` 边界把现有 RPITIT provider 适配给同步 `ToolHandler`，既不改 `CdpProvider`，也能用 fake runtime 做确定性的完整 ToolBus 测试。
- 把 `browser` 留在 tags / adapter 分类中，而工具名使用真正的 `<app>.<domain>.<action>`，保留 `MountSelection::Apps("edge"|"chrome")` 语义。

### 7. 遗留问题

- form fill、download、screenshot、click 与 provider close 尚未实现，继续保持 unmounted / `CapabilityMissing`。
- registry builder 尚未接入 CLI `ProductionConfig`；专用浏览器完整生产装配与真实 Edge 验收仍需后续卡。
- TASK-056 的 10/10、外部 3/3 smoke 与人工登录仍需人类在场。

### 8. 新增长期记忆

- FACT：`ToolRegistry` 三段名允许 action 内使用下划线，browser 工具统一为 `edge.cdp.navigate` / `edge.cdp.dom_read` / `edge.cdp.wait_network_idle`。
- PITFALL：四段 browser 工具名会在注册期被硬拒绝；浏览器族必须放在 `<app>` 段，不能把 `browser` 当作共享 app 前缀。

### 9. 给审阅者的关注点

1. DOM 读取表达式把 selector 作为 JSON 字符串嵌入，并在浏览器侧按 `max_nodes` 裁剪克隆树；返回载荷再次校验 node_count。
2. `BrowserRegistryBuild` 只公开 registry 与 Planner schema；未实现工具即使仍在 adapter 声明中也不会挂载。
3. fake runtime 测试没有启动真实浏览器，不能作为 TASK-056 真机证据。
