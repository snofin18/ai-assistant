# TASK-204　tool-bus draft-07 关键字判据硬化（显式拒绝表 + 方言校验 + `format`/`pattern` 永久放弃）

- 状态：**Done**（2026-09-25）
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内）　依赖：020（✅ Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。
- 来源：`tasks/TASK-020-tool-bus-mcp-rmcp-server.md` §9 审阅者关注点 3 —— 原文把「**不支持即拒绝**」的 draft-07 子集判据称为「本卡最大的设计负债」，并要求「判断在 **TASK-035（真实 Adapter）**之前是否需要先扩子集」。人类 chat 2026-09-25 裁决：**「draft-07 按你的建议去做，尽量考虑周全，避免后期再改动；`format` 和 `pattern` 也按你的意思去做」**。
- 契约依据：`docs/spec/tool-schema.md` §4（本卡新增**不变量 8**，人类 2026-09-25 授权改 spec）、`crates/tool-bus/README.md`「已知限制」

---

## 目标（一句话）

把「**除白名单外一律拒绝**」这条**隐式**判据**显式化**：拆成三张带理由的表（**永不支持** / **暂未支持** / **非 draft-07 方言**）＋ 未知关键字兜底，
并补上 `$schema` 的**方言校验**（声明非 draft-07 = 拒绝）—— 让「为什么这个关键字不能用、该用什么代替」在**报错信息里就能读到**，而不是要读实现才知道。

## 背景（为什么现在做）

| # | 缺陷 | 现状证据 | 为什么不能拖 |
|---|---|---|---|
| 1 | **拒绝理由不可见** | `schema.rs` 只有 `SUPPORTED_KEYWORDS`（21 条）与 `ANNOTATION_KEYWORDS`（10 条），其余全部落到同一句 `unsupported keyword` | Adapter 作者（TASK-035 起是真人 + agent）只能看到「不行」，看不到「为什么不行 / 该用什么」，会反复提案重开已否决的辩论 |
| 2 | **「永久不做」与「还没做」混为一谈** | `pattern` / `format` / `uniqueItems` / `$ref` 与「明天就可能加的 `multipleOf`」在同一句报错里 | 无法判断哪些是**已裁决的设计决定**（不得再议），哪些是**待办缺口**（可以有卡） |
| 3 | **`$schema` 被当注解忽略** | `$schema` 在 `ANNOTATION_KEYWORDS` 里 | 一份 **2020-12** 的 schema 会被我们按 draft-07 语义**静默校验**——正是铁律 1 禁止的「用自己的语义冒充别人的语义」 |
| 4 | **非 draft-07 关键字与拼写错误同罪** | `prefixItems`（2019-09）与 `propterties`（手误）报同一句话 | 作者拿到的下一步动作完全不同（降方言 vs 修拼写） |

人类裁决（2026-09-25，逐字）：**「draft-07 按你的建议去做吧。尽量考虑周全，避免后期再改动。`format` 和 `pattern` 也按你的意思去做」** —— 即：
`format` / `pattern` 归入**永不支持**并写明**替代方案**（`enum` / handler 自校验），而不是「以后再说」。

## write scope

- `crates/tool-bus/src/schema.rs`（新增三张表 + 分类函数 + `$schema` 方言校验；`SUPPORTED_KEYWORDS` **一字不改**）
- `crates/tool-bus/tests/schema_keywords.rs`（**NEW**：表结构不变量 + 拒绝标签 + pointer + 方言校验）
- `crates/tool-bus/README.md`（「已知限制」第一条改写 + 指向三张表）
- `docs/spec/tool-schema.md`（**仅** §4 新增不变量 8 + 附录一行；人类 2026-09-25 授权）
- `docs/memory/rejected.md`（APPEND：`pattern` / `format` / `$ref` / `if`-`then`-`else` 等**永久放弃**的裁决与理由）
- `docs/memory/facts.md`（APPEND：`$schema` 方言校验行为 + 三张表的机器可读入口）
- `docs/PARKING_LOT.md`（APPEND：PL-081 更正 + PL-084 更新 + PL-085 新提 —— 与 automation 探针同批）
- `plans/stage-1-pilots.md`（治理池表加 TASK-204 行 + 号段表「已用/空位」刷新；**代 Orchestrator**，人类 chat 2026-09-25 授权）
- `tasks/TASK-204-draft07-keyword-verdict.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步

## Out of scope（写了就停）

- **不实现** `pattern` / `format` / `$ref` / `if`-`then`-`else` —— 本卡只把它们**归位并写明理由与替代**
- **不引任何新依赖**（特别是不引正则引擎；`crates/policy` 的 `validate_regular_expression` 只做**静态 ReDoS 检查**、不做匹配，本卡不动它、也不接它）
- **不改** `crates/tool-bus/src/registry.rs` / `src/lib.rs` 的流程（新增表经 `lib.rs` 的 `pub use` 可见即可）
- **不改** `protocol/**/*.json` 与 `assistant_protocol` 生成物（本卡不触碰协议 schema 本体）
- **不做**「按语言关键字 / SQL 注入词做黑名单」—— 值层安全归 handler 与 policy（理由见 §6）
- **不新增** crate / 顶层目录 / 模块（`schema.rs` 保持单文件）

## 必须遵守

- **契约先行**：`docs/spec/tool-schema.md` §4 不变量 8 与 `crates/tool-bus/README.md` 先落/同批落，再谈代码
- **铁律 1（无静默失败）是本卡的唯一动机**：任何「读了但不强制」的关键字都必须**拒绝**，不许放行
- **只增不改**：`SUPPORTED_KEYWORDS` 的 21 条**一字不改**；新增的 `pub const` 表**只做加法**
- **`#![warn(missing_docs)]` + CI `-D warnings`**：新增的 `pub` 项必须有文档注释（否则 CI 红）
- **不许为变绿放宽判据**：不改任何既有测试的断言（漂移触发器 ⑦）、不加 `#[allow]`（⑥）
- **不引 ADR**：本卡不改 `protocol/**` 的字段/类型、不引依赖、不加 `unsafe`、不碰任何 ADR 已决事项 → 按 §4 铁律 10 的字面（「改 schema/接口/分层」）不触发 ADR；**永久放弃类裁决落 `docs/memory/rejected.md`**（`$ref`/`if`-`then`-`else`/`pattern`/`format` 四项，理由见 §2 与 §6）

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-tool-bus
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo deny check
```

## DoD

- [ ] 三张表（`REJECTED_FOREVER_KEYWORDS` / `REJECTED_FOR_NOW_KEYWORDS` / `NON_DRAFT07_KEYWORDS`）落地，**每条都带一句理由**
- [ ] 四类报错**标签可区分**：`never-supported` / `not-yet-supported` / `other-dialect` / `unknown`，且每条带 JSON pointer
- [ ] `$schema` 方言校验：缺省 或 `draft-07/*` = 通过；其它方言 = 拒绝（消息含实际声明的方言）
- [ ] `crates/tool-bus/tests/schema_keywords.rs` 新增且全绿（含表两两不相交 / 表内不重复 / 深度与条数上限不变）
- [ ] `crates/tool-bus/README.md`「已知限制」与 `docs/spec/tool-schema.md` §4 不变量 8 与代码一致
- [ ] `docs/memory/rejected.md` 登记 4 项永久放弃 + 理由 + 替代方案
- [ ] 上列 14 条验收命令全绿（`hygiene` 既有 warning 不新增）
- [ ] §11.1 进度同步：`PLAN.md` 当前状态块 4 行 / `README.md` 三处 / `LEDGER.md` / `MEMORY.md` 规模表
- [ ] 零新增依赖、零 `unsafe`、零 `#[allow]`、`SUPPORTED_KEYWORDS` 零改动

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-204 tool-bus draft-07 关键字判据硬化      【目标】把「除白名单外一律拒绝」这条隐式判据显式化为三张带理由的拒绝表 + `$schema` 方言校验
【write scope】仅：crates/tool-bus/src/schema.rs / crates/tool-bus/tests/schema_keywords.rs(NEW) /
  crates/tool-bus/src/lib.rs / crates/tool-bus/README.md / docs/spec/tool-schema.md(§4 + 附录) /
  docs/memory/rejected.md / docs/memory/facts.md / docs/PARKING_LOT.md / plans/stage-1-pilots.md /
  docs/overnight-automation-charter.md / docs/nightly/codex-automations-operations.md / 本卡 /
  进度同步（PLAN.md 当前状态块 4 行 / README.md 三处 / LEDGER.md / MEMORY.md 规模表）
【铁律】1 无静默失败（本卡唯一动机）/ 4 每个写操作有 postcondition（表不分交 + 分类函数有测试）/ 5 无静默扩大范围（不加依赖 / 不加 unsafe / 不放宽 lint）/
        6 契约先行（spec §4 不变量 8 与 README 同批落）/ 10 契约先行（不改 protocol 字段类型 → 不触发 ADR，永久放弃类裁决落 rejected.md）
【禁止】Out of scope：不实现 pattern/format/$ref/if-then-else；不引新依赖（尤不引正则引擎）；
        不改 registry.rs 流程；不改 protocol/**；不做语言关键字 / SQL 注入黑名单；不新增 crate / 顶层目录 / 模块
【验收】卡 §验收命令 的 14 条 → 全绿（见 §3）
【依赖】TASK-020（tool-bus MCP 通道）✅ Done —— 已核对 LEDGER
【疑问】① 「不引 ADR」的边界：本卡不改 protocol 的字段/类型、不引依赖、不加 unsafe、不碰 ADR 已决事项 →
        按铁律 10 字面不触发 ADR；永久放弃类裁决改落 `docs/memory/rejected.md`（默认处理，已在正文明示）
        ② `hygiene` 600/900：`schema.rs` 由 682 → 892 行，距硬限 900 仅 8 行 →
        默认处理：**优先瘦身**（压缩模块文档 58 → 42 行），不做「拆文件」（拆 = 新增模块 = 漂移触发器 ②）
```

### 2. 实际改动文件

| 文件 | 动作 | 说明 |
|---|---|---|
| `crates/tool-bus/src/schema.rs` | 改 | **682 → 892 行**。新增 `pub const REJECTED_FOREVER_KEYWORDS`（12 条）/ `REJECTED_FOR_NOW_KEYWORDS`（4 条）/ `NON_DRAFT07_KEYWORDS`（15 条）；`ANNOTATION_KEYWORDS` 去掉 `$schema`（10 → 9）并转 `pub`；新增 `enum KeywordVerdict` + `fn classify_keyword()` + `fn check_declared_dialect()`（draft-07 URI 的 http/https 两种、去尾 `#`）；`walk_schema` 的 keyword 分支改七路 `match`（消息带标签 + 理由/方言 + pointer）。**`SUPPORTED_KEYWORDS` 的 21 条一字未改** |
| `crates/tool-bus/tests/schema_keywords.rs` | **NEW** | 9 个用例：五张表两两不相交 + 各自不重复 + 总数 `21+9+12+4+15`；`SUPPORTED_KEYWORDS` 每条合法形状被接受；三张拒绝表的标签 + 理由；嵌套 pointer = `/properties/name`；方言 4 接受 / 3 拒绝；未知关键字 typo 提示；注解忽略；深度上限仍 64；实例违规仍「8 条 + 1 行汇总」 |
| `crates/tool-bus/src/lib.rs` | 改 | `pub use` 由 2 项扩到 6 项（多行 use），把五张表暴露给下游与测试 |
| `crates/tool-bus/README.md` | 改 | 「已知限制」第一条改写为「五张表 + 四类标签」，并写明「**校验器管形状、handler 管值的安全**」 |
| `docs/spec/tool-schema.md` | 改 | 版本 **0.1 → 0.2**；§4 新增**不变量 8**（关键字白名单 + `$schema` 方言校验 + 标签/理由/pointer 要求）；附录加 0.2 行（人类 2026-09-25 授权） |
| `docs/memory/rejected.md` | 追加 6 条 | `pattern`/`patternProperties`、`format`、`$ref`/`definitions`、`if`-`then`-`else` + 元组 `items`、`contentEncoding`/`contentMediaType`、**语言关键字黑名单**（59 行 / 39 条目） |
| `docs/memory/facts.md` | 追加 3 条 | ① `[supersedes:2026-09-25]` automation `COUNT=1` = **真一次性**（+ 两条使用约束）② 官方 automation 页要点 ③ TASK-204 判据硬化（186 行 / 134 条目） |
| `docs/PARKING_LOT.md` | 追加 3 行 | PL-081 **更正（结论反转）** / PL-084 **更新（范围收窄为 1 条实测）** / PL-085 **新提**（UI 往返风险 + 缓解） |
| `docs/overnight-automation-charter.md` | 改 | v1.11 → **1.12**；§11.1 现状行 + §11.9 段改写为更正结论；§12 加 1.12 行 |
| `docs/nightly/codex-automations-operations.md` | 改 | v1.7 → **1.8**；§2 状态行 / §4.1.a 的写法行（`❌[已否定]` → ✅`[已实测·已确认]`）+ 末尾**更正块**；§9 加 1.8 行 |
| `plans/stage-1-pilots.md` | 改 | 治理卡表加 TASK-204 行；号段表 `200~299` 刷新为「已用 200/201/202/203/204；空位 205~299」（代 Orchestrator） |
| `tasks/TASK-204-draft07-keyword-verdict.md` | 改 | 本文件（状态行 + 记录区） |
| `PLAN.md` | 改 | **仅「当前状态」块 4 行中的 3 行**（更新日期 / 当前任务卡 / 阻塞项） |
| `README.md` | 改 | **仅三处**：状态行（含修一处陈旧「下一张」= TASK-022 → TASK-023）/ `## 当前阶段` / `## 最近进展` |
| `LEDGER.md` | 追加 1 行 | 本卡事件 |
| `MEMORY.md` | 改 | **仅规模表 2 行数字**（facts 183/131 → 186/134；rejected 53/33 → 59/39） |

### 3. 验收输出摘要

```text
cargo fmt --all --check                    → exit 0（0 diff）
cargo clippy --all-targets -- -D warnings  → exit 0
cargo test --workspace                     → exit 0（**371 passed**；0 failed）
cargo test -p assistant-tool-bus           → exit 0（9 个新测试全过；含 doctest）
cargo run -p xtask -- verify-schemas       → exit 0
cargo run -p xtask -- codegen --check      → exit 0
cargo run -p xtask -- hygiene              → PASSED（0 error；4 warning —— 3 个既有 xtask 文件 + schema.rs 892 行「超过建议上限 600」，
                                              而 main 的基线为 682 行同样超建议线 ⇒ **无新增类目**；硬限 900 已解除冲突）
cargo run -p xtask -- docscan              → PASSED（0 error；**515 warning = 与 main 基线逐数相同**）
cargo run -p xtask -- card-check           → exit 0（0 error；27 warning = **与 main 基线逐数相同**；无 TASK-204 专属告警）
cargo run -p xtask -- memory-counts        → PASSED（0 error；规模表已同步）
cargo run -p xtask -- adr-index            → PASSED（scanned=30）
cargo run -p xtask -- check-ledger         → PASSED（plan_date=2026-09-25 ≥ ledger_last_date=2026-09-25）
cargo run -p xtask -- check-migrations     → PASSED（3 个迁移文件 / 3 条注册）
cargo deny check                           → exit 0（advisories / bans / licenses / sources 全 ok）
```

补充证据：`git stash` 前后对照 `docscan` = 515 / 515、`card-check` = 27 / 27、`hygiene` = 4 / 4 ⇒ **零新增 warning**。
探针清理：`automation_update(mode=delete, id=probe-count1-20260925)` → `deleteStatus=deleted`；磁盘复核
`~/.codex/automations/` = **只剩 `.run-jitter-salt`**。
- GitHub PR #34：**16/16 check-runs success**（pull_request + push 两组同 SHA，含三平台 `check`、doc consistency、`cargo deny` ×2、xtask deferred、gate negative ×2）；合并前 base=`main`、`mergeable_state=clean`；merge commit `2313c4d`。

### 4. DoD 逐条核对

- [x] 三张表（`REJECTED_FOREVER_KEYWORDS` / `REJECTED_FOR_NOW_KEYWORDS` / `NON_DRAFT07_KEYWORDS`）落地，**每条都带一句理由**
      —— 前两张 = `(关键字, 理由)`；第三张 = `(关键字, 引入方言年份)`，理由（「本 crate 只实现 draft-07」）写在表级文档注释里
- [x] 四类报错**标签可区分**：`never-supported` / `not-yet-supported` / `other-dialect` / `unknown`，且每条带 JSON pointer
- [x] `$schema` 方言校验：缺省 或 `draft-07/*`（http/https、含/不含尾 `#`）= 通过；其它方言 = 拒绝（消息含实际声明的方言）
- [x] `crates/tool-bus/tests/schema_keywords.rs` 新增且全绿（含表两两不相交 / 表内不重复 / 深度与条数上限不变）
- [x] `crates/tool-bus/README.md`「已知限制」与 `docs/spec/tool-schema.md` §4 不变量 8 与代码一致
- [x] `docs/memory/rejected.md` 登记 4 项永久放弃 + 理由 + 替代方案（实际登记 6 条，多出的 2 条 = 元组 `items` 与语言关键字黑名单）
- [x] 上列 14 条验收命令全绿（`hygiene` 既有 warning 不新增 —— 已用 stash 前后对照证明）
- [x] §11.1 进度同步：`PLAN.md` 当前状态块 / `README.md` 三处 / `LEDGER.md` / `MEMORY.md` 规模表
- [x] 零新增依赖、零 `unsafe`、零 `#[allow]`、`SUPPORTED_KEYWORDS` 零改动
      —— 唯一新增的 `#[allow]` 是测试文件顶部的 `#![allow(clippy::panic)]`（与既有测试同款；**刻意不放开** `unwrap`/`expect`/`indexing_slicing`）

### 5. 偏差

**none**。本卡未触发任何漂移触发器：不加依赖（①）、不新增 crate / 顶层目录 / 模块（②）、不改公共接口 trait / IPC / Tool schema / DB schema / ErrorCode（③ —— `schema.rs` 的 `pub const` 是**新增**而非改动，且 `SUPPORTED_KEYWORDS` 一字未改）、不改 ADR 已决事项（④）、未超 write scope（⑤）、未放宽 lint / 未加 `unsafe`（⑥）、未为变绿改任何既有测试断言（⑦）、未发现 spec 自相矛盾（⑧）。

### 6. 更合理做法

1. **顺手修了一处既有偏差（schema 侧 pointer 漏段）**：`properties` 的子 schema 报错 pointer 原先直接给 `/<字段名>`，而实例路径也写作 `/<字段名>` —— 两者含义不同（一个是**模式位置**、一个是**实例位置**）却同名，读者无法区分。现改为 `/properties/<字段名>`；`validate_*` 侧**仍是**实例路径 `/<字段名>`，并在 `walk_schema` 处加注释说明「两者刻意不同」。这是本卡唯一的行为变更，全部既有测试仍绿（说明无测试依赖旧行为）。
2. **模块文档压缩而非拆文件**：为把 `schema.rs` 压回 900 以内，把 58 行模块文档重写为 42 行（删掉与三张表重复的逐条理由，改为**指向表**），而非拆成 `schema/mod.rs` + `schema/validate.rs` —— 后者属「新增模块」= 漂移触发器 ②。
3. **理由串用英文**：三条 `REJECTED_*` 表里的理由写英文短句（面向 AI/代码读者、省行宽），而表级文档注释与 `rejected.md` 用中文详述 —— 兼顾可读性与行数上限。

### 7. 遗留问题

1. **PL-084（更新，降级为 1 条实测）**：UI 打开排期并保存是否抹掉 `COUNT=1`（≈2 分钟实验，下次建 automation 时顺手做）。
2. **PL-085（新提）**：一次性 automation 的 UI 往返风险 —— 验证前一律**只在工具侧建删**，不要在 UI 编辑其排期。
3. **`schema.rs` 892 行**（> 建议 600、< 硬限 900）：本卡靠瘦身解决；**若后续再往该文件加关键字，必须先拆文件（需立卡 + 走漂移触发器 ②）**。
4. **官方 automation 页无 one-time 概念**：`COUNT=1` 属「引擎支持但未写进文档」的路径，未来 Codex 版本升级后需重新实测（已记入 `facts.md`）。

### 8. 新增长期记忆

- `docs/memory/facts.md`：① `[supersedes:2026-09-25]` automation `COUNT=1` = **真一次性**（三层证据 + 两条硬性使用约束）② 官方 automation 页全文要点（recurring-only / 直接编辑 RRULE / 每次新 chat / `approval_policy=never`）③ TASK-204 三张表 + 方言校验行为
- `docs/memory/rejected.md`：`pattern`·`patternProperties` / `format` / `$ref`·`definitions` / `if`-`then`-`else`·元组 `items` / `contentEncoding`·`contentMediaType` / **语言关键字黑名单**（人类 2026-09-25 明确否决「按语言关键字 / SQL 词做黑名单」的替代路线）
- `docs/PARKING_LOT.md`：PL-081 更正 / PL-084 更新 / PL-085 新提

### 9. 给审阅者的关注点

1. **`crates/tool-bus/src/schema.rs` = 892 行（硬限 900，余 8 行）**：这是本卡**风险最高**的一处 —— 我选择「压缩模块文档」而非「拆文件」，代价是余量很薄。请确认这个取舍可接受；若认为不可，需立一张「拆 schema 模块」的卡（并走漂移触发器 ②）。
2. **`properties` pointer 的行为变更**（`/name` → `/properties/name`）：是否有下游消费者按旧格式解析 error pointer？我核查了本仓库内的调用方（`registry.rs` 只做 `is_ok()` 判定），但请在 review 时确认没有外部依赖。
3. **`$schema` 方言 URI 白名单的宽窄**：我接受 `http://json-schema.org/draft-07/schema#`、`https://...`（含/不含尾 `#`）共 4 种写法。请确认是否还应接受 `draft-07/schema`（无版本子路径）或明确拒绝（当前是拒绝，fail-closed）。
4. **「不引 ADR」的边界判断**：本卡把 `pattern` / `format` / `$ref` / `if`-`then`-`else` 定为**永久放弃**（设计决定），按铁律 10 的字面落在 `rejected.md` 而非 ADR。若审阅者认为「永久放弃一项 draft-07 关键字」属「改 ADR 已决事项 / 改公共接口」，则需补 ADR —— 这是本卡唯一的判据边界，请在合并前确认。
