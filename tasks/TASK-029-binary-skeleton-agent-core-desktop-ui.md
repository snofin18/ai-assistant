# TASK-029　二进制骨架：`apps/agent-core` + `apps/desktop-ui`（Tauri 2 + React + TS + Tailwind）+ capabilities 最小化 + CSP ＋ **Host 装配**（把 `core` 组件与 platform / tool-bus / policy / storage / audit 组装起来）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A3**　依赖：028 / 206 / 207 / 208　预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。
- 来源：**ADR-0053 D1 / D5**（人类 2026-09-26 裁决「drift-028 的 5 点都按照你的建议做」→ 采纳 **DRIFT-028-4** 的建议 ②）—— 把「组装」从原 TASK-028 下沉到本卡（binary 才是**唯一装配点**）。
- 契约：`docs/spec/core-orchestration.md`（`core` 的依赖白名单与「不装配」口径）。

---

- **依赖**：028 / 206 / 207 / 208　**预估**：L　**难度**：L
- **write scope**：`apps/agent-core/**`、`apps/desktop-ui/src-tauri/**`、`apps/desktop-ui/*.config.*`
- **关联**：`plans/stage-1-pilots.md` 批次表 A3（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

二进制骨架：`apps/agent-core` + `apps/desktop-ui`（Tauri 2 + React + TS + Tailwind）+ capabilities 最小化 + CSP；
**并承担 Host 装配**：把 `core` 的编排组件与 `platform` / `tool-bus` / `policy` / `storage` / `audit` 组装成可运行的 Host。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`apps/agent-core/**`、`apps/desktop-ui/src-tauri/**`、`apps/desktop-ui/*.config.*`

**Host 装配（本卡新增，2026-09-26 ADR-0053 D1 / D5）**

- 装配点**唯一**在本卡（binary 层）：`core` 只提供可装配组件，**不**依赖 `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease`（黑名单见 `docs/spec/core-orchestration.md` 不变量 1 与 ADR-0053 D3）。
- 装配必须**显式**且可审计：每个组件的构造依赖（时钟 / 随机 / UUID / FS / 网络 / `ModelProvider` / storage 句柄）都在装配处注入，不在组件内部自取。
- **不新增第二装配点**：不得让 `core` 或 UI 侧各装配一份（否则「策略引擎是唯一放行点」与「无静默失败」都会出现第二个事实源）。
- 装配失败必须**显式**失败（缺组件 / 版本不匹配 / 句柄不可用 → 带 `ErrorCode` 报错退出），**禁止**用默认实现顶替。

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] **Host 装配**：`apps/agent-core` 能在测试中装配出 Host（组件全部经注入构造）；缺组件 / 版本不匹配 / 句柄不可用 → **显式失败**（负向用例）
- [ ] **装配点唯一**：`crates/core` 的 `[dependencies]` ⊆ ADR-0053 D2 白名单（黑名单 crate 出现即失败）；装配代码只在 `apps/agent-core`
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

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
