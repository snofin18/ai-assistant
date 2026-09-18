# 多 Agent 编排与任务分配

> 版本：1.2　日期：2026-09-18（1.0：2026-09-16；1.1 新增 §10.1「派生任务卡会话的前置条件」；1.2 新增 §11「并行改写的锁协议」—— ADR-0028）　上位：`AGENTS.md`、`docs/governance-ai-agent-execution.md`（gov）§8
> 目的：把「主要由 AI coding agent 完成实现」这件事，从"随手叫几个 agent"变成**有角色、有边界、有交接协议、可审计**的工程流程。

---

## 1. 三条原则

1. **一个 agent = 一张任务卡 = 一个会话 = 一个互不重叠的 write scope。** 三者绑定，任一被打破就会掩盖漂移。
2. **人类审阅速度决定项目速度。** agent 产出不是瓶颈，因此并行度由"人类每天能认真审阅多少行 diff"倒推，而不是由 agent 数量决定。
3. **决策不下放。** 契约变更、DRIFT 裁决、范围调整一律回到 Orchestrator → 人类。subagent 只能**提案**。

---

## 2. 角色与职责矩阵

| 角色 | 输入 | 产出 | 可写 | 硬性禁止 |
|---|---|---|---|---|
| **Orchestrator**（主 agent，与人类对话） | PLAN、阶段计划、Spike 结论 | 任务卡、write scope 分配、进度汇总、DRIFT 升级 | `tasks/**`、`PLAN.md`（提案）、`LEDGER.md`、`MEMORY.md` §1 | 不写实现代码（或仅 ≤50 行小卡）；**不代人类裁决 DRIFT**；不改 spec/ADR |
| **Implementer** | 一张任务卡 + 最小上下文包 | 代码 + 测试 + 文档同步 + 完成报告 | 卡内 write scope + `LEDGER.md`/`MEMORY.md`/`PARKING_LOT` 追加 | 不改 spec/ADR/PLAN/他人 crate；不改验收标准；不判定自己完成；不顺手重构 |
| **Reviewer** | diff + 任务卡 + 相关 spec | 问题清单（按 gov §7.1 十项） | 无（只读） | 不改代码；不放行未跑验收命令的卡 |
| **Spike/Researcher** | Spike 卡 + 判据 | `docs/spike-reports/SPIKE-<X>.md` | `spikes/**`、报告文件 | **不写产品代码**（`crates/`、`apps/` 禁入） |
| **Docs** | 已合并的实现 | README/spec 草案/MEMORY 条目 | `docs/**`、`crates/*/README.md` | 不改代码逻辑 |
| **Auditor** | 阶段全部产出 + spec + PLAN | `docs/audits/STAGE-N-audit.md` | 报告文件 | **必须由未参与该阶段实现的 agent 担任**；不改代码 |

---

## 3. 何时用 subagent，何时不用

### 3.1 适合派给 subagent（ bounded、可独立验收、write scope 清晰）

- 单张任务卡的实现（尤其是有明确 In/Out scope 与验收命令的）
- 独立模块的单元测试补齐
- Spike 技术验证（只产出报告）
- 代码审阅（只产出问题清单）
- 文档同步（README、spec 草案、MEMORY 条目整理）
- 重复性但规则明确的改造（例如"给所有 crate 补 README 的不变量一节"）
- 阶段末对齐审计

### 3.2 不适合派给 subagent（必须由 Orchestrator/人类处理）

| 情形 | 原因 |
|---|---|
| 需要先做架构决策的工作 | subagent 会自行决策 → 漂移；决策必须先落成 ADR |
| 契约变更（schema/trait/ErrorCode/DB） | 影响面跨模块，write scope 无法界定 |
| 探索性设计（"想清楚这个模块该怎么设计"） | 产出是选项与取舍，不是代码；应由 Orchestrator + 人类完成 |
| DRIFT 裁决 | 只能人类裁决 |
| 跨多 crate 的重构 | write scope 必然重叠，且难以独立验收 → 应先拆成多张有序卡 |
| 调试"只有人类环境能复现"的问题（真机应用、许可、登录） | subagent 无法自测 → 变成盲改 |
| 修 CI/门禁本身 | 护栏的修改必须人类审阅（否则 agent 可能"放宽护栏让测试通过"） |

---

## 4. 任务分配算法（Orchestrator 用）

```text
输入：阶段计划（plans/stage-N.md）中的卡列表 + 依赖关系
① 拓扑排序：按依赖分批（wave），同一批内互不依赖
② 对每张卡确定 write scope（目录/文件级），并检查：
     同批内任意两张卡的 write scope 交集 == ∅ ？
     若不为空 → 合并为一张卡，或调整边界，或改为串行
③ 估算人类审阅成本：Σ(该批卡的预期 diff 行数)
     若 > 人类日审阅上限（建议 800~1200 行）→ 降低该批并行度
④ 决定并行度 = min(3, 该批卡数, 审阅上限 / 单卡平均 diff)
⑤ 为每张卡生成派单包（§5）并 spawn
⑥ 收集完成报告 → 校验（§6）→ 派 Reviewer → 人类合并 → 更新 LEDGER/PLAN
⑦ 任一卡失败/DRIFT → 不阻塞同批其他卡，但必须立刻升级人类
```

**write scope 冲突的常见来源与处理**：

| 冲突 | 处理 |
|---|---|
| 两张卡都要改 `crates/platform/api` 的 trait | 合并为一张卡，或先做"trait 定义卡"再做"实现卡" |
| 两张卡都要改同一个 Adapter 的不同工具 | 按文件切分（`tools/read.json` vs `tools/write.json`），且 `adapter.toml` 只由其中一张卡改 |
| 两张卡都要加 CI 步骤 | CI 配置由**单张卡**统一改（护栏类文件不并行） |
| 两张卡都要改 `protocol/` schema | 禁止并行；schema 变更必须串行 + ADR |

---

## 5. 派单包（最小上下文，**不要 fork 会话历史**）

### 5.1 为什么不用 `fork_context`

fork 父会话会把大量**与当前卡无关的讨论**注入 subagent，既浪费上下文预算，又带来漂移风险（讨论中的临时想法会被当成结论）。**默认 `fork_context: false`**，用显式的派单包传递上下文。

例外：仅当子任务**必须**依赖父会话中尚未落文档的推理时才用 fork，且事后必须把结论落进 MEMORY/ADR。

### 5.2 派单包内容（总量 ≤ 800 行）

```text
1. AGENTS.md 全文（~164 行）
2. tasks/TASK-NNN-<slug>.md 全文（正文区 + 记录区骨架；ADR-0031）
3. plans/stage-N.md 中该卡所属批次的段落（In/Out scope + DoD）
4. 引用的 docs/spec/*.md 相关章节（只给章节，不给全文）
5. 目标 crate 的 README.md（职责/边界/不变量）
6. MEMORY.md §2（已确认事实）、§4（已否决方案）、§5（踩坑）
7. 一句话说明它在整体中的位置与下游依赖者
```

### 5.3 派单消息模板

```text
你是 Implementer，负责且仅负责 TASK-0NN。

【必读顺序】AGENTS.md → tasks/TASK-0NN-<slug>.md → 下方 spec 摘录 → crates/<x>/README.md → MEMORY.md §2/§4/§5
【第一步】输出 AGENTS.md §3 的【约束回执】，然后停下等我确认。不要先写代码。

【任务卡】<粘贴 tasks/TASK-0NN-<slug>.md 全文>
【write scope】<文件/目录清单>——超出即停并记 DRIFT
【相关 spec 摘录】<章节内容>
【不变量】<目标 crate README 的不变量清单>
【禁止】<Out of scope 要点 + AGENTS.md §7 中最相关的 5 条>
【验收】<命令清单>，全部执行并把输出粘进完成报告
【完成报告格式】AGENTS.md §9
【特别注意】<该卡最容易漂移的 1~2 处，例如"不要为了让测试通过而放宽 lint">
```

---

## 6. 交接与校验协议

```text
Implementer 提交完成报告
      ↓
Orchestrator 机械校验（不需要读代码就能做的检查）：
  ① 改动文件是否全部在 write scope 内？（git diff --name-only 对比）
  ② 验收命令是否全部执行且通过？（看报告里的输出）
  ③ LEDGER 是否追加？MEMORY 是否按需追加？crate README 是否同步？
  ④ 是否出现新依赖/新 crate/新顶层目录而无 ADR？
  ⑤ 是否有 DRIFT 记录未处理？
      ↓ 任一不通过 → 退回（不派 Reviewer，省人类时间）
Reviewer（独立 agent，只读）按 gov §7.1 十项清单审阅 → 输出问题清单
      ↓
Orchestrator 汇总 → 人类做最终合并决定
      ↓
合并后：更新 PLAN 任务队列状态、LEDGER、（若产生新事实）MEMORY
```

**Orchestrator 的机械校验清单要写成脚本**（`xtask card-check TASK-0NN`），避免 Orchestrator 自己"看一眼觉得没问题"——这也是护栏。

---

## 7. 失败、超时与升级

| 情况 | 处理 |
|---|---|
| subagent 报 DRIFT | **原样升级人类**，不代为决策、不"先按你的理解做" |
| subagent 超预算（时间/diff 行数/文件数） | 终止 → 回收 write scope → 把卡拆更小 → 重派 |
| 同一张卡连续失败 2 次 | **不要再派第三次同样的卡** → 说明卡本身有问题（范围不清/依赖未就绪/spec 有歧义）→ Orchestrator 重新拆卡或升级 |
| subagent 悄悄扩大范围（改了 scope 外文件） | 视为缺陷：回退该部分改动 + 记入 LEDGER + 在 Reviewer 清单中重点标注 |
| subagent 放宽了 lint / 加了 `#[allow]` / 改了测试断言 | 一律回退并要求解释；这是最典型的"局部最优"漂移 |
| Reviewer 与 Implementer 意见冲突 | 不由 Orchestrator 裁决 → 升级人类 |
| 需要真实应用/凭据/网络的验证 | 不由 subagent 做 → 转为人类手工验收项，写进卡的 DoD |

---

## 8. 反模式（明确禁止）

1. ❌ 让 subagent 自己拆卡或自己决定范围
2. ❌ 给 subagent "顺便把这里也优化一下" 的许可
3. ❌ 两个 agent 共享同一 write scope "这样合并方便"
4. ❌ 用 subagent 绕过人类审阅（"它自己 review 过了"）
5. ❌ 让写代码的 agent 同时当 Reviewer
6. ❌ fork 长会话历史给 subagent（污染 + 浪费预算）
7. ❌ 一次派 5 个以上 agent（人类审阅必然成为瓶颈，且冲突概率上升）
8. ❌ 让 subagent 修改 CI 门禁、lint 配置、`AGENTS.md`、`deny.toml`
9. ❌ 把"跑不通就重试"当作策略（连续失败 2 次必须重新拆卡）
10. ❌ 编排过程不留痕（谁派了谁、write scope、结果 → 必须记 LEDGER）

---

## 9. 本项目的具体编排方案

### 9.1 阶段 0（Spike）

| 波次 | 并行 agent | 卡 |
|---|---|---|
| W0 | 1（Orchestrator 自己做或 1 个 Implementer） | TASK-001 仓库骨架 |
| W1 | 3 | 002 Notepad、005 工具选择、009 存储 PoC |
| W2 | 3 | 003 Paint、004 跨进程 Host、008 Edge CDP |
| W3 | 3 | 006 UI 原型、007 撤销闭环、010 Linux 侦察 |
| W4 | 1（Auditor） | 汇总 8 份 Spike 报告 → 回填 MEMORY → 起草 spec/ADR → 生成阶段 1 任务卡 |

> 阶段 0 的 agent 全部是 **Spike/Researcher 角色**，禁写 `crates/`、`apps/`。

### 9.2 阶段 1（见 `plans/stage-1-pilots.md` 的批次表）

| 批次 | 建议并行度 | 说明 |
|---|---|---|
| A1 地基（011~015） | **1** | 必须串行；011/012 是后续一切的基础，建议人类逐行审阅 |
| A2 平台（016~019） | 2 | 016 先行，然后 {017, 019} 并行，018 接 017 |
| A3 内核（020~028） | **2**（不建议 3） | 8 张卡、每张 diff 可达 400 行；022/024/028 三张关键卡建议人类逐行审阅 |
| A4 应用与 UI（029~032） | 2 | 029 先行，然后 {030, 032} 并行，031 需 017 就绪 |
| A5 Notepad 闭环（033~039） | 1~2 | 033 ∥ 034，之后串行 |
| 1b Paint（040~047） | 2 | {040, 041} 并行 → 042 → 043 → 044~046 串行 |
| 1c Edge（048~058） | 2~3 | {048, 050, 053} 并行 → {049, 051, 052} → 054 → 055 → 056~058 串行 |

### 9.3 人类审阅预算（决定并行度的硬约束）

```text
假设：人类每天可认真审阅 ~1000 行 diff
单卡平均 diff：基础设施卡 ~300 行，Adapter 卡 ~200 行，UI 卡 ~350 行
→ 每天可合并 3 张卡 → 并行度 2~3 是上限（考虑返工与 DRIFT）
→ 阶段 1 的 48 张卡 ≈ 16 个工作日的人类审阅时间；实现时间通常不是瓶颈
```

**推论**：如果要加快，优先手段是**减小单卡 diff**（拆更细的卡），而不是增加 agent 数量。

---

## 10. 与工具机制的对应（Codex / opencode / Claude Code）

| 概念 | 实现方式 |
|---|---|
| 派单 | `spawn_agent`（**`fork_context: false`**）+ §5.3 的派单消息 |
| 派生**任务卡会话**（人类可见的独立 Codex 任务） | ⚠️ **`create_thread` 当前不可用**（见下方 §10.1）→ 由**人类手工新建任务**，或 `fork_thread` |
| 追问/补充上下文 | `send_input`（不中断）；需要立即改变方向时用 `interrupt: true` |
| 等待结果 | `wait_agent` **尽量不用**（会阻塞 Orchestrator）；优先在等待期间做不重叠的工作（例如起草下一批卡） |
| 释放并发额度 | 完成后及时 `close_agent` |
| 独立审阅 | 用**新的** agent 做 Reviewer，不复用 Implementer 的会话 |
| 阶段审计 | 用**未参与实现**的新 agent 做 Auditor |
| 规范文件 | 三者统一读 `AGENTS.md`；`CLAUDE.md` 仅一行转发 |

**Orchestrator 的自我约束**（写给主 agent）：
- 不要把所有事都派出去：**拆卡、校验完成报告、汇总 DRIFT、维护 PLAN/MEMORY 是 Orchestrator 自己的活**；
- 不要在等待 subagent 时空转：用这段时间起草下一批任务卡或整理 MEMORY；
- 不要重复 subagent 已完成的工作（不要"我再改一下"，应退回让它改）；
- 编排本身要留痕：每次派单在 `LEDGER.md` 记一行（卡号、角色、write scope、结果）。

### 10.1 前置条件：派生「任务卡会话」当前不能靠 `create_thread`（2026-09-18 实证）

**结论：TASK-002 及后续任务卡的会话，一律由人类手工新建（或用 `fork_thread`），不由 agent 调 `create_thread` 派生。**

| 项 | 内容 |
|---|---|
| 现象 | 2026-09-17 连续 3 次调用 `create_thread` 全部返回 `create_thread received invalid arguments.`，wall time 0.048~0.056 s；同插件的 `list_threads` 正常 → 不是连通性问题 |
| 根因（rollout 直接取证） | **不是权限，是参数结构**：前两次只给了 `prompt`、**缺 `target`**；第三次 `target` 被序列化成了 **JSON 字符串**而非对象。0.05 s 的耗时说明是**本地 schema 校验**当场拒绝，请求根本没发到服务端 |
| 正确结构 | `target` 必须是**对象**：形如 `type=project` + `projectId`（由 `list_projects` 取）+ `environment.type=worktree 或 local`；工作区是 git 仓库时默认 `worktree`，否则 `local` |
| 为什么仍不采用 | 上游 **openai/codex#36315**（`create_thread` 拒绝合法的 project+worktree 请求）与 **#36250**（project-aware `create_thread` 的原子性/幂等）**仍 open**；且本机从未验证过「结构写对就能成功」——验证本身会在人类侧边栏里真的建出一个任务，**代价不对称** |
| 替代做法 | ① **人类手工新建任务**（首选）；② `fork_thread`（注意反模式 6：不要 fork 长会话历史给 subagent）；③ 会话第一句必须贴 `AGENTS.md` §3 的**约束回执**；回执与任务卡不符 = 上下文已污染 → **重开会话**，不要纠正 |
| 跟踪 | `docs/memory/open.md` **N3** + `docs/PARKING_LOT.md` **PL-024**：#36315 关闭后重测一次；成功则把本条降级为备选 |

> **勿混淆两种「派活」**：`spawn_agent`（subagent，进程内、write scope 由 Orchestrator 当场界定）
> **可用且仍是本节主路径**；`create_thread`（在 Codex 应用里新建一个**人类可见的独立任务**）
> 才是本条限制的对象。§2 角色矩阵、§4 分配算法、§5 派单包、§9 各阶段并行度 **全部照旧**。
> 任务卡会话之所以要「独立任务」而不是 subagent，是因为它需要**干净的上下文**（AGENTS.md §3 启动协议）
> 与**人类可直接接管**的会话窗口；subagent 的中间过程对人类不可见，不适合承载一张完整任务卡。

---

## 11. 并行改写的锁协议（ADR-0028，v1.2 新增）

> **一句话**：`write scope 互不重叠`**仍然必须**（§4 的分配算法不变），`xtask guard` 是**第二道保险**，
> 不是它的替代品。两者拦的是不同的失效模式。

### 11.1 为什么 write scope 不够

| 防线 | 粒度 | 缺口 |
|---|---|---|
| §4 / §8 的 write scope 划分 | **文档约定**（每个 agent 声明自己只改哪些文件） | 没有运行时强制。「读文件 → 想 → 写回」之间**没有任何原子性**；两个 agent 各自基于旧内容写回 = 经典 **lost update**，而 **git 不会报冲突**（两次写都「成功」了），丢失的内容无声无息 |
| `xtask guard`（ADR-0028） | **单个文件**，运行时 | 协作式，**无法技术强制**（agent 可以绕过它直接写文件） |

风险最高的恰恰是三个**只追加的公共热点文件**：`LEDGER.md`、`docs/memory/*.md`、`docs/PARKING_LOT.md`。
它们是「所有会话都必须写」的文件，因此**天然无法用 write scope 划分**；而「只追加」的语义又让覆盖
**特别难被发现** —— 别人追加的那一行消失了，没有任何工具会报错，直到几天后有人发现记忆断层。

### 11.2 Orchestrator 派单时的三条硬规则

1. **能划分就不要共享**：并行的 subagent 之间 write scope **必须互不重叠**（§4 照旧）。
   只有「所有会话都要写」的公共热点文件才进入锁的适用范围。
2. **`--owner` 必须会话级唯一**：格式 `<agent>-<thread 前 8 位>`（例如 `codex-1a2b3c4d`）或 `TASK-NNN`；
   夜间运行固定用 `nightly-<当夜日期>`（章程 §11.2）。
   **两个 agent 用相同 owner 会被判为同一持有者而直接复用锁** —— 那等于没锁。
   派单包（§5.2）里必须把 owner 一并下发。
3. **锁的获取/释放写进任务卡的验收步骤**，不靠 agent 自觉：
   `guard acquire <文件> --owner <owner> --task TASK-0NN --intent "<一句话>"` → 改 → `guard release`。

### 11.3 超时放弃（退出码 5）之后必须做什么

`guard` 的默认等待是 30 s，超时即**放弃**并做三件事：退出码 **5**、stdout 打印机器可读行
`-- guard-result: ABANDONED target=… held_by=… held_age_secs=… waited_ms=…`、往
`target/locks/abandonments.log` 追加一行。**第四件事是协议义务，工具做不了**：

- 放弃方**必须在 `LEDGER.md` 追加一行**，说明放弃了什么、为什么、打算怎么办；
- 放弃方**不得**改用 `--force` 硬抢（那会把别人的改动直接踩掉）；正确做法是**改做别的卡**，
  或升级给 Orchestrator / 人类；
- Orchestrator 在 §6 的交接校验里必须核对：本次并行的每个 agent 是否都留下了「取锁—释放」或「放弃通报」的痕迹。
  缺痕迹 = 有人绕过了 guard，按 §7 升级处理。

> **缺了第四条，前三条就只是「日志」而不是「通报」**（ADR-0028 D5）。这是本协议最容易被忽略的一点：
> 工具能保证「放弃是可见的」，但**不能保证「放弃被处理」**。

### 11.4 与夜间自动化的关系

夜间路径是**两层锁**（章程 §11.2）：`.nightly.lock`（仓库级，粗：夜间 vs 白天）＋ `guard`（文件级，细：
任意两个进程）。两层都要，不能只留一层 —— ①挡不住「两个白天会话同时改 `MEMORY.md`」，
②挡不住「夜间任务与人类同时改整个仓库」（guard 是文件级、协作式，且锁在 `target/` 下会被 `cargo clean` 删掉）。
