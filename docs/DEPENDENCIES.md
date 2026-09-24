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
| — | — | — | **产品 workspace 仍为零第三方依赖**：`xtask` 刻意 std-only（零供应链风险 + 编译 <1s）。下面两行属 `spikes/`（被根 `Cargo.toml` `exclude`，不在产品 workspace 内） | — | — | — | — | 2026-09-16 |
| `serde` | **`1.0`**（caret 语义：允许 1.x 内升级；`Cargo.toml` 与 `Cargo.lock` 实测一致） | `crates/protocol` | serde derive + `serde_json::Value` for envelope.data / audit_event.target 等开放字段 | MIT OR Apache-2.0 | 替代 = 手写 Serialize/Deserialize impl = 出错率高且协议多版本难兼容 | **Approved** | 人类（chat 2026-09-23 TASK-103） | 2026-09-23 |
| `serde_json` | **`1.0`**（caret 语义：允许 1.x 内升级；`Cargo.lock` 实测解析到 1.0.151） | `crates/protocol` | JSON Value 类型（用于 envelope.data / error.details / metadata / audit_event.target 等开放字段） | MIT OR Apache-2.0 | 替代 = serde_json::Value 是 serde_json 生态默认；不允许直接 String 传开放字段（违反 naming.md §5 newtype 约束） | **Approved** | 人类（chat 2026-09-23 TASK-103） | 2026-09-23 |
| `windows` | **`=0.62.2`**（精确钉定；0.x 版本间**有**破坏性变更，升级 = 漂移触发器） | `spikes/spike-a-notepad`（阶段 0）→ `crates/platform/windows`（阶段 1） | Win32/WinRT 官方投影。**Spike A 只用其 UI Automation 客户端 COM 绑定**（`IUIAutomation` / `CUIAutomation` / `IUIAutomationElement` / `IUIAutomationValuePattern`），验证「Rust 走 COM 是否与 PowerShell 走托管封装表现一致」 | MIT OR Apache-2.0（`cargo deny check licenses` 2026-09-18 本机实测 `licenses ok`，exit 0） | 替代方案 = 第三方封装 crate `uiautomation`，**已否决**（ADR-0024 D1）：封装层自带缓存会**掩盖**真实 COM 开销，污染 Spike A 的 go/no-go 判据，并把风险推迟到阶段 1；且它不在本清单内，等于多引入一个外部维护者 | **Approved for spikes**（产品侧待阶段 1 走漂移升级） | 人类（指示 #6） | 2026-09-18 |
| `uiautomation` | — | — | （曾考虑用于 spike 的 UIA 访问） | 未核实 | **Rejected**：见 ADR-0024 D1 的对比表与裁决理由（决定性一条 = spike 必须走生产路径去撞墙） | **Rejected** | 人类（指示 #6） | 2026-09-18 |

> 计划中的首批依赖（**尚未引入**，引入时逐条登记并走漂移升级）：
> `tokio`、`serde`、`rusqlite`、`rmcp`、`tauri`、`windows`（crate）、`zstd`、`tracing`。
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
