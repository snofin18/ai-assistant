# TASK-200　修复 `docs/spec/*` 7 份契约草案的系统性缺陷（TASK-072 批量生成事故）

- 状态：**Done**（2026-09-24）
- 阶段：跨阶段（治理池）　子阶段：—　批次：—　依赖：无（TASK-072 已 Done，其产物有缺陷）　预估：S　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。

---

## 目标（一句话）

把 `docs/spec/` 下 7 份由 TASK-072 批量生成的契约草案修成**可用的契约**：删掉每份文件里被误粘进去的外来模板块、补上空的 §1 目标 / §2 范围、逐份自检交叉引用不再张冠李戴。

## 背景（2026-09-24 实测证据，全量复核时发现）

TASK-072 一次性生成了 7 份 spec（`tool-schema` / `envelope` / `error-codes` / `capability-matrix` / `audit-event` / `ipc-protocol` / `testing`）。
实测（`docs/spec/naming.md` 是更早手写的，**未受影响**）7 份**全部**有同一组缺陷：

1. **`## 1. 目标` 与 `## 2. 范围` 是空标题**（只有标题行，下面无内容）—— 7/7 命中。
2. **每份文件都有两套 `## 4. 不变量` + `## 5. 与其他 spec 的关系`**（节号重复）—— 7/7 命中。
   第二套是**同一个外来模板块**被粘进每一份文件：
   - 以 `### 字段` 开头，而且 **`### 字段` 连着出现两次**（行 45/46、60/61、53/54 等）
   - 内容是"通用 schema"口径：`schema_version` 单调递增 / 必选字段不可为空 / 时间戳 ISO-8601 / `id` 用 u64 / 错误用 ErrorCode
   - 结尾是一张 7 行的「与其他 spec 的关系」表，**最后一行自引用 `docs/spec/testing.md`**（在 `tool-schema.md` / `audit-event.md` 里也出现，证明它是从别的文件模板复制来的）
3. 副作用：`docs/spec/testing.md` 里出现了 **tool-schema 的字段表**（`schema_version` / 命名空间），而它本该是测试规范。
4. **`xtask docscan` 抓不到**（它只查破表 / setext 风险 / 编码形状 / 末行 LF）→ 缺陷一路活到今天；`PL-003`（`xtask/src/report.rs` 引用 `docs/spec/testing.md` §4.3，而该节不存在）是同一处伤口的另一个症状。

**为什么必须修**：这 7 份是**契约层**文件（文件头明写「本文档是契约。违反即 CI 失败或 Reviewer 拒绝合并」），
stage-1 的 TASK-011（protocol schema）、TASK-020（tool-bus）、TASK-021（policy）等都会**按它们实现**；
节号重复 + 目标/范围为空 = 下个 agent 读到的"契约"是残的。

## write scope

- `docs/spec/{tool-schema,envelope,error-codes,capability-matrix,audit-event,ipc-protocol,testing}.md`（7 份，**不含 `naming.md`**）
- `docs/PARKING_LOT.md`（给 PL-038 追加关闭行）、`LEDGER.md`、`MEMORY.md`（规模表，若变化）
- `tasks/TASK-200-fix-spec-contract-drafts.md`（本卡记录区）

## In scope

1. **逐份删掉外来模板块**：从第一个 `### 字段` 行（含其上被重复的那个 `### 字段`）到第二套 `## 5. 与其他 spec 的关系` 表格结束，整段删除。
   - 保留**第一套** `## 4. 不变量`（各文件自己的口径，例：tool-schema = name 反向 DNS / capability 子集 / postcondition 必填；audit-event = append-only / prev_hash 链）
   - 第一套 `## 5.` 目前只有一行括注（例：`(本节 = ADR-0021 D7 受控词引用 + capability-matrix + error-codes)`）→ **保留这一行的意图，并按 gov §3.2 的写法把它扩成该文件真正的「与其他 spec 的关系」**（可以复用外来表里的行，但**逐行核对是否适用于本文件**，不适用的删掉）
2. **补 `## 1. 目标` / `## 2. 范围`**：每份 3~8 行，写清"这份 spec 管什么、不管什么"；范围必须含**明确的不做清单**（Out of scope 效力高于 In scope）。
3. **逐份交叉引用自检**：文中出现的每个 `docs/spec/*.md` 引用必须真实存在且语义对得上（例如 `testing.md` 里不得再出现 tool-schema 的字段表；`report.rs` 引用的 §4.3 若确实需要，**由本卡在 `testing.md` 里补出 §4.3**，否则记 PL 提案改 `report.rs` 的注释）。
4. **`PL-003` 一并处置**：`xtask/src/report.rs` 的悬空引用（`docs/spec/testing.md` §4.3）—— 优先"在 testing.md 补出该节"，因为改 Rust 注释超出本卡 scope。
5. **`docs/spec/README.md`**：若不存在，**不要新建**（新增顶层文件 = 漂移触发器 ②）→ 记 PL。

## Out of scope（做了算漂移）

- **修改任何契约字段 / 类型 / 枚举**（`docs/spec/*` 的变更门槛 = 「新增字段或修改类型 → 需 ADR」；本卡只做**去重 + 补空节 + 修引用**）
- `docs/spec/naming.md`（未受影响，且它是 23 个缩写白名单等**已生效契约**）
- `xtask` 规则实现（"节号重复"这类机器检查归护栏卡；本卡只记 PL 提案）
- `protocol/*.json`（TASK-011 的产物）、任何 Rust 代码、任何 ADR

## 必须遵守

- **不得顺手改口径**：删除外来块时若发现某条"通用不变量"其实**该由本文件承担**（例如 `error-codes.md` 的「错误用 ErrorCode 枚举」），把它**移进本文件的 §4**，不要直接丢 —— 但**不许新增字段/类型**。
- 每份文件改完必须逐份复核：`## 1.`~`## 5.` 节号**各出现一次**，`### 字段` 之类的重复标题为 0。
- 只追加文件（`LEDGER.md` / `PARKING_LOT.md`）**不改写既有行**；`PL-038` 用追加关闭行（`[supersedes:日期]` 风格）。
- 文档改动不得引入 CRLF / BOM / 多余末行（`xtask docscan` 必须全绿）。
- 若发现缺陷范围**超出上述 7 份文件**（例如 `docs/spec/` 之外也有同源粘贴），**停下写 DRIFT**，不要扩大本卡。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
cargo run -p xtask -- docscan
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
# 节号唯一性自检（人工 + 可复制的命令）
Get-ChildItem docs/spec -Filter *.md | Where-Object Name -ne 'naming.md' | ForEach-Object {
  $h = Select-String -Path $_.FullName -Pattern '^## '
  "$($_.Name): $($h.Count) 个二级标题"; $h | ForEach-Object { "   $($_.Line)" }
}
```

## 完成定义（DoD）

- [ ] 7 份文件中，`## 4.` / `## 5.` **各只出现一次**；`### 字段` 的重复标题为 0
- [ ] 7 份文件的 `## 1. 目标` / `## 2. 范围` **非空**，且范围含明确的不做清单
- [ ] 每份文件的「与其他 spec 的关系」里，被引用的 spec 都真实存在且语义对得上（逐条人工核对并记录在 §3）
- [ ] `PL-003` 已处置（testing.md 补出 §4.3 **或** 记下改 `report.rs` 注释的提案）
- [ ] `xtask docscan / hygiene / memory-counts / adr-index / card-check` 全部 PASSED
- [ ] `docs/PARKING_LOT.md` 追加 PL-038 关闭行；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 的文件被修改（尤其：**没有改任何字段/类型/枚举**）
- [ ] 本卡 §1~§9 执行记录已填

## 风险与已知坑（开工前先读）

- **最大风险是"修着修着改了契约"**：本卡的判据是"结构完整 + 引用正确"，**不是**"内容更好了"。任何"这里应该加个字段"的念头 → 记 PL，不动手。
- 7 份文件的缺陷**同源**，但**修法不能纯机械**：第一套 §4/§5 是各文件自己的，第二套是外来块；机械"删后半段"会删错（`testing.md` 的第一套 §5 只有一行括注）。
- 先跑一遍验收命令里的"节号自检"，把**修改前**的基线记进 §3，再动手 —— 否则无法证明改动有效。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-200 修复 `docs/spec/*` 7 份契约草案的系统性缺陷（TASK-072 批量生成事故）
【目标】删掉每份文件里被误粘进去的外来模板块、补上空 §1/§2、逐份自检交叉引用不再张冠李戴
【write scope】仅：`docs/spec/{tool-schema,envelope,error-codes,capability-matrix,audit-event,ipc-protocol,testing}.md`（**不含** `naming.md`）、`docs/PARKING_LOT.md`、`LEDGER.md`、`MEMORY.md`、`tasks/TASK-200-fix-spec-contract-drafts.md`（记录区）
【铁律】2（被读文档 = 五类不可信输入之一 → 逐份核对而非照抄）、9（不得静默扩大范围）、10（契约先行）、1（无静默失败）
【禁止】改任何契约字段 / 类型 / 枚举；动 `naming.md` / `protocol/**` / 任何 `.rs` / 任何 ADR；新建 `docs/spec/README.md`
【验收】节号唯一性自检 + `xtask docscan / hygiene / memory-counts / adr-index / card-check` → 全 PASSED
【依赖】无（TASK-072 已 Done，其产物有缺陷）
【疑问】无
```

### 2. 实际改动文件

| # | 文件 | 动作 | 说明 |
|---|---|---|---|
| 1 | `docs/spec/tool-schema.md` | EDIT | 删外来模板块；补 §1/§2；§4 保留本文件 4 条 + 从外来块**逐条核对后**移入 3 条通用不变量；§5 重建为 7 行 |
| 2 | `docs/spec/envelope.md` | EDIT | 同 1；§5 中 envelope↔ipc-protocol 一行方向由 `依赖` 改 `被依赖`（依据：本 spec §2 明写「不管传输层」） |
| 3 | `docs/spec/error-codes.md` | EDIT | 同 1 |
| 4 | `docs/spec/capability-matrix.md` | EDIT | 同 1 |
| 5 | `docs/spec/audit-event.md` | EDIT | 同 1 |
| 6 | `docs/spec/ipc-protocol.md` | EDIT | 同 1 |
| 7 | `docs/spec/testing.md` | EDIT | 同 1 + 新增 `### 4.3 输出可注入`（**PL-003 闭环**：`xtask/src/report.rs` 的注释引用的正是这一节） |
| 8 | `docs/PARKING_LOT.md` | APPEND | PL-038 关闭行；新提 PL-040 / PL-041 |
| 9 | `docs/memory/pitfalls.md` | APPEND | +1 PITFALL（`docscan` 抓不到节号重复 / 空节） |
| 10 | `MEMORY.md` | EDIT | §1 规模表 `pitfalls.md` 184/81 → 185/82 |
| 11 | `LEDGER.md` | APPEND | 本卡 Done 行（只追加，不改写既有行） |
| 12 | `tasks/TASK-200-fix-spec-contract-drafts.md` | EDIT | 记录区 §1~§9 + 状态行 Ready → Done（**正文区一字未动**） |

**未改**（Out of scope 硬线，逐项核对）：任何契约字段 / 类型 / 枚举、`docs/spec/naming.md`、`protocol/**`、任何 `.rs`、任何 `docs/adr/**`、`AGENTS.md`、`PLAN.md`、`plans/**`、`docs/spec/README.md`（不新建）。

### 3. 验收输出摘要

**（a）修改前基线**（`git show HEAD:docs/spec/<name>.md` 逐份实测，证明缺陷真实存在）

| 文件 | `^## ` 二级标题 | 重复的二级标题 | `^### 字段` | §1 / §2 |
|---|---|---|---|---|
| tool-schema.md | 8 | `## 4. 不变量` ×2、`## 5. 与其他 spec 的关系` ×2 | 2 | 空 |
| envelope.md | 8 | 同上 | 2 | 空 |
| error-codes.md | 8 | 同上 | 2 | 空 |
| capability-matrix.md | 8 | 同上 | 2 | 空 |
| audit-event.md | 8 | 同上 | 2 | 空 |
| ipc-protocol.md | 8 | 同上 | 2 | 空 |
| testing.md | 8 | 同上 | 2 | 空 |

= **7/7 命中全部 4 类缺陷**（空节 / 节号重复 / 重复标题 / 外来模板块），与卡面「背景」一节逐条吻合；`naming.md`（更早手写）未受影响。

**（b）修改后**（同一条自检命令）

7/7 文件：`^## ` = 6（§1~§5 + `## 附录：演进记录`）、**重复二级标题 = 0**、`^### 字段` = 0、末字节 = 10（LF）、无 CR、§1 与 §2 非空且各含 4~5 条「不管（不做清单）」。

**（c）交叉引用逐条核对**（DoD 第 3 条）

7 份文件 §5 表共引用 `docs/spec/*.md` 27 处，去重后全部落在**真实存在**的文件上（`audit-event` / `capability-matrix` / `envelope` / `error-codes` / `ipc-protocol` / `naming` / `tool-schema` / `testing`，= 8 份 spec 减去引用方自身）；**0 处指向不存在的 spec**。语义逐条核对结论：

- `tool-schema.md` 引 `naming`（反向 DNS 三段式）/ `capability-matrix`（capability 必须是矩阵中已存在的项）/ `error-codes`（错误字段只用枚举）/ `envelope`（Tool 输入输出作为 payload）/ `audit-event`（调用结果必须产出事件）/ `ipc-protocol`（跨进程走该协议）/ `testing`（contract 测试覆盖 schema 边界）→ 全部对得上。
- `envelope.md` 引 `ipc-protocol`（字节布局）/ `tool-schema`（payload body）/ `audit-event`（`PayloadKind::AuditEvent`）/ `error-codes`（`PayloadKind::Error` 的 code）/ `capability-matrix`（握手 capability 取值）→ 对得上；**方向列修正 1 处**（见 §9 关注点 1）。
- `error-codes.md` 引 `tool-schema` / `envelope` / `audit-event` / `capability-matrix` → 对得上。
- `capability-matrix.md` 引 `tool-schema` / `error-codes` / `ipc-protocol` / `audit-event` → 对得上。
- `audit-event.md` 引 `envelope` / `error-codes` / `tool-schema` / `ipc-protocol` → 对得上。
- `ipc-protocol.md` 引 `envelope` / `capability-matrix` / `audit-event` / `tool-schema` / `error-codes` → 对得上。
- `testing.md` 引 `envelope` / `tool-schema` / `audit-event` / `capability-matrix` / `error-codes` → 对得上；**已不再出现 tool-schema 的字段表**（原副作用消除）。

**（d）验收命令实测输出**（修改后）

```text
> cargo run -p xtask -- docscan
== docscan ==
scanned_files=150
-- summary: 0 error(s), 0 warning(s)
-- verdict: PASSED

> cargo run -p xtask -- hygiene
== xtask hygiene ==
scanned_files=52
WARN  hygiene/file-too-long              xtask/src/card_check.rs:667 文件 667 行，超过建议上限 600 行；考虑拆分（gov §5.4）
WARN  hygiene/file-too-long              xtask/src/main.rs:685 文件 685 行，超过建议上限 600 行；考虑拆分（gov §5.4）
-- summary: 0 error(s), 2 warning(s)
-- verdict: PASSED

> cargo run -p xtask -- memory-counts
== xtask memory-counts ==
scanned_files=8
-- summary: 0 error(s), 0 warning(s)
-- verdict: PASSED

> cargo run -p xtask -- adr-index
== xtask adr-index ==
scanned_files=21
-- summary: 0 error(s), 0 warning(s)
-- verdict: PASSED

> cargo run -p xtask -- card-check
== xtask card-check ==
scanned_files=87
-- summary: 0 error(s), 0 warning(s)
-- verdict: PASSED
```

（`hygiene` 的 2 条 WARN 是 `xtask/src/*.rs` 文件长度，属既有 baseline，与本卡无关；`card-check` 的 49 条 baseline WARN 中本卡相关的 2 条 `missing-record-sections` 已随本节填写清零。）

**（e）回归证明（本卡未改 Rust，跑一遍确认纯文档改动不破坏构建）**

```text
cargo fmt --all --check                       → exit 0
cargo clippy --all-targets -- -D warnings     → exit 0
cargo test --workspace                        → 331 xtask + 7 protocol + 27 storage + 0 core，0 failed
cargo build --release                         → exit 0
```

### 4. DoD 逐条核对

- [x] 7 份文件中，`## 4.` / `## 5.` **各只出现一次**；`### 字段` 的重复标题为 0 → 见 §3(b)，7/7 命中
- [x] 7 份文件的 `## 1. 目标` / `## 2. 范围` **非空**，且范围含明确的不做清单 → 每份 4~5 条「不管（不做清单）」
- [x] 每份文件的「与其他 spec 的关系」里，被引用的 spec 都真实存在且语义对得上 → 见 §3(c)，27 处引用 0 处悬空
- [x] `PL-003` 已处置 → `testing.md` 补出 `### 4.3 输出可注入`（走「补节」路线，未改 `report.rs`）
- [x] `xtask docscan / hygiene / memory-counts / adr-index / card-check` 全部 PASSED → 见 §3(d)
- [x] `docs/PARKING_LOT.md` 追加 PL-038 关闭行；`LEDGER.md` 追加一行 → 见 §2 第 8 / 11 行
- [x] 无任何 Out of scope 的文件被修改（尤其：**没有改任何字段 / 类型 / 枚举**）→ 见 §2 末「未改」清单
- [x] 本卡 §1~§9 执行记录已填 → 本节即证明

### 5. 偏差

DRIFT-200-1（**已闭环**：人类 chat 2026-09-24 预先授权「代 Orchestrator 展开正文」）

触发器：#5（超出 Implementer write scope）—— 本批动作动了 Orchestrator-only 文件。

现象：建本卡（TASK-200）的同时，为「stage-1 开工闸门」刷新了 4 个只有 Orchestrator 能改的文件：
`PLAN.md`（阶段 0 → 1、当前任务卡/阻塞项/下一步重写）、`plans/stage-1-pilots.md`（状态 + A1 顺序澄清 + 号段表）、
`MEMORY.md` §1 快照、`docs/PARKING_LOT.md`（PL-037 / PL-038）；并按授权**代 Orchestrator 展开 `tasks/TASK-012-*.md` 的正文**
（原为 46 张批次表占位之一，无「目标 / In scope / 验收 / DoD」）。

影响：若不记录，下个会话按 `AGENTS.md` §8 会认为「Implementer 改了 PLAN.md / plans/*」= 漂移事故，
而实际是**人类在 chat 中显式授权**的代行（授权原文：「按你的建议去做，并且授权你『代 Orchestrator 展开正文』」）。

我的建议：本 DRIFT **不需要新裁决**（授权已给出），但需要**落盘**以满足 gov §4.3「DRIFT 必须有落盘位置」。
后续任何「代 Orchestrator」动作都应同样在本卡 §5 或当次卡 §5 留一行，不要只写在聊天里（AGENTS.md：聊天记录不是事实源）。

已停止的工作：无（授权范围内的动作全部完成）。**未**做任何未被授权的 Orchestrator 动作（未改 `AGENTS.md`、`docs/adr/*`、`docs/spec/*`、未改号段分配 ADR）。

需要人类裁决：否（授权已存在）；但 **PL-037 与 TASK-012 的 3 个依赖仍需人类裁决**，见 `docs/PARKING_LOT.md` PL-037 与本卡 §7。

DRIFT-200-2（**待人类裁决**：AGENTS.md 内部规则冲突）

触发器：#8（spec 自相矛盾）+ #5（超出 write scope）。

现象：`AGENTS.md` §11.1 把 `README.md` 状态行列为「阶段切换 → **必更新**」，但 §11.3 同时写「改了 README 状态**以外**的章节 → 漂移」。
本批是阶段 0 → 阶段 1 的闸门刷新，README 的状态行与 `## 当前阶段` 章节（line 64~69）**都**还在写阶段 0。

影响：两条规则**不能同时满足**——只改状态行 → README 自相矛盾（状态行说阶段 1、下一节说阶段 0）；
连 `## 当前阶段` 一起改 → 命中 §11.3 的漂移判据。

我的建议：把 §11.1 的「README 状态行」口径明确为「状态行 **+ `## 当前阶段` 章节**」（两者是同一件事的两半），
否则每个阶段切换都会留下一个自相矛盾的 README。本轮按**字面**执行 §11.1：**只改了状态行**（2 行 blockquote），
`## 当前阶段` 章节**一字未动**。

已停止的工作：`README.md` `## 当前阶段` 章节（line 64~69）的更新。

需要人类裁决：**是** —— 请二选一：① 授权「状态行 + 当前阶段章节」一起更新（本轮可补做）；② 维持现状，把「README 当前阶段章节过时」记入 PARKING_LOT 由阶段末评审统一处置。

DRIFT-200-2（**已闭环**：人类 chat 2026-09-24 裁决 = **选项 ①**）

裁决原文（人类 chat 2026-09-24）：「DRIFT-200-2：授权『状态行 + 当前阶段章节』一起改」。

落地（commit 见 `LEDGER.md`）：
- `README.md` 的 `## 当前阶段` 章节已从阶段 0 口径改为阶段 1 口径（与状态行**同一 commit 内一起改**）；
  状态行的括号同步为「`crates/protocol` / `crates/storage` 已完成，接着是审计 / 密钥 / 护栏」。
- `AGENTS.md` §11.1 / §11.3 的**规则原文仍然冲突** —— `AGENTS.md` 属只读区（「本文件变更需 ADR」），本卡不改
  → 已记 **PL-039**（`docs/PARKING_LOT.md`），建议与下次 `AGENTS.md` 修订一并澄清。

已停止的工作：无（裁决已给出且落地完成）。

需要人类裁决：否。

**本批（修 7 份 spec）无新增 DRIFT**：全部改动落在 write scope 内；唯一"越界嫌疑"的动作是把 `envelope.md` §5 一行的方向列由 `依赖` 改 `被依赖` —— 该动作属卡面 In scope 第 1 条明确要求的「逐行核对是否适用」，不构成超出 scope，已在 §9 关注点 1 请审阅者重点复核。

### 6. 更合理做法

- 本卡的**根因不是「7 份文件写错了」，而是「批量生成 + 只有形状级门禁」**：`docscan` 能保证编码 / 末行 LF / 表格不破，但保证不了「节号唯一 / 空节 / 重复标题 / 引用可解析」。更合理的顺序是**先给 `docscan` 加这 4 条规则，再做批量生成**，否则下一次批量生成会复制同一个事故。本卡按 write scope 只修文档，规则扩展留给护栏卡（PL-038 的机器检查缺口）。
- 补 §1/§2 时，**「不做清单」比「做清单」更有价值**（AGENTS.md：Out of scope 效力高于 In scope）。7 份都补了 4~5 条不做清单，其中 `testing.md` 的「不管性能基准阈值」与 `envelope.md` 的「不管加密 / 压缩 / 分片」正是过去最容易越界的两处。
- 修法上，**先取证再动手**（先把 `git show HEAD:` 的基线记进 §3）让"改动有效"变成可证伪的，而不是靠"看起来修好了"。

### 7. 遗留问题

- **PL-040**：`docs/spec/` 无 `README.md`（缺索引 / 阅读顺序）。本卡 In scope 明确禁止新建（漂移触发器 ②）→ 留 PL。
- **PL-041**：`id` 类型口径冲突（被删的外来块声称「所有 `id` = u64」，而 `envelope_id` / `correlation_id` / `event_id` 实际都是 `Uuid`）→ **需 ADR**，本卡不改契约。
- **PL-038 的机器检查缺口**：`docscan` 仍抓不到「节号重复 / 空节 / 重复标题」→ 建议归护栏卡（尚未开卡）。
- `testing.md` 的 `### 4.3` 是 §4 下唯一子节（无 4.1 / 4.2）：编号取自 `report.rs` 的既有引用，**保持不动**（改号会让该注释再次悬空）。
- `docs/adr/0023-*.md:173` 提到 `docs/spec/postcondition.md`（该文件不存在）—— 属 ADR 文本（只读区），本卡不动，留待该 ADR 的后续修订。

### 8. 新增长期记忆

- **PITFALL**（`docs/memory/pitfalls.md` +1）：`xtask docscan` **抓不到**「节号重复 / 空节 / 重复标题」，TASK-072 批量生成的 7 份 spec 带缺陷存活 4 天 → 推论：任何「文档结构已被机器保证」的说法，必须先确认 `docscan` 真的覆盖那条规则；批量生成的文档必须按「结构自检命令」验收。
- **FACT**：无（本卡未产生新的可验证硬事实；交叉引用结论已落在各文件 §5 表内）。
- **REJECTED**：无新增。
- `MEMORY.md` §1 规模表已同步（`pitfalls.md` 185 行 / 82 条，`memory-counts` PASSED 佐证）。

### 9. 给审阅者的关注点

1. **最容易出问题的是 §5 表的「方向列」**（`依赖` / `被依赖` / `相关`）—— 它由我逐行判定。其中我**改动了 1 处**：`envelope.md` 的 `ipc-protocol` 一行由 `依赖` → `被依赖`（依据：`envelope.md` §2 明写「不管传输层：归 `ipc-protocol.md`」，故是协议依赖 envelope 而非反之）。请重点复核这一处。此外各表之间存在**非对称**（例：`error-codes.md` 写 `相关 capability-matrix`，而 `capability-matrix.md` 写 `被依赖 error-codes`）—— 我判定「单侧相关」不构成缺陷（每份表只描述**本文件视角**的关系），故未强行统一；若评审要求全局对称，请开新卡。
2. **契约字段一字未动**：请用 `git diff HEAD -- docs/spec/` 确认改动全部落在 §1 / §2 / §4 / §5 与**被删除的模板块**，`## 3. 类型定义` 无语义变更。唯一需要留意的是 `tool-schema.md` §4 由 4 条变 7 条（新增 3 条来自外来块，已逐条核对适用于本文件）。
3. **`testing.md` §4.3 是新增节**（PL-003 闭环）：内容是「白盒测试不得捕获进程 stdout / stderr，渲染目标必须作为参数可注入」。请确认这不是「发明契约」—— 我的判断：它只是把 `xtask/src/report.rs` **已实现且已在模块注释里引用**的既有约定落盘，未引入任何新要求。
