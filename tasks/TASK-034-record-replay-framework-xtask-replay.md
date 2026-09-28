# TASK-034　录制回放框架 v0：树快照录制 + 离线回放（替代真实平台调用）+ `xtask replay`

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A3**　依赖：017,022　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：017,022　**预估**：M　**难度**：M
- **write scope**：`crates/replay/**`、`xtask/src/replay*`、`fixtures/recordings/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A3（1a）、`docs/wbs-overview.md` §6（DoD）

## 目标

交付一个最小但真实的录制回放框架 v0：定义可版本化的树快照录制格式，提供严格的录制校验、由录制构造的离线 `UiAutomationProvider` / `WindowProvider`，并让 `xtask replay` 能校验和摘要录制文件。它替代真实平台调用，使逻辑层回归可在无真机 CI 上运行。

## In scope

- `crates/replay/**`：录制模型、JSON 编解码、校验、离线 provider、专项测试与 README。
- `xtask/src/replay*`：读取 Recording v1 格式，校验节点、边与引用，输出确定性摘要。
- `fixtures/recordings/**`：至少一份 core 录制 fixture，覆盖稳定 AutomationId、父子链、读文本与歧义/缺失负向样本。

## Out of scope（做了算漂移）

- 真实平台录制器 / 屏幕录制 / 鼠标键盘 hook。
- 修改 `crates/platform/api` 的公共接口或 `TreeSnapshot` 形状。
- 把 replay 接入 `core` / `apps/agent-core` 的生产装配。
- 完整回放所有平台动作；写动作在 v0 可显式返回 `CapabilityMissing`。
- 外部 MCP、视觉回放、录制压缩或跨进程传输。

## 必须遵守

- Recording 格式必须版本化、拒绝未知版本、孤儿父节点、重复 handle、环与悬空交互引用。
- 离线 provider 必须复用 `assistant-platform-api` 的 trait；不得复制平台公共类型。
- 元素解析必须按候选链顺序，命中多节点返回 `TargetAmbiguous`，未命中返回 `TargetNotFound`。
- 未录制的写动作必须显式返回 `CapabilityMissing`，不得假装成功。
- `xtask` 继续保持零第三方依赖；`crates/replay` 不新增第三方 crate，只依赖已登记的 workspace crate。
- 不修改 `crates/platform/api`、spec、ADR、CI workflow 或其他生产路径。

## 验收命令

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [ ] `assistant-replay` 可从 JSON 加载、校验并重建离线 provider。
- [ ] core fixture 能解析、通过校验，并由 replay 测试复现 AutomationId/父链/读文本结果。
- [ ] 重复 handle、孤儿父节点、环、未知版本、悬空文本引用均被拒绝。
- [ ] 歧义选择返回 `TargetAmbiguous`，缺失目标返回 `TargetNotFound`。
- [ ] 未录制写动作返回 `CapabilityMissing`。
- [ ] `xtask replay <fixture>` 输出确定性摘要并在合法 fixture 上 exit 0。
- [ ] `cargo fmt --all --check` 0 diff。
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [ ] `cargo test --workspace` 全绿。
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger` 全部 PASSED。
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-034 录制回放框架 v0          【目标】交付版本化 Recording v1、严格校验、离线平台 provider 与 xtask replay
【write scope】crates/replay/**、xtask/src/replay*、fixtures/recordings/**
【铁律】1 无静默失败；8 element/handle 不跨进程；9 不静默扩大范围
【禁止】真实录制器、改 platform/api、接生产装配、完整写动作回放、外部 MCP/视觉/压缩/跨进程
【验收】fmt；clippy；workspace tests；replay fixture；全部 xtask 门禁
【依赖】017、022 已 Done；已核对 LEDGER
【疑问】无；新增 workspace crate 会使 Cargo.lock 自动增加 package 条目，按 DRIFT-034-1 记录
```

### 2. 实际改动文件

- `crates/replay/Cargo.toml`、`README.md`、`src/{lib,error,model,provider}.rs`
- `crates/replay/tests/replay.rs`
- `xtask/src/replay.rs`
- `fixtures/recordings/README.md`、`fixtures/recordings/core/notepad-like-basic.json`
- `Cargo.lock`（新增 workspace package 自动更新，DRIFT-034-1）

### 3. 验收输出摘要

- `cargo fmt --all --check`：0 diff。
- `cargo clippy --all-targets -- -D warnings`：exit 0。
- `cargo test --workspace`：全绿；`assistant-replay` 专项 **16 passed**；`xtask` **379 passed**。
- `cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json`：PASSED，节点 4、稳定 AutomationId 4、0 error。
- `hygiene` 0E/4W（既有 file-too-long 基线）；`memory-counts` 0E/0W；`adr-index` 0E/0W；`refscan` 0E/0W；`docscan` 0E/414W；`card-check` 0E/27W；`check-ledger` 0E/0W。
- `verify-schemas`、`codegen --check`、`arch`、`cargo deny check`：PASS（arch 6 条既有 warning，deny 既有 warning）。
- PR #84：push / pull_request 两个 run 的 **16/16 check-run 全 success**；merge commit `167c3f24166a2c4533f4a4c45e38b54bfb511cfe`。

### 4. DoD 逐条核对

- [x] `assistant-replay` 可从 JSON 加载、校验并重建离线 provider。
- [x] core fixture 可解析并通过校验；测试复现 AutomationId、父链与读文本结果。
- [x] 重复 handle、重复文本结果、孤儿父节点、环、未知版本、悬空文本引用均被拒绝。
- [x] 歧义选择返回 `TargetAmbiguous`，缺失目标返回 `TargetNotFound`。
- [x] 未录制写动作返回 `CapabilityMissing`；`bring_to_front` 也不再伪造成功。
- [x] `xtask replay <fixture>` 输出确定性摘要并在合法 fixture 上 exit 0。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] 全部要求的 xtask 门禁 PASSED。
- [x] LEDGER 已追加；新增 FACT / PITFALL 已写入长期记忆。

### 5. 偏差

**DRIFT-034-1（`Cargo.lock` 自动更新）**：新增 `crates/replay` workspace package 后，Cargo 自动在根 `Cargo.lock` 增加 `assistant-replay` 及其对 `assistant-platform-api` / `assistant-protocol` 的依赖边；未新增第三方 crate、未改变既有版本。这是新增 workspace 成员的必要结果，随本卡提交。

合并前自审新增两条 fail-closed 收紧：同一节点重复文本结果改为显式 `DuplicateTextOutcome`；未知窗口上的整窗指纹改为 `TargetNotFound`，不再误报 `CapabilityMissing`。两者均有负向测试，修正后 CI 16/16 全绿。

### 6. 更合理做法

没有修改 `crates/platform/api` 或给 `TreeSnapshot` 塞节点数组。`assistant-replay` 自己持有完整 Recording 节点模型，离线 provider 只实现既有平台 trait；既不复制平台公共类型，也不把“只回放摘要”误当完整树对象。未录制的写动作统一 fail-closed，后续若要支持写动作，应扩展 Recording schema 并先更新契约，而不是在 provider 里猜结果。

### 7. 遗留问题

- `AGENTS.md` §6 的 `replay --suite core` 仍与当前 CLI 的 `<fixture>` 口径不一致；归 **PL-062**，本卡未越界修改 `AGENTS.md`。
- v0 不实现真实录制器、树 diff、截图/视觉回放与写动作回放，均保持 Out of scope。
- `RoleAndParent` 的离线解析按链内父候选递归实现；平台 helper 候选标记争议仍以 **PL-094** 为准。

### 8. 新增长期记忆

- FACT：见 `docs/memory/facts.md` 2026-09-28 TASK-034 条。
- PITFALL：见 `docs/memory/pitfalls.md` 2026-09-28 TASK-034 条。

### 9. 给审阅者的关注点

- 重点检查 `RoleAndParent` 递归解析与 PL-094 的关系：replay 是否应在 helper 语义正式裁决后继续保留当前实现。
- 重点检查未知版本、孤儿、环、重复 handle、悬空引用的拒绝顺序和错误语义。
- 确认 `fixtures/recordings/core/notepad-like-basic.json` 可作为 TASK-035 Adapter 回放基线，而不是把 fixture 当成真实 UIA 全树。
