# TASK-344　T5.2 浏览器表单确认与提交契约

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：治理契约
- 依赖：TASK-056、TASK-343；ADR-0082
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`docs/adr/0101-browser-form-confirmation-and-submit-contract.md`、
  `docs/adr/README.md`、`docs/spec/browser-form-confirmation.md`、
  `docs/spec/core-orchestration.md`、`docs/memory/decisions.md`、`MEMORY.md`、
  `PLAN.md`、`plans/stage-1-pilots.md`、`LEDGER.md`
- 本卡自己的执行记录区默认可写，不重复列入 write scope。

## 目标

用 ADR-0101 冻结 T5.2 的只读表单快照、字段 diff、来源归因、一次性人工审批和专用
同源提交；不实现产品代码，先解除 TASK-057 的契约阻塞。

## In scope

- 新建 ADR-0101 并登记编号与 decisions；
- 新建 `browser-form-confirmation` spec，固定 `form_read` / `form_fill` / `form_submit`
  工具边界、负向矩阵和本地 loopback fixture；
- 更新 core orchestration 的 InstructionOrigin 跨层投影说明；
- 同步本卡、阶段索引、PLAN 当前状态与 LEDGER Review 事件。

## Out of scope（做了算漂移）

- 不新增或修改 Edge/Chrome tool schema、handler、registry 或 runtime；
- 不修改 UI/IPC、protocol、数据库、task package、fixture 或真实浏览器；
- 不执行真实表单填写、提交或外部站点访问；
- 不把 TASK-057 标为 Ready / Done，不替代真实 Edge 人工验收。

## 必须遵守

1. `form_fill` 保持 TASK-343 的“永不提交”合同；提交必须是独立高风险 L3 步骤。
2. 不引入通用 click、通用脚本或任意页面内容作为 diff / 指令来源。
3. 来源归因只复用 ADR-0082 的四类 token；`app_content` 默认拒绝且覆盖不放宽 policy deny。
4. `form_submit` 只允许 `once`；跨 origin、action 或字段状态漂移必须在点击前失败。
5. 点击后未知状态必须交人，禁止自动重试或二次提交。
6. 本卡 docs-only，不新增依赖、不修改产品公共 API/schema。

## 验收命令

```powershell
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
```

## 完成定义（DoD）

- [ ] ADR-0101 已登记为 Accepted，下一可用编号顺延。
- [ ] spec 明确三类工具、字段 diff、来源归因、一次性审批、同源/TOCTOU 与点击后未知状态。
- [ ] TASK-057 仍保持 Blocked；没有把契约卡冒充实现卡。
- [ ] 上述文档门禁全部 PASSED；只修改 write scope 内文件。
- [ ] LEDGER.md 追加 Review 行。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-344 T5.2 浏览器表单确认与提交契约
【目标】以 ADR-0101 + spec 冻结 diff、来源归因和安全提交边界，解除 TASK-057 契约阻塞
【write scope】见卡片正文；另含本卡执行记录区
【铁律】契约先行；策略引擎唯一放行；不可逆动作人工确认；不得静默扩范围；无静默失败
【禁止】产品代码、实际工具/schema 实现、真实表单提交、外部站点、依赖或 UI 运行逻辑改动
【验收】adr-index / refscan / docscan / card-check / check-ledger / hygiene / memory-counts
【依赖】TASK-056、TASK-343 Done；TASK-057 继续 Blocked
【疑问】无；按已确认的本地 fixture 与三分段契约自决
```

### 2. 实际改动文件

- `docs/adr/0101-browser-form-confirmation-and-submit-contract.md`：新增 Accepted ADR，
  冻结三段工具、diff、归因、同源 TOCTOU 与点击后失败语义。
- `docs/spec/browser-form-confirmation.md`、`docs/spec/core-orchestration.md`：新增实现契约，
  允许已校验归因投影到 approval UI / audit 且仍不判定权限；登记 ADR-0101 与 decisions 规模。
- `tasks/TASK-344-t5-2-form-approval-contract.md`、`PLAN.md`、`plans/stage-1-pilots.md`、
  `LEDGER.md`：Review 状态、当前任务与事件留痕。

### 3. 验收输出摘要

- `git diff --check` → PASSED。
- `cargo run -p xtask -- adr-index` → 0 error / 0 warning，PASSED；ADR-0101 Accepted，
  下一可用编号 0102。
- `cargo run -p xtask -- refscan` → 0 error / 0 warning，PASSED。
- `cargo run -p xtask -- docscan` → 0 error / 175 warnings，PASSED；均为仓库既有告警。
- `cargo run -p xtask -- card-check` → 0 error / 35 warnings，PASSED；均为既有卡告警。
- `cargo run -p xtask -- check-ledger` → 0 error / 0 warning，PASSED。
- `cargo run -p xtask -- hygiene` → 0 error / 153 warnings，PASSED；均为仓库既有告警。
- `cargo run -p xtask -- memory-counts` → 0 error / 1 warning，PASSED；仅既有 Paint 归档阈值告警。
- 2026-10-29 merge evidence：PR #643 head
  `3ba0154c3c86aa63a5fc0895689fb448e823de42` 的 11 项 checks 全部 success；
  合并提交 `b1b48c0e4c23a815a66bb68ec04bd1d971122848`；PR base=`main`。
- post-merge main CI `40842663573`（head
  `674e9a0d1749e56e3bb03806c550f5630f560ae2`）= **success**，0 failed。

### 4. DoD 逐条核对

- [x] ADR-0101 已登记为 Accepted，下一可用编号顺延为 0102。
- [x] spec 明确三类工具、字段 diff、来源归因、一次性审批、同源 / TOCTOU 与点击后未知状态。
- [x] TASK-057 仍保持 Blocked；没有把契约卡冒充实现卡。
- [x] 上述文档门禁全部 PASSED；只修改 write scope 内文件。
- [x] LEDGER.md 已追加 Review 行。
- [x] 工作 PR 已合并，PR checks 与 post-merge main CI 均由 `main` 上的真实证据确认。

### 5. 偏差

- 首轮 `docscan` 将 ADR 表格中的 `edge|chrome` 按管道符拆列，已在表格语境改为
  `edge / chrome`。
- 首轮 `memory-counts` 因新增 decisions 条目后规模表仍旧报 2 errors；已同步为
  `310` 行 / `110` 条并重跑全绿。
- 当前人类会话共 14 次误填 `shell: "bash"`；全部被 PreToolUse hook 重写为 PowerShell，
  没有通过 bash 写入产品文件。后续命令全部显式 PowerShell；本项不隐藏。

### 6. 更合理做法

- 采用契约先行，先冻结 form_read / form_fill / form_submit 的分层，再让实现卡一次性落地
  工具、UI 事件和 fixture，避免在占位 TASK-057 上边实现边改公共形状。

### 7. 遗留问题

- TASK-057 仍需正式展开并实现；本卡的契约工作与合并后收口均已完成，不替代实现或真实 Edge 验收。

### 8. 新增长期记忆

- ADR-0101 / `docs/memory/decisions.md`；未有新的应用运行事实或 pitfall。

### 9. 给审阅者的关注点

- `form_submit` 的高风险 / L3 / once 语义是否足以表达真实提交的不可逆性。
- 同一 DOM 表达式内校验字段、origin、action 再点击，是否能满足 TOCTOU 和单次点击证据。
- 来源归因与 `app_content` 覆盖是否严格保持“UI 提示不等于 policy 放行”。
