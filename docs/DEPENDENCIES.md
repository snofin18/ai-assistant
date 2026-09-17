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
| — | — | — | **目前 workspace 零第三方依赖**：`xtask` 刻意 std-only（零供应链风险 + 编译 <1s） | — | — | — | — | 2026-09-16 |

> 计划中的首批依赖（**尚未引入**，引入时逐条登记并走漂移升级）：
> `tokio`、`serde`、`rusqlite`、`rmcp`、`tauri`、`windows`（crate）、`zstd`、`tracing`。
> 它们的选型理由见 `cross-platform-ai-assistant-architecture-v2.md` §15 与 `docs/storage-design.md`。

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

> **已知覆盖缺口（PL-019）**：`Cargo.toml` 的 `exclude = ["spikes", "fixtures/apps", "tools"]`
> 使 `cargo deny` / fmt / clippy / test 四道硬门禁**都覆盖不到 spike 代码**。
> 阶段 0 的 spike 引入依赖时，只有本表规则 1「先登记后引入」的政策约束、没有机器约束。
