# TASK-361 desktop-ui real Core pipe bootstrap

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：T5.2 产品化前置
- 依赖：ADR-0057、TASK-213、TASK-358、TASK-359
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

- **write scope**：
  `apps/desktop-ui/src-tauri/src/main.rs`、
  `apps/desktop-ui/src-tauri/src/core_pipe.rs`、
  `apps/desktop-ui/README.md`、
  `tasks/TASK-361-desktop-ui-core-pipe-bootstrap.md`、
  `docs/memory/facts.md`、
  `PLAN.md`、
  `plans/stage-1-pilots.md`、
  `README.md`、
  `LEDGER.md`

## 目标

把 desktop-ui 二进制的启动入口从无传输的 `run()` 切换为真实 `CorePipeConfig` +
`run_with_core_pipe()`。配置必须由受控环境变量提供，缺 token、空 pipe 或空
token 环境变量名时在启动阶段显式失败，不得静默退回 unavailable transport。

本卡只闭合正式 UI 到 Core 的真实进程边界。Tauri 页面、UI wire schema、浏览器
任务包、真实 Edge 和人类审批验收均不在本卡内。

## In scope

- desktop-ui 启动时读取 `ASSISTANT_UI_PIPE` 与 `ASSISTANT_UI_TOKEN_ENV`。
- 默认值与 Core 对齐：pipe=`assistant-agent-core-ui`，
  token 环境变量名=`ASSISTANT_AGENT_CORE_UI_TOKEN`。
- 校验 pipe 非空、token 环境变量名非空、token 值已设置且非空。
- 通过现有 `run_with_core_pipe()` 组装命令 transport 与单实例事件泵。
- 对默认值、自定义值、空 pipe、空 token 名、缺失/空 token 做纯单元测试。
- 更新 desktop-ui README，记录启动环境与安全边界。

## Out of scope

- 不修改 UI/Core wire schema、Tool schema、DB schema 或 `ErrorCode`。
- 不新增第三方依赖、环境变量解析 crate 或新的配置协议。
- 不修改 React 页面、审批卡布局或 Tauri command 名称。
- 不启动真实浏览器、不执行 `form_submit`、不做真实 GUI 验收。
- 不改 TASK-057 / TASK-058 正文或状态。

## 必须遵守

1. 配置缺失或非法必须在 Tauri 启动前失败，不得退回 unavailable state。
2. token 值不得进入日志、错误消息、命令行或 UI 事件。
3. desktop-ui 继续只依赖 `assistant-ipc` client，不链接 Core、policy 或 task-engine。
4. pipe/token 名只做最小格式校验；真实连接失败仍由既有 transport 显式报告。
5. 不修改既有 `run()` / `run_with_core_pipe()` 的测试语义，只增加安全启动入口。

## 验收命令

```powershell
cargo fmt --manifest-path apps/desktop-ui/src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path apps/desktop-ui/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-ui/src-tauri/Cargo.toml
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
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
cargo deny check
cargo run -p xtask -- replay --suite core
```

期望：默认配置与显式配置可构造；缺失/空 token、空 pipe、空 token 环境变量名
均返回明确配置错误；所有既有 UI/Core transport 测试不回归；全部门禁通过。

## DoD

- [ ] `main` 启动时读取受控环境并调用 `run_with_core_pipe()`。
- [ ] 默认值与 Core 参数一致，自定义 pipe/token 名可用。
- [ ] 缺 token、空 pipe、空 token 名在启动前 fail-closed，错误不含 token 值。
- [ ] desktop-ui 专项测试、UI checks、workspace 全门禁通过。
- [ ] PR 合并并记录 checks / merge / post-merge main CI；状态与进度文件同步 Done。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-361 desktop-ui real Core pipe bootstrap
【目标】让 desktop-ui 启动时读取受控环境并调用现有 run_with_core_pipe，显式失败非法配置
【write scope】desktop-ui main/core_pipe/README、本卡、facts、PLAN/plans/README/LEDGER
【铁律】无静默失败；UI/Core 输入先校验；UI 不链接 Core；不改公共 wire；新增行为有负向测试
【禁止】不改 React/UI IPC/Tool/DB schema、ErrorCode、依赖；不启动浏览器；不改 TASK-057/058
【验收】desktop-ui fmt/clippy/test、UI lint/typecheck/test/build、workspace 全门禁
【依赖】ADR-0057、TASK-213、TASK-358、TASK-359 Done
【疑问】无；默认值与 Core production/browser UI 参数保持一致
```

### 2. 实际改动文件

- `apps/desktop-ui/src-tauri/src/core_pipe.rs`：新增环境常量、默认值、Core-compatible
  `CorePipeConfig::from_environment()` 与纯函数配置校验；错误码
  `core_transport_config_invalid` 不包含 token 值。
- `apps/desktop-ui/src-tauri/src/main.rs`：启动时加载配置并调用既有
  `run_with_core_pipe()`；配置错误在 Tauri 启动前显式退出。
- `apps/desktop-ui/README.md`：记录 `ASSISTANT_UI_PIPE` /
  `ASSISTANT_UI_TOKEN_ENV` 与默认 token 环境变量。
- `docs/memory/facts.md`、`MEMORY.md`：记录真实 Core pipe bootstrap 与规模表更新。
- `tasks/TASK-361-desktop-ui-core-pipe-bootstrap.md`、`PLAN.md`、
  `plans/stage-1-pilots.md`、`LEDGER.md`：卡与进度同步。

### 3. 验收输出摘要

- desktop-ui tests：21 passed / 0 failed；新配置默认/自定义/空值/缺 token 用例全绿。
- `cargo fmt --manifest-path ... --all --check`、desktop-ui clippy：通过。
- desktop-ui binary 缺 token 实测：exit 1，错误
  `core_transport_config_invalid`，仅包含环境变量名。
- UI `lint` / `typecheck` / `test` / `build`：通过；model 89、DOM 5。
- `cargo fmt --all --check`、workspace clippy、`cargo test --workspace`：通过。
- arch tests 10/10；verify-schemas、codegen、hygiene、memory-counts、adr-index、
  refscan、docscan、card-check、check-ledger、check-comments：通过。
- `replay --suite core`：2 steps / 5 changes / 0 mismatch，通过。
- TASK-362 合入后 `cargo deny check`：advisories / bans / licenses / sources
  全部 ok；最终安全版本为 `rmcp 3.9.0`、`rusqlite 0.41.1`、
  `libsqlite3-sys 0.39.1`。

### 4. DoD 逐条核对

- [x] `main` 启动时读取受控环境并调用 `run_with_core_pipe()`。
- [x] 默认值与 Core 参数一致，自定义 pipe/token 名可用。
- [x] 缺 token、空 pipe、空 token 名在启动前 fail-closed，错误不含 token 值。
- [x] desktop-ui / UI / workspace 门禁通过，`cargo deny` 四类全绿。
- [ ] PR 合并并记录 checks / merge / post-merge main CI。

### 5. 偏差

- 当前 user turn 最终机器统计 `shell_misuse=bash=60` / `cmd=0`；均被 ADR-0100 hook
  重写为 PowerShell，未产生非预期仓库写入。
- 初次全门禁被 main 的四个新增 RUSTSEC advisory 阻断；独立 TASK-362 已完成安全
  升级，rebase 后本卡全门禁通过。

### 6. 实施中发现的更合理做法

- 复用已有 `CorePipeConfig` / `run_with_core_pipe()`，只补环境解析与启动接线，
  不新增 transport 或配置抽象层。

### 7. 遗留问题

- 真实 UI approval 与真实 Edge T5.2 验收仍由后续 TASK-057 承接。

### 8. 新增长期记忆

- FACT 已写入：desktop-ui 正式二进制已使用真实 Core UI pipe，默认值与 fail-closed
  配置边界已记录。

### 9. 给审阅者的关注点

- token 值是否绝不会进入错误消息、日志或命令行。
- 非法配置是否在 Tauri 启动前失败，且不会退回 unavailable transport。
