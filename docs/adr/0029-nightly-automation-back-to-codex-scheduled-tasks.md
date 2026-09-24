# ADR-0029　夜间自动化的目标机制改回 Codex 原生 scheduled tasks

状态：**Accepted**（2026-09-18，人类指示 #2）　日期：2026-09-18　Supersedes：`ADR-0018`（Windows 任务计划程序 + `codex exec`）　Superseded by：—

> **本 ADR 是「方向」决策，不是「启用」决策。**
> 启用前置门禁见 D3：**端点兼容性实测通过之前，禁止创建任何真实 automation。**
> 本轮交付物是**调研与落档**（操作手册 + 验收清单 + 章程 v1.4），不是可运行的夜间任务。
>
> **2026-09-24 状态更新（GATE-0 已通过）**：D3 的启用前置门禁已于 **2026-09-24 实测通过**
> （cron 与 heartbeat 两种形态都验过；投递形态已变为正常 user message，E1 的 400 根因消失 ——
> 证据见 `docs/nightly/codex-automations-operations.md` §2 / §8.2 与 `docs/memory/open.md` N9）。
> → 本 ADR 从「**方向已定、未启用**」转为「**已启用（尚未排期）**」；章程 §11.9 状态随之改为 ✅、§11 状态字段回填为 v1.5。
> **方向决策（D1~D6）一字未改**，本段是状态行更新，授权来源 = 本文件「验证方式」第 7 条第 ① 项。

来源：2026-09-18 人类会话。原话：「**夜间自动化我的建议是查 codex 官方文档，明确建立、修改、删除、
运行、暂停、停止等具体操作写法，不是去做 windows 计划任务。到时候查清楚了专门找时间调试。
夜间自动化、memory.md 分层这些应该是属于该项目本身之外的辅助设计环节，和最终生成的项目无关吧？**」
关联：`docs/nightly/codex-automations-operations.md`（本 ADR 的主交付物）、
`docs/overnight-automation-charter.md` §11（v1.4 全章重写）/§12/§13、
`docs/nightly/scheduler-acceptance-test.md`（ADR-0018 的验收清单，降级为回退方案清单）、
`docs/adr/0018-nightly-automation-delivery-mechanism.md`、`README.md`（新增「产品层 / 工程元层」分类表）、
`docs/memory/open.md` N4/N5、`docs/PARKING_LOT.md` PL-012/PL-013/PL-015/PL-023

---

## 背景（为什么现在要决定）

人类指示包含**两个独立诉求**，必须分开处理，否则会互相污染：

### 诉求 A：机制方向改回 Codex 原生 scheduled tasks

ADR-0018（2026-09-17/18，Accepted）在探针证据下否决了 Codex automation 的两条路径，改用
「Windows 任务计划程序 + 包装脚本调 `codex exec`」。否决的技术根因是**真实且仍未消失**的：

| 证据 | 内容 | 2026-09-18 是否仍然成立 |
|---|---|---|
| E1 | automation 不把 prompt 当 user message 投递，而是注入一条 `FunctionCallOutput { id: None, call_id: None, name: "automation_update" }` 合成项 | ✅ 成立（机制未变） |
| E2 | 本机用第三方 Responses 兼容端点（阿里云百炼 compatible-mode，`qwen3.8-max`），对请求体严格校验 → **400 拒绝** E1 的畸形项 | ✅ 成立（端点未换） |
| E3 | `disable_response_storage = true` → 每轮重放全量历史 → 毒项每次都被重发 → **所挂 thread 永久损坏** | ✅ 成立 |
| E4 | cron 路径首轮投递时 Codex 生成非法 id `at_<uuid>`（端点要求 `msg_` 前缀），且该 id 从不落盘 → 等长补丁法无效 | ✅ 成立 |

人类的指示并没有推翻 E1~E4，而是指出**方案选择的取向**：Windows 任务计划程序是「在 Codex 之外
自建一套调度器」，它带来三个 ADR-0018 自己就承认的代价（章程 §11.6/§11.7）：

1. **没有应用内通知** —— v1.1 依赖的「仅失败时通知」消失了，失败可见性退化成三层文件信号，
   人类必须主动去翻 `LastTaskResult` / 日志 / 报告；
2. **依赖本机 PowerShell 脚本与计划任务的正确性** —— 这是一套**没有任何测试覆盖**的新代码
   （`scripts/nightly-run.ps1` 至今未创建，PL-023），而本项目的铁律是「无静默失败」；
3. **与 Codex 的产品演进脱钩** —— 官方在持续改进 scheduled tasks（侧边栏 Scheduled 视图、
   All/Active/Paused 分组、worktree 环境、`approval_policy`），自建方案享受不到任何改进，
   反而要在每次 Codex 升级时重新验证 CLI 行为（ADR-0018 风险表已登记此风险）。

**结论**：E1~E4 是**端点兼容性缺陷**，不是「Codex automation 本身不可用」。把它当成
永久否决理由，等于让一个可修复的环境问题决定项目的长期机制。正确的处理是：
**目标机制回到官方路径，把端点兼容性变成一道显式的启用门禁**，而不是变成方向选择的依据。

### 诉求 B：明确「工程元层」与「产品层」的边界（Q1）

人类问：「夜间自动化、memory.md 分层这些应该是属于该项目本身之外的辅助设计环节，
和最终生成的项目无关吧？」

**答案是：是。** 但这条边界此前**只存在于口头**，从未落档，导致三个真实症状：

- `README.md` 的文档地图把 `overnight-automation-charter.md` 与 `storage-design.md`
  并列，读者（尤其是未来开源后的外部读者）无法区分「哪些是产品的设计」与「哪些是我们怎么干活」；
- 每次讨论夜间自动化，都要重新解释一遍「它不影响 crates/ 里的任何代码」；
- 护栏工具（`xtask`）与治理文档的体积已经接近产品文档，若不分类，
  「阶段 0 零产品代码」这个事实会被掩盖（README 已因此加了一句免责声明，但那是补丁不是结构）。

边界不清的代价是**注意力错配**：工程元层的讨论会被误当成产品需求变更，反之亦然。

## 决策（一句话）

**夜间自动化的目标机制改回 Codex 原生 scheduled tasks（ADR-0018 降级为回退方案）；本轮只交付
「操作手册 + 验收清单 + 章程 v1.4」，在端点兼容性实测通过之前禁止创建任何真实 automation；
同时把「产品层 / 工程元层」的边界正式落档到 `README.md`。**

拆成六条：

- **D1　目标机制 = Codex 原生 scheduled tasks**（`automation_update` 工具 + 侧边栏 **Scheduled** 视图）。
  首选形态为 **standalone（cron 类）**：每次运行开一个全新 chat，天然避免 E3 的「毒项沉入长驻会话」
  与上下文累积漂移。**heartbeat（in-chat）不作为夜间主方案** —— 它复用既有 chat 的上下文，
  一旦 E1/E2 复现就是 E3 的完整重演（永久损坏一个长驻会话）。
- **D2　本轮范围 = 只调研落档，不做端到端调试**。交付物：
  ① `docs/nightly/codex-automations-operations.md`（建立/修改/删除/运行/暂停/停止/恢复逐条写法，
  **每条结论都带 `[官方]` / `[实测]` / `[未验证]` 证据标签**）；
  ② 该文件 §8 的启用前验收断言清单；③ 章程 §11 重写为 v1.4。
  **本轮不创建、不修改、不删除任何真实 automation**（探针只用「缺字段报错」方式安全探明 schema）。
- **D3　启用前置门禁（硬性，写进章程 §11.9）**：必须在**一个专门新建的废弃 thread** 上
  实测「automation 触发 → 端点接受合成项 → 模型正常回复 → 产物落盘」全链路成功，
  才允许把 automation 指向真实工作 thread / 项目。
  **严禁在主线工作 thread 上做此实验**（E3 已证明毒项会永久损坏该 thread，损失不可逆）。
  门禁未通过时的处置：① 保持 ADR-0018 的回退方案可用；② 把实测结果记入 `docs/memory/open.md`；
  ③ 若确认是端点侧校验，评估「换官方端点跑 automation」或「等第三方端点放宽校验」。
- **D4　ADR-0018 的地位 = Superseded，但保留为回退方案**。
  `docs/nightly/scheduler-acceptance-test.md` **不删除**，改挂「已被取代（当前非主方案），
  保留为回退方案验收清单」横幅；章程 §11.8 中 10 条 `codex exec` 本机实测**全部保留**
  （回退方案一旦启用就要用），只在表头注明其归属。
  理由：ADR-0018 的方案是**本机唯一被证据支持可运行**的路径（走标准 user message 投递，
  rollout 中 `function_call_output` 计数 0）；把它删掉等于在门禁失败时无路可退。
- **D5　边界落档（回答诉求 B）**：在 `README.md` 新增「产品层 / 工程元层」分类表，明确：
  **工程元层** = 治理与协作机制（`AGENTS.md`、`docs/governance-*`、`docs/subagent-orchestration.md`、
  `docs/overnight-automation-charter.md`、`docs/nightly/*`、`MEMORY.md` + `docs/memory/*`、
  `LEDGER.md`、`docs/PARKING_LOT.md`、`docs/adr/*`、`xtask/`、`.github/workflows/*`）；
  **产品层** = 会被构建进发布物的东西（`crates/`、`protocol/`、`adapters/`、`adapters-private/`、
  `apps/`、`fixtures/`、`eval/`）。
  判定标准一句话：**「删掉它，产品的行为会变吗？」不会 → 工程元层。**
  工程元层**不进入产品构建**、不出现在发布二进制里、不受产品 spec 约束（但受 `AGENTS.md` 约束）。
- **D6　章程 §11 → v1.4**：机制描述改回 Codex automation，但**保留 v1.3 中与机制无关的部分**
  （§11.2 锁、§11.3 当夜定义与轮次编号、§11.4 报告触发条件、§11.5 上报事件清单）——
  这些是「夜间工作纪律」，换任何调度器都成立。§11.2 同时按 ADR-0028 升级为**两层锁**。
  §12 变更历史追加 v1.4 行。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 维持 ADR-0018（Windows 任务计划程序） | ❌ 否决 | 三个代价见「诉求 A」：无应用内通知、需维护一套无测试覆盖的 PowerShell 调度代码、与官方产品演进脱钩。且人类明确指示不走这条路 |
| 2 | 改回 Codex automation，并**立即**创建真实任务试跑 | ❌ 否决 | E1~E4 一个都没修。立即试跑的最坏结果是**再一次永久损坏一个长驻 thread**（E3），代价不可逆。人类也说了「到时候查清楚了专门找时间调试」 |
| 3 | **改回 Codex automation，本轮只调研落档，启用设硬门禁**（D1~D3） | ✅ 采纳 | 方向对齐官方与人类意图；风险被一道显式门禁挡住；产出是可审阅的文档而不是一次可能炸掉会话的实验 |
| 4 | 双轨并行（automation 与计划任务都保留为正式方案） | ❌ 否决 | 「两个都是主方案」= 没有主方案。夜间自动化最忌机制不确定：锁语义、报告位置、通知策略全都依赖机制。改为**一主一备**（D4），备用只在主方案门禁失败时启用 |
| 5 | 彻底放弃夜间自动化 | ❌ 否决 | 夜间自动化是本项目「AI agent 实现 + 人类审阅」执行模式的产能放大器（章程 §0）。放弃它等于把项目速度压到人类审阅速度 —— 而 `AGENTS.md` §8 已明说「人类审阅速度决定项目速度」，不能再自我设限 |
| 6 | 把 `codex-automations-operations.md` 写进章程 §11 而不单独成文 | ❌ 否决 | 章程是**纪律**（该做什么、不许做什么），操作手册是**how-to**（具体点哪里、传什么字段）。混在一起会让章程膨胀到无法每次全读，违反 `AGENTS.md` §1「只读需要的」 |

## 影响（需要改的文档 / 代码 / 任务卡）

| 对象 | 改动 | write scope 授权 |
|---|---|---|
| `docs/adr/0029-*.md` | 新建（本文件） | ✅ |
| `docs/nightly/codex-automations-operations.md` | 新建：操作手册（本 ADR 主交付物） | ✅ |
| `docs/overnight-automation-charter.md` | §11 全章重写为 v1.4；文件头版本/日期；§12 追加 v1.4 行；§11.2 两层锁（ADR-0028） | ✅ |
| `docs/adr/0018-*.md` | **只改状态行**：`Superseded by：ADR-0029`（ADR 正文只增不改，状态行是唯一例外） | ✅ |
| `docs/nightly/scheduler-acceptance-test.md` | 顶部加「已被取代 / 保留为回退方案」横幅；正文不改 | ✅ |
| `README.md` | 新增「产品层 / 工程元层」分类表（D5）；文档地图补一行指向操作手册 | ✅ |
| `docs/adr/README.md` | §1 表新增 0028/0029/0030 行；0018 状态改 Superseded；下一可用号 → 0031 | ✅ |
| `docs/memory/open.md` | N4/N5 追加「机制已改回 Codex automation，验收对象随之改变」的解决行；新增 N9（端点兼容性门禁未过） | ✅（只追加） |
| `docs/memory/decisions.md` / `facts.md` / `pitfalls.md` | 追加本 ADR 的 DECISION / FACT 条目 | ✅（只追加） |
| `LEDGER.md` / `docs/PARKING_LOT.md` | 追加事件行与 PL-012/013/015/023 的处置行 | ✅（只追加） |
| `scripts/nightly-run.ps1` | **不创建**（PL-023 随之降级：仅在 D3 门禁失败、回退方案启用时才需要） | — |
| 产品代码 / `docs/spec/*` / schema / 任务卡 | **不改**（本 ADR 属工程元层，零产品影响 —— D5） | — |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| **D3 门禁始终过不去**（第三方端点持续 400） | D4 保留 ADR-0018 为回退方案，章程 §11.8 的 10 条 CLI 实测原样可用；同时评估「换官方端点跑 automation」（ADR-0018 已列为备选） |
| 实验时误在主线 thread 上创建 heartbeat → **永久损坏该 thread** | D3 明文规定「只能在专门新建的废弃 thread 上实验」；操作手册 §2 把它写成 GATE-0.2 / GATE-0.3 放在最前面；probe thread **强制命名前缀 `nightly-probe-`**，挂 automation 前由人工核对标题前缀。**不得**指望 `mode=view` 做这道确认 —— 探针实测它对不存在的 id 也渲染空卡（操作手册 §4.8 / R4），用它判断等于没判断 |
| 官方文档未覆盖 heartbeat 字段 / `notificationPolicy` / `automation.toml` schema → 手册里出现猜测 | 强制证据分级标签（`[官方]`/`[实测]`/`[未验证]`）；`[未验证]` 项**不得当作结论使用**，一律进 §7 缺口清单或 `open.md` |
| 抖动 +2 分钟导致「23:30 轮」实际 23:32 触发，跨日归属判断出错 | 章程 §11.3 的「当夜归属」判据本就基于**运行开始时刻**且窗口是 00:00~11:59，2 分钟抖动不影响归属；只有触发时刻恰好压在 00:00 边界时才有风险 → 手册建议避开 23:58~00:02 排程 |
| worktree 型 standalone 任务堆积（官方明确提醒） | 手册 §7 登记；章程要求夜间任务**只在 `nightly/<date>` 分支提交、不合并 main**，worktree 用完即归档 |
| 「一主一备」被误读成「两个都跑」 | 章程 §11.1 用表格写明：同一时刻**只允许一种机制处于 Active**；启用备用前必须先暂停/删除主方案的 automation |
| 本轮不真跑 → 手册里的写法可能有错 | 诚实标注：手册的 `[实测·探针]` 只覆盖四件事（schema 探明 / `mode=view` 的返回 / `~/.codex/automations/` 磁盘现状 / `codex --help` 子命令列表），**不覆盖创建成功后的运行行为**；§8 验收清单就是为此存在 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `docs/nightly/codex-automations-operations.md` 存在，含 §1~§9 全部九节，且**每条操作性结论都带证据标签**。
2. 该文件 §2「启用前置门禁」在最前面，且明文含「禁止在主线工作 thread 上实验」。
3. `git status --porcelain` 确认 `~/.codex/automations/` 下**没有新增任何 automation 目录**
   （本轮红线：不创建真实任务）。
4. 章程 §11 标题为 v1.4；§12 变更历史末行为 v1.4；§11.3/§11.4/§11.5 的纪律性内容仍然在
   （证明「保留与机制无关的部分」这条被真的执行了，而不是整章删掉重写）。
5. `docs/adr/0018-*.md` 状态行含 `Superseded by：ADR-0029`，且**正文其余部分逐字节未变**
   （`git diff --stat` 应只显示 1 行变更）。
6. `README.md` 含「产品层 / 工程元层」分类表，且表内每个路径都真实存在。
7. **重新评估触发条件**：① D3 门禁实测**通过** → 本 ADR 从「方向已定、未启用」转为「已启用」，
   章程 §11.9 状态改为 ✅，并把手册 §8 的断言结果落成基线；② 门禁实测**失败且确认不可修** →
   正式回退到 ADR-0018（届时 ADR-0018 需要一份新的 ADR 把它从 Superseded 恢复为 Accepted，
   不能靠口头）；③ Codex 官方文档补齐 heartbeat / `automation.toml` schema →
   手册的 `[未验证]` 项应逐条升级为 `[官方]`；④ 本机换用官方 OpenAI 端点 →
   E2 消失，D3 门禁可大幅简化。
