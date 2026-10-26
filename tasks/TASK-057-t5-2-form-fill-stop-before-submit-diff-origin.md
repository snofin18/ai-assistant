# TASK-057　T5.2：表单填写 → **停在提交前** → 展示 diff 与来源归因 → 用户确认后提交

- 状态：**Blocked**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：056　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：056　**预估**：M　**难度**：M
- **write scope**：同上
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

T5.2：表单填写 → **停在提交前** → 展示 diff 与来源归因 → 用户确认后提交。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

同上

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
【任务】TASK-057 T5.2 表单填写 → 停在提交前 → diff/来源归因 → 用户确认后提交
【目标】核实可执行契约；依赖或正文不满足时记录 DRIFT 并转为 Blocked
【write scope】仅：本卡状态行与执行记录区、LEDGER.md
【铁律】无静默失败；策略引擎唯一放行；写操作必须有 postcondition；不得静默扩大范围；契约先行
【禁止】真实浏览器、真实站点/profile、真实表单填写或提交、产品代码/schema/依赖改动
【验收】依赖与占位正文复核 → 不满足即停；hygiene / card-check / docscan / check-ledger → 0 error
【依赖】TASK-056，当前为 Blocked（DRIFT-056-1）
【疑问】无；按预授权直接记录阻塞
```

### 2. 实际改动文件

- `tasks/TASK-057-t5-2-form-fill-stop-before-submit-diff-origin.md`：状态行与执行记录区。
- `LEDGER.md`：追加 TASK-057 阻塞事件。

### 3. 验收输出摘要

- 正文复核：步骤仍为“占位，派单前补全”，write scope 仍写“同上”，未给出目标站点、表单字段、diff 结构、来源归因字段、fixture、提交前停止判据或人工确认流程。
- 依赖复核：TASK-056 已因 `DRIFT-056-1` 转为 Blocked；本卡依赖 `056`，不能进入实现。
- 未启动浏览器、未连接 CDP、未读取真实站点/profile、未执行表单填写或提交。
- 产品测试不适用：本卡未进入实现，未修改代码、schema、依赖或测试断言。

### 4. DoD 逐条核对

- [ ] 卡片标题声明的能力可被测试用例覆盖：未执行，正文和依赖尚未满足。
- [ ] `cargo fmt --all --check`：未作为实现门禁运行，本卡为文档阻塞记录。
- [ ] `cargo clippy --all-targets -- -D warnings`：未作为实现门禁运行。
- [ ] `cargo test --workspace`：未运行，避免把文档阻塞记录伪装成实现验收。
- [x] 文档门禁：`hygiene` 0 error / 136 warnings，`card-check` 0 error / 35 warnings，`docscan` 0 error / 184 warnings，`check-ledger` 0 error。
- [x] `LEDGER.md` 追加阻塞事件。

### 5. 偏差

```text
DRIFT-057-1
触发器：#8（卡片契约自相矛盾）+ #11（真实浏览器/网络/人工确认）
现象：卡片仍为批次占位稿；步骤未展开，write scope 仅写“同上”，且依赖 TASK-056 已 Blocked。
影响：无法确定目标站点、表单字段、fixture、diff/来源归因结构、提交前停止边界和人工确认流程。
我的建议：由 Orchestrator 在派单前补齐上述契约，并在 TASK-056 解锁后安排真实浏览器与人工确认会话。
已停止的工作：未启动浏览器、未连接 CDP、未填写或提交真实表单、未修改产品代码。
需要人类裁决：是
```

- 本次启动与选卡复核有 **2** 条只读命令误用默认 bash；均因 `rg` / PowerShell cmdlet 不存在而失败，未写入仓库，随后以显式 `shell: "powershell"` 重跑。

### 6. 更合理做法

- 在依赖未满足且正文未展开时直接记录 Blocked，避免靠猜测构造真实表单流程或伪造验收结果。

### 7. 遗留问题

- 等待 Orchestrator/人类补齐 TASK-056 与 TASK-057 的输入契约、fixture、目标站点和人工确认流程。

### 8. 新增长期记忆

- 无。该问题属于既有“批次占位卡派单前补全”规则，不新增长期记忆。

### 9. 给审阅者的关注点

- 是否认可将 TASK-057 直接转为 Blocked，而不是在 056 未解锁时继续保留 Ready。
- 后续派单必须同时给出 diff 展示结构、来源归因字段、提交前停止判据和人工确认边界。

---

**2026-10-26 · 真实应用人工验收边界（TASK-328）**

- 本卡涉及真实表单和不可逆提交，任何真实执行都必须在人类在场会话中完成。
- automation 不得实现“自动提交”捷径，也不得用真实站点测试替代 diff / 来源归因 / 人工确认
  的离线契约。
- 当前卡面仍是占位稿，`form_fill` provider 与提交前确认链路尚未实现；人工点击不能补足缺失实现。
  本卡继续保持 **Blocked**，等待独立展开与实现卡。
