# TASK-049　专用 profile 管理：自定义 `--user-data-dir` 生命周期 + 独立窗口标识 + **禁止复制用户 profile** 的硬约束 + 首次登录引导

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：048　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：048　**预估**：M　**难度**：M
- **write scope**：`crates/browser-profile/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

专用 profile 管理：自定义 `--user-data-dir` 生命周期 + 独立窗口标识 + **禁止复制用户 profile** 的硬约束 + 首次登录引导。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/browser-profile/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-049 专用 profile 管理
【目标】为 Edge/Chrome CDP 提供专用 `--user-data-dir` 生命周期、独立窗口标识与 no-copy-user-profile 硬约束
【write scope】仅：`crates/browser-profile/**`
【铁律】契约先行；不新增第三方依赖；不句柄窗口 / UIA 树；不碰策略审批；不复制用户 profile
【禁止】新增浏览器进程管理 / CDP 传输 / policy / credential / filesystem / persistence
【验收】`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、
       `cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments`
【依赖】TASK-048 Done
【疑问】无
```
### 2. 实际改动文件

- `crates/browser-profile/Cargo.toml`
- `crates/browser-profile/README.md`
- `crates/browser-profile/src/lib.rs`
- `crates/browser-profile/src/error.rs`
- `crates/browser-profile/src/profile.rs`
### 3. 验收输出摘要

- `cargo fmt --all --check` → 0 diff
- `cargo clippy --all-targets -- -D warnings` → 0 error / warning
- `cargo test --workspace` → 全绿
- `cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments` → 全部 PASSED

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [x] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

### 5. 偏差

- none

### 6. 更合理做法

- 先定义 profile / error / lifecycle 契约，再由真实浏览器启动与 CDP 接入层复用。
### 7. 遗留问题

- 真实浏览器进程启动、CDP 传输、profile 持久化与清理仍由后续 Stage 1c 卡承接。
### 8. 新增长期记忆

- 无新增事实 / 坑。
### 9. 给审阅者的关注点

1. `BrowserProfile` 是纯值对象，不启动浏览器、不写文件。
2. `copy_user_profile()` 恒为失败，这是 no-copy-user-profile 硬约束的显式表达。
3. `launch_args()` 只包含 `--user-data-dir`，不含任何凭据或登录态。
