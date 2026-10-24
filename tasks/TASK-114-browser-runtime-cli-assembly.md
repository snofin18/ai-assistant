# TASK-114　CLI browser runtime 生产装配

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：TASK-049、TASK-107、TASK-109、TASK-110、TASK-112、TASK-327　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-049、TASK-107、TASK-109、TASK-110、TASK-112、TASK-327
- **write scope**：`apps/agent-core/src/browser_runtime.rs`、`apps/agent-core/src/browser_runtime_tests.rs`、`apps/agent-core/src/main.rs`、`apps/agent-core/Cargo.toml`、本卡与状态同步文件
- **关联**：`PL-120`、`PLAN.md`「下一步动作」、`crates/platform/windows/src/browser_launcher.rs`、`crates/platform/windows/src/cdp/runtime.rs`、`apps/agent-core/src/browser_registry.rs`

**目标**

在 `assistant-agent-core` 的 CLI 二进制私有装配层新增显式浏览器 runtime 模式，把专用 profile launcher、真实
`RuntimeCdpProvider` 连接和 Edge/Chrome 三工具 registry 串成一个可关闭的生产生命周期；自动化测试不启动
真实浏览器。

**必须遵守**

1. 浏览器必须由 `BrowserLauncher` 使用调用方给出的专用 profile 根启动；不得读取、复制或附加用户日常 profile。
2. CDP 连接只允许 launcher 发布的 `127.0.0.1` 动态端口；页面目标固定为启动参数中的 `about:blank`。
3. registry 只允许 `navigate`、`dom_read`、`wait_network_idle` 三个已实现工具；任何 adapter 声明或注册失败必须显式失败。
4. 启动后连接或 registry 构建失败时，已启动浏览器必须被终止并 reap；成功路径在 CLI 检查完成前显式关闭。
5. 不修改既有 public trait、Tool schema、ErrorCode、IPC、DB schema、ADR、spec 或 CI。
6. 不新增第三方依赖；只允许把工作区内既有 `assistant-browser-profile` 声明为 `assistant-agent-core` 的直接依赖。

**In scope**

- `--browser-runtime-check` CLI 模式及 Edge/Chrome 参数解析。
- 专用 profile launcher、动态 CDP 端口、`RuntimeCdpProvider::connect` 与 browser registry 的串联。
- 浏览器 family / adapter root / executable / profile 输入的离线校验。
- 三工具 registry 的无浏览器专项测试，以及成功、失败路径的生命周期绑定。
- 进程树回收继续复用 TASK-110 已验证的 `LaunchedBrowser` / `Drop` / bounded `close`。

**Out of scope**

- 不启动自动化测试中的真实 Edge/Chrome，不访问外网、不登录、不碰凭据。
- 不实现 click、form fill、download、screenshot、close provider 行为。
- 不执行 TASK-056 的 10/10、`books.toscrape.com` smoke 或任何 GUI 验收。
- 不重构现有 Notepad/Paint production composition root，不把浏览器 registry 并入 `ProductionConfig`。

**DoD**

- [ ] 显式 CLI 模式能解析 Edge/Chrome、可执行文件、专用 profile 根、profile id、adapter root 与有界 session timeout。
- [ ] 缺参、非法 family、非法 profile id、相对 profile 根、非法 timeout 均显式失败。
- [ ] registry 装配测试证明 Edge/Chrome 各只含三个已实现工具，且不启动浏览器。
- [ ] 启动后任一连接/注册失败均触发 `LaunchedBrowser` 生命周期回收；成功路径显式 `close`。
- [ ] fmt / clippy / workspace test、8 项 xtask 文档门禁与 `cargo deny check` 全绿。
- [ ] LEDGER、`PL-120` 部分闭环、FACT / PITFALL 与规模表按实际情况同步。

**验收命令**

```powershell
cargo test -p assistant-agent-core --bin assistant-agent-core browser_runtime
cargo test -p assistant-agent-core --lib browser_registry
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
【任务】TASK-114 CLI browser runtime 生产装配
【目标】专用 launcher + 真实 RuntimeCdpProvider + Edge/Chrome registry 的 CLI 生命周期
【write scope】仅：browser runtime 私有模块 / main.rs / Cargo.toml / 本卡与状态同步文件
【铁律】输入先校验；错误不吞；子进程终止并 reap；不静默扩大范围；状态证据真实
【禁止】public trait/schema/ErrorCode 改动；测试期真实 GUI/外网/登录；未实现 browser 动作
【验收】专项测试、workspace fmt/clippy/test、8 项 xtask、cargo deny
【依赖】TASK-049/107/109/110/112/327 已 Done
【疑问】无；仅新增既有 workspace crate 的直接依赖，不引入第三方依赖
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_runtime.rs`（新增）
- `apps/agent-core/src/browser_runtime_tests.rs`（新增）
- `apps/agent-core/src/main.rs`
- `apps/agent-core/Cargo.toml`
- `Cargo.lock`（仅新增既有 workspace crate 的内部依赖边）
- `tasks/TASK-114-browser-runtime-cli-assembly.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/PARKING_LOT.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`
- `docs/automations/2026-10-24-round-2.md`

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --bin assistant-agent-core browser_runtime` → 5 passed / 0 failed。
- `cargo test -p assistant-agent-core --lib browser_registry` → 5 passed / 0 failed。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 unknown-lint warning。
- `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-protocol --all-targets -- -D warnings` → exit 0。
- `cargo clippy --target aarch64-apple-darwin -p assistant-protocol --all-targets -- -D warnings` → exit 0。
- `cargo test --workspace` → exit 0，0 failed；仅既有真机 ignored 用例。
- `cargo test -p assistant-core arch::` → 10/10 passed。
- `cargo run -p xtask -- verify-schemas` → 0 error，PASSED。
- `cargo run -p xtask -- codegen --check` → 0 drift / 0 error，PASSED。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。
- `cargo llvm-cov --workspace --fail-under-lines 75` → exit 0；workspace 行覆盖 **80.04%**。
- `cargo run -p xtask -- replay --suite core` → 2 steps / 5 changes / 0 mismatch，PASSED。
- `pnpm lint` / `pnpm typecheck` / `pnpm test`（`apps/desktop-ui`）→ 三项 exit 0；78 Node tests + 3 Vitest tests passed。
- 8 项 xtask 文档门禁 → 0 error；既有 warnings 未新增为 error。

### 4. DoD 逐条核对

- [x] 显式 CLI 模式解析 Edge/Chrome、可执行文件、专用 profile 根、profile id、adapter root 与有界 session timeout。
- [x] 缺参、非法 family、非法 profile id、相对路径与非法 timeout 均显式失败。
- [x] Edge/Chrome registry 各只含三个已实现工具；Planner catalog 的 11 个保留 schemas 保留。
- [x] 成功路径显式 close；连接或 registry 构建失败时 `LaunchedBrowser` 已在作用域内，按既有 `Drop` 回收进程树。
- [x] fmt / clippy / workspace test、8 项 xtask 文档门禁与 `cargo deny check` 全绿。
- [x] LEDGER、`PL-120`、FACT / PITFALL 与规模表同步。

### 5. 偏差

- 本轮 `shell_misuse=28`：均为只读命令或一次锁创建命令误填 `shell: "bash"`；bash 缺少 `cargo` / `rg` / PowerShell cmdlet，或在解析阶段失败，均未执行目标写入、未伪造输出。所有实际验收与写入均使用显式 PowerShell 重跑。
- 首轮 PR CI 的 Ubuntu/macOS clippy 在 `browser_launcher` 不存在处失败：该类型仅在 `cfg(windows)` 下编译。已把私有 `browser_runtime` 模块同样设为 Windows-only，并让非 Windows 的同名 CLI flag 显式返回不支持；Windows 专项 5/5、host clippy 与 workspace tests 重跑全绿。
- 无契约冲突、超 write scope、公共 trait/schema/ErrorCode、真实 GUI、外网、登录或凭据操作。
- `assistant-agent-core` 仅新增对工作区内既有 `assistant-browser-profile` 的直接依赖；无第三方依赖变化。

### 6. 更合理做法

- 把浏览器装配放在 CLI 二进制私有模块，而不是扩张公开 `ProductionConfig` / `AdapterKind`：现有 Notepad/Paint composition root 保持零改动，也避免公共接口变更。
- 真实浏览器只在显式 `--browser-runtime-check` 时启动；自动化测试只验证参数和三工具 registry，避免把离线 fixture 冒充真实 Edge 证据。

### 7. 遗留问题

- click / screenshot / download / close provider 行为与 form fill 仍未实现，保持 unmounted / `CapabilityMissing`。
- TASK-056 的真实 Edge 10/10、专用 profile 人工登录、外部 `books.toscrape.com` smoke 仍需人类在场。
- 完整任务执行尚未切换到 browser runtime；TASK-114 只完成 CLI runtime 自检生命周期与 registry 装配。

### 8. 新增长期记忆

- FACT：CLI browser runtime 已能串联专用 launcher、真实 loopback provider 与三工具 registry，并在成功/失败路径绑定进程树回收。
- PITFALL：`BrowserRegistryBuild::tool_schemas()` 含 11 个 reserved runtime schemas；浏览器工具范围必须看 registry 和 browser 前缀过滤，不能看 catalog 总数。

### 9. 给审阅者的关注点

1. 公开库 API 未改；新类型都位于私有 binary module，CLI 之外的调用方不可见。
2. 连接或 registry 失败时依靠 `LaunchedBrowser` 的既有 Drop 测试过的清理路径；成功路径显式调用 bounded `close`。
3. 本轮没有启动真实 Edge/Chrome，不能作为 TASK-056 真机证据。
