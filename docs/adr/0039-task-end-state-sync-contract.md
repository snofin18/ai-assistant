# ADR-0039　任务结束时的状态同步契约：`PLAN.md` + `README.md` 必须随卡同步

状态：**Accepted**（2026-09-24，人类 chat 裁决「DRIFT-202-2：授权你做。必须要做到计划列表每次更新为最新状态，包括 readme 里的相关部分。需要每次新任务结束，自动更新这些」）　日期：2026-09-24　Supersedes：—　Superseded by：—

来源：**DRIFT-202-2**（TASK-202 附带发现：`PLAN.md` 的「当前状态」块停在 TASK-012 开工前）；
关联：`AGENTS.md` §11（进度同步规则）、`docs/governance-ai-agent-execution.md` §3 / §9.4、
`PLAN.md`（索引，≤60 行）、`README.md`（对外状态行）、`LEDGER.md`、
`tasks/TASK-202-storage-migration-registry.md` §5 DRIFT-202-2、ADR-0030（机器校验优于手工回填）。

---

## 背景（为什么现在要决定）

TASK-202 收尾时发现一个**反复发生的**症状：`PLAN.md` 的「当前状态」块仍写着
「当前任务卡：**TASK-012 存储层 = 下一张**」「阻塞项 ① TASK-012 需人类先批准 3 个依赖」，
而 TASK-011 / 012 / 013 / 200 / 201 / 202 早已 Done。`README.md` 的状态行同样停在
「`crates/protocol` / `crates/storage` 已完成，接着是审计 / 密钥 / 护栏」。

这不是第一次：`AGENTS.md` §11 早就要求「每次 commit / push 之后必须同步进度文件」，
但那张表里**只**列了 `LEDGER.md` / `MEMORY.md` / `README.md 状态行` / `docs/memory/*`，
**没有** `PLAN.md`；而 `PLAN.md` 自己的抬头写着「agent **不得修改本文件**，只能提案」，
`AGENTS.md` §11.1 也把它标成 **Orchestrator-only**。于是每个 Implementer 都
**正确地**不去碰它 —— 结果就是**所有人都不改**，它只能靠 Orchestrator 偶然想起。

| # | 后果 | 证据 |
|---|---|---|
| 1 | 「下一步做什么」这个最高频问题的答案**长期是错的** | `PLAN.md` 停在 012 开工前（DRIFT-202-2） |
| 2 | 读者要交叉 `LEDGER.md` 才能还原真实状态 | `PLAN.md` 与 `LEDGER.md` 末行互相矛盾 |
| 3 | 「只读文件」这个保护**变成了漂移的成因** | 无人可写的文件必然腐化（除非有专职维护者） |
| 4 | 新会话启动按 `PLAN.md` 领卡 → 领到一张已 Done 的卡 | `AGENTS.md` §3 启动第 ② 步读的就是它 |

ADR-0030 的教训在这里第二次出现：**靠「记得回填」的护栏会失效**。

## 决策（一句话）

**把「卡 Done」与「`PLAN.md` + `README.md` 同步」绑成同一件事**：每张卡 Done 时，
在同一分支、同一 PR 内更新这两份文件；无阶段级变化时也**必须显式写「无变化」**，
不得沉默跳过。可机器校验的部分（`PLAN.md` 更新日期的新鲜度、`README.md` 状态行存在性）
交给 xtask 的 `check-ledger`，归 TASK-015。

拆成五条：

**D1　同步义务的落点（三份文件，一次 PR）**

| 文件 | 何时 | 写什么 | 谁写 |
|---|---|---|---|
| `PLAN.md` | **每张卡 Done**（不再只是「阶段切换」） | 「当前状态」块的 4 行：更新日期 / 当前任务卡 / 阻塞项 / 下一步动作 | Implementer **可代 Orchestrator 写**（本 ADR 即授权，见 D4） |
| `README.md` | **每张卡 Done** | ① 状态行 ② `## 当前阶段` 的进展句 ③ `## 最近进展` 追加本条 | 同上 |
| `LEDGER.md` | 每张卡 Done（**既有要求**，本 ADR 不动） | 一行一事件 | Implementer |

**D2　「无变化」必须显式**

阶段未切换、公开宣告未变时，`PLAN.md` 的「更新日期」**仍然要改**（改成本次 Done 的日期），
`README.md` 的 `## 最近进展` **仍然要追加**本条卡；只有「阶段索引表」「平台基线」这类
真正没变的段落可以不动。**沉默跳过 = 违反本 ADR**（铁律 1：不允许静默）。

**D3　可机器校验的部分（归 TASK-015 的 `check-ledger`）**

人写不出「`PLAN.md` 说的是不是真话」的机器判据，但**新鲜度**可以：

1. `PLAN.md` 「当前状态」块的「更新日期」**不得早于** `LEDGER.md` **最后一条记录的日期**
   —— 这一条恰好抓住 DRIFT-202-2 的形态（LEDGER 有 09-24 的行，PLAN 写 09-23）。
2. `README.md` 必须存在一行以 `> 状态：` 开头的状态行，且其中出现当前阶段名（`阶段 N`）
   —— 抓住「状态行被删 / 被清空」。
3. 两条规则都按 ADR-0025 D1 的口径落地（先 Warning、清扫后升 Error；若上线即全绿则可直接 Error）。

**为什么不做「自动写回」**：ADR-0030 选项 1 已否决「工具替你改文档」——
写回会让「谁在什么时候声称了什么」失去作者署名，且掩盖「作者其实没想清楚」。
本 ADR 只做**红灯**，不做自动改。

**D4　`PLAN.md` 的权限：从「Orchestrator-only」改为「Orchestrator 所有 + Implementer 可代写」**

- `PLAN.md` 仍是 **Orchestrator 所有的文件**（内容口径、阶段划分由它定）
- 但 **Implementer 在自己的卡 Done 时，被授权更新「当前状态」块的 4 行**——
  这是本 ADR 的明确授权（对应 `AGENTS.md` §8 的「要改 → 提案 → DRIFT/ADR」）
- **越界判定**：Implementer 改了「阶段索引」「范围冻结提示」「关联文件」「变更历史」等
  其它段落 → 仍是漂移触发器 ⑤（超出 write scope），`card-check` / review agent 应拦

**D5　`README.md` 的权限不变：只允许改「状态行 / 当前阶段 / 最近进展」三处**

`AGENTS.md` §8 早已写「`README.md` **仅状态行**，其他章节只读」。本 ADR **明确扩到三处**
（状态行 + `## 当前阶段` + `## 最近进展`），因为「最近进展」天然是逐卡追加的流水，
把它排除在外正是 DRIFT-202-2 里 README 腐化的直接原因。其余章节（项目介绍、
产品层 vs 工程元层、文档地图、构建与验证、许可证）**仍只读**。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 保持现状（`PLAN.md` 只有 Orchestrator 能改） | ❌ | 就是 DRIFT-202-2：无人可写 → 必然腐化；ADR-0030 的教训 |
| 2 | **卡 Done = 同一 PR 内同步 `PLAN.md` + `README.md`；`check-ledger` 管新鲜度**（本 ADR 采纳） | ✅ | 义务落在**做事的那个人**身上；有红灯；不发明新文件 |
| 3 | 删掉 `PLAN.md`，只留 `LEDGER.md` + `plans/*` | ❌ | `PLAN.md` 是 `AGENTS.md` §3 启动第 ② 步的入口（≤60 行的最小读取面）；删了就得读整个 LEDGER |
| 4 | 让 xtask 从 `LEDGER.md` **自动生成** `PLAN.md` | ❌ | ADR-0030 选项 1 同款否决：自动写回掩盖作者意图；且 `PLAN.md` 的「下一步动作」是**判断**不是**记录** |
| 5 | 把同步义务塞进 `LEDGER.md` 的行里（不单独改 PLAN/README） | ❌ | 读者不会去读 300 行的台账找「下一步是什么」；两个文件的读者面不同 |
| 6 | 只在 review 阶段人工检查 | ❌ | 已有 review 环节，DRIFT-202-2 仍然发生 —— 人眼不是机器判据 |

## 影响范围（本 ADR 实施时）

| 类别 | 文件 | 改动 |
|---|---|---|
| 文档 | `docs/adr/0039-task-end-state-sync-contract.md` | **新增**（本文件） |
| 文档 | `docs/adr/README.md` | §1 加 0039 / 0040 两行 +「下一个可用编号」→ 0041 |
| 文档 | `docs/memory/decisions.md` | 追加 `[DECISION][src:ADR-0039]` |
| 文档 | `AGENTS.md` §3 / §8 / §11.1 / §11.2 / §11.3 | §11.1 表内 `PLAN.md` 行由「阶段索引变化」改「**每张卡 Done**」+ 新增 `README.md` 三处行 + `PLAN.md` 其余段落行；§11.2 次序插入状态同步（7 步）；§11.3 加 3 条 fail 信号；§8 标注 `PLAN.md` 的唯一例外；§3 的 guard 名单加 `PLAN.md` / `README.md` / `plans/*` |
| 文档 | `AGENTS.md` §8 | `PLAN.md` 行加「Implementer 可在卡 Done 时代写『当前状态』块（ADR-0039 D4）」 |
| 文档 | `docs/governance-ai-agent-execution.md` | §9.4 交付前清单 + §3 会话协议「流程」段加「同步 `PLAN.md` / `README.md`」 |
| 文档 | `PLAN.md` | 「当前状态」块按 D1 重写（本次即 DRIFT-202-2 的处置） |
| 文档 | `README.md` | 状态行 + `## 当前阶段` + `## 最近进展` 三处更新 |
| 任务卡 | `tasks/TASK-015-xtask-hygiene-archtest-replay-skeleton.md` | DoD / 步骤加 `check-ledger` 的 D3 两条规则 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Implementer 代写 `PLAN.md` 时越界改了别的段落 | D4 明确越界判定 + `card-check` 的「正文区 diff 非空 → Error」同型思路；review agent 检查项加一条 |
| 「`PLAN.md` 更新日期 ≥ LEDGER 末行日期」被无意义地满足（只改日期不改内容） | D2 要求「无变化」也要改日期 —— 该规则只抓**整段没动**的形态；内容真实性仍靠 review（本 ADR 明确承认这一点，不假装机器能读语义） |
| 每张卡都改 `PLAN.md` → 与其它分支冲突 | `PLAN.md` 的「当前状态」块是**短小的四行**，冲突面远小于 `LEDGER.md`；且栈式 PR 顺序合并下天然串行 |
| 两个并行会话同时改 `PLAN.md` | `PLAN.md` 加入 ADR-0028 的 guard 锁名单（写入前取锁）—— 与 `LEDGER.md` 同等待遇 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **下一次卡 Done 时**：`git show --stat` 里必须同时出现 `PLAN.md` + `README.md` + `LEDGER.md`；
   只有 `LEDGER.md` = 本 ADR 被违反。
2. **`PLAN.md` 的「更新日期」不再落后**：任意时刻 `PLAN.md` 的日期 ≥ `LEDGER.md` 末行日期
   （TASK-015 落地 `check-ledger` 后由 CI 硬拦）。
3. **`README.md` 状态行始终存在且提到当前阶段**（同上，机器判据）。
4. **`PLAN.md` 的正文其它段落未被 Implementer 改动**：review agent 检查项 + 每次 diff 复核。
5. 重新评估触发条件：① `check-ledger` 上线后若红灯**频繁误报**（例如 LEDGER 有同日补记行
   而 PLAN 日期语义含糊）→ D3 的判据需细化；② 若 Implementer 代写 `PLAN.md` 造成
   阶段划分被悄悄改动 ≥ 2 次 → D4 的授权应收回到 Orchestrator。

## 相关 ADR

- ADR-0030（机器校验优于手工回填 —— 本 ADR D3 是它的第三次应用）
- ADR-0028（公共热点文件写前取锁 —— `PLAN.md` 按 D-风险表加入名单）
- ADR-0031（一卡一文件 —— 「卡 Done」是本 ADR 的触发事件）
- ADR-0025（卫生规则先 Warning 后 Error 的上线口径 —— D3 的落地方式）
