# AI Agent 执行模式下的规划守护与代码规范

> 状态：设计讨论稿（文档阶段）
> 版本：1.1　日期：2026-09-16（v1.1：加入 MEMORY.md、PLAN 拆分、命名与注释规范、多 agent 编排引用、开源准备）
> 配套：`AGENTS.md`（铁律速查，由本文档派生）、`cross-platform-ai-assistant-architecture-v2.md`、`target-apps-feasibility.md`

---

## 0. 为什么需要这份文档

本项目的执行主体是 **AI coding agent（Codex / opencode / Claude Code）**，人类的角色是**规划者、审阅者与裁决者**。这带来一个传统项目没有的核心风险：

> **随着会话上下文增长，执行会偏离规划；而偏离是渐进的、无声的、且每一步看起来都合理。**

### 0.1 漂移的五个根因

| # | 根因 | 表现 |
|---|---|---|
| 1 | **上下文淘汰** | 早期确立的约束被挤出窗口或被有损摘要，agent 只记得"大概要做什么" |
| 2 | **讨论取代规范** | 聊天里达成的临时共识变成事实上的规范，与书面 spec 分叉；下次会话无人记得 |
| 3 | **局部最优** | agent 优化"让这个测试通过 / 让这个函数工作"，而非"符合架构分层"，于是引入捷径、绕过抽象、复制代码 |
| 4 | **无机器护栏** | 分层被破坏、公共接口被随意改、依赖悄悄增加，但 CI 全绿 → 漂移不可见，直到重构成本极高 |
| 5 | **任务粒度过大** | 一张"实现任务引擎"的卡迫使 agent 即兴设计，等于把架构决策交给了每次会话的随机性 |

### 0.2 治理的四条对策（本文档的骨架）

```text
对策一：唯一事实源 + 冲突裁决顺序     → 消除「讨论取代规范」（§1）
对策二：小任务卡 + write scope        → 消除「任务粒度过大」与「局部最优」（§3）
对策三：会话引导协议 + 周期性重锚     → 消除「上下文淘汰」（§4）
对策四：机器可执行护栏 + 验收分离     → 让漂移在 CI 暴露，而不是在评审时靠肉眼（§5、§6）
```

一句话原则：

> **凡是不能机器校验的约束，都会在第三周被违反。**
> 所以本文档的每一条规范，都要尽量落到 lint 规则、CI 检查、schema 校验或 arch test 上。

---

## 1. 唯一事实源（SSOT）与文档分层

### 1.1 七层文档体系

| 层 | 文件 | 回答什么 | 变更门槛 | 谁改 |
|---|---|---|---|---|
| **L0** | `AGENTS.md` | 铁律 + 文档地图 + 验证命令（**每次会话必读**，≤ 300 行） | 高（需 ADR） | 人类 |
| **L1** | `PLAN.md`（索引，≤60 行）+ `plans/stage-N.md`（当前阶段详情） | 阶段、里程碑、**当前任务队列**、In/Out of scope 冻结 | 中 | 人类 |
| **L2** | `docs/spec/*.md` | 规范：协议、schema、接口契约、错误码、命名（**what**） | 高（需 ADR） | 人类 + agent 提案 |
| **L3** | `docs/adr/NNNN-*.md` | 决策与理由（**why**），不可变，只能被新 ADR 取代 | 只增不改 | 人类批准 |
| **L4** | `tasks/TASK-NNN-*.md` | 本次做什么 / **不做什么** / 怎么验收 | 低（人类签发） | 人类签发，agent 填写执行记录 |
| **L5** | `LEDGER.md` | 做了什么、结果如何、有无偏差（**执行台账**） | 只追加 | agent |
| **L6** | 代码 + 测试 | 实现 | — | agent |
| **L5b** | `MEMORY.md` | **认知**：已确认事实(FACT)、已决策(DECISION)、**已否决方案(REJECTED)**、踩坑(PITFALL)、未决(OPEN)、假设(ASSUMPTION) | 长期，硬上限 300 行 | agent 追加，Orchestrator 压缩 |
| 辅助 | `crates/*/README.md` | 该模块的职责、边界、**不变量**、已知限制 | 随代码 | agent |
| 辅助 | `docs/spike-reports/`、`docs/audits/` | 技术验证结论、阶段末对齐审计 | 只增 | Spike/Auditor agent |
| 辅助 | `docs/DEPENDENCIES.md`、`docs/PARKING_LOT.md`、`docs/OPEN_SOURCE_CHECKLIST.md` | 依赖登记、跑题想法停车场、开源前脱敏清单 | 只追加 | agent |

### 1.2 冲突裁决顺序（★ 最重要的一条规则）

```text
AGENTS.md 铁律  >  ADR  >  docs/spec  >  PLAN.md  >  任务卡  >  代码现状  >  聊天讨论
```

**`MEMORY.md` 的位置**：它是**认知记录**，不是权威规范。若 MEMORY 的条目与 spec/ADR 冲突，**以 spec/ADR 为准**，并立刻用新条目更正（标 `[supersedes:日期]`，不改写旧条目）。MEMORY 的三项价值：①让新会话不必重新验证已知事实 ②**让已否决的方案不被重新提出** ③让踩过的坑不再踩第二次。

推论：

1. **聊天记录永远不是事实源。** 任何在对话中达成的决定，若未落进 ADR / spec / 任务卡，**视为未发生**。这是防止"讨论取代规范"的硬规则。
2. **代码现状不能证明规范。** 如果代码与 spec 矛盾，默认是**代码错了**（除非有 ADR 说明 spec 已过时）。
3. **发现矛盾必须停下。** 若 agent 发现上层文档之间互相矛盾（例如 spec 与 ADR 冲突），**不得自行选择一方**，必须记录 `DRIFT` 并升级（§4.3）。

### 1.3 文档写作约束

- `AGENTS.md` **必须短**（≤ 300 行）：因为它每次都被读，长度直接消耗上下文预算；细节一律链接到 `docs/`。
- `docs/spec/*` 写**契约**，不写实现思路（"必须/禁止"多于"可以/建议"）。
- `ADR` 用固定模板（§8.4），**只增不改**：决策变了就写新 ADR 并标 `Supersedes: NNNN`。
- `PLAN.md` 只保留**当前阶段**的详细内容和后续阶段的一句话概要（避免过期信息误导）。
- 所有文档必须有 `状态 / 版本 / 日期 / 关联文档` 头部。

---

## 2. 规划守护：范围冻结与变更控制

### 2.1 范围冻结（Scope Freeze）

每个阶段开始时，`PLAN.md` 中必须写明：

```markdown
## 阶段 1 范围（已冻结）
### In scope
- Notepad Adapter：T1.1 / T1.2 / T1.3
- Excel Adapter：T2.1 / T2.2 / T2.3
- 策略引擎：白名单 + 风险分级 + 人工确认
- ...
### Out of scope（本阶段明确不做，做了算漂移）
- Paint / Photoshop / Edge 的任何代码
- macOS / Linux 平台层
- 外部 MCP server 加载
- WASM 插件
- 无人值守模式
- 技能市场 / 插件签名
- 向量检索记忆
```

**Out of scope 清单的价值高于 In scope 清单**——它直接定义了"什么算漂移"。

### 2.2 变更控制流程

```text
需要变更范围/设计？
  ├─ 属于「澄清」且不改变契约 → 在任务卡里记 CLARIFY，继续
  ├─ 属于「契约变更」（schema/接口/分层/依赖/Non-goal）
  │    → 停止编码
  │    → 起草 ADR（含：背景、选项、决定、影响、需回改的文档与代码）
  │    → 人类批准 → 更新 spec/PLAN → 拆出新的任务卡 → 才继续
  └─ 属于「发现 spec 有误」→ 记 DRIFT + 停止 + 升级
```

硬规则：

- **不允许"先实现再补文档"**。契约变更必须文档先行（否则下个会话的 agent 会照着旧文档把新实现改回去，形成来回震荡——这是 AI 协作项目特有的失败模式）。
- **不允许无声扩大范围**。任何 out-of-scope 的代码进入仓库，视为缺陷。
- ADR 未批准前，agent 可以**继续做不受影响的其他任务卡**，但不要"先写一半等批准"。

### 2.3 防止"讨论重点随进度漂移"

真实模式：做到第 3 周，发现某个技术问题很有趣/很难，讨论逐渐转向它，规划被无形改写。

对策：

| 机制 | 做法 |
|---|---|
| **主题停车场（Parking Lot）** | `docs/PARKING_LOT.md`：任何"想到但不属于当前任务卡"的事项，一行记入，**不讨论、不实现**。阶段末统一评审 |
| **阶段末对齐审计** | 每阶段结束由**独立 agent**（未参与实现）对比 代码 vs spec vs PLAN，输出偏差清单（§7） |
| **PLAN.md 变更历史** | 每次范围变更必须在 PLAN.md 底部追加一行（日期、变更、ADR 编号、批准人），使漂移可追溯 |
| **每会话开头声明当前任务卡** | 强制 agent 先说"我现在做 TASK-0NN"，人类一眼就能发现跑题 |

---

## 3. 任务卡制度

### 3.1 粒度规则

| 约束 | 上限 | 理由 |
|---|---|---|
| 单卡预估工作量 | ≤ 1 个 agent 会话（约 1~3 小时） | 超过就必须拆 |
| 单卡涉及文件数 | ≤ 8 | write scope 可控 |
| 单卡 diff 行数 | ≤ 400 行 | 人类可完整审阅 |
| 单卡验收命令 | ≤ 5 条 | 可机器执行 |
| 一张卡只做一件事 | — | **禁止 drive-by refactor（顺手重构）** |

拆分信号：如果一张卡里出现"并且"、"同时"、"顺便"，就该拆。

### 3.2 任务卡模板

> **本模板 = `tasks/TASK-NNN-<slug>.md` 的整个文件**（ADR-0031「一卡一文件」）。
> `plans/<阶段>.md` 只留**阶段级**信息（In/Out scope、阶段 DoD、卡号索引、批次与并行建议），
> **不放卡片正文**；`<slug>` 用 kebab-case 英文，且与分支名 `task/TASK-0NN-<slug>` 用同一个 slug。
> 卡片文件内有一条 HTML 注释**分界线**：以上是正文（Orchestrator 所有，Implementer **只读**），
> 以下是执行记录（Implementer 填写，9 节骨架见 §3.4）。
> **状态只写在卡片文件的 `- 状态：` 行**，`plans` 的索引表刻意不设「状态」列（ADR-0031 D2：
> 同一事实手写两处必然漂移，与 ADR-0030 同源）。

````markdown
# TASK-012　实现 Notepad 文本读取工具

- 状态：Ready / InProgress / Review / Done / Blocked
- 阶段：1
- 依赖：TASK-008（UIA Host 骨架）、TASK-011（TargetDescriptor 解析）
- 关联：spec/tool-schema.md、ADR-0007（element 不跨进程）、feasibility P1
- 预估：≤ 2 小时　难度：S/M/L

## 目标（一句话）
实现 `notepad.read_text` 工具，从已绑定的记事本文档读取全文并返回结构化结果。

## In scope
- crates/platform/windows/src/uia/text_reader.rs（新增）
- crates/tool-bus/src/builtin/notepad.rs（新增 read_text 注册）
- protocol/tool-schema/notepad.read_text.json（新增）
- crates/platform/windows/tests/notepad_read.rs（新增）

## Out of scope（做了算漂移）
- 写入/替换/保存（TASK-013）
- 多标签管理（TASK-015）
- 大文件的文件通道降级（记入 PARKING_LOT）
- 任何其他 adapter

## 必须遵守
- 文本经统一信封返回，标记 untrusted（spec/envelope.md）
- 超过 max_bytes 必须截断并显式标注 truncated
- 所有 UIA 调用带超时（默认 3 s）与可取消
- 错误必须用 ErrorCode 枚举（spec/error-codes.md），禁止字符串拼接错误
- 不得在 core crate 中直接调用 windows crate（arch test 会失败）

## 验收命令（agent 必须全部执行并粘贴输出）
```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-windows notepad_read
cargo run -p xtask -- verify-tool-schema protocol/tool-schema/notepad.read_text.json
cargo test -p assistant-core arch::dependency_direction
```

## 完成定义（DoD）
- [ ] 上述命令全部通过
- [ ] 靶机应用（fixtures/apps/notepad-like）上读取 1 KB / 100 KB / 1 MB 三档文本，结果正确
- [ ] 1 MB 场景返回 truncated=true 且不 OOM
- [ ] crates/platform/windows/README.md 更新（新增模块的职责与不变量）
- [ ] LEDGER.md 追加一行
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）
### 1. 约束回执（动手前填）      ### 2. 实际改动文件（逐个核对 In scope）
### 3. 验收输出摘要              ### 4. DoD 逐条核对
### 5. 偏差：none / DRIFT-0NN-x  ### 6. 更合理做法（非漂移，已落地 + 理由）
### 7. 遗留问题（PARKING_LOT 编号） ### 8. 新增长期记忆（FACT/PITFALL/REJECTED 原文）
### 9. 给审阅者的关注点（风险最高的 1~3 处）
（9 节的完整骨架与填写要求见 §3.4）
````

### 3.3 write scope 规则

- 任务卡显式列出**允许修改的文件/目录**；
- **超出即停**：需要动别的文件 → 记 DRIFT → 升级；
- 多 agent 并行时，**write scope 必须互不重叠**（否则合并冲突与相互覆盖会掩盖漂移）；
- **默认项（ADR-0031 D5）**：`tasks/TASK-NNN-*.md`（**仅本卡号那一个文件**）的**执行记录区**
  默认在每张卡的 write scope 内，不需要在卡里显式列出；**正文区（分界线以上）不在**。
  这样并行 agent 天然不重叠（一人一卡一文件），上一条自动满足；
- 只读文件（spec、ADR、其他 crate）可以读，不可以改。

### 3.4 执行记录文件骨架（ADR-0031 D3 / D4）

`tasks/TASK-NNN-<slug>.md` 的分界线**以下**必须含下面 9 节，标题文字与顺序都固定
（`xtask card-check` 会按标题扫描；缺节即 Warning，`Ready` 状态的卡豁免，因为它还没开工）。

| # | 小节 | 填什么 | 为什么必须有 |
|---|---|---|---|
| 1 | 约束回执 | `AGENTS.md` §3 的固定格式（任务 / 目标 / write scope / 铁律 / 禁止 / 验收 / 依赖 / 疑问），**动手前**填 | 回执与正文区不符 = 上下文已污染。落盘之后这个检测才有可回查的证据（此前只在聊天里，而「聊天记录不是事实源」） |
| 2 | 实际改动文件 | 清单，并**逐个核对**是否在 In scope 内 | 超范围是漂移触发器 ⑤，靠事后核对而不是靠记忆 |
| 3 | 验收输出摘要 | 命令 → 结果（全绿 / 失败项），粘关键输出 | 「无静默失败」的证据；也是 review agent 唯一能独立复核的东西 |
| 4 | DoD 逐条核对 | 打勾或写明未达成原因 | 防止「大概做完了」 |
| 5 | 偏差 | `none` 或 `DRIFT-0NN-x` 全文（现象 / 影响 / 建议 / 已停工作，格式见 §4.3） | DRIFT 必须有落盘位置，否则裁决时无从回看 |
| 6 | 更合理做法 | 非漂移、已直接落地的改进 + 理由 | 这类改动最容易被误当成漂移；写清理由才能被审阅 |
| 7 | 遗留问题 | 进 `docs/PARKING_LOT.md` 的编号 | 禁止 drive-by refactor 的配套：不相关的问题要有去处 |
| 8 | 新增长期记忆 | FACT / PITFALL / REJECTED 条目**原文**（无则写「无」） | 这是 DoD 的一部分（`AGENTS.md` §3）；写原文而不是「已追加」，便于 review 时核对措辞 |
| 9 | 给审阅者的关注点 | 风险最高的 1~3 处 | 人类审阅速度决定项目速度（§9.3）—— 把注意力引到最该看的地方 |

**分界线原文**（逐字符固定，`card-check` 靠它定位正文区/记录区的边界）。
**本处是该原文的唯一事实源**：ADR-0031 D3 刻意**不抄**一遍（抄两份就是「同一事实手写两处」，
ADR-0030 的整条动机）；卡片文件必须与下面三行**逐字符相等**。两条硬要求：
① 分界线**必须顶格**（列 0，不得缩进）—— `card-check` 按行前缀匹配，缩进一格就漏；
② 每个卡片文件**有且仅有一条**（多了就无法判定哪条是边界）。
差一个空格的后果不是「描述不一致」而是**全部卡片集体解析失败**，因此不得就地改措辞。

```markdown
<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->
```

**历史记录的合规通道**（ADR-0031 D6 判据 ③）：若某张卡的执行记录写于本规定之**前**（小节名与 9 节不同），
**不得为了过检查而改名/重排历史小节**（那是改写已提交的记录）；改为在记录区顶部追加一张
**旧格式 → 9 节骨架的对照表**（两列：「9 节骨架」与「本卡的对应位置」），且表中**必须逐节列全 9 节**。
`card-check` 见到这张表就视为判据 ③ 满足（表的第一列是机器可读的事实源）。
某节确实无对应内容时，对应行写「无（当时尚无此约定）」而**不是**留空 —— 留空与「忘了填」无法区分。
实例见 `tasks/TASK-001-repo-skeleton.md`（全仓唯一一张早于本规定的卡）。

**为什么用 HTML 注释而不是 `---`**：`---` 在 Markdown 里既可能是分隔线、也可能把上一行变成
setext 标题（本项目已为此设了扫描判据），而 HTML 注释渲染后不可见、也不会与既有分隔线混淆。

**`card-check` 的四条判据**（ADR-0031 D6；子命令本身归 PL-002，尚未实现）：
① 正文区的 `git diff` 非空 → **Error**；② 状态非 `Ready` 的卡必须有对应 `tasks/TASK-NNN-*.md` → 缺失即 Error；
③ 记录区 9 节标题必须齐全 → 缺节即 Warning；④ `plans/*.md` 里出现 `## TASK-0NN` 形态的卡片正文 → **Error**
（防止形态回退造成两处正文）。

---

## 4. 会话协议

### 4.1 会话启动协议（每个新会话 / 每个新 agent，前 5 分钟）

```text
① 读 AGENTS.md（全文，**实际 185 行**，`wc -l AGENTS.md` = 185；不要手抄行数 = PL-035 根因）
② 读 PLAN.md（索引，≤60 行）→ 再读 plans/<当前阶段>.md（**阶段索引与批次表**）的 In/Out of scope 与本卡所属批次
③ 读本次要做的任务卡 tasks/TASK-NNN-<slug>.md（**全文**：正文区 + 记录区 9 节骨架，§3.4）
④ 读任务卡引用的 spec / ADR（只读相关章节）+ 目标 crate 的 README（**不变量**一节必读）
⑤ 读 MEMORY.md §2（已确认事实）、§4（**已否决方案**）、§5（踩坑）
⑥ 读 LEDGER.md 最后 10 行（了解上一步到哪了）
⑦ 输出【约束回执】，等人类确认后才开始编码

**最小化读取原则**：以上七步总量应控制在 **≤ 800 行**。这正是 `PLAN.md` 拆分为「索引 + 按阶段文件」的原因——agent 不需要读后续阶段的细节。
```

### 4.2 约束回执（Constraint Echo）模板 ★

这是防止漂移**性价比最高**的一个机制：强制 agent 在动手前把约束复述一遍，人类只需扫一眼就能发现理解偏差。

```markdown
【任务】TASK-012 实现 notepad.read_text
【目标】从绑定的记事本文档读取全文，返回带 untrusted 标记的结构化结果
【write scope】仅：platform/windows/src/uia/text_reader.rs、tool-bus/src/builtin/notepad.rs、
              protocol/tool-schema/notepad.read_text.json、platform/windows/tests/notepad_read.rs
【铁律】不静默失败 / 模型输出不可信 / 所有 IO 带超时 / core 不碰平台 API / 错误用 ErrorCode
【禁止】改 schema 而不写 ADR；新增依赖；动其他 adapter；顺手重构；改测试断言让其通过
【验收】将运行：fmt --check、clippy -D warnings、cargo test notepad_read、verify-tool-schema、arch test
【依赖】TASK-008、TASK-011 已 Done（已核对 LEDGER）
【我的疑问】1 MB 截断的 max_bytes 默认值 spec 未写明 → 我先按 256 KB 实现并记 CLARIFY，可以吗？
```

**人类只需回复"确认"或纠正**。若 agent 的回执与任务卡不符，说明上下文已经污染，此时应**重开会话**而不是继续纠正（成本更低）。

### 4.3 漂移触发器（命中任一 → 立即停止编码 → 记录 → 升级）

| # | 触发器 | 处理 |
|---|---|---|
| 1 | 需要新增第三方依赖 | 起草 ADR-lite（登记进 `docs/DEPENDENCIES.md`：名称、用途、许可证、替代方案、体积），等批准 |
| 2 | 需要新增 crate / 顶层目录 / 模块 | ADR |
| 3 | 需要修改公共接口（trait、IPC 方法、Tool schema、DB schema、错误码） | ADR + spec 先行 |
| 4 | 需要改动已被 ADR 决定的事项 | 新 ADR（Supersedes） |
| 5 | 需要超出任务卡 write scope | 请求扩权或拆新卡 |
| 6 | 需要放宽 lint / 加 `#[allow]` / 加 `unsafe` | 请求批准，且必须写明原因与影响面 |
| 7 | 需要修改测试断言才能通过 | **默认视为缺陷**，除非能证明断言本身写错（需引用 spec） |
| 8 | 发现 spec 内部矛盾，或 spec 与代码矛盾 | 记 DRIFT，停止，等人裁决 |
| 9 | 想引入新的抽象层（"这样设计更好"） | 记入 PARKING_LOT，本卡不做 |
| 10 | 实际工作量超预估 2 倍 | 停止，重新拆卡 |
| 11 | 需要访问真实网络 / 真实凭据 / 真实商业应用（超出靶机范围） | 请求批准（涉及安全与合规） |
| 12 | 需要删除或重命名既有公共 API | ADR |

**DRIFT 记录格式**（写在任务卡的「执行记录」里，并同步一行到 LEDGER）：

```markdown
DRIFT-012-1
触发器：#8（spec 矛盾）
现象：spec/tool-schema.md 要求 postconditions 至少 1 条，但 spec/envelope.md 的示例中只读工具没有 postcondition
影响：无法确定 read 类工具是否必须有 postcondition
我的建议：只读工具允许 postconditions 为空，改为要求 state_unchanged 断言
已停止的工作：notepad.read_text 的 schema 文件
需要人类裁决：是 / 否（若否，请指定一方）
```

### 4.4 周期性重锚（Re-anchor）★

长会话中上下文会稀释。强制机制：

- **每完成一个子步骤**，agent 自问三题（写在思考里或简短输出）：
  1. 我现在做的事在任务卡 In scope 里吗？
  2. 我是否引入了任务卡未提及的文件 / 依赖 / 抽象？
  3. 我是否修改了任何公共接口？
  → 任一异常即触发 §4.3。
- **每 20~30 轮对话**，重新读一遍 `AGENTS.md` 与当前任务卡（成本低，收益高）。
- **单会话上限**：一个会话最多完成 **1~2 张任务卡**。做完就提交、更新 LEDGER、结束会话。
  > 理由：新会话从干净上下文 + 完整文档开始，比一个被稀释的长会话更可靠。**"重开会话"是本项目的一项正常操作，不是失败。**


---

## 5. 机器护栏（让漂移在 CI 暴露）

### 5.1 CI 门禁清单（全部必过，任一失败即阻塞合并）

| # | 检查 | 命令 | 挡住什么漂移 |
|---|---|---|---|
| 1 | 格式 | `cargo fmt --all --check` / `pnpm prettier --check .` | 风格分歧引发的无意义 diff |
| 2 | Lint（零警告） | `cargo clippy --all-targets -- -D warnings` / `pnpm eslint . --max-warnings=0` | unwrap、死代码、可疑模式 |
| 3 | 禁用项 lint | 见 5.2 | 静默失败、调试残留、生产路径 panic |
| 4 | 单元测试 | `cargo test --workspace` / `pnpm vitest run` | 逻辑回归 |
| 5 | **架构依赖方向测试** | `cargo test -p assistant-core arch::` | ★ 分层被破坏（core 反向依赖 platform 实现） |
| 6 | **Schema 校验** | `cargo run -p xtask -- verify-schemas` | Tool/Adapter/审计事件定义不合规 |
| 7 | **协议类型同步** | `cargo run -p xtask -- codegen --check` | Rust 与 TS 类型与 schema 漂移 |
| 8 | 依赖治理 | `cargo deny check` / `cargo audit` | 未登记依赖、许可证污染、已知漏洞、重复版本 |
| **8b** | **spike 依赖治理**（ADR-0024 D2） | `cargo deny --config deny.toml --manifest-path spikes/<name>/Cargo.toml check licenses sources`（逐个枚举） | ★ `spikes/` 被 workspace `exclude` → 主 deny 门禁（#8）覆盖不到它；这条挡住「spike 里图方便 `cargo add` 一个许可证不兼容或来源不明的包」。**只查 licenses + sources**（免联网、免 advisory-db）；不查 bans（spike 之间版本重复是正常的）、不查 advisories（spike 不在发布链路上）。无 spike manifest 时必须**显式打印「无」再 exit 0**（铁律 1），否则「一个都没查到」与「查了都通过」无法区分 |
| 9 | 覆盖率门槛 | `cargo llvm-cov --fail-under-lines 75`（core/policy/task-engine ≥ 85） | 无测试的"完成" |
| 10 | 构建（三平台矩阵） | `cargo build --release` on win/macos/linux runner | 平台特定编译错误 |
| 11 | 文档完整性 | `cargo doc --no-deps` + `#![warn(missing_docs)]` | 公共 API 无文档 |
| 12 | **仓库卫生** | `cargo run -p xtask -- hygiene` | ★ 见 5.4 |
| **12b** | **文档一致性**（ADR-0030） | `cargo run -p xtask -- memory-counts` + `cargo run -p xtask -- adr-index` | ★ 两处**派生事实**被手写进文档而必然漂移：① `MEMORY.md`「各文件当前规模」表的行数/条目数（8 条规则；已造成两次真实事故，第二次就发生在刚修完第一次之后）；② `docs/adr/README.md` 编号登记表 ↔ `docs/adr/NNNN-*.md` ↔ `decisions.md` 的三方一致性（11 条规则；0019 号双重占用事故的机器判据是 `adr/number-collision`）。两个子命令都**只读**，发现不一致时打印可直接粘贴的正确值让人改（ADR-0030 选项 1 已否决自动写回）；表结构解析不到必须**报错**而不是静默当作一致（`memory/scale-table-unparsable` / `adr/registry-section-missing`，铁律 1） |
| 13 | 回放基准 | `cargo run -p xtask -- replay --suite core` | 用录制的树快照跑逻辑回归（v2 §17.4），不依赖真机 |
| 14 | 提交规范 | commitlint / 钩子 | 无法追溯到任务卡 |
| 15 | **命名与注释规范** | `cargo run -p xtask -- check-comments` | ★ 缩写命名、公共 API 缺文档注释、`TODO`/`PITFALL` 无卡号、注释掉的代码 |
| 16 | **台账与记忆同步** | `cargo run -p xtask -- check-ledger` | 卡已完成但未追加 LEDGER/MEMORY 条目 |

> 第 **8b** 与 **12b** 项都用**子编号**而不占用新的一级行号，是为了让本表的 16 行主编号保持稳定 ——
> 主编号被 `tasks/TASK-001-repo-skeleton.md`、`xtask/src/deferred.rs`、`ci.yml` 头部注释
> 与 `plans/stage-1-pilots.md` 多处交叉引用，重编号会引发一轮无价值的口径漂移（PL-001 的教训）。
> 12b **不并入 #12（hygiene）** 的理由：hygiene 的规则总数已被 ADR-0025 钉死为 13（5.4 表格行数是 SSOT），
> 把这 19 条文档一致性规则塞进去会立刻作废那个口径并牵连 `deferred.rs` 的数量自洽测试；
> 且两者查的对象不同 —— hygiene 查**源码形态**，12b 查**文档之间的交叉一致性**（ADR-0030 D5）。

> 第 5、6、7、12、12b、13、15、16 项是本项目**特有**的护栏，也是防漂移最有效的一组——它们检查的是"结构"与"过程"，而不只是"功能"。

> **元门禁（ADR-0019，2026-09-17 生效）**：上表每一项**硬**门禁都必须配一条「负向验证」——
> 能证明它在该红的时候真的会红的机制（N1 单元负向用例 / N2 CI 显式失败步骤 / N3 canary 工作流）。
> 理由：护栏**静默失效**比没有护栏更危险（真实事故：`deny.toml` 的 `db-path` 写成数组 →
> cargo-deny 配置解析失败 → 整条门禁形同虚设，却没有任何红灯）。登记表与规程见
> `docs/adr/0019-hard-gate-negative-verification.md`，canary 在 `.github/workflows/gate-selftest.yml`。
> **把软门禁转硬的任务卡，必须同时提交该门禁的负向验证并在登记表补一行，否则不得转硬。**

### 5.2 禁用项（用 lint 表达，而不是靠自觉）

**Rust**（在各 crate 的 `lib.rs` 顶部声明，`tests/` 中按需 allow）：

```rust
#![deny(clippy::unwrap_used)]        // 生产路径禁止 unwrap
#![deny(clippy::expect_used)]        // 禁止 expect
#![deny(clippy::panic)]              // 禁止 panic!（用 Result）
#![deny(clippy::todo)]               // 禁止 todo!() 进主干
#![deny(clippy::unimplemented)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::print_stdout)]       // 用 tracing，不用 println!
#![deny(clippy::print_stderr)]
#![deny(clippy::indexing_slicing)]   // 用 get()，避免 panic
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs)]               // 公共 API 必须有文档
```

**TypeScript**（`.eslintrc` / `eslint.config.js`）：

```jsonc
{
  "rules": {
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/no-non-null-assertion": "error",
    "@typescript-eslint/strict-boolean-expressions": "error",
    "no-console": "error",                       // 用统一 logger
    "react-hooks/exhaustive-deps": "error",
    "import/no-restricted-paths": ["error", {    // UI 不得直接调用平台/业务逻辑
      "zones": [{ "target": "./src/components", "from": "./src/ipc/impl" }]
    }]
  }
}
```

`tsconfig.json`：`strict: true`、`noUncheckedIndexedAccess: true`、`exactOptionalPropertyTypes: true`、`noImplicitOverride: true`、`verbatimModuleSyntax: true`。

### 5.3 架构依赖方向测试（arch test）

用代码把 v2 §3.1 的分层固化下来（示例思路，实现方式可选 `cargo-modules` / 自研 xtask 扫描 `Cargo.toml` + `use` 语句）：

```text
允许：
  apps/*            → crates/*
  crates/core       → protocol, policy, task-engine, verify, undo, hitl, memory, audit, tool-bus, model-gateway, platform/api
  crates/platform/* → platform/api
禁止（测试失败）：
  crates/core       → crates/platform/windows|macos|linux   （只能依赖 platform/api 的 trait）
  crates/platform/* → crates/core
  crates/policy     → 任何执行层 crate（策略不得依赖执行）
  apps/desktop-ui   → 任何 crates/*（前端只能走 IPC，不链接 Rust 逻辑）
  任意 crate        → std::process::Command / 直接 FFI（除 platform/* 与 ipc/*）
```

> 这条测试是**防止 agent "图方便直接调 API"** 的最有效手段。AI agent 特别容易写出"能跑但破坏分层"的代码。

### 5.4 仓库卫生检查（`xtask hygiene`）

自研的小工具，检查以下**AI 协作项目特有的退化信号**（共 **13** 项；口径由 **ADR-0025** 统一，本表行数即唯一事实源，`xtask/src/deferred.rs` 的 `TOTAL_HYGIENE_RULE_COUNT` 由它派生）：

| 检查 | 阈值 |
|---|---|
| 单文件行数 | > 600 行警告，> 900 行失败（**CI 门禆；作者上限见 §6.2 「写作规范」 = 400/600**，ADR-0033） |
| 单函数行数 | > 80 行警告 |
| 函数参数个数 | > 6 个警告（应用结构体） |
| 圈复杂度 | > 15 警告 |
| `TODO` / `FIXME` / `HACK` 注释 | 必须带 `TASK-NNN` 或 `ADR-NNNN` 引用，否则失败 |
| 被注释掉的代码块（> 5 行连续注释代码） | 失败 |
| 重复代码（跨文件相似度） | 警告（agent 爱复制粘贴） |
| 新增顶层目录 | 必须在 ADR 白名单中，否则失败 |
| 新增依赖 | 必须已在 `docs/DEPENDENCIES.md` 登记，否则失败 |
| 空实现 / `Ok(())` 直接返回的 stub | 必须带 `// STUB: TASK-NNN` 标记 |
| 测试文件是否被跳过（`#[ignore]` / `.skip`） | 必须带原因与任务卡号 |
| **文件不得含 CRLF**（`hygiene/crlf-line-endings`，ADR-0025 D1） | **Error**：任何文本文件的字节里出现 `\r` 即失败。`.gitattributes` 只管入库形态、**管不住工作区**，而 `cargo fmt --check` 会在 Linux runner 上因此变红 |
| **必须以单个 `\n` 结尾**（`hygiene/missing-final-newline`，ADR-0025 D1） | 先 **Warning**，清扫完 15 个既有文件后升 **Error**。真实事故：末行无换行会让「以整行 + `\n` 为锚点」的编辑脚本断言失败，**而报错信息与真因毫无关系** |

### 5.5 提交与分支规范

```text
分支：task/TASK-012-notepad-read-text        一卡一分支
提交：Conventional Commits
      feat(platform-windows): add notepad text reader
      fix(policy): reject ambiguous target without disambiguation
      docs(adr): ADR-0011 prefer UXP over ExtendScript
      范围(scope) = crate 名或 docs/adr/spec
页脚：Task: TASK-012
      ADR: 0011            # 若涉及
      Drift: none          # 或 DRIFT-012-1
禁止：直接 push main；一个提交混多张卡；"wip"/"fix" 类无信息提交
合并：CI 全绿 + 人类审阅（或独立 review agent 审阅 + 人类抽查）
```

---

## 6. 代码规范

### 6.1 通用原则（跨语言）

1. **无静默失败**（v2 §1.4 第一红线）：任何操作要么返回可验证的成功，要么返回带 `ErrorCode` 的失败。**禁止吞异常、禁止返回默认值冒充结果、禁止 `let _ =` 丢弃 Result**。
2. **显式优于隐式**：超时、重试、取消、错误处理都写在代码里，不藏在默认值里。
3. **禁止 stringly-typed**：id、状态、错误、通道类型一律用 newtype / enum。
4. **所有 IO 带超时且可取消**：无超时的 await 视为缺陷。
5. **不可信输入必经校验**：模型输出、UI 输入、IPC 消息、工具返回、文件内容——四类都算不可信。
6. **平台调用只存在于 `crates/platform/*`**：core 只见 trait。
7. **纯逻辑与副作用分离**：状态机、策略、验证逻辑必须是纯函数（可单测、可回放），IO 在边界。
8. **注释解释 why，不解释 what**；公共 API 必须有文档注释（含错误语义与不变量）。
9. **不留死代码**：删除而不是注释掉。
10. **一次只做一件事**：一个函数一个职责，一张卡一个目标。

### 6.1.1 命名规范（要求「一眼可懂」）

> 决策 Q3 的配套要求：人类通过阅读逐步熟悉 Rust，因此**命名的自解释性优先于简洁性**。

| 规则 | 反例 | 正例 |
|---|---|---|
| 禁止缩写（除白名单） | `resolve_tgt_desc()`、`calc_fp()` | `resolve_target_descriptor()`、`compute_state_fingerprint()` |
| 禁止无信息名 | `data`、`info`、`temp`、`helper`、`manager2` | `resolved_window`、`audit_event`、`shadow_copy_path` |
| 禁止拼音与中英混杂 | `getChuangKou()` | `list_windows()` |
| 函数 = 动词 + 宾语 | `window()`、`policy()` | `acquire_target_lease()`、`evaluate_policy()` |
| 布尔量带助动词前缀 | `visible`、`dirty` | `is_visible`、`has_unsaved_changes` |
| 枚举变体说清语义 | `TaskStatus::Wait`、`Error::E1` | `TaskStatus::AwaitingApproval`、`TargetNotFound` |
| 集合复数、单个单数 | `task: Vec<Task>` | `tasks: Vec<Task>` |
| trait 名 = 能力，struct 名 = 名词 | `struct WindowProvider`（应为 trait） | `trait WindowProvider`、`struct ResolvedWindow` |
| 避免双重否定 | `is_not_disabled` | `is_enabled` |

**缩写白名单**（仅这些可用）：`id, url, uri, ui, os, db, ipc, mcp, uia, ax, atspi, cdp, dpi, ocr, ttl, http, json, sql, fs, vm, px, ms, us, ns`。

**受控词汇表（controlled vocabulary）** ★
同一概念**全项目只用一个词**。这在 AI 协作项目里尤其重要：不同会话的 agent 会各自选词，造成「看起来是两个东西其实是一个」的混乱。

| 用这个词 | 不要用 | 含义 |
|---|---|---|
| `Target` | Element / Object / Item / Control | 操作对象（身份 + 解析结果） |
| `Step` | Action / Operation / Stage | 任务计划中的一个执行单元 |
| `Tool` | Command / Function / API | 模型可见的单个能力 |
| `Skill` | Plugin / Extension / Module | 可分发单元 |
| `Adapter` | Driver / Connector / Integration | 某个应用的适配包 |
| `Lease` | Lock / Mutex / Reservation | 目标级排他租约 |
| `Anchor` | Snapshot / Backup / Checkpoint | 撤销锚点（内容快照/影子副本/undo 预算的统称） |
| `Fingerprint` | Hash / Digest / Signature | 状态指纹 |
| `Capability` | Feature / Support / Flag | 运行时探测出的能力 |
| `Evidence` | Artifact / Attachment / Proof | 树快照、截图等取证物 |
| `Egress` | Upload / Send / Network | 数据出域 |
| `Taint` | Dirty / Unsafe / Flagged | 污点（不可信内容标记） |

完整表与新增词流程见 `docs/spec/naming.md`（**新增受控词需 ADR**）。

### 6.1.2 注释规范（要求「密度偏高」，但始终以 why 为主）

| 位置 | 必须写什么 | 强制性 |
|---|---|---|
| 模块/文件头 | 职责、**边界（不做什么）**、不变量、典型用法、相关 spec 章节链接 | 必须 |
| 公共 API（`pub`） | 语义、参数、返回、**错误语义（何时返回哪个 `ErrorCode`）**、副作用、是否幂等、超时与取消行为、示例 | 必须（`missing_docs`） |
| 关键私有函数 | why + 非显然的 what | 必须 |
| 复杂算法 / 状态机 | 分步注释 + 一个具体输入输出示例 | 必须 |
| `unsafe` / FFI | `// SAFETY:` 说明为何安全（仅允许在 `crates/platform/*`） | 必须 |
| 应用/平台坑 | `// PITFALL(app=excel): COM 修改会清空 undo 栈，故此处强制快照` | 必须；`xtask` 会汇总进 `MEMORY.md` §5 与 App Map |
| 性能相关代码 | `// PERF:` 说明预算与实测依据 | 必须（涉及 v2 §7.1 预算处） |
| 临时方案 | `// TODO(TASK-0NN):` / `// STUB(TASK-0NN):` | **无卡号即 CI 失败** |

**密度目标**：公共 API 100% 有文档注释；每 20~40 行有效代码至少一条解释性注释；每个 crate README 的「不变量」≥ 3 条。

**禁止**：注释掉的代码（删除，用 git 找回）；无信息注释（`// 把 x 设为 1`）；与代码矛盾的过期注释（**改函数必须同步改注释**，Reviewer 清单第 9 项）；用注释弥补糟糕命名（若注释在解释名字，说明名字该改）。

**风格基准示例**：

```rust
//! # 目标租约（Target Lease）
//!
//! 保证「同一目标同一时刻只有一个写者」。这是防止 Agent 与用户、
//! 或两个任务同时操作同一文档导致内容互相覆盖的唯一机制。
//!
//! ## 边界（不做什么）
//! - 不做跨进程互斥（由 Core 单点仲裁，见 docs/spec/ipc-protocol.md §4）
//! - 不做权限判断（权限在 crates/policy）
//!
//! ## 不变量
//! 1. 同一 key 上同时最多存在一个 Exclusive 租约
//! 2. 租约必有 TTL，过期未续租自动释放
//! 3. 用户操作目标应用时 Agent 租约被强制释放（用户优先）
//! 4. DB 是唯一仲裁者；内存副本在崩溃后一律失效
//!
//! 相关：架构 v2 §8.8

/// 获取目标租约。
///
/// # 错误
/// - `LeaseConflict`：已有其他写者持有该目标的排他租约；调用方（模型）
///   会收到可读原因，可选择等待或改道。
/// - `LeaseUserPreempted`：获取过程中用户操作了目标应用。
///
/// # 超时与取消
/// `timeout` 默认 3 s；`cancel` 触发时立即返回 `UserCancelled`，
/// 且不会留下半获取状态（获取是原子的）。
///
/// # 幂等性
/// 对同一 `lease_key` 重复调用会续租而非新建（幂等）。
pub async fn acquire_target_lease(
    lease_key: &LeaseKey,
    mode: LeaseMode,
    timeout: Timeout,
    cancel: CancellationToken,
) -> Result<LeaseGuard, ErrorCode> {
    // PERF: 本函数在每步执行的热路径上，预算 < 1 ms（含 DB 仲裁）。
    //       内存快路径命中时不应产生任何 IO。
    todo_impl()
}
```

### 6.2 Rust 具体规范

| 项 | 规定 |
|---|---|
| Edition / MSRV | Rust edition 2024；MSRV 固定在 `Cargo.toml` `rust-version`，CI 校验 |
| 错误处理 | 库 crate 用 `thiserror` 定义**领域错误枚举**；二进制用 `anyhow` 只做顶层封装。**禁止 `Box<dyn Error>` 满天飞** |
| 错误码 | 所有对外错误必须携带 `ErrorCode`（v2 §8.7 分类学），定义在 `crates/protocol`，**新增码需改 spec** |
| 依赖注入 | 时钟、随机、UUID、文件系统、网络全部通过 trait 注入（便于回放测试与确定性） |
| 并发 | 明确标注 cancellation-safety；共享状态用 `Arc<RwLock<>>` 或 channel，**禁止裸 `Mutex` 跨 await 持有** |
| FFI / unsafe | 只允许在 `crates/platform/*`；每个 `unsafe` 块上方必须有 `// SAFETY:` 说明；PR 中 unsafe 变更必须人类审阅 |
| 模块规模 | 单文件 ≤ 400 行（软）/ 600 行（硬）；超出即拆（**写作规范，不是 CI 门禆**；CI 门禆见 §5.4 = 600/900，ADR-0033） |
| 命名 | 类型 `PascalCase`；函数/变量 `snake_case`；常量 `SCREAMING_SNAKE`；crate 名 `assistant-*` |
| 日志 | 只用 `tracing`；span 层级遵循 v2 §17.1；**禁止在日志中输出密钥、全文内容、截图**（用引用 id） |
| 序列化 | 所有跨进程/持久化结构都放 `crates/protocol` 并由 schema 生成或校验，**禁止各处手写重复结构体** |
| 测试 | 单元测试同文件 `#[cfg(test)]`；集成测试 `tests/`；命名 `test_<unit>_<condition>_<expected>`；**单元测试禁止真实 IO/网络** |

### 6.3 TypeScript 具体规范

| 项 | 规定 |
|---|---|
| 类型来源 | **全部由 `protocol/` schema 生成**，禁止手写重复类型；生成物纳入版本控制并在 CI 校验同步 |
| 组件 | 单文件 ≤ 200 行；容器组件与展示组件分离；hooks 独立文件 |
| 业务逻辑 | **不得写在组件里**：状态与流程放 store/hooks，组件只渲染 |
| IPC | 只通过 `src/ipc/client.ts` 的**类型化封装**调用；禁止散落 `invoke()` |
| 网络 | UI 层**禁止** `fetch` / `WebSocket`（一切出网走 Core） |
| 校验 | 运行时对 IPC 返回做 zod 校验（生成自 schema），防止 Rust 侧变更后前端静默出错 |
| 样式 | Tailwind；禁止行内魔法数字，间距/颜色用 design token |
| i18n | 所有文案走 i18n key，禁止硬编码中文/英文字符串 |
| 测试 | `vitest` + `testing-library`；审批卡片、时间线、拾取器必须有交互测试 |

### 6.4 测试规范

| 层 | 内容 | 是否进 CI |
|---|---|---|
| 单元 | 纯逻辑：状态机、策略、验证、selector 排序、指纹、undo 决策 | ✓ 必须，覆盖率门槛 |
| 契约 | schema 校验、协议类型同步、错误码完整性 | ✓ 必须 |
| 回放 | 用录制的 UI 树快照跑端到端逻辑（不依赖真机） | ✓ 必须 |
| 靶机 | 对 `fixtures/apps/*` 的真实平台交互 | ✓（Windows runner）；平台不可用时 skip 并记录 |
| 真机 | 对真实应用（Notepad/Excel/Paint/PS/Edge） | ✗ 手工验收 + 每阶段末跑一次，结果记入 LEDGER |
| 安全回归 | 注入靶页、污点追踪、L3 阻断 | ✓ 必须（v2 §21 S5、feasibility T5.3） |

原则：**能在回放层测的，不要放到真机层**（真机慢且不稳定）；**能用靶机测的，不要用商业应用测**。

### 6.5 文档规范（代码内）

每个 crate 必须有 `README.md`，含四节（模板见 §8.6）：

```markdown
# crate 名
## 职责（一句话 + 3~5 条）
## 边界（不做什么；依赖方向；谁是它的调用方）
## 不变量（Invariants：本模块保证的性质，改动前必读）
## 已知限制与技术债（带 TASK/ADR 引用）
```

> **不变量（Invariants）一节是防漂移的关键**：它把"这个模块为什么这么写"固化下来，下个会话的 agent 就不会"优化"掉它。

---

## 7. 验收分离与对齐审计

### 7.1 作者不自证

| 角色 | 职责 | 不得做 |
|---|---|---|
| 实现 agent | 按任务卡实现 + 自跑验收命令 + 填执行记录 | 不得判定自己"完成"；不得修改验收标准 |
| Review agent（独立会话/子 agent） | 对照任务卡与 spec 审阅 diff，输出问题清单 | 不得直接改代码 |
| 人类 | 裁决 DRIFT、批准 ADR、最终合并 | — |

Review agent 的固定检查清单：

```text
1. 是否只改了 write scope 内的文件？（逐文件核对）
2. 验收命令是否全部执行且通过？（核对粘贴的输出）
3. 是否引入了新依赖 / 新抽象 / 新公共接口？（有则必须有 ADR）
4. 是否存在静默失败路径？（吞错误、返回默认值、忽略 Result）
5. 是否所有 IO 带超时与取消？
6. 错误是否用 ErrorCode 且分类正确？
7. 是否有测试？测试是否真的验证了行为（而不是验证实现）？
8. 是否有 TODO/HACK 未带任务卡号？
9. README/spec 是否同步更新？
10. 有没有"顺手"改了不相关的东西？
```

### 7.2 阶段末对齐审计（Drift Audit）

每阶段结束执行一次，由**未参与该阶段实现的 agent** 完成：

```text
输入：PLAN.md（该阶段范围）、docs/spec/*、docs/adr/*、LEDGER.md、代码库
输出：docs/audits/STAGE-N-audit.md
  ① 范围符合度：In scope 是否都做了；Out of scope 是否有代码渗入（列文件）
  ② 契约一致性：代码与 spec 的偏差清单（逐条：位置、spec 条目、实际实现、严重度）
  ③ 分层健康度：arch test 结果、依赖图变化、crate 规模变化
  ④ 质量指标：覆盖率、lint 豁免数量与理由、TODO 数量、重复代码
  ⑤ ADR 完整性：是否有"代码已改但无 ADR"的决策（★ 最典型的漂移证据）
  ⑥ 文档完整性：README/spec/PLAN 是否与代码同步
  ⑦ 建议：需要回改的项 + 需要补的 ADR + 下阶段范围调整建议
人类裁决：接受 / 修正 / 记入 PARKING_LOT
```

---

## 8. 多 Agent 协作与工具差异

> **完整编排规则见 `docs/subagent-orchestration.md`**：角色矩阵、何时用/不用 subagent、分配算法、派单包模板、交接校验、失败升级、10 条反模式、本项目各阶段的并行度建议与人类审阅预算。本节只保留要点。

### 8.1 分工模式（建议）

```text
主 agent（Orchestrator）
  · 读 PLAN.md，拆任务卡，分配 write scope（保证互不重叠）
  · 不写实现代码（或只写很小的卡）
实现 agent × N（并行）
  · 每个领 1 张卡，独立会话，独立分支
  · 完成即提交 + 更新 LEDGER + 结束会话
Review agent
  · 独立会话审阅 diff（§7.1 清单）
Spike agent
  · 阶段 0 的技术验证，产出 SPIKE_REPORT.md（不改产品代码）
```

规则：
- **一张卡一个 agent 一个会话**；
- **write scope 不重叠**（重叠必然产生掩盖漂移的合并冲突）；
- 实现 agent **不得**修改 spec/ADR/PLAN（只能提案）；
- 并行数不宜过多（建议 ≤ 3），否则人类审阅成为瓶颈——**审阅速度决定项目速度**。

### 8.2 各工具的规范文件约定

| 工具 | 读取的文件 | 本项目做法 |
|---|---|---|
| Codex（CLI / 桌面） | `AGENTS.md`（根 + 嵌套）、`~/.codex/AGENTS.md` | **`AGENTS.md` 为唯一事实源**；在 `crates/platform/windows/AGENTS.md` 等放模块级补充 |
| opencode | `AGENTS.md` | 同上，天然兼容 |
| Claude Code | `CLAUDE.md`（支持 `@path` 导入） | 根目录放 `CLAUDE.md`，内容仅一行：`See @AGENTS.md`（避免两份规范分叉） |
| 其他（Cursor 等） | 各自规则文件 | 一律用一行指向 `AGENTS.md` |

**硬规则**：任何工具专属的规范文件**都不得包含实质内容**，只能是"指向 AGENTS.md"的转发，否则必然分叉。

### 8.3 嵌套 AGENTS.md 的使用

- 根 `AGENTS.md`：全局铁律、文档地图、验证命令（≤ 300 行）；
- `crates/platform/<os>/AGENTS.md`：平台特有约束（如 Windows 的 unsafe/COM 规则、Linux 的 a11y 激活流程）；
- `adapters/<app_id>/AGENTS.md`：该应用的已知坑与禁止事项（来自 App Map）；
- 嵌套文件**只写增量**，不重复根文件内容。

---

## 9. 模板附录

### 9.1 `PLAN.md` 模板

```markdown
# PLAN　（唯一进度事实源）
状态：Active　版本：N　更新日期：YYYY-MM-DD

## 当前阶段：阶段 1　Notepad + Excel 闭环
### 阶段目标（一句话）
### In scope（冻结）
### Out of scope（做了算漂移）
### 完成定义（DoD）
- [ ] …（可机器验证的条目）

## 任务队列
| 卡号 | 标题 | 状态 | 负责 | 依赖 | write scope | 预估 |
|---|---|---|---|---|---|---|
| TASK-008 | UIA Host 骨架 | Done | agent-a | 006 | crates/platform/windows/** | M |
| TASK-012 | notepad.read_text | Ready | — | 008,011 | 见卡 | S |

## 后续阶段（一句话概要，不展开）
- 阶段 2：Adapter 抽象 + Paint + Photoshop
- 阶段 3：Edge + 评测与安全回归
- …

## 变更历史
| 日期 | 变更 | ADR | 批准 |
|---|---|---|---|
| 2026-09-16 | 阶段 1 由 1 个应用改为 2 个 | ADR-0003 | 项目负责人 |
```

### 9.2 `LEDGER.md` 模板（只追加）

```markdown
| 日期 | 卡号 | 状态 | commit | 验收命令结果 | 偏差 | 备注 |
|---|---|---|---|---|---|---|
| 2026-09-20 | TASK-008 | Done | a1b2c3d | 5/5 通过 | none | — |
| 2026-09-21 | TASK-012 | Blocked | — | — | DRIFT-012-1 | spec 矛盾，待裁决 |
```

### 9.3 ADR 模板

```markdown
# ADR-0011　Photoshop 通道优先 UXP Scripting
状态：Accepted　日期：2026-09-16　Supersedes：—　Superseded by：—
关联：feasibility P4、spec/adapter.md

## 背景（为什么现在要决定）
## 决策（一句话）
## 考虑过的选项（至少 2 个，含被否理由）
## 影响（需要改的 spec / 代码 / 文档 / 任务卡）
## 风险与缓解
## 验证方式（怎么知道这个决策是对的；何时应重新评估）
```

### 9.4 PR 模板

```markdown
Task: TASK-012
## 变更摘要（3~5 行）
## write scope 核对
- [ ] 仅修改任务卡列出的文件（附文件清单）
## 验收
- [ ] 命令与输出（粘贴关键输出）
## 偏差
- [ ] 无 / 有（DRIFT-编号 + 处理结果）
## 文档同步
- [ ] README / spec / LEDGER / PARKING_LOT 已更新
## 审阅者关注点（作者主动指出风险处）
```

### 9.5 `docs/PARKING_LOT.md` 模板

```markdown
| 日期 | 提出者 | 事项 | 相关卡 | 处置（阶段末评审） |
|---|---|---|---|---|
| 2026-09-21 | agent-a | 大文件应降级到文件通道读取 | TASK-012 | 待评审 |
```

### 9.6 crate `README.md` 模板

```markdown
# assistant-policy
## 职责
策略引擎：白名单、参数校验、权限、污点追踪、租约检查、预算。**唯一放行点**。
## 边界
- 不执行任何动作（执行在 automation-host）
- 不依赖 platform/* 的具体实现（只依赖 platform/api 的 Capability 类型）
- 不做 UI 决策（审批交互在 hitl）
## 不变量（改动前必读）
1. 默认拒绝：未匹配任何规则 → deny
2. 决策必须是纯函数：相同 (请求, 策略, 上下文) → 相同结果，无副作用
3. 每个 deny 必须带可读 reason 与 rule_id
4. 污点标记只能由 SessionManager 清除，policy 不得自行清除
5. 高风险 + 污点 → 必须 deny，不可被任何授权覆盖
## 已知限制 / 技术债
- 规则 DSL 暂不支持时间窗口条件（TASK-045）
```

---

## 10. 落地检查清单（本项目开工前必须就位）

- [x] 根 `AGENTS.md`（实际 185 行 = `wc -l` 权威实测，含文档地图与验证命令；删旧手抄 ~164 行 = PL-035 现场修复）
- [x] `CLAUDE.md`（仅一行转发到 AGENTS.md）
- [x] `MEMORY.md`（§1 快照 + 首批 FACT/DECISION/REJECTED/PITFALL/OPEN 条目）
- [x] `PLAN.md`（索引 ≤60 行）+ `plans/stage-0-spikes.md` + `plans/stage-1-pilots.md`（含 In/Out scope 冻结）
- [x] `docs/subagent-orchestration.md`、`docs/storage-design.md`、`docs/wbs-overview.md`
- [ ] `docs/spec/naming.md`（受控词汇表 + 缩写白名单）
- [ ] `docs/OPEN_SOURCE_CHECKLIST.md`（开源前脱敏清单，见 v2 §18.1）
- [ ] `xtask` 的 `check-comments` 与 `check-ledger` 护栏子命令
- [ ] `LEDGER.md`（空表头）
- [ ] `docs/PARKING_LOT.md`（空表头）
- [ ] `docs/spec/`：tool-schema、envelope、error-codes、capability-matrix、audit-event、ipc-protocol、naming
- [ ] `docs/adr/0001~000N`：把 v2 与 feasibility 中的关键决策落成 ADR（语言选型、MCP-first、element 不跨进程、L1 优先、Excel 用快照不用 undo、Edge 专用 profile、股票交易 Non-goal 等）
- [ ] `docs/DEPENDENCIES.md`（空表头 + 登记规则）
- [ ] `tasks/` 目录 + TASK-001~0NN（阶段 1 全部任务卡，含 write scope 与验收命令）
- [ ] CI：§5.1 的 **18 行清单 ↔ 17 个步骤（8 硬 + 9 软）**（口径见 ADR-0025 D4 与 ADR-0030 D5；可分批上线，但第 1~5 项必须第一天就有）
- [ ] `xtask`：verify-schemas / codegen --check / hygiene / replay
- [ ] arch test（§5.3 依赖方向）
- [ ] `fixtures/apps/`：靶机应用 v0（至少一个"类记事本"应用，控件 id 稳定）
- [ ] 阶段 0 Spike 任务卡（A~G）与 `SPIKE_REPORT.md` 模板

> **建议顺序**：先把 `AGENTS.md` + `PLAN.md` + 3~5 张 Spike 任务卡做出来，跑完 Spike，再写 spec 与 ADR，最后才开阶段 1 的卡。
> **不要在没有 AGENTS.md 的情况下让 agent 写第一行产品代码。**
