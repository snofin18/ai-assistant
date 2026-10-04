# TASK-211　阶段 1a 复验准备与停车位收口

- 状态：**Done（2026-09-30；LEDGER Done + `stage-1a-reaudit-checklist-2026-09-30.md` 在 main）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：102~105、087、210
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-039、`docs/audits/stage-1a-integration-audit-2026-09-29.md`、PL-018 / PL-056 / PL-058 / PL-092 / PL-094

## 目标

在所有补救卡完成后，更新阶段 1a 的停车位状态、汇总新证据，并准备一次
`stage-1a-integration-audit` 复验；本卡只做治理和复验，不实现产品能力。

## In scope

- `docs/PARKING_LOT.md` 的状态更新和关闭记录。
- `docs/audits/**` 的复验准备清单或新审计入口。
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`LEDGER.md` 的状态同步。
- 本卡记录。

## Out of scope

- 修改产品代码、CI workflow、测试或依赖。
- 在未完成依赖卡前提前宣布 1a 通过。
- 删除历史缺口记录。

## 必须遵守

- 只关闭有实际证据的 PL；先核对新增验证输出。
- PL-018、PL-056、PL-058 必须各自有明确证据后再更新。
- 1a 复验仍由独立会话执行。
- 不把“计划完成”写成“能力已通过”。

## 验收命令

```powershell
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
```

## 完成定义（DoD）

- [ ] PL-018 / PL-056 / PL-058 的证据矩阵完整。
- [ ] 未闭环项保持 open，不被误关。
- [ ] 新审计复验清单和入口明确。
- [ ] PLAN / README / plans / LEDGER 同步。
- [ ] 验收命令全部通过。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-211 阶段 1a 复验准备与停车位收口    【目标】更新停车位状态、汇总新证据、准备一次独立复验；不实现产品能力
【write scope】仅：docs/PARKING_LOT.md、docs/audits/**、PLAN.md、README.md、plans/stage-1-pilots.md、LEDGER.md、本卡记录区
【铁律】1 无静默失败（只关闭有实际证据的 PL）；9 不得静默扩大范围；10 契约先行（不改产品/CI/spec）
【禁止】修改产品代码 / CI workflow / 测试 / 依赖；在依赖卡未完成前宣布 1a 通过；删除历史缺口记录
【验收】xtask memory-counts / adr-index / check-ledger / refscan / docscan / card-check → 全 PASS
【依赖】102~105、087、210 —— **其中 TASK-105 未完成**（被 PL-095 阻塞）→ 见 §5 DRIFT-211-1，本卡只做不依赖运行证据的"准备"部分
【疑问】无；1a 结论保持 NO-GO，本卡不试图改变它
```

### 2. 实际改动文件

| 文件 | 改动 |
|---|---|
| `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md`（新增） | 复验入口：gov §5.1 逐行矩阵（含首轮→当前的变化）+ ADR-0019 负向验证登记 + **仍 open 事项清单** + 复验判定基线（明示"TASK-105 未跑出证据前 1a 不能转 GO"）+ 给复验会话留空的"复验实测"列 |
| `docs/PARKING_LOT.md` | 追加两行：**PL-056 已关闭**（`check-migrations` 已在 `doc-consistency` 每次执行）、**PL-058 已关闭**（`refscan` 已入 CI 且基线由 151 error 清零到 0E/0W） |
| `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`LEDGER.md` | 状态同步（见 §4） |

### 3. 验收输出摘要

```text
cargo run -p xtask -- memory-counts / adr-index / check-ledger / refscan / docscan / card-check → 全 PASS
cargo run -p xtask -- check-migrations → 0 error / 0 warning（PL-056 证据）
cargo run -p xtask -- refscan          → 0 error / 0 warning（PL-058 证据）
```

> 复验会话需要复跑的是 `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md` §2 的**逐行命令**，
> 不是本行摘要。

### 4. DoD 逐条核对

- [x] **PL-018 / PL-056 / PL-058 的证据矩阵完整**：PL-018 的 N3 canary 与成功 run `36598959358`（TASK-087 交付）编入复验清单 §3；PL-056 / PL-058 各有关闭行 + 可复跑命令（§2 实测 / PARKING_LOT 新增行）。
- [x] **未闭环项保持 open、不被误关**：复验清单 §4 明确列出 TASK-105 / PL-095 / PL-092 / PL-094 / TASK-002 / gov #9 / #11（SOFT）/ `check-comments` 67 warning，并写明各自"为什么仍 open"。
- [x] **新审计复验清单和入口明确**：`docs/audits/stage-1a-reaudit-checklist-2026-09-30.md` 给出入口、逐行矩阵、留空的实测列与判定基线；并重申"复验仍由独立会话执行"。
- [x] **PLAN / README / plans / LEDGER 同步**：见 §2 末行。
- [x] **验收命令全部通过**：见 §3。
- [x] **未修改 Out of scope 文件**：未动产品代码、CI workflow、测试或依赖；四份 spec/ADR 未被改写。

### 5. 偏差

- **DRIFT-211-1（依赖未满足，已按最保守方案处理）**：本卡声明依赖 `102~105`，而 **TASK-105 未完成** —— 它被 **PL-095**（UI↔Core 真实传输）阻塞，并且真实 ModelProvider / Notepad Host handler 也仍不存在。按卡面「不得在未完成依赖卡前提前宣布 1a 通过」的要求，本卡**只交付不依赖运行证据的部分**（证据矩阵 + 复验清单 + 停车位收口 + 状态同步），并在复验清单 §5 把判定基线写死为"TASK-105 未跑出证据前 1a 不能转 GO"。**没有**任何文字把 1a 写成已通过；首轮审计的 NO-GO 结论原样保留。
- 无其他偏差。

### 6. 更合理做法

**把"复验要做什么"与"复验结论"彻底分开**：本卡只产出清单与证据矩阵，并把"复验实测"列留空给独立会话填。
这样复验者既不会把这里的"现状描述"当成自己的实测结论（首轮审计批评过"工作流里有 step ≠ 门禁已启用"），
也不会被本卡预先带偏 —— 判定基线明确写成"门禁全绿也不够，缺 TASK-105 运行证据就是不能转 GO"。

### 7. 遗留问题

- **TASK-105 仍是 1a 转 GO 的唯一硬缺口**，其前置链是：**人类接受 ADR-0057 → TASK-213（UI↔Core 传输）→ 真实 ModelProvider + Notepad Host handler → TASK-105**。
- **gov #9 覆盖率与 #11 `cargo doc` 仍是 SOFT 门禁**：转硬需要单独卡 + ADR-0019 负向验证，本卡不做。
- **`check-comments` 67 warning**：非阻塞，但量偏大；建议单独判断是清理还是明确容忍口径。
- 本卡**不关闭自己之外的任何 PL**；PL-092 / PL-094 / PL-095 保持 open。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目：本卡结论是"阶段状态 + 证据位置"，已落在 `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md`
与 `docs/PARKING_LOT.md`；未产生新的跨应用事实或坑。

### 9. 给审阅者的关注点

1. **本卡没有宣布 1a 通过**，也不应被引用为"1a 的证据"——它只是复验入口。请确认这个边界（DRIFT-211-1）是你接受的处置。
2. **PL-056 / PL-058 的关闭是否够硬**：两条都未改 `gov §5.1` 的编号（PL-056 原文担心重编号漂移），只凭"命令已在 `doc-consistency` 内 + 本地 0E/0W + TASK-039 审计复核"关闭。若你认为还需要 ADR 级登记，请在裁决时指出。
3. **复验清单的判定基线**（§5）故意写得比首轮审计更死：门禁全绿也不转 GO。若你认为 1a 的 DoD 可以拆分（治理部分先转 GO），那是另一个决策，需要动 `plans/stage-1-pilots.md` 的 DoD —— 不在本卡范围。
