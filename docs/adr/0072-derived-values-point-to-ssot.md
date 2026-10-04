# ADR-0072　动态派生值只指向唯一事实源

状态：**Accepted**（2026-10-05，按用户 2026-10-05 预授权代为裁决；TASK-241）　日期：2026-10-05
Supersedes：—　Superseded by：—

来源：2026-10-05 自动化轮次 `ai-assistant-task-round-0530-derived-value-sweep`。用户要求根治
「同一数字/进度被手抄多处 → 必然漂移」，并明确本轮不加 hygiene 规则，机器证据使用定向 `rg`。
关联：ADR-0030（规模表与 ADR 编号机器校验）、ADR-0039（卡 Done 状态同步）、
ADR-0041（计划文件可写面）、`AGENTS.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`MEMORY.md`、
`docs/subagent-orchestration.md`、`docs/governance-ai-agent-execution.md`、
`cross-platform-ai-assistant-architecture-v2.md`、`docs/PARKING_LOT.md`。

---

## 背景（为什么现在要决定）

项目已多次出现同一根因：一个会随仓库演变的当前值被复制进多个文档，随后其中一个副本更新，
其余副本静默过期。实例包括：

- `MEMORY.md` 的规模表曾与 `docs/memory/` 实测计数不一致，形成 ADR-0030 的动机。
- `PL-022` 记录了门禁/规则数量被硬编码后再漂移的问题。
- `PL-035` 记录了同一条 `AGENTS.md` 行数在多个派单文档中出现且互不一致的问题。
- `PL-065` / `PL-066` 记录了 `MEMORY.md` §1 与 `plans/*` 手抄逐卡进度后落后的问题。

ADR-0030 已经把“规模表 + ADR 编号”改成机器校验，ADR-0039 / ADR-0041 已经明确卡 Done 的同步
义务，但“哪些当前派生值不得手抄、应该指向哪里”仍然分散在多个局部规则中。继续依赖注释和人工
提醒，会让同类问题在下一个文件再次出现。

## 决策（一句话）

**会随仓库演变的当前派生值不得手抄进任何文档；文档只写指向唯一事实源的指针或可直接重跑的
查询命令。** 历史观测值可以保留，但必须带日期和测量来源，且不得被当作当前状态引用。

## 决策细化

### D1　受约束的派生值

以下类别禁止在文档里手写“当前值”：

- 文件行数、文件个数、任务卡总数、条目总数、测试数、覆盖率数值；
- 当前进度、下一张卡、已完成卡清单、批次完成度；
- 硬门禁/软门禁数量、hygiene 规则总数、ADR 下一可用编号；
- 机器可重算的清单、枚举、统计量、提交哈希和 CI 计数。

规范常量不算派生值。例如协议版本、超时上限、资源上限、风险等级和 ADR 已冻结的判据必须在
其唯一契约文件中写清；其它文档应引用该契约，不得另抄一套。

### D2　唯一事实源与指针

文档需要这些值时应使用指针或命令，而不是复制结果：

| 需要什么 | 唯一事实源 / 指针 |
|---|---|
| 文件行数 | `wc -l <file>` 或等价命令的输出 |
| 当前阶段与下一张卡 | `PLAN.md` 的「当前状态」块 |
| 卡的状态推进 | `LEDGER.md` 的卡事件行 |
| 阶段批次的完成标记 | `plans/<阶段>.md` 的任务条目行首标记；正文不改 |
| memory 文件规模 | `MEMORY.md` 的规模表，并由 `cargo run -p xtask -- memory-counts` 校验 |
| ADR 编号 | `docs/adr/README.md` 的登记表与「下一个可用编号」 |
| 门禁数量与清单 | `docs/governance-ai-agent-execution.md` §5.1 与 `.github/workflows/ci.yml` |
| xtask 子命令、测试数、生成物清单 | 对应命令输出，例如 `cargo run -p xtask -- help` |

如果某个读者需要稳定入口，可以写“见 <源文件> / 运行 <命令>”，但不得再抄一份结果值。

### D3　历史观测值的边界

`LEDGER.md`、审计报告、任务卡执行记录、`docs/memory/facts.md` 里的历史观测值可以保留，
因为它们回答“当时测到了什么”，不是当前状态源。保留时必须满足：

1. 带日期或事件上下文；
2. 能追溯测量命令、环境或原始证据；
3. 后续文档引用它时明确写“历史值”，不得当作当前值。

### D4　本轮清扫范围

TASK-241 只做“删掉当前派生值、改成指针”的清扫：

- `AGENTS.md` 里重复引用的 `PLAN.md` / `MEMORY.md` 行数约束改成以文件自身为准；
- `plans/stage-1-pilots.md` 当前进度句改成指向 `PLAN.md` 当前状态块；
- `MEMORY.md` §1「下一步 ②」只保留稳定约束并指向 `PLAN.md` / `LEDGER.md`；
- `docs/subagent-orchestration.md` 删除 `AGENTS.md` 当前行数；
- `docs/governance-ai-agent-execution.md` 与架构 v2 已是指针的同类表述保持不动。

批次表、任务条目、排期、依赖和验收要点不改。历史只追加文件不改旧行。

### D5　机器防复发边界

本轮**不新增 hygiene 规则**。`gov §5.4` 的 hygiene 规则总数是 13 项 SSOT，新增第 14 项需要
单独 ADR，并可能牵连 `xtask/src/deferred.rs` 的自洽测试。本轮机器证据限定为：

```powershell
rg -n "~[0-9]+ ?行|下一张 = TASK-" <changed-files>
```

结果必须为空。后续卡可在 ADR 评估后决定是否把“派生值指针化”纳入 `docscan` 或独立子命令；
在新的 Accepted ADR 出现前，不得借本 ADR 增加规则或改变现有门禁。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 每次更新时人工把数字改对 | ❌ 否决 | 已多次失败；手抄多处仍是 SSOT 违规。 |
| 2 | 把当前派生值集中在 `MEMORY.md` 一处手写 | ❌ 否决 | 除规模表等已有机械校验对象外，集中手写仍会产生新的第二事实源。 |
| 3 | 所有文档引用同一个生成文件 | ❌ 否决 | 生成文件本身仍需权限与写回协议；本轮只冻结指针原则，不引入生成器。 |
| 4 | 立刻新增第 14 条 hygiene 规则 | ❌ 否决 | 改现有门禁口径的 SSOT；用户明确本轮不做，留后续卡评估。 |
| 5 | **只允许指向源文件/可重跑命令 + 定向清扫**（本 ADR） | ✅ 采纳 | 与 ADR-0030 / 0039 / 0041 一致，最小改动，能覆盖 PL-022 / PL-035 / PL-065 / PL-066。 |

## 影响

| 对象 | 改动 |
|---|---|
| `docs/adr/0072-derived-values-point-to-ssot.md` | 新增本 ADR。 |
| `docs/adr/README.md` | §1 登记 0072；下一可用编号顺延。 |
| `docs/memory/decisions.md` | 追加 ADR-0072 决策条目。 |
| `AGENTS.md` / `plans/stage-1-pilots.md` / `MEMORY.md` §1 | 只做当前派生值指针化与同步。 |
| `docs/subagent-orchestration.md` | 删除 `AGENTS.md` 行数副本与审阅预算中的近似行数。 |
| `docs/governance-ai-agent-execution.md` | `AGENTS.md` 行数表述已是指针；本轮只把两个数字范围改为“到”式写法以通过定向证据。 |
| 架构 v2 | `AGENTS.md` 行数表述已是指针，保持不动。 |
| `docs/PARKING_LOT.md` | 追加 PL-022 / PL-035 / PL-065 / PL-066 的本轮结论。 |
| 产品代码 / schema / 依赖 / CI 判据 | **不改**。 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| “指针化”被误解成可以删历史证据 | D3 明确历史观测值边界；只删当前状态副本。 |
| 指针指向的文件本身也会漂移 | D2 把每类事实源固定到当前治理链的现有唯一落点；ADR-0039 / 0041 管同步。 |
| 未来把规范常量误判为派生值 | D1 明确规范常量仍在其契约文件定义，其它文档只引用。 |
| 没有新门禁导致复发 | 本轮保留定向 `rg` 证据；后续卡在单独 ADR 下评估机器规则。 |

## 验证方式

1. `cargo run -p xtask -- adr-index` 为 0E0W，登记表三方一致。
2. `cargo run -p xtask -- refscan` 无 `BARE-PENDING`，ADR-0072 引用可解析。
3. 对改动文件运行第二轮验收命令，输出为空。
4. `plans/stage-1-pilots.md` 的 diff 只包含当前进度句与完成标记，不含批次表/位置表条目正文。
5. `MEMORY.md` 的规模表仍由 `cargo run -p xtask -- memory-counts` 校验通过。
6. 后续若新增机器规则，必须另立 Accepted ADR，不得直接扩 `gov §5.4` 的 13 项。

## 重新评估触发条件

- 若同类派生值在三次后续任务中再次被手抄并漂移，应立卡评估 `docscan` 规则。
- 若 xtask 获得自动生成文档的能力，应把“打印正确值让人粘贴”的剩余对象重新评估为生成式校验。
- 若 `docs/adr/README.md` 或 `PLAN.md` 本身被拆分，则 D2 的指针目标必须同步更新。
