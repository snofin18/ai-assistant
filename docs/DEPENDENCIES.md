# DEPENDENCIES.md — 第三方依赖登记表

> **用途**：每一个进入仓库的第三方依赖都必须在这里有一行。
> **为什么**：AI agent 最容易做的一个"图方便"动作就是 `cargo add` / `pnpm add`。
> 没有登记表，依赖会悄悄堆积，随之而来的是供应链风险、许可证污染、编译时间膨胀，
> 以及"没人知道为什么用了这个库"。登记表让每一次引入都留下**理由**与**批准人**。
> **门禁**：`cargo deny check`（gov §5.1 #8）+ 未来的 `xtask hygiene` 规则
> 「新增依赖必须已登记」（gov §5.4，归属 TASK-015）。

## 登记规则

1. **先登记，后引入**：在本表加行 → 再改 `Cargo.toml` / `package.json`。顺序反了算漂移。
2. 一个依赖一行；同一个 crate 被多个包使用时，"使用方"列写全部。
3. `替代方案与否决理由` 是**必填**：它回答"为什么不用 std / 为什么不用另一个库"。
   写不出替代方案的依赖，说明还没想清楚要不要引入。
4. 许可证必须在 `deny.toml` 的白名单内（MIT / Apache-2.0 / BSD / ISC / Zlib / MPL-2.0）。
5. 删除依赖时**不删行**：把状态改为 `Removed` 并写日期，保留决策历史。

## Rust（cargo）

| crate | 版本要求 | 使用方 | 用途（一句话） | 许可证 | 替代方案与否决理由 | 状态 | 批准 | 日期 |
|---|---|---|---|---|---|---|---|---|
| — | — | — | **`xtask` 仍刻意 std-only**（零供应链风险 + 编译 <1s）；产品侧首批第三方依赖从 2026-09-23 起逐行登记在本表下方（`serde` / `serde_json` / `rusqlite` / `zstd` / `sha2`）；再下面一行属 `spikes/`（被根 `Cargo.toml` `exclude`，不在产品 workspace 内） | — | — | — | — | 2026-09-16 |
| `serde` | **`1.0`**（caret 语义：允许 1.x 内升级；`Cargo.toml` 与 `Cargo.lock` 实测一致） | `crates/protocol` / `crates/platform/api`（TASK-016） / `crates/ipc`（TASK-019） / `crates/tool-bus`（TASK-020） / `crates/task-engine`（TASK-022） / `crates/verify`（TASK-023） | serde derive + `serde_json::Value` for envelope.data / audit_event.target 等开放字段；**TASK-022**：检查点快照、Plan 与状态枚举的确定性序列化；**TASK-023**：`Observation` / `Postcondition` / `VerifyOutcome` 的确定性序列化（供审计与回放） | MIT OR Apache-2.0 | 替代 = 手写 Serialize/Deserialize impl = 出错率高且协议多版本难兼容 | **Approved** | 人类（chat 2026-09-23 TASK-103） | 2026-09-23 |
| `serde_json` | **`1.0`**（caret 语义：允许 1.x 内升级；`Cargo.lock` 实测解析到 1.0.151） | `crates/protocol` / `crates/audit` / `crates/ipc`（TASK-019） / `crates/tool-bus`（TASK-020） | JSON Value 类型（用于 envelope.data / error.details / metadata / audit_event.target 等开放字段）与 IPC 传输消息编码 | MIT OR Apache-2.0 | 替代 = serde_json::Value 是 serde_json 生态默认；不允许直接 String 传开放字段（违反 naming.md §5 newtype 约束）；IPC 需要同一份协议序列化路径 | **Approved** | 人类（chat 2026-09-23 TASK-103） | 2026-09-23 |
| `rusqlite` | **`0.37`**（caret；启用 `bundled` feature） | `crates/storage` / `crates/audit` | SQLite 驱动 = 主库（WAL）：任务 / 步骤 / 检查点 / 用量 / blob 元数据 | MIT | 替代 = `sqlx`：本卡只需**同步单写者**，async 驱动会连带引入运行时与编译期 SQL 校验的构建复杂度（`storage-design.md` §3.2 允许 `sqlx` 但非必需）；`bundled` = 自带 SQLite C 源码编译 → 三平台可复现、不依赖系统库 | **Approved** | 人类（chat 2026-09-24，TASK-012） | 2026-09-24 |
| `zstd` | **`0.13`**（caret） | `crates/storage` | 内容寻址 blob 池的压缩（level 3） | MIT | 替代 = `flate2`（zlib/gzip）：压缩比与解压速度均不如 zstd，且 `docs/storage-design.md` §3.3 已把「zstd level 3」定为口径 → 换库 = 改口径 | **Approved** | 人类（chat 2026-09-24，TASK-012） | 2026-09-24 |
| `sha2` | **`0.10`**（caret） | `crates/storage` / `crates/audit` / `crates/tool-bus`（TASK-020，工具集指纹） / `crates/verify`（TASK-023，状态指纹） | 内容寻址的 SHA-256 摘要（blob_id 与完整性校验）；**TASK-023**：§7.3 状态指纹 = canonical form 的 SHA-256（与审计 hash chain 同口径） | MIT OR Apache-2.0 | 替代 = `blake3`：更快，但 `sha256` 是**既定口径**（架构 v2 §15.3、审计 hash chain 同源），换算法 = 改契约 | **Approved** | 人类（chat 2026-09-24，TASK-012） | 2026-09-24 |
| `keyring` | **`4.2`**（caret；默认 feature `v1`） | `crates/secrets` | OS 密钥库后端（Windows 凭据管理器 / macOS Keychain / Linux Secret Service）；架构 v2 §12.5 明文指定 | MIT OR Apache-2.0 | 替代 = 自己写 DPAPI / Security.framework / Secret Service 三套 FFI：**需要 `unsafe`**（本 workspace `[lints.rust] unsafe_code = deny`，§5.2 只允许平台层局部放开），且三平台行为差异要自己扛；架构 §12.5 已指定 `keyring` → 换库 = 改口径 | **Approved** | 人类（chat 2026-09-24，TASK-014） | 2026-09-24 |
| `zeroize` | **`1.9`**（caret） | `crates/secrets` | 内存中的密钥用后清零；架构 v2 §12.5 明文指定 | Apache-2.0 OR MIT | 替代 = 手写 `Drop` 覆盖字节：编译器可把"写后不再读"的内存优化掉（dead-store elimination），手写清零**不可靠**；`zeroize` 用 volatile 写 + 内存屏障解决这一点 | **Approved** | 人类（chat 2026-09-24，TASK-014） | 2026-09-24 |
| `tokio` | **`1`**（caret；feature 白名单 `io-util` / `macros` / `rt` / `rt-multi-thread` / `sync` / `time`；`Cargo.lock` 实测 1.53.1） | `crates/tool-bus`（TASK-020） | in-process MCP 传输（`tokio::io::duplex` 内存管道）+ 两侧 serve 循环的运行时；`rmcp` 的 `serve_client` / `serve_server` 本身是 async API，必须有一个 runtime | MIT | 替代 = 不用 async 运行时：`rmcp` 的公开接口是 async 的（架构 v2 §5.4 已定 `rmcp`），改用同步自研传输 = 换库 + 改口径；feature 只开 6 项（**不**开 `fs` / `net` / `process`）→ 本 crate 结构上摸不到网络与子进程 | **Approved** | 人类（chat 2026-09-25 授权自决，TASK-020 Q1 连带） | 2026-09-25 |
| `rmcp` | **`3.4`**（caret；`default-features = false`，只开 `client` + `server`；`Cargo.lock` 实测 3.4.1） | `crates/tool-bus`（TASK-020） | MCP 官方 Rust SDK：`tools/list` / `tools/call`、生命周期握手与 `IntoTransport`（in-process 传输）。架构 v2 §5.4 明文把 MCP 定为**唯一**工具协议 | Apache-2.0 | 替代 = 手写 JSON-RPC 2.0 + MCP 生命周期：MCP 规范演进（2026-07-28 的 stateless-first / Extensions / Tasks 移出核心）要自己追，且「与生态一致」是本项目的目标之一；架构 v2 §5.4 已指定 `rmcp` → 换库 = 改口径。`default-features = false` + 不 enabled `http` / `reqwest` / `auth` → 零网络传输 | **Approved** | 人类（chat 2026-09-25 授权自决，TASK-020 Q1） | 2026-09-25 |
| `thiserror` | **`2`**（caret；`Cargo.lock` 实测 2.0.21） | `crates/tool-bus`（TASK-020） / `crates/task-engine`（TASK-022） / `crates/verify`（TASK-023） / `crates/hitl`（TASK-027） | `ToolBusError` / `TaskEngineError` / `VerifyError` / `HitlError` 用 `derive(Error)` 的 `Display` 文本（铁律 1 要求每个失败都带可读原因） | MIT OR Apache-2.0 | 替代 = 手写 `Display` + `std::error::Error`：带结构化字段的变体，手写既冗长又容易漏分支；std 没有字段插值式错误派生的等价物。**已登记在案**：`crates/core` 等的错误类型由各自任务卡按同一规则登记 | **Approved** | 人类（chat 2026-09-25 授权自决，TASK-020 Q5） | 2026-09-25 |
| `windows` | **`=0.62.2`**（精确钉定；0.x 版本间**有**破坏性变更，升级 = 漂移触发器） | `spikes/spike-a-notepad`（阶段 0）＋ `crates/platform/windows`（阶段 1，TASK-017 起为**产品代码**）＋ `crates/ipc`（TASK-019 的 NamedPipe / 对端进程查询） | Win32/WinRT 官方投影。Spike A 验证 UIA COM；产品侧提供 UIA、合成输入与 NamedPipe / `GetNamedPipeClientProcessId` / `QueryFullProcessImageNameW` / `BCryptGenRandom` | MIT OR Apache-2.0（`cargo deny check licenses` 2026-09-18 本机实测 `licenses ok`，exit 0） | 替代方案 = 第三方封装 crate `uiautomation`，**已否决**（ADR-0024 D1）；NamedPipe 与进程身份没有 std 等价物，第三次复用同一官方投影比手写 FFI 更可审计 | **Approved**（产品侧；TASK-017 Q1 升级，TASK-019 沿用） | 人类（指示 #6）；人类（chat 2026-09-24，TASK-017 Q1 批准） | 2026-09-18 / 2026-09-24 |
| `uiautomation` | — | — | （曾考虑用于 spike 的 UIA 访问） | 未核实 | **Rejected**：见 ADR-0024 D1 的对比表与裁决理由（决定性一条 = spike 必须走生产路径去撞墙） | **Rejected** | 人类（指示 #6） | 2026-09-18 |

> **TASK-023 零新增第三方 crate**：`crates/verify` 只是 `serde` / `sha2` / `thiserror` 三行的
> **新使用方**（三行均已 `Approved` 在案），未引入任何新 crate；按登记规则 2「使用方列写全部」
> 同批更新。记录为 `DRIFT-023-3`（沿用 TASK-016 的 `DRIFT-016-1` 先例）。

> **TASK-021 零新增第三方依赖**：`crates/policy` 只依赖 workspace crate `assistant-protocol`，
> 并复用它对 `serde_json` 的 re-export 做 DSL v0 的 JSON 解析；未直接引入新的第三方 crate。

> **TASK-027 零新增第三方 crate**：`crates/hitl` 使用已批准的 `thiserror`，并依赖
> workspace crate `assistant-policy` / `assistant-protocol` / `assistant-task-engine`；
> 只更新 `thiserror` 的使用方列，不新增供应链依赖。

> 计划中的依赖（**尚未引入**，引入时逐条登记并走漂移升级；已引入的不再列在这里）：
> `tauri`、`tracing`（`tokio` / `rmcp` / `windows` 已于 2026-09-25 由 TASK-020 / TASK-017 引入，见上表）。
> 它们的选型理由见 `cross-platform-ai-assistant-architecture-v2.md` §15 与 `docs/storage-design.md`。

> **`windows` crate 的 feature 名（最容易记错的一条，ADR-0024 D1a 实证）**：
> UI Automation 的**客户端 COM 绑定不在** `Win32_UI_UIAutomation`（**该 feature 不存在**，
> 0.62.2 是最新版，26 个 `Win32_UI_*` feature 里没有它），而在 **`Win32_UI_Accessibility`**
> —— 它与 MSAA、provider 侧接口同住一个模块。另外 `VARIANT` 被 `Win32_System_Ole` 双重 gate，
> 只开 `Win32_System_Com` 会报 `no VARIANT in Win32::System::Variant`。
> Spike A 实际启用的 7 项及其逐条理由见 `spikes/spike-a-notepad/Cargo.toml` 的行内注释。

## Node / 前端（pnpm）

| 包 | 版本要求 | 使用方 | 用途 | 许可证 | 替代方案与否决理由 | 状态 | 批准 | 日期 |
|---|---|---|---|---|---|---|---|---|
| — | — | — | 前端工程尚未开始（阶段 1 的 TASK-006 才引入 Tauri 原型） | — | — | — | — | 2026-09-16 |

## 开发工具（不进入产物，但影响 CI 与本地验收）

| 工具 | 版本 | 用途 | 安装方式 | 状态 |
|---|---|---|---|---|
| rustup / cargo / rustc | stable（`rust-toolchain.toml` 固定） | 构建与测试 | `winget install Rustlang.Rustup` | 已安装（1.98.1） |
| clippy / rustfmt / llvm-tools | 随 toolchain | lint / 格式化 / 覆盖率 | `rustup component add` | 已安装 |
| cargo-deny | **0.20.2**（必须与 CI 的 `cargo-deny-action@v2.1.1` 同版本，ADR-0019） | 依赖治理（gov §5.1 #8） | GitHub release 预编译包装入 `~/.cargo/bin`；配方待落 `docs/dev-env-setup.md`（PL-017） | **已安装**（2026-09-17，PL-006 解除） |
| cargo-llvm-cov | 0.9.1 | 覆盖率门槛（gov §5.1 #9） | 同上 | **已安装**（2026-09-17；实测 `xtask` 行覆盖 96.31%） |
| git | 系统自带（2.51.0） | 版本控制 | — | 已安装；`origin = https://github.com/snofin18/ai-assistant.git`，经本机代理 `127.0.0.1:30000` |

> **本地验收追加项（ADR-0019 落地动作 #5）**：`cargo deny check licenses bans sources`
> —— 它**不需要联网**（只有 `advisories` 要 clone advisory-db），专门用来证明
> `deny.toml` 能被当前版本的 cargo-deny **加载解析**。这条命令存在的理由见 PL-016：
> 配置解析失败时，门禁会「什么都没查却显示通过」。
> 两个 CLI 反直觉点：`-c` 是 `--color` 不是 `--config`；`check` 不接受 `--offline`。

> **~~已知覆盖缺口（PL-019）~~ → 已关闭（2026-09-18，ADR-0024 D2）**：
> `Cargo.toml` 的 `exclude = ["spikes", "fixtures/apps", "tools"]` 仍使 fmt / clippy / test
> 三道门禁覆盖不到 spike 代码（这是**有意的**豁免，理由见 ADR-0024 D2 表格），
> 但 `cargo deny` 的缺口已由 `ci.yml` 新增的 **`spike-deny`** 硬门禁（gov §5.1 **#8b**）补上：
> 它枚举 `spikes/*/Cargo.toml` 并逐个跑 `cargo deny check licenses sources`；
> 无 manifest 时**显式打印「无」再 exit 0**（铁律 1）。
> 负向验证见 `gate-selftest.yml` 的 `spike-deny-gate` canary（ADR-0019 N3，登记表已补 #8b 行）。
> **本机实测**：负向 GPL fixture → exit 4 且输出含 `license is not explicitly allowed`；
> 正向 `spikes/spike-a-notepad` → `licenses ok, sources ok` exit 0。
