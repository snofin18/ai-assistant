# TASK-110　专用浏览器启动与进程树回收

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：049、109、327　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-049、TASK-109、TASK-327
- **write scope**：`crates/browser-profile/**`、`crates/platform/windows/src/browser_launcher{,_tests}.rs`、`crates/platform/windows/src/lib.rs`、`crates/platform/windows/Cargo.toml`、`crates/platform/windows/README.md`、`crates/platform/windows/tests/**`、本卡与状态同步文件
- **关联**：`plans/stage-1-pilots.md` 子阶段 1c、`docs/PARKING_LOT.md` `PL-120`、`docs/spike-reports/SPIKE-G.md`、`target-apps-feasibility.md` §3 P5

**目标**

在 `assistant-platform-windows` 内提供专用 Edge/Chrome 进程启动器：只使用调用方指定的独立 profile 根目录，使用动态 `--remote-debugging-port=0`，有界读取浏览器生成的 `DevToolsActivePort`，并在显式关闭或 `Drop` 时终止整棵进程树后 reap。

**必须遵守**

1. 拒绝空、NUL、路径分隔符或 `.` / `..` 形式的 profile id；专用 profile 目录必须落在调用方给出的专用根目录下。
2. 拒绝标准 Edge/Chrome 用户 profile 根目录及其子目录；代码中不得出现复制 Cookie、凭据、登录库或用户 profile 的路径。
3. 浏览器命令参数由固定白名单构造，不接受 shell 字符串或任意用户追加参数；启动默认页面为 `about:blank`。
4. `DevToolsActivePort` 是不可信文件：限制大小、行数、端口范围和 browser WebSocket path；解析失败必须显式失败。
5. 启动失败、早退、超时和关闭路径都必须尝试终止并 `wait()` reap；Windows 使用 `taskkill /T /F`，失败时不得静默成功。
6. 不新增第三方依赖、不修改 `CdpProvider` trait、`ErrorCode`、Tool schema、IPC、DB schema 或 CI。

**In scope**

- 专用 profile 根的路径校验与浏览器可执行文件校验。
- 固定参数构造、动态 CDP 端口文件等待、启动结果对象与显式 close。
- 进程树终止、重试有界等待、Drop 兜底回收。
- 纯逻辑单测与 Windows 子进程树回收测试；不启动真实浏览器。

**Out of scope**

- 不启动真实 Edge/Chrome，不访问外部网络、不登录、不碰凭据或用户 profile。
- 不实现 production `browser.edge.cdp.*` handlers、ToolBus 接线、click、screenshot、download、close provider 行为。
- 不执行 TASK-056 的 10/10、外部 `books.toscrape.com` smoke 或任何 GUI 验收。

**DoD**

- [ ] launcher 的 path / arg / `DevToolsActivePort` / cleanup 负向测试覆盖。
- [ ] Windows 子进程树在关闭后被终止且已 reap；资源失败路径无孤儿进程。
- [ ] `cargo test -p assistant-browser-profile` 与 `cargo test -p assistant-platform-windows --lib browser_launcher` 全绿。
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
- [ ] `xtask` 文档门禁与 `cargo deny check` 全绿。
- [ ] LEDGER 追加一行；`PL-120` 记录 launcher 部分闭环；必要时追加 `docs/memory/{facts,pitfalls}.md`。

**验收命令**

```powershell
cargo test -p assistant-browser-profile
cargo test -p assistant-platform-windows --lib browser_launcher
cargo test -p assistant-platform-windows --test handle_discipline
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
【任务】TASK-110 专用浏览器启动与进程树回收
【目标】专用 profile 启动器 + 动态 DevToolsActivePort + 有界进程树回收
【write scope】仅：crates/browser-profile/**、crates/platform/windows/{src/browser_launcher.rs,src/lib.rs,Cargo.toml,README.md,tests/**}、本卡与状态同步文件
【铁律】专用 profile / 禁止复制用户 profile；不可信文件先校验；资源有界并 reap；不静默失败
【禁止】真实浏览器 / 外部网络 / 凭据 / production handlers / 公共 trait 或 schema / 第三方依赖 / TASK-056
【验收】专项测试、workspace fmt/clippy/test、8 项 xtask 文档门禁、cargo deny check
【依赖】TASK-049、TASK-109、TASK-327 已 Done
【疑问】无；按人类预授权离线推进
```

### 2. 实际改动文件

- `crates/browser-profile/src/profile.rs`
- `crates/browser-profile/README.md`
- `crates/platform/windows/src/browser_launcher.rs`（新增）
- `crates/platform/windows/src/browser_launcher_tests.rs`（新增）
- `crates/platform/windows/src/lib.rs`
- `crates/platform/windows/Cargo.toml`
- `crates/platform/windows/README.md`
- `Cargo.lock`
- `tasks/TASK-110-browser-launcher-process-cleanup.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/PARKING_LOT.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-browser-profile` → 4 passed / 0 failed。
- `cargo test -p assistant-platform-windows --lib browser_launcher` → 7 passed / 1 ignored（ignored 项仅作为同一测试二进制内的子进程 helper）。
- `cargo test -p assistant-platform-windows --test handle_discipline` → 7/7。
- `cargo test -p assistant-platform-windows --test cdp_provider_contract` → 6/6。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有 `clippy::assert_is_empty` unknown-lint 警告。
- `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings` → exit 0。
- `cargo test --workspace` → 0 failed。
- `cargo deny check` → advisories / bans / licenses / sources ok。
- 8 项 xtask 文档门禁 → 全绿（最终计数见 LEDGER）。

### 4. DoD 逐条核对

- [x] launcher 的 path / arg / `DevToolsActivePort` / cleanup 负向测试覆盖。
- [x] Windows 子进程树在关闭后被终止且已 reap；失败路径统一走有界终止。
- [x] profile crate 与 launcher 专项测试全绿。
- [x] fmt / clippy / workspace test 全绿。
- [x] xtask 文档门禁与 `cargo deny check` 全绿。
- [x] LEDGER、`PL-120`、FACT / PITFALL 与规模表已同步。

### 5. 偏差

- 本会话共有 **22** 条只读 / 验收命令误填 `shell: "bash"`；均在执行前因 `cargo` / `rg` / PowerShell cmdlet 不存在、或脚本语法不兼容而失败，未写仓库，随后均改用显式 PowerShell 重跑。
- 首轮 launcher 专项测试发现两个实现缺陷：Windows canonicalized user-data 根带 verbatim 前缀，导致默认 profile 包含判断失效；进程树测试用 `OpenProcess` 存在性会把已终止但句柄仍存活的进程误判为运行中。已分别改为路径前缀归一化与 `GetExitCodeProcess + STILL_ACTIVE`，未放宽测试判据。
- 无契约冲突、超 write scope、真实 GUI、凭据或商业应用操作。

### 6. 更合理做法

- 把启动器放在 Windows 平台层而不是继续扩张纯值对象 `assistant-browser-profile`：profile crate 保持无 I/O，浏览器命令、动态端口文件与 Windows 进程树生命周期留在唯一 FFI 平台层。
- 使用 `--remote-debugging-port=0` + `DevToolsActivePort`，避免固定端口与其他浏览器实例冲突；browser path 也只从该文件读取并严格校验。

### 7. 遗留问题

- `browser.edge.cdp.*` production handlers 与 ToolBus 接线尚未实现。
- provider 的 `click` / `capture_screenshot` / `set_download_directory` / `close` 仍显式 `CapabilityMissing`。
- 真实 Edge、专用 profile 首次登录、`books.toscrape.com` smoke 与 TASK-056 10/10 仍需人类在场验收。

### 8. 新增长期记忆

- FACT：`DevToolsActivePort` 由动态端口与 browser-level WebSocket path 两行组成。
- PITFALL：Windows 默认 user-data 根 canonicalize 后可能出现 `\\?\` 前缀；仅按字符串 `starts_with` 比较会漏判，必须按路径组件并归一化前缀。
- PITFALL：进程对象在终止后可能因打开句柄短暂仍可 `OpenProcess`；运行态测试必须看 `GetExitCodeProcess` / `STILL_ACTIVE`，不能把“可打开句柄”等同于“仍在运行”。

### 9. 给审阅者的关注点

1. `BrowserLauncher::prepare_profile` 同时做 lexical 与 canonical 的默认 user-data 根拒绝；这是 no-copy 红线的运行时边界。
2. `terminate_child_tree` 对 taskkill 非成功不会静默返回成功；显式 `close` 可报告失败，`Drop` 只做有界兜底回收。
3. 测试没有启动真实 Edge；`cargo test --workspace` 中的 ignored helper 由测试断言主动拉起并验证 root + child 均退出。
