# Codex 原生 scheduled tasks（automations）操作手册

> 状态：Active　版本：**1.6**　日期：**2026-09-25**　变更历史见 §9
> 上位：**ADR-0029**（夜间自动化的投递机制改回 Codex 官方 scheduled tasks，Supersedes ADR-0018 的「Windows 任务计划程序 + `codex exec`」方案）、`docs/overnight-automation-charter.md` §11
> **用途**：把「Codex 桌面应用的 scheduled tasks / automations 怎么建、怎么改、怎么删、怎么跑、怎么暂停、怎么停」写成可以照着做的操作手册，并且**逐条标明证据强度** —— 哪些是官方写明的、哪些是本机实测的、哪些其实谁都不知道。
> **适用范围**：本机（Windows + Codex 桌面应用 + 第三方 Responses 兼容端点）。**不覆盖** ChatGPT Work 云端任务；**不覆盖** `codex exec` CLI 路径（那是 ADR-0018 的旧方案，只在 §1 门禁不通过时作为回落手段出现）。
> **本手册不是授权**：读完它不等于可以创建 automation。动手前必须先过 §2 的门禁。

---

## 1. 证据分级图例与术语对齐

每一条操作性结论后面都带下列标签之一。**没有标签的句子 = 组织性说明或推论，不是事实断言。**

| 标签 | 含义 | 能不能当结论用 |
|---|---|---|
| `[官方]` | 官方文档页《Scheduled tasks》写明。本机副本：`C:\Users\fexfe\AppData\Local\Temp\cx_automations.md`（14671 字节，2026-09-18 抓取；文档索引见 `https://learn.chatgpt.com/llms.txt`，页面 URL 后加 `.md` 即得 Markdown 版） | 能。但副本在临时目录、会随清理丢失 → 复核时重新抓 |
| `[实测·探针]` | 本机**真的下发过**工具调用或命令，并观察到返回（2026-09-17 与 2026-09-18 两轮） | 能 |
| `[实测·声明]` | 本机 `automation_update` 工具的自述文本、或 CLI `--help` 的自述；**未实际下发验证** | 能当线索，**不能当已验证行为**；首次真跑时必须复核 |
| `[未验证]` | 官方未文档化 + 本机未验证（含「从字段名推测」） | **不能**。写出来只为登记缺口，不得据此行动 |

**术语对齐**（三套词指同一件事，别混用）：官方文档叫 **scheduled task**；Codex 桌面应用的工具与磁盘目录叫 **automation**（`automation_update`、`~/.codex/automations/`）；本项目章程 §11 沿用旧词 **heartbeat** 指「挂回既有 thread 的那一类」。本手册统一用 **automation**，必要时标注它对应官方的哪一类。

> 一个必须先说清的命名事实：官方页通篇用 "scheduled tasks"，全文**没有出现** `cron`、`heartbeat`、`notification`、`delete`、"run now" 这些词（2026-09-18 对副本做过全文检索）`[实测·探针]`。`cron` / `heartbeat` 是**本机工具侧**的判别式取值。所以「官方怎么描述 cron」这类问题没有答案 —— 官方只区分 **standalone** 与 **in-a-chat** 两类。

> **本手册 `[实测·探针]` 的覆盖面（诚实声明）**：只覆盖四件事 —— 「schema 探明（判别式取值与必填字段）」、「`mode = "view"` 的返回」、「`~/.codex/automations/` 的磁盘现状」、「`codex --help` 的子命令列表」。**不覆盖任何「创建成功之后的运行行为」**，那部分全部是 §8 的待验项。这是 ADR-0029 D2「本轮只调研落档、不做端到端调试」的直接后果。

---

## 2. ✅ 启用前置门禁（GATE-0）：**已通过（2026-09-24）** —— 先证明端点吞得下 automation 的投递方式

**一句话：在「端点兼容性实测通过」之前，禁止创建任何真实 automation —— cron 与 heartbeat 都禁。**

> **状态（2026-09-24）**：✅ **GATE-0 已通过**（执行人 = 本项目 agent，人类已授权本次调试时段）。
> GATE-0.1 ~ GATE-0.4 四条全部实测成立；结论、证据与「哪些断言**没有**测」见 **§8.2 基线表 2026-09-24 行**与 §8.3。
> 机制**已可用**，且**当前 automation 数 = 2**（均为**一次性**、2026-09-25 按人类指令建立；更早建的两个**常驻** automation 已按人类指示**全部删除**）
> （磁盘复核：本机回到只剩 `.run-jitter-salt` 的基线）。人类同日给出的**正确形态**（**ADR-0046**）= **人类指令驱动的「一次性」任务**：
> 建立需人类指令；**不同自动化之间间隔 ≥ 2.5 小时（默认 3 小时）**；**任务内容按实际进度现场决定**（prompt 不写死卡号）；
> **任何时间**均可提出。→ ⚠ 「一次性任务」的建法已写进 §4.1 节末子块（`#### 4.1.a`），但**具体写法**仍为 `[未验证]`，见该子块与 §9 v1.2 ~ v1.5。
> 下文的四条「为什么」是 2026-09-17 的原始实测记录（**只追加原则，保留不改**）；
> 其中 E1 的投递形态已在 2026-09-24 改变 —— 见**下文四条之后的更新块**。

为什么（ADR-0018 的实测结论，在被新实测推翻之前仍然成立）：

- automation **不把 prompt 当 user message 投递**，而是注入一条合成的工具执行结果
  `FunctionCallOutput { id: None, call_id: None, name: "automation_update", namespace: "codex_app" }` `[实测·探针]`（2026-09-17）
- 本机使用第三方 Responses 兼容端点（阿里云百炼 compatible-mode，`model = qwen3.8-max`；连接串与令牌在本机 `~/.codex/config.toml` 的 `[model_providers.custom]`，**本手册不复制其中任何值**），它对请求体做严格校验 → **400 拒绝** `[实测·探针]`
- 首轮投递时 Codex 还会临时生成非法 id `at_<uuid>`（该端点要求 `msg_` 前缀），而这个 id **从不落盘** → 「等长补丁修 rollout」的办法无效 `[实测·探针]`
- **heartbeat 的额外毒性**：毒项沉入长驻 thread，加上 `disable_response_storage = true` 每轮重放全量历史 → 该 thread **此后每次提问都失败**，等于永久报废（新开 thread 正常）`[实测·探针]`。ADR-0018 已经为此付过一个 thread 的代价。

> **2026-09-24 更新（GATE-0 实测，推翻 E1 的形态）**：automation 现在把 prompt 当作**正常 user message** 投递
> （`role = "user"`、`msg_…` 前缀的合法 id，正文前缀为 `Automation: <name>` / `Automation ID:` /
> `Automation memory:` / `Last run:`），**不再注入 `call_id: None` 的合成 `function_call_output`** `[实测·探针]`。
> 后果：① 端点 400 的根因**消失**；② 「毒项沉入长驻会话」的失效模式**未复现**（heartbeat 探针跑完后同一 thread 仍能正常回答）。
> **上面的旧条目按只追加原则原样保留** —— 它们仍是回退方案（ADR-0018）的历史依据，也是「为什么当初要设这道门禁」的记录。
> 本机端点与模型同期也换了（阿里云百炼 `qwen3.8-max` → `opencode_go` / `deepseek-v4.1-flash`），见 §8.2。

门禁规则：

| # | 规则 | 理由 |
|---|---|---|
| GATE-0.1 | 端点兼容性未实测通过前，不创建任何真实 automation | 上面四条 |
| GATE-0.2 | 兼容性验证**只允许挂在专门新建的废弃 thread 上**，该 thread 标题**必须以 `nightly-probe-` 开头**（挂 automation 前人工核对前缀；**不要**用 `mode = "view"` 判断，理由见 §4.8），且**两种形态都要验**：先用一次性 heartbeat 做最小验证，再用一个 standalone（cron）探针跑完整链路 | heartbeat 是最小可验证单位，但**生产形态是 standalone / cron**（ADR-0029 D1：heartbeat 不作为夜间主方案）。ADR-0018 记录两者的失败症状并不同（heartbeat 缺 `call_id`；cron 首轮生成 `at_<uuid>`）→ 只验一种会漏判 |
| GATE-0.3 | **绝不在主线工作 thread 上验证** | 主线 thread 一旦中毒，白天的交互一并废掉 —— 这是四个候选方案里唯一会污染主线工作的失效模式 |
| GATE-0.4 | 验证跑完必须做 §8 的 D8 清理：删掉探针 automation、处置 probe thread、确认 `~/.codex/automations/` 回到只剩 `.run-jitter-salt` | 不留半状态（AGENTS.md 铁律 1：无静默失败，也不留静默残留） |
| GATE-0.5 | 验证结论（通过/不通过 + rollout 证据）回填本手册 §8.2 基线表与 ADR-0029 的「生效前提」 | 否则下个会话的 agent 会重蹈覆辙（铁律 10：契约先行） |
| GATE-0.6 | **一主一备**：同一时刻只允许一种夜间机制处于 Active。启用回退方案（ADR-0018 的 CLI + 任务计划程序）之前，必须先暂停或删除 automation | 两套调度同时跑会抢 `.nightly.lock`、产生无法归因的混合改动（章程 §11.2 的理由）。ADR-0029 D4 把 ADR-0018 定为**回退方案**，不是并行方案 |

**判定「兼容性通过」的硬标准（三条全中才算过）**：

1. 触发后目标 thread 出现**真实的模型回复**（不是 2.6 秒零产出失败）；
2. 触发后在同一 thread 再手工发一条消息，**仍能正常回答**（证明没中毒）；
3. 该会话 rollout 里的合成项要么带合法 `call_id`，要么根本不出现 `at_` 前缀 id。

**2026-09-24 实测结果：三条全中**（cron 2 个 + heartbeat 5 个 `function_call_output` **全部**带合法 `call_id`，
`at_` 前缀 id **0 个**，无 400）→ **GATE-0 通过**。

**不通过怎么办**：回落到 ADR-0018 的路径（Windows 任务计划程序 + `codex exec`，验收清单见 `docs/nightly/scheduler-acceptance-test.md`），或直接暂停夜间自动化、回到纯白天推进。ADR-0029 D4 已把 ADR-0018 定为「Superseded 但**保留**」的回退方案，那份清单不删除；启用它之前先按 GATE-0.6 停掉 automation。**不要试图改 rollout 来「修毒项」** —— 首轮投递的 `at_` id 从不落盘，无从修补 `[实测·探针]`。

---

## 3. 概念与两种任务类型对照

| 维度 | **standalone**（本机 `kind = "cron"`） | **in-chat**（本机 `kind = "heartbeat"`） |
|---|---|---|
| 每次运行是否新开 chat | **是**，每次 run 开一个全新 chat，结果报进 **Scheduled** 视图 `[官方]` | **否**，回到既有 chat `[官方]` |
| 是否复用上下文 | 不复用（每次从 prompt 重新开始）`[官方]` | **复用该 chat 的既有上下文** `[官方]` |
| 周期粒度 | 自定义节奏用 custom schedule 控件；高级排期改 RFC 5545 RRULE，官方例：`RRULE:FREQ=MONTHLY;BYMONTHDAY=1;BYHOUR=9;BYMINUTE=0` `[官方]` | **可用分钟级间隔**做主动跟进循环，也可日/周排期 `[官方]` |
| 作用域 | 项目级；**同一个任务可以覆盖多个项目** `[官方]` | thread 级（挂在当前本地 thread 上）`[实测·声明]` |
| 运行位置（Git 仓库） | 二选一：跑在 **local project**（会改你正在编辑的文件，官方明确警告）或 **专用 background worktree**（隔离改动）；两者都在后台跑。非版本控制项目直接跑在项目目录 `[官方]` | 跟随所挂 thread 的 checkout（本机未见独立选项）`[未验证]` |
| 适用场景 | 每次运行应当独立、或希望每个 run 在 **Scheduled** 里单独成条 `[官方]` | 盯长任务直到结束、按固定节奏看某个连接源、按节奏推进 review loop、跑用插件的技能流程、延续长期 research/triage chat 而不丢上下文 `[官方]` |
| 本项目风险 | worktree 会堆积（官方要求归档不再需要的 run）`[官方]`；报告可能落在 worktree 里而主 checkout 看不到（§6 第 7 行） | **毒项永久损坏所挂 thread** `[实测·探针]` → 主线 thread 永久禁挂（GATE-0.3） |
| prompt 写法要求 | 每次都要自足（没有历史可依赖） | 官方要求「耐久」：写清每次运行做什么、怎么判断有没有值得报告的事、什么时候该停下或问人 `[官方]` |

**共同点（两类都适用）**：需要本地文件时，机器必须开机且桌面应用在运行，且所选项目在触发时刻仍在磁盘上 `[官方]`；无人值守运行，使用你的**默认沙箱设置** `[官方]`；组织策略允许时用 `approval_policy = "never"`，若管理端 `requirements.toml` 禁止，则回落到所选权限模式的批准行为 `[官方]`；可以用插件与技能，prompt 里写 `$skill-name` 可显式触发某个技能 `[官方]`；模型与 reasoning effort 可留默认也可显式指定 `[官方]`。

---

## 4. 操作逐条手册

### 4.0 三条通用规则（先读，能省掉大部分踩坑）

1. **优先用自然语言请 Codex 建/改，不要手拼字段。** 官方就是这么设计的：在 chat 里描述「做什么、什么时候跑、每次是回到当前 chat 还是新开 chat」，Codex 会起草 prompt、选目的地，并在范围或节奏变化时更新它 `[官方]`。本机理由更硬：`automation_update` 对外声明的 schema 是**空的**（`properties: {}`）`[实测·探针]`，手拼字段极易撞上只回一句 `Failed to create automation.` 且不指明字段的报错 `[实测·探针]`。
2. **不要手写 raw automation 指令，也不要把 raw RRULE 当交付甩给用户。** 工具声明明确禁止这两件事 `[实测·声明]`。本手册里出现的 RRULE 字符串只是**文档说明**；实际创建一律走 `automation_update` 或应用内 UI。
3. **首次实验优先用 `suggested_create` 而不是 `create`。** 工具声明列出了这个 mode 但没解释语义 `[实测·声明]`；从命名推测是「先渲染一张待人类确认的卡片」`[未验证]`。若推测成立，字段写错也不会真建出 automation，正好配合 GATE-0。**用之前必须先按 §8 的 D3 验证它的语义。**

### 4.1 建立（standalone / `kind = "cron"`）—— **本项目只允许「一次性」形态**（见节末 `4.1.a` 子块）

**走 UI / 自然语言** `[官方]`：在 chat 里描述工作与周期让 Codex 创建；或用 custom schedule 控件；高级排期直接编辑 RRULE。Git 仓库里要选 local project 还是 background worktree（取舍见 §3 表）。

**走 `automation_update` 工具** `[实测]`：`mode = "create"` + `kind = "cron"`，并且**八个字段一个都不能少**（缺任一 → 只回 `Failed to create automation.`，不指明是哪个）：

| 字段 | 说明与本项目取值建议 | 证据 |
|---|---|---|
| `name` | 人类可读、可 grep 的名字，例：`ai-assistant-nightly-2330` | 必填 `[实测·探针]`（2026-09-17 探明，记录于 `docs/memory/pitfalls.md`） |
| `prompt` | 每次运行重放的指令。**用户可见**，写成连贯散文；**通知偏好不要写进来**（用 `notificationPolicy`） | 必填 `[实测·探针]`；写法要求 `[实测·声明]` |
| `rrule` | RFC 5545 递推规则。本项目「每夜 23:30 与 02:30」怎么写见 §6 第 12 行 | 字段必填 `[实测·探针]`；RRULE 语法 `[官方]` |
| `status` | **取值 = `ACTIVE` / `PAUSED`**（2026-09-24 实测） | 字段必填 `[实测·探针]`；取值 `[实测·探针]`（2026-09-24） |
| `projectId` | 用 `list_projects` 查本项目的 id；顺带确认 `isGitRepository`。⚠ **必须用 legacy project id** —— 本机 `ai-assistant` = `5b628eec-9ca4-4768-8891-c7744a9dab5e`；app-server id（`01a0a80a-d1e7-7551-9d27-6518a58b78ed`）会被拒且只回 `Failed to create automation.` | 必填 `[实测·探针]`；查法 `[实测·声明]`；legacy 口径 `[实测·探针]`（2026-09-24） |
| `model` | **不要留默认**。本机走第三方端点，默认值可能是官方模型名 → 端点不认识。填本机 `~/.codex/config.toml` 里的 `model` 值 | 必填 `[实测·探针]`；「可留默认」是官方说法 `[官方]`；「本机必须显式填」是推论 |
| `reasoningEffort` | 显式给，避免默认值随版本漂移 | 必填 `[实测·探针]` |
| `executionEnvironment` | `"local"`（对应官方的「跑在本地项目或 worktree」）。**本机实测只接受 `"local"`** —— 经工具选 worktree 的写法未找到 | 必填 `[实测·探针]`；取值 `[实测·探针]`（2026-09-24） |

**仍然是 `[未验证]` 的字段**（不要猜着填）：`notificationPolicy` 的确切取值集、时区、单次运行时长上限、并发实例策略、失败重试策略。
**2026-09-24 已探明**（从上面这份「未验证」清单里移出）：`status` = `ACTIVE` / `PAUSED`；
`executionEnvironment` 只接受 `"local"`；`projectId` 必须用 **legacy id**。`destination` **不是**必填（2026-09-17 记的「八字段」口径成立）。

> **探 schema 的办法**（下次补字段时用）：故意只传 `kind`，让校验器把缺失字段一次性全列出来 `[实测·探针]`。这比逐个猜快得多 —— 2026-09-17 那次就是因为不知道这招，把「缺字段」误判成「cron 需要 ChatGPT 鉴权、本环境不可用」，白烧一晚。

**创建后自检**（不要依赖 `mode = "view"`，理由见 §4.8）：

```powershell
Get-ChildItem -Force -Recurse "$env:USERPROFILE\.codex\automations"
```

期望出现 `<id>\automation.toml`（本轮基线：该目录下只有 `.run-jitter-salt`，见 §5）。

#### 4.1.a 本项目只允许的建立形态：人类指令驱动的「一次性」任务（做法 `[未验证]`）

> **依据 = ADR-0046**（人类 2026-09-25 完整规格）：本项目建的 automation **必须是一次性的**，
> 且**建立前必须有人类指令**（agent 不得自行建立，含探针）。

**已知 / 未验证**：

| 项 | 状态 |
|---|---|
| 「建立需人类指令」 | ✅ 规则已定（ADR-0046 A1）；本机习惯 = 由人类在 chat 里给指令，agent 调 `automation_update` |
| 「一次性」的**具体写法** | ⚠ **`[部分验证]`（2026-09-25 实测落盘）** —— 采用 **RRULE 加 `COUNT=1`**（实测 `rrule = "RRULE:FREQ=DAILY;COUNT=1;BYHOUR=13;BYMINUTE=30"`，落盘于 `~/.codex/automations/ai-assistant-one-shot-1330/automation.toml`），**工具接受该写法、创建返回成功**；**仍未验证的部分** = 「**第二次触发不发生**」（待 2026-09-26 13:30 / 16:30 观察）→ **PL-081**。附带实测：用 **`BYHOUR` / `BYMINUTE`** 指定时刻可用；`COUNT=1` 与 `FREQ=DAILY` 可共存、无校验报错 |
| 「不同自动化之间间隔 ≥ 2.5 h（默认 3 h）」 | ✅ 规则已定；**由人类给时刻时核对**，agent 只负责发现冲突并提示 |
| 验证判据 | **必须实测**：建立后核对磁盘 `automation.toml` 的 `rrule`，并观察**第二次触发不发生**；结论回填本表（ADR-0046 D2 / 验证方式 1） |

**建完自检（与本节上文同）**：

```powershell
Get-ChildItem -Force -Recurse "$env:USERPROFILE\.codex\automations"
```

期望出现 `<id>\automation.toml`；**并额外把该文件的 `rrule` 抄进当轮报告**，作为「一次性」的第一手证据。

> **2026-09-25 实测（本轮建立的两个一次性任务）**：`automation_update` 两次均返回 `{"automationId": "...", "mode": "create", "status": "ACTIVE"}`；
> 磁盘上出现 `~/.codex/automations/ai-assistant-one-shot-1330/automation.toml`（3837 字节）与 `...-1630/automation.toml`，
> `rrule` 逐字为 `RRULE:FREQ=DAILY;COUNT=1;BYHOUR=13;BYMINUTE=30` / `...;BYHOUR=16;BYMINUTE=30`；
> 落盘字段另有 `kind = "cron"` / `status = "ACTIVE"` / `execution_environment = "local"` / `model` / `reasoning_effort` / `notification_policy` /
> `target = { type = "project", project_id = "..." }` / `cwds = ["D:\\csart\\ai-assistant"]` —— **cron 不需要 `destination`**（与 facts.md 的必填口径一致）。
> **诚实边界**：「**第二次触发不发生**」还没观察到（要等 2026-09-26 的同一时刻）→ 结论只到 `[部分验证]`，见 **PL-081**。

### 4.2 建立（in-chat / `kind = "heartbeat"`）

**走 UI / 自然语言** `[官方]`：在既有 chat 里说明「按什么节奏回到这个 chat 做什么」。官方对 in-chat 的 prompt 有额外要求：必须**耐久** —— 写清每次运行做什么、怎么判断有没有值得报告的事、什么时候该停下或问人 `[官方]`。这条与章程「仅失败时通知」直接相关（§6 第 6 行）。

**走工具** `[实测·声明]`：`mode = "create"` + `kind = "heartbeat"`。heartbeat 是「挂在**当前本地 thread** 上的主动跟进」，也是循环性请求的默认形态；工具声明还要求：除非用户明确要「每次一个新任务」，否则不要拿 cron 去变通实现 thread heartbeat。

**具体字段名（2026-09-24 已探明）** `[实测·探针]`：`mode = "create"` + `kind = "heartbeat"` + **`targetThreadId`**（挂载目标 thread）+ **`rrule`**（节奏）；
落盘到 `automation.toml` 时字段名是 **`target_thread_id`**；**其余字段可省**（这与 cron 的「八个必填」相反，是本手册里最容易踩的差异）。
另有两条已确知：`kind` 的判别式取值是 `cron` / `heartbeat` `[实测·探针]`；**一个 thread 只允许一个 heartbeat** `[实测·探针]`
（2026-09-16 记录于 `docs/memory/facts.md`；注意同一条里「cron 创建失败」的记载已被 2026-09-17 的探针推翻，别一起采信）。

⚠ **本项目额外禁令**：① **GATE-0 的验证**绝不许挂在主线工作 thread 上（GATE-0.3，与门禁是否已过无关）；
② 夜间主方案**永久不用** heartbeat（章程 §11.1）—— 理由是**上下文累积漂移**，与 2026-09-24 已推翻的「中毒」不是同一件事，
所以「GATE-0 已通过」**不等于**「可以在主线 thread 上挂 heartbeat」。

**本项目定位（ADR-0029 D1）**：夜间主方案是 **standalone（cron）**；heartbeat **不作为夜间主方案** —— 它复用既有 chat 的上下文，一旦端点问题复现就是「毒项沉入长驻会话」的完整重演。heartbeat 在本项目只有一个合法用途：**GATE-0 的最小探针**（§2、§8 的 D1）。

### 4.3 修改

| 路径 | 做法 | 证据 |
|---|---|---|
| UI / 自然语言 | 在 chat 里说「把这个任务的节奏/范围改成 X」，Codex 会更新它；官方明写它能在 scope 或 cadence 变化时更新任务。技能也能创建或更新 scheduled task | `[官方]` |
| 工具 | `mode = "update"`（或 `suggested_update`），带 **resolved id + 全部更新后字段**；未要求改的字段要**原样保留** | `[实测·声明]` |
| 通知策略 | 「别通知我 / 静音」→ `notificationPolicy = failed_runs_only`；「取消静音」→ `notificationPolicy = null`。**不要写进 prompt** | `[实测·声明]`；官方页无 notification 一词 `[官方未写]` |

**`update` 是全量替换语义**：漏字段就可能把既有配置抹掉，或换来一句不指字段名的失败。所以改之前先抄全量字段 —— 读磁盘 `automation.toml`（§5）。`mode = "view"` **不返回字段值**给 agent `[实测·探针]`，指望不上。`suggested_update` 的语义同样 `[未验证]`。

**本项目纪律**：一次只改一个字段，改完立刻按 §4.1 的自检确认落盘，并在 LEDGER 追加一行（改排期属于「影响夜间行为」的变更，必须可归因）。

### 4.4 删除

- **工具** `[实测·探针]`：`mode = "delete"` + `id`（string，必填）。这是少数几个**报错会说清缺什么**的地方 —— 缺 `id` 时校验器明确回「`id` 必填（string）」。
- **UI**：官方页**没有**给逐步点击路径，全文甚至没有出现 "delete" 一词 `[官方未写]` → 具体步骤 `[未验证]`。**Scheduled** 视图里可编辑、暂停、归档（见 §4.7）；「删除」是否等同「归档」，未知。
- **删除后自检**：`Get-ChildItem -Force -Recurse "$env:USERPROFILE\.codex\automations"` 应回到只剩 `.run-jitter-salt`；被删任务关联的 worktree / 分支是否一并清理 `[未验证]`（官方只说 worktree 会堆积、要归档 run，没说删除任务会回收 worktree）→ 删完顺手 `git worktree list` 核对。
- **2026-09-24 实测** `[实测·探针]`：删除后目录**确实**回到只剩 `.run-jitter-salt`，`git worktree list` 无新增；且 **automation 自己（心跳 run）也能调 `mode = "delete"` 把自己删掉**（GATE-0.4 的清理就是这样完成的）。

### 4.5 立即运行（run now）

**结论：没有这个能力。** 两条都查过：

- 官方页全文没有 "run now" / "immediate" 这类手动触发说明 `[官方未写]`；
- `automation_update` 的 `mode` 判别式只有 `view` / `create` / `suggested_create` / `update` / `suggested_update` / `delete`，**没有 run 或 trigger** `[实测·探针]`。

可用的替代（按推荐顺序）：

1. **在普通 chat 里手工把同一个 prompt 跑一遍** `[官方]`。官方本来就要求排期前先这么测：确认 prompt 清晰且范围正确、模型/reasoning/工具行为符合预期、产出可审。这也是本项目**唯一被批准**的「立即运行」。
2. **建一个近时刻触发的排期**当一次性任务 —— 但「不带 RRULE 的单次任务」是否被支持 `[未验证]`（RFC 5545 里单次事件靠 DTSTART 而不是 RRULE）；而且它有约 +2 分钟抖动（§5），本来就不适合「立刻」。
3. **只是想立刻开个同样的活儿**：用 Codex 的 thread 工具新建 thread 或往既有 thread 发消息。那是普通会话，**不是 automation run，不进 Scheduled 视图**，也不受本手册 §2 门禁约束（因为它不走 automation 的投递路径）。

### 4.6 暂停

- **UI** `[官方]`：侧边栏 **Scheduled** 视图有 All / Active / Paused 三个分组（官方页附有该视图的示意图说明），说明暂停控件存在；但官方**没有写逐步点击路径** → 具体步骤 `[未验证]`。
- **工具**：没有 `pause` 模式 `[实测·探针]`。走 `mode = "update"` 改 `status` 字段（`status` 是 cron 必填项 `[实测·探针]`），**取值 = `ACTIVE` / `PAUSED`** `[实测·探针]`（2026-09-24）。
  ⚠ 「暂停 / 恢复」这条路径本身**仍未端到端实测**（§8 的 D7 没跑）→ 正式排期启用前补测。
- **本项目约定**：夜间 automation 一旦连续两夜 0 产出（章程 §11.5 的上报条件），**先暂停再排查**，不要边跑边改 —— 边跑边改会让你分不清「失败是旧配置造成的还是新配置造成的」。

### 4.7 恢复 / 停止 / 归档

「停止」这个词在本项目里必须拆成三件不同的事，别混：

| 动作 | 语义 | 做法 | 证据 |
|---|---|---|---|
| **暂停 → 恢复** | 任务还在，只是不触发；恢复后继续按原排期跑 | 恢复的官方步骤未写；推测是 §4.6 的反向操作 | `[未验证]` |
| **归档** | 把 run 或 thread 收起来。官方明确要求：worktree 模式下频繁排期会堆积大量 worktree，**不再需要的 run 要归档**，且**除非打算保留它的 worktree，否则不要 pin run** | 归档 run：在 **Scheduled** 视图操作（步骤未写）。归档 thread：用 Codex 的 `set_thread_archived` 工具，**不要**在 automation prompt 里写「跑完把自己归档」这类指令 | 归档 run 的必要性 `[官方]`；视图步骤 `[未验证]`；`set_thread_archived` `[实测·声明]` |
| **删除** | 彻底没了 | §4.4 | `[实测·探针]` |

### 4.8 查看（以及为什么 agent 不能靠它自检）

- **工具** `[实测·探针]`：`mode = "view"` + `id` → 返回 `Rendered automation card in the app.`。三个要点：① 它是**按 id 查单个**，不是列表；② 给一个**不存在的 id 会渲染一张空卡而不报错**；③ 它只渲染 UI，**不把字段值返回给 agent**。
- 第 ② 点直接违反本项目铁律 1「无静默失败」的精神。它是 Codex 侧行为、我们改不了，只能防：**任何「automation 是否存在」的判断都不许用 view 的结果**，一律看磁盘（§5）。
- **找 id**：读 `$CODEX_HOME/automations/*/automation.toml`，按 `name` 或 `prompt` 匹配 `[实测·声明]`。
- **UI** `[官方]`：侧边栏 **Scheduled** 视图就是收件箱 —— 有发现的 run 会出现在那里，需要人注意时有未读标记；可查看 active / paused / completed 任务与 recent runs。

---

## 5. 磁盘形态（能看，但**不许手改**）

| 路径 | 内容 | 证据 |
|---|---|---|
| `~/.codex/automations/` | automation 的落盘根目录（即 `$CODEX_HOME/automations`） | 路径来自工具声明 `[实测·声明]`；目录存在 `[实测·探针]`（本轮） |
| `~/.codex/automations/<id>/automation.toml` | 单个 automation 的定义文件。找 id、以及 §4.3 要求的「改前抄全量字段」都读它 | 路径 `[实测·声明]`；**内部 schema 已可观察** `[实测·探针]`（2026-09-24）—— 含 `target = { type = "project", project_id = "<legacy id>" }`、heartbeat 的 `target_thread_id` 等；字段名与取值见 §8.2 |
| `~/.codex/automations/.run-jitter-salt` | 一个 UUID（本机 37 字节），用于给触发时刻加随机抖动 | `[实测·探针]`（本轮 `Get-ChildItem -Force` 确认它是该目录下**唯一**条目） |
| `~/.codex/automations/<id>/memory.md` | automation 的**跨轮记忆**文件（自动创建，agent 每轮可读写）。与仓库内的 `docs/memory/*` 是两回事，**不得**拿它承载项目结论 | `[实测·探针]`（2026-09-24） |

**触发抖动 ≈ +2 分钟** `[实测·探针]`：2026-09-17 的 cron 探针约定 11:16 / 11:24，实际 11:17:54 / 11:25:54 触发。抖动由上面那个 salt 决定；**是否可配置、算法是什么，官方未写** `[未验证]`。

**2026-09-24 复测（数值已变，与上面那次不矛盾 —— salt 换了）** `[实测·探针]`：cron 约定 17:40:00 → 实际 **17:40:46（+46 s）**；
heartbeat 约定 18:04:23 → 实际 **18:04:24（约 +1 s）**。→ 计划时刻仍应**避开 00:00 与整点边界至少 10 分钟**（章程 §11.3）。

**本项目纪律：不得靠手改这些文件来管理 automation。** 三条理由：

1. **不可审计**：手改不留痕。出问题时无法回答「这个排期是谁、什么时候、为什么改的」，而 AGENTS.md 铁律 1 与 gov 的台账义务都要求变更可归因。
2. **易与 app 状态失步**：桌面应用在内存里持有 automation 状态，手改磁盘可能与它不一致；下一次应用写入会**静默覆盖**你的手改，且不会有任何报错。
3. **绕过锁协议**：ADR-0028 要求文件改写走「带等待、超时放弃、放弃必通报」的锁；这些文件在仓库外，`xtask guard` 覆盖不到 → 手改等于开一条无锁旁路。

要改就走 §4 的工具或 UI 路径。（同理，`~/.codex/config.toml` 在本手册里只被**引用**、从不被复制 —— 尤其 `[model_providers.custom]` 的连接串与令牌。这是 AGENTS.md §7 的密钥红线，本项目未来要开源。）

---

## 6. 与本项目夜间章程的映射

现行章程 = `docs/overnight-automation-charter.md` §11（**v1.9**，2026-09-25；机制 = Codex 原生 scheduled tasks；**排期形态 = ADR-0046 的「人类指令驱动的一次性任务」**）。ADR-0029 把机制从 ADR-0018 的 CLI 路径换回 Codex automation，下表据此逐条对表。下表的「机制缺口」= automation 表达不了、必须靠 prompt 纪律或人类审查兜住的部分。

| # | 章程要求（出处） | 在 automation 机制下怎么表达 | 结论 |
|---|---|---|---|
| 1 | 「当夜日期」：00:00–11:59 开始的运行归属前一夜（§11.3） | 机制没有「当夜」概念 → 把归属规则写进 prompt，由 agent 自己算日期 | ⚠ 可表达但脆弱：+2 分钟抖动（§5）会把贴近午夜的触发推过日界。**建议**计划时刻距 00:00 至少 10 分钟（ADR-0046 D5 起为建议、非硬规定） |
| 2 | 每夜 ≤3 轮、每次运行 ≤2 轮（§11.3） | 轮次 N = `docs/nightly/<当夜日期>-round-*.md` 文件数 + 1，由 agent 数文件；prompt 只**引用**章程、不复制规则 | ✅ 可表达（章程本来就是文件系统计数，与调度机制无关） |
| 3 | `.nightly.lock` 互斥（§11.2 / 门禁 G8） | 只能由 prompt 指示 agent 获取与释放 | ❌ **机制缺口**：没有包装脚本的 `try/finally`，agent 中途失败或被 kill 时**锁不会释放**。缓解：保留章程的「3 小时陈旧锁」规则；获取锁优先用 ADR-0028 的 `xtask guard acquire`（自带等待/超时/放弃通报），但 guard 是**文件级**，仓库级互斥仍需 `.nightly.lock` |
| 4 | 只在 `nightly/<date>` 分支提交（§0 底线 1） | **local 模式**：要 agent 自己 `git switch -c nightly/<date>`（prompt 指令，无强制；官方警告 local 模式会改你正在编辑的文件 `[官方]`）。**worktree 模式**：隔离好，但分支名由 Codex 决定、能否指定 `[未验证]` | ❌ **机制缺口**：无法机器强制分支名。缓解：晨间报告 §① 必须抄 `git log` 的分支与提交清单，人类核对 |
| 5 | 不合并 main（§0 底线 1） | prompt 禁令 | ❌ 无机器强制。而且官方说组织策略允许时 automation 用 `approval_policy = "never"` `[官方]` → 无人值守时**不会向任何人确认**。缓解：夜间白名单只留文档/测试类可逆工作（章程 §2），沙箱取最窄（官方也是这么建议的 `[官方]`） |
| 6 | 仅失败时通知（§11.6 旧依赖） | `notificationPolicy = failed_runs_only`；取消静音设为 `null` `[实测·声明]` | ⚠ 语义与章程要求**高度吻合**，但官方页全文没有 notification 一词 `[官方未写]` → 启用前必须实测「成功运行是否真的静默」（§8 D4） |
| 7 | 晨间报告落 `docs/nightly/<当夜日期>-report.md`（§11.4） | prompt 表达；需要沙箱允许写工作区 | ⚠ **worktree 模式下报告落在 worktree 里，主 checkout 看不到** → 人类早上打开仓库找不到报告。缓解：夜间 automation 用 **local 模式**，或规定人类从该 run 的 worktree/分支把报告取回 |
| 8 | 失败可见性三层信号：退出码 / 日志 / 报告（§11.6） | automation 路径**没有退出码**；官方给的是 **Scheduled** 视图里的 run 记录与未读标记 `[官方]` | ❌ **机制缺口**：没有机器可读的成功/失败判据。缓解：人类每天先看 Scheduled 的未读标记；报告 §① 由 agent 自述结果与门禁状态；细粒度失败原因是否落盘 `[未验证]` |
| 9 | 前置门禁 G1–G7（`cargo fmt` / `clippy` / `test` 全绿才开工，§1） | prompt 引用章程 §1，由 agent 自己跑 | ⚠ 沙箱坑：官方说 **workspace-write 下需要联网的工具调用会失败** `[官方]`。若 crates 未预先 fetch，`cargo test` 会因拉依赖而失败 → 夜间开工前必须**由人类先预热依赖**，或按官方说法用 rules 放行特定命令 `[官方]` |
| 10 | 运行前提：关机/睡眠不补跑（§11.7 ①） | 官方只要求「需要本地文件时保持电脑开机、应用运行，且项目在触发时刻仍在磁盘上」`[官方]` | ⚠ 错过之后是否补跑 `[未验证]`。章程的取舍是「跳过也无害」，两种行为都能接受，但要**实测记录**（§8 D6） |
| 11 | 硬超时 90 分钟（§11.7 ④、ADR-0018 风险表） | 没有对应字段 | ❌ **机制缺口**：无法设运行时长上限。缓解：prompt 写时间预算 + 章程 G7「当前时间 < 05:00」自检 + 人类早上确认没有卡死进程 |
| 12 | 排期形态：**人类指令驱动的一次性任务**（建立需人类指令；不同自动化之间**间隔 ≥ 2.5 h（默认 3 h）**；任务内容按实际进度现场决定）（ADR-0046 / 章程 §11.11） | 每个时刻**各建一个** automation，不用多时刻 RRULE（出问题时可以只暂停 / 只删除一个）；「一次性」的**具体写法** `[未验证]`（候选 RRULE `COUNT=1` / `UNTIL`，见 §4.1.a） | ⚠ **机制未验证**：一次性语义必须在下一次建立时实测（磁盘核对 `rrule` + 观察**第二次触发不发生**）；另外 RRULE 按哪个时区展开（本机 Asia/Shanghai）`[未验证]` |
| 13 | 不操作真实应用；资金/发送/对外发布类永久禁止自动化（章程 §2、AGENTS.md 铁律 6） | prompt 禁令 + 沙箱最窄 | ✅ 与官方建议一致：「从能让任务成功的最窄访问开始，只在必要时才给网络或更大文件访问」`[官方]` |
| 14 | 每轮重锚（重读 AGENTS.md / PLAN.md / 任务卡） | prompt 里要求；而 standalone 每次 run 新开 chat `[官方]`，天然没有上下文累积漂移 | ✅ 这一点比 ADR-0018 的 CLI 路径更省事：机制本身就保证了「每次全新会话」 |

**小结**：14 条里 **4 条 ✅、6 条 ⚠、4 条 ❌**（#3 锁释放、#4 分支强制、#8 机器可读结果、#11 时长上限）。这 4 条是 ADR-0029 相对 ADR-0018 的**净损失**，必须由「prompt 纪律 + 晨间人工核对」承接；承接不了的就是 §7 里的 OPEN 项。作为交换，换回来的是：应用内可见性（Scheduled 收件箱与未读标记）、`notificationPolicy` 的可能性、以及「每次 run 天然新会话」。

---

## 7. 已知机制缺口与风险清单

| # | 缺口 / 风险 | 影响 | 证据 | 缓解或处置 |
|---|---|---|---|---|
| R1 | 端点 400 拒绝 automation 注入的合成工具结果项 | automation 完全不可用；heartbeat 还会永久毒死所挂 thread | `[实测·探针]`（ADR-0018） | **→ 2026-09-24：根因已消失** —— 投递形态改为正常 user message，合成项不再出现（§2 更新块 / §8.2）。本条**保留**为历史记录与回退方案（ADR-0018）的依据；GATE-0 仍是「换端点 / 升级 Codex 后必须重跑」的门禁 |
| R2 | 触发有随机抖动（2026-09-17 记 ≈ +2 分钟；**2026-09-24 复测 cron +46 s、heartbeat 约 +1 s**） | 定时不准；贴近午夜会把「当夜日期」算错 | `[实测·探针]` | 计划时刻避开 00:00 与整点边界至少 10 分钟；日期归属规则写进 prompt（§6 第 1 行） |
| R3 | 创建失败只回 `Failed to create automation.`，不指明字段 | 排错成本高；曾因此误判「cron 需 ChatGPT 鉴权、本环境不可用」，白烧一晚 | `[实测·探针]` | 按 §4.1 的八个必填字段一次给全；仍失败就用「只传 `kind`」让校验器把缺失项全列出来 |
| R4 | `mode = "view"` 对不存在的 id 渲染空卡而不报错 | agent 自检会被骗，以为 automation 还在 | `[实测·探针]` | 存在性判断一律看磁盘 `automation.toml`，不看 view（§4.8） |
| R5 | 没有机器可读的「上次运行结果」（无退出码） | 失败可能无声无息；章程 §11.6 的第①层信号消失 | `[官方]` 只提供 Scheduled 视图与未读标记；细节 `[未验证]` | 人类每天先看 Scheduled 未读；报告 §① 强制自述结果；**登记为 OPEN**（`docs/memory/open.md`） |
| R6 | worktree 堆积 | 磁盘占用与分支噪音 | `[官方]` | 夜间用 local 模式，或按官方要求定期归档不再需要的 run、且不 pin run |
| R7 | heartbeat 污染长驻 thread | 主线工作会话永久报废，白天交互一并受牵连 | `[实测·探针]` | 主线 thread 永久禁挂；验证只用废弃 probe thread（GATE-0.2 / GATE-0.3） |
| R8 | `notificationPolicy` 与 `failed_runs_only` 官方未文档化 | 「仅失败时通知」可能不成立 → 要么每夜两条噪音通知，要么反过来失败也不通知 | `[实测·声明]`；官方页无此词 `[官方未写]` | §8 D4 实测；不成立就退回「只有文件、没有推送」（章程 §11.6 的现状） |
| R9 | `update` 是全量替换语义 | 漏一个字段就可能抹掉既有配置 | `[实测·声明]` | 改前先抄 `automation.toml` 全量字段；一次只改一个字段并立刻自检 |
| R10 | 无运行时长上限、无并发实例策略 | 一次卡死可能占住整夜和 `.nightly.lock` | `[未验证]`（未见对应字段） | prompt 写时间预算 + 陈旧锁规则 + 早晨人工检查（§6 第 3、11 行） |
| R11 | 组织策略允许时 automation 用 `approval_policy = "never"` | 无人值守时不会向任何人确认，与 AGENTS.md 铁律 6 直接冲突 | `[官方]` | 夜间白名单只留可逆的文档/测试类工作；沙箱取最窄；真实应用与资金/发送类永久排除 |
| R12 | 官方模型退役时间线（GPT-5.5 → 2026-10-14；GPT-5.4 / 5.4-mini → 2026-08-31） | 若 `model` 填了将退役的官方模型，任务会在退役后失效 | `[官方]` | 本机走第三方端点、`model` 必填且显式 → 不受该时间线影响；但**每次升级 Codex 后重跑 §8** |
| R13 | CLI 侧没有 automation 子命令 | 无法用脚本或 CI 管理 automation，不能纳入 `xtask` 门禁 | `[实测·探针]`（本轮 `codex --help`，`codex-cli 0.154.0-alpha.6.2` 的子命令列表里没有任何 automation 相关项） | 接受现实：automation 只能在应用内管理，因此天然不可 CI 化 → 台账义务落到本手册与 LEDGER |
| R14 | ~~heartbeat 的具体字段未探明~~ **→ 2026-09-24 已探明**：`kind = "heartbeat"` + `targetThreadId` + `rrule`（落盘 `target_thread_id`） | 原风险：手拼必失败，且失败不指字段 | `[实测·探针]`（2026-09-24） | 按 §4.2 的字段清单给全；仍优先用自然语言让 Codex 自己组装（§4.0 规则 1） |
| R15 | **沙箱开关不在 `config.toml`**：应用 UI 的权限模式（`~/.codex/.codex-global-state.json` → `permission-selection-by-host-id:local`）覆盖 `sandbox_mode` | 以为改了 `config.toml` 就收紧了沙箱 → **实际仍是 full access**（静默地不安全）；且 automation 用「你的默认沙箱设置」，所以夜间也照此 | `[实测·探针]`（2026-09-24：改 `config.toml` 后新建的 run 仍报 `sandbox=dangerFullAccess`，工作区外写入成功） | **收紧沙箱必须去应用 UI 改权限模式**（config.toml 不够）；改完用「工作区外写入应失败」做**负向验证**。另注：`workspace-write` 下需要联网的工具调用会失败（§6 第 9 行）→ 先 `cargo fetch` 预热 |

---

## 8. 验证清单（**2026-09-24 已执行** —— 结论见 §8.2 / §8.3）

> 断言对象换成 Codex automation；格式沿用 `docs/nightly/scheduler-acceptance-test.md`。按 ADR-0029 D4，那份清单**不删除**，改挂「已被取代（当前非主方案），保留为回退方案验收清单」横幅 —— 该标注由主会话执行，**不在本手册的 write scope 内**。
> **谁执行：必须人类在场** —— 首次要现场记录字段名与触发时刻，异常时要能立刻停。**GATE-0 通过前不得建正式排期。**
> **2026-09-24 执行记录**：人类授权调试时段，由本项目 agent 执行；触发时刻与字段名已落进 §8.2 基线表。

### 8.0 前置条件

| # | 条件 | 检查方式 | 期望 |
|---|---|---|---|
| P1 | Codex 版本已记录 | `codex --version` | 记入 §8.2 基线；**版本变了就重跑本清单**（R12 / R13） |
| P2 | 能拿到本项目 projectId | 调 `list_projects` | 非空，且 `isGitRepository` 为真 |
| P3 | 磁盘现状快照 | `Get-ChildItem -Force -Recurse "$env:USERPROFILE\.codex\automations"` | 只有 `.run-jitter-salt`（本轮基线） |
| P4 | 已准备一个**废弃 probe thread** | 新建、标题以 `nightly-probe-` 开头 | 存在，且确认它不是任何在用的工作 thread |
| P5 | 工作区干净、无残留锁 | `git status --porcelain`；`Test-Path .nightly.lock` | 空输出；`False` |

### 8.1 断言

| # | 断言 | 判定方法 | 期望 |
|---|---|---|---|
| D1 | **端点兼容性（GATE-0 核心）**：一次性 heartbeat 挂在 probe thread 上能真实产出 | 触发后看 probe thread 是否有模型回复；随后在同一 thread 手工再发一条消息 | 两次都正常；**没有** 400、没有「2.6 秒零产出」 |
| D1b | **cron 全链路**（ADR-0029 D3 的原文要求）：automation 触发 → 端点接受合成项 → 模型正常回复 → 产物落盘 | 建一个 standalone 探针任务（近时刻触发），观察新开的 chat 是否有真实回复、约定的产物文件是否落盘 | 四步全部成立。**门禁要 D1 与 D1b 都过才算通过**（两者失败症状不同，见 GATE-0.2） |
| D2 | automation 定义真的落盘，且字段名可抄录 | 同 P3 的命令 | 出现 `<id>\automation.toml`；把**字段名与取值抄进本手册 §5**，将相应 `[未验证]` 升级为 `[实测·探针]` |
| D3 | `suggested_create` / `suggested_update` 的真实语义 | 用 `suggested_create` 建一个探针任务，观察是否只渲染待确认卡片而不真建 | 记录实际行为：§4.0 规则 3 的推测要么被证实、要么被推翻 |
| D4 | `notificationPolicy = failed_runs_only` 是否真的只在失败时通知 | 让一次运行成功、一次故意失败，观察应用内通知 | 成功静默、失败有通知；不成立 → 按 R8 处置 |
| D5 | standalone 是否每次新开 chat、产出落在哪 | 触发一次 cron 探针，分别检查主 checkout 与各 worktree | 记录：新 chat 的 id、文件落在主 checkout 还是 worktree、分支名是什么 → 决定 §6 第 4、7 行怎么落地 |
| D6 | 抖动量与「错过是否补跑」 | 连续 3 次记录「约定时刻 vs 实际触发时刻」；再把时刻设到过去、让机器睡 5 分钟后唤醒 | 抖动量回填 §5；补跑行为回填 §6 第 10 行 |
| D7 | 暂停 / 恢复 / 删除三条路径都可用 | 依次操作，每次都复查磁盘与 Scheduled 视图 | 暂停后不触发；恢复后触发；删除后 `automation.toml` 消失、目录回到 P3 基线 |
| D8 | **清理**：不留任何探针残留 | 删除所有探针 automation、处置 probe thread、`git worktree list` 无残留 | 全部回到基线；probe thread 若已中毒则**直接删除**，不要试图修 rollout（`at_` id 从不落盘，无从修补） |

### 8.2 基线（首跑时填；**只追加，不改写既有行**）

| 项 | 值 | 首次观察日期 |
|---|---|---|
| `codex --version` | `codex-cli 0.154.0-alpha.6.2` | 2026-09-18 |
| `~/.codex/automations/` 内容 | 仅 `.run-jitter-salt`（37 字节，内容是一个 UUID） | 2026-09-18 |
| `automation_update` 的对外 schema | 空（`properties: {}`） | 2026-09-17 |
| `mode` 判别式取值 | `view` / `create` / `suggested_create` / `update` / `suggested_update` / `delete` | 2026-09-17 |
| `kind` 判别式取值 | `cron` / `heartbeat` | 2026-09-17 |
| 触发抖动 | 约 +2 分钟（约定 11:16 → 实际 11:17:54；约定 11:24 → 实际 11:25:54） | 2026-09-17 |
| `mode = "view"` 的返回 | `Rendered automation card in the app.`（不存在的 id 也返回同一句） | 2026-09-18 |
| 本机 automation 定义文件数量 | 0 | 2026-09-18 |
| `codex --version` | `codex-cli 0.155.0-alpha.16.3`（2026-09-18 记的 `0.154.0-alpha.6.2` 已过时） | 2026-09-24 |
| 桌面应用版本 | `26.917.51856` | 2026-09-24 |
| 端点 / 模型 | 端点 `opencode_go`，`model = deepseek-v4.1-flash`（**已不是** 2026-09-17 记的阿里云百炼 `qwen3.8-max`） | 2026-09-24 |
| **投递形态** | prompt 以**正常 user message** 投递（`role = "user"`、`msg_…` id、正文前缀 `Automation: <name>` / `Automation ID:` / `Automation memory:` / `Last run:`）；**不再**注入 `call_id: None` 的合成 `function_call_output` | 2026-09-24 |
| 触发抖动（cron / standalone） | **+46 s**（约定 17:40:00 → 实际 17:40:46） | 2026-09-24 |
| 触发抖动（heartbeat） | **约 +1 s**（约定 18:04:23 → 实际 18:04:24） | 2026-09-24 |
| cron 必填字段（实测） | 8 个：`kind` / `name` / `prompt` / `rrule` / `status` / `projectId` / `model` / `reasoningEffort` / `executionEnvironment`（`destination` **不需要**） | 2026-09-24 |
| `projectId` 取值口径 | **必须用 legacy project id**（本项目 `ai-assistant` = `5b628eec-9ca4-4768-8891-c7744a9dab5e`）；app-server id（`01a0a80a-d1e7-7551-9d27-6518a58b78ed`）会被拒，只回 `Failed to create automation.` | 2026-09-24 |
| `executionEnvironment` 取值 | 只接受 `"local"` | 2026-09-24 |
| `status` 取值 | `ACTIVE` / `PAUSED` | 2026-09-24 |
| heartbeat 必填字段（实测） | `kind = "heartbeat"` + `targetThreadId` + `rrule`（落盘字段名 `target_thread_id`）；其余字段可省 | 2026-09-24 |
| `automation.toml` 内部 schema | 可观察（原 `[未验证]`）：含 `target = { type = "project", project_id = "<legacy id>" }` 等 | 2026-09-24 |
| 运行位置（cron + local 模式） | 每次 run 开**全新 chat**；产物落在 **`cwd = D:\csart\ai-assistant`（主 checkout，非 worktree）** | 2026-09-24 |
| automation 跨轮记忆 | `~/.codex/automations/<id>/memory.md` 自动创建，agent 每轮可读写 | 2026-09-24 |
| 心跳 agent 能否删掉自己 | **能**（实测调 `automation_update(mode = "delete")` 成功） | 2026-09-24 |
| `::inbox-item` 指令 | `::inbox-item{title="…" summary="…"}` → 把「有发现」推进 Scheduled 收件箱（章程 §11.6 依赖的未读通道） | 2026-09-24 |
| `model_context_window` | 实际 **950000**（≠ `config.toml` 里写的 1000000） | 2026-09-24 |
| 沙箱开关的**实际位置** | **应用 UI 的权限模式**（`~/.codex/.codex-global-state.json` → `permission-selection-by-host-id:local`）**覆盖** `config.toml` 的 `sandbox_mode`；只改 `config.toml` **不生效**（实测：改后新建的 run 仍报 `sandbox=dangerFullAccess`，且工作区外写入成功） | 2026-09-24 |

### 8.3 判定与后续

- **全绿** → ① 把 §2 的 GATE-0 标为已通过 + 日期 + 执行人；② ADR-0029 的「生效前提」标为已满足；③ 章程 §11 按 §6 的映射表重写（改章程需 ADR，ADR-0029 即该授权）；④ 才可创建**正式**的夜间排期；⑤ `LEDGER.md` 追加一行；⑥ 把 D2–D6 的实测结论回填本手册，替换相应的 `[未验证]` 标签。
- **任一项失败** → 不建正式排期；失败项与证据记入 `docs/PARKING_LOT.md`；判断是回落到 ADR-0018 的 CLI 路径，还是触发「连续失败即暂停夜间自动化、回到纯白天推进」的同类条款。

**2026-09-24 执行结果：全绿**（证据 = §8.2 的 2026-09-24 行）。已按上面的「全绿」分支执行：

1. §2 的 GATE-0 已标为**已通过（2026-09-24）**；
2. ADR-0029 的「生效前提」标为已满足（依据其「验证方式」7①）；
3. 章程状态字段回填 → `docs/overnight-automation-charter.md` **v1.5**（§11.1「主方案」现状 + §11.9 当前状态；
   授权来源 = ADR-0029「验证方式」7① 与 **ADR-0041 D5**）；
4. **未创建任何正式排期** —— 人类 2026-09-24 指示「何时排期另行说明」；
5. `LEDGER.md` 已追加一行；
6. D2 / D5 / D6（部分）的实测结论已回填本手册 §4.1 / §4.2 / §5 / §8.2。

**诚实标注：本轮没有测的断言** —— D3（`suggested_create` 语义）、D4（`notificationPolicy = failed_runs_only`
是否真的只在失败时通知）、D7（暂停 / 恢复路径）、D6 的「错过触发是否补跑」→ **仍为 `[未验证]`**。
正式排期启用前应补测这几条（它们不影响「机制可用」这个结论，但影响「失败可见性」的可靠性）。

---

## 9. 变更历史

| 版本 | 日期 | 变更 | 依据 |
|---|---|---|---|
| 1.0 | 2026-09-18 | 初稿：证据分级图例与术语对齐、GATE-0 端点兼容性门禁、两类任务对照、八类操作的逐条做法（建立 cron / 建立 heartbeat / 修改 / 删除 / 立即运行 / 暂停 / 恢复与归档 / 查看）、磁盘形态与「不许手改」纪律、章程 14 条映射（4 ✅ / 6 ⚠ / 4 ❌）、14 条风险、验证清单 P1–P5 与 D1–D8 | ADR-0029；官方文档副本 `cx_automations.md`（2026-09-18 抓取）；ADR-0018 及其 2026-09-17 探针实测；`docs/overnight-automation-charter.md` §11（v1.3）；本轮 `codex --help` 与 `~/.codex/automations/` 目录实测；`docs/memory/facts.md`、`pitfalls.md`、`rejected.md` 的 2026-09-16 / 09-17 条目 |
| 1.1 | 2026-09-24 | **GATE-0 执行结果回填**：§2 标为**已通过（2026-09-24）** + 追加「投递形态已变」的更新块 + 硬标准三条全中的结论；§4.1 `status` / `projectId`（legacy id）/ `executionEnvironment` 三个字段从 `[未验证]` 升级为 `[实测·探针]`；§4.2 heartbeat 字段清单（`targetThreadId` + `rrule`）从 `[未验证]` 升级为 `[实测·探针]`，并把「额外禁令」写准（验证禁挂主线 / 夜间永久不用）；§4.4 删除路径补实测确认；§4.6 `status` 取值 = `ACTIVE` / `PAUSED`；§5 `automation.toml` schema 与 `memory.md` 补实测、抖动补 2026-09-24 复测值；§6 章程版本引用 v1.3 → v1.5；§7 R1（端点 400）标为「根因已消失」、R2 补复测值、R14 升级为已探明、**新增 R15（沙箱开关在应用 UI）**；§8 标为已执行 + §8.2 追加 18 行基线 + §8.3 写「全绿」分支的执行记录与**未测断言清单** | ADR-0029「验证方式」7①（GATE-0 实测通过 → 状态改 ✅、断言结果落基线）；人类 2026-09-24 授权调试时段并指示「何时排期另行说明」 |
| 1.2 | 2026-09-25 | **正式排期回填**：§2 的「尚未排期 —— 正式排期由人类另行说明后再创建（2026-09-24 指示）」→「**已排期（2026-09-25）**」并写明 automation 名与参数；文件头版本 1.0 → **1.2**（1.1 行写于 2026-09-24 但当时漏改文件头，属既有漂移，随本次一并追平）；§1~§8 其余内容一字未改 | 人类 chat 2026-09-25 授权「允许你进行夜间自动化做 task-019，020 和 021」；ADR-0029「验证方式」7①（GATE-0 通过即可建正式排期）；`~/.codex/automations/ai-assistant-nightly-task-chain/automation.toml` 实测落盘 |
| 1.3 | 2026-09-25 | **补建第二个单时刻 automation**：§2 状态行由「单个 02:30」改为「02:30 + 23:30 两个」，与 §6 第 12 行的建议口径（建两个可独立暂停的单时刻任务）和章程 §11.3（每夜两次）对齐；文件头版本 1.2 → **1.3**。§1 / §3~§8 一字未改 | 人类 chat 2026-09-25 复核请求（「自动化只跑了 019」）；首夜实测证据（02:30 计划 → 02:32:24 实际触发 → 03:22:25 完成，抖动 +2m24s）已记入 `LEDGER.md`；**ADR-0041 D5**（状态字段回填授权） |
| 1.4 | 2026-09-25 | **排期全部撤回**：§2 状态行由「已排期（02:30 + 23:30 两个常驻 automation）」改为「**当前未排期**（已按人类指示删除全部 automation）+ 正确形态 = 人类每晚上手动建立**一次性**任务（一晚 3 个 / 间隔 3 小时 / 按实际进度决定卡号）」，并显式指出 §4.1 尚未记录一次性任务的建法；文件头版本 1.3 → **1.4**。§1 / §3~§8 一字未改 | 人类 chat 2026-09-25 指示删除全部 automation 并说明正确形态；`automation_update(mode=delete)` × 2 + `~/.codex/automations/` 磁盘复核；**ADR-0041 D5** |
| 1.5 | 2026-09-25 | **§2 状态行改写 + §4.1 节末新增「一次性任务」子块（`#### 4.1.a`）**：§2 由「已排期（两个常驻 automation）」改为「**当前 automation 数 = 0**（已按人类指示删除）+ 正确形态 = 人类指令驱动的一次性任务（ADR-0046）」，并显式标注「**一次性写法 `[未验证]`**」；§4.1 标题补「**本项目只允许「一次性」形态**」并在节末新增子块，记录①「建立需人类指令」②「一次性写法」（`COUNT=1` / `UNTIL` 候选，未验证）③「间隔 ≥ 2.5 h（默认 3 h）」④「验证判据 = 磁盘核对 `rrule` + 观察第二次触发不发生」；文件头版本 1.4 → **1.5**。**章节编号保持 §4.0~§4.8 不变**（新内容放进 §4.1 的子块，不新开 §4.x —— 否则 §4.2 等既有交叉引用会断）；**§6 映射表**因**逐字引用**被 ADR-0046 改写的章程口径而同步 —— 表头版本行 v1.5 → **v1.9**、第 1 行「距 00:00 至少 10 分钟」由硬规定改**建议**、第 12 行由「每夜两次 23:30 + 02:30」改为「人类指令驱动的一次性任务 + 间隔 ≥ 2.5 h」，14 行的小结计数（4 ✅ / 6 ⚠ / 4 ❌）**不变**。§1 / §3 / §4.2~§4.8 / §5 / §7 / §8 正文一字未改 | 人类 chat 2026-09-25 完整 7 条规格；**ADR-0046**（D9 显式授权修订 §4.1 并新增一次性建法）；`~/.codex/automations/` 磁盘复核 |
| 1.6 | 2026-09-25 | **一次性写法回填（ADR-0046 D2）+ §2 状态行同步**：§4.1.a 的「「一次性」的具体写法」由 `[未验证]` 改为 **`[部分验证]`**（写法 = RRULE 加 `COUNT=1` + `BYHOUR`/`BYMINUTE`，已实测落盘；「第二次触发不发生」仍未观察 → PL-081），并在该子块末尾追加 2026-09-25 的实测证据块（两个 automation 的 id / `rrule` 逐字 / 落盘字段 / `cwds`）；§2 状态行的「**当前 automation 数 = 0**」改为「**= 2**（均为一次性）」；文件头版本 1.5 → **1.6**。§1 / §3 / §4.0~§4.8 其余 / §5~§8 一字未改 | 人类 chat 2026-09-25 指令建立两个自动化（13:30 → TASK-021 / 16:30 → TASK-022）；`automation_update(mode=create)` × 2 + `~/.codex/automations/` 磁盘复核；**ADR-0046 D2**（下次建立时当场实测并回填本手册） |
