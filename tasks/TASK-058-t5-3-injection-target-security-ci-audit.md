# TASK-058　T5.3：注入靶页安全验收 + **安全回归常驻 CI** + 阶段 1c/阶段 1 总验收与对齐审计

- 状态：**Blocked**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：053,054,055　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：053,054,055　**预估**：M　**难度**：M
- **write scope**：同上、`.github/workflows/**`、`docs/audits/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

T5.3：注入靶页安全验收 + **安全回归常驻 CI** + 阶段 1c/阶段 1 总验收与对齐审计。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

同上、`.github/workflows/**`、`docs/audits/**`

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
【任务】TASK-058 T5.3 注入靶页安全验收 + 安全回归常驻 CI + 阶段 1c/阶段 1 对齐审计
【目标】核实可执行契约；正文或阶段前置未满足时记录 DRIFT 并转为 Blocked
【write scope】仅：本卡状态行与执行记录区、LEDGER.md
【铁律】无静默失败；不可信输入必须校验；不得静默扩大范围；契约先行
【禁止】在占位稿上猜写安全断言、workflow、审计判据，或提前宣称阶段验收完成
【验收】正文、写入范围与阶段前置复核 → 不满足即停；hygiene / card-check / docscan / check-ledger → 0 error
【依赖】TASK-053/054/055 已 Done；阶段 1c 的 TASK-056/057 均 Blocked
【疑问】无；按预授权直接记录阻塞
```

### 2. 实际改动文件

- `tasks/TASK-058-t5-3-injection-target-security-ci-audit.md`：状态行与执行记录区。
- `LEDGER.md`：追加 TASK-058 阻塞事件。

### 3. 验收输出摘要

- 正文复核：步骤仍为“占位，派单前补全”，write scope 仍写“同上”，未给出注入靶页安全断言、fixture 范围、workflow 接线、阶段 1c/阶段 1 对齐判据或必需证据。
- 阶段前置复核：TASK-056 与 TASK-057 均 Blocked，无法执行依赖完整 Stage 1c 结果的阶段总验收。
- 未修改 `.github/workflows/**`、产品代码、测试、schema、ADR、spec 或依赖。

### 4. DoD 逐条核对

- [ ] 卡片标题声明的能力可被测试用例覆盖：未执行，正文与阶段前置尚未满足。
- [ ] `cargo fmt --all --check`：未作为实现门禁运行，本卡为文档阻塞记录。
- [ ] `cargo clippy --all-targets -- -D warnings`：未作为实现门禁运行。
- [ ] `cargo test --workspace`：未运行，避免把文档阻塞记录伪装成实现验收。
- [x] 文档门禁：`hygiene` 0 error / 136 warnings，`card-check` 0 error / 35 warnings，`docscan` 0 error / 175 warnings，`check-ledger` 0 error。
- [x] `LEDGER.md` 追加阻塞事件。

### 5. 偏差

```text
DRIFT-058-1
触发器：#8（卡片契约自相矛盾）
现象：安全验收/CI/阶段总验收卡仍为批次占位稿，write scope 仅写“同上”，且完整 Stage 1c 前置 TASK-056/057 均 Blocked。
影响：无法确定安全断言、fixture 范围、CI 接线方式、阶段对齐判据与验收证据。
我的建议：先解锁 TASK-056/057，再由 Orchestrator 展开本卡正文与明确 write scope。
已停止的工作：未修改 workflow、产品代码或测试，未提前宣称阶段 1c/阶段 1 验收完成。
需要人类裁决：是
```

- 本卡有 **2** 条只读命令误用默认 bash：取锁重跑时 `cargo` 不存在、CI 监视时 `gh` 不存在；均未取得锁或写入仓库，随后均以显式 `shell: "powershell"` 重跑。

### 6. 更合理做法

- 在安全断言、CI 接线和阶段前置尚未确定时直接记录 Blocked，避免靠猜测新增 workflow 或放宽安全判据。

### 7. 遗留问题

- 等待 TASK-056/057 解锁，并由 Orchestrator 补齐注入靶页安全断言、fixture 范围、CI 接线与阶段审计口径。

### 8. 新增长期记忆

- 无。该问题属于既有“批次占位卡派单前补全”规则，不新增长期记忆。

### 9. 给审阅者的关注点

- 是否认可 TASK-058 在完整 Stage 1c 前置阻塞时直接转为 Blocked。
- 后续展开本卡时必须明确安全一票否决断言、workflow 文件和阶段对齐证据，不能只保留标题式占位。
