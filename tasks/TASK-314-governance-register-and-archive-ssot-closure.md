# TASK-314　跨阶段治理登记与归档 SSOT 收口（archive README / 治理池行 / 台账缺口）

- 状态：**Done**（2026-10-18）
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-309 / TASK-312 / TASK-313（均已落地）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`（跨阶段治理卡表）。

---

- **write scope**：
  - `docs/memory/archive/README.md`（L3 归档层说明，ADR-0021 的规则页）
  - `docs/PARKING_LOT.md`（只追加：PL-114 证据回填、PL-116 新提＋闭环、PL-117 / PL-118 新提）
  - `LEDGER.md`（只追加：TASK-313 台账补记行、本卡行）
  - `plans/stage-1-pilots.md`（跨阶段治理卡表新增完成标记行 + 头部「当前进度」块内容）
  - `PLAN.md`「当前状态」块（ADR-0039 D1/D2 的 4 行）
  - `README.md` 三处（状态行 / `## 当前阶段` / `## 最近进展`）
  - `docs/automations/2026-10-17-round-4.md`（本轮留痕）
  - 本卡
- **关联**：ADR-0021（记忆分层与按体积归档）、ADR-0030 D2（派生值不手写两处）、ADR-0039（卡 Done
  ⇒ PLAN / README 同批）、ADR-0041（`plans/*` 完成标记与「当前进度」句）、ADR-0072（派生值指针化）、
  ADR-0037（任务卡号段分配）、TASK-309 记录区 §7、`docs/automations/2026-10-17-round-2.md` §5、
  `docs/automations/2026-10-17-round-3.md` §5

**目标**

把三处「已确认但无人承接」的治理欠账收口：① L3 归档说明页与 `MEMORY.md` §7 抢同一事实；
② 治理池表缺 TASK-312 / TASK-313 的完成标记行；③ TASK-313 连台账行都没有（且全仓无卡文件）。

**现象**

1. `docs/memory/archive/README.md` 第一句写「**当前为空**（还没有任何 L1/L2 文件超过 400 行）」，
   而 `docs/memory/archive/paint-early-probes.md`（429 行）已于 2026-10-17 落地。同一事实
   （归档清单）在 `archive/README.md` 与 `MEMORY.md` §7 里各写一份 —— 正是 ADR-0072 要消除的
   「同一事实手写多处」。TASK-309 记录区 §7 与 round-2 / round-3 的 §5 三次登记「无卡可改」。
2. `plans/stage-1-pilots.md` 的「跨阶段治理卡」表停在 TASK-310 / TASK-311，缺 **TASK-312**
   的完成标记行（ADR-0041 D1：卡 Done 必须在条目行首打完成标记）。**TASK-313 的号连计划行都不能加** ——
   `xtask card-check` 的 `card-check/number-missing-file` 判据要求「`plans/` 引用的每个 TASK-NNN
   必须有 `tasks/TASK-NNN-*.md`」，实测一次 5 个 Error（2026-10-18，本卡首跑）。
3. **TASK-313 的留痕缺口更彻底**：`git log --all --diff-filter=A -- tasks/TASK-313*` 为空
   （从未有过卡文件），`LEDGER.md` 与 `plans/stage-1-pilots.md` 也都没有它的行，只有
   `docs/PARKING_LOT.md` 的 PL-115 行与 round-2 / round-3 的正文提到它。
4. 号段池（ADR-0037 D1：072~099 / 100~199 / 200~299）没有 300+ 号段，而 TASK-310~313 与
   本卡都落在 300+；`plans/stage-1-pilots.md` 号段表的「现状」列仍写「空位 209~299」。

**本卡修法**

1. `docs/memory/archive/README.md`：删掉「当前为空」，改为**指针**——归档清单的唯一落点 =
   `MEMORY.md` §7；本文件只保留归档规则。附一行现场教训（哪次、为什么）。
2. `plans/stage-1-pilots.md`：治理池表补 TASK-312 与 TASK-314 完成标记行；头部「当前进度」
   块内容同步。**TASK-313 的号不进计划表**（理由见「现象」#2），它的缺口只落 PL-117 与台账事实行。
3. `LEDGER.md`：追加 TASK-313 的**事实行**（只记 PR #453 / merge / 改动文件清单，来源是 gh 与
   `git show --stat`）＋ 本卡行。不改写任何既有行（gov §9.2 只追加）。
4. `docs/PARKING_LOT.md`：追加 PL-114 闭环证据回填（PR #447 / merge `39d0835` / CI
   `38986790658`）、PL-116（新提＋闭环）、PL-117（新提，剩余项 Open）、PL-118（新提，Open）。
5. `PLAN.md` 当前状态块 4 行 + `README.md` 三处：按 ADR-0039 D1/D2 同步本卡 Done。

**Out of scope（做了算漂移）**

- 回溯补建 `tasks/TASK-313-*.md` 卡片正文（依据只有一份已合并的 diff，正文该写什么属于
  Orchestrator 派单；本卡只登记 PL-117）。
- 改号段池定义或给 TASK-310~314 改号（ADR-0037 D3：主动改号必须开 DRIFT-ADR）。
- 给 `xtask memory-counts` 增加「归档索引 ↔ archive 目录」规则：ADR-0030 D1 把 `memory-counts`
  的规则集写死为 **8 条**，加第 9 条 = 漂移触发器 ④，须先有 ADR。
- 任何产品代码 / schema / trait / ErrorCode / lint / CI 改动。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-comments
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo deny check
rg -n "当前为空" docs/memory/archive/README.md     # 期望 0 命中
```

**DoD**

- [ ] `docs/memory/archive/README.md` 不再声明「当前为空」，改为指向 `MEMORY.md` §7
- [ ] 治理池表含 TASK-312 / TASK-314 的完成标记行，且**不含**任何没有卡文件的号（`card-check` 零 Error）
- [ ] `LEDGER.md` 有 TASK-313 的事实行（含 PR / merge / 文件清单）与本卡行
- [ ] `docs/PARKING_LOT.md` 有 PL-114 证据回填 + PL-116 / PL-117 / PL-118 行
- [ ] `PLAN.md` 当前状态块 4 行与 `README.md` 三处按 ADR-0039 同步
- [ ] 静态验收全绿（含 `check-ledger` 的日期新鲜度）；9 节记录填写
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-314 跨阶段治理登记与归档 SSOT 收口。
- **write scope**：`docs/memory/archive/README.md`、`docs/PARKING_LOT.md`、`LEDGER.md`、
  `plans/stage-1-pilots.md`、`PLAN.md` 当前状态块、`README.md` 三处、本轮轮次文件、本卡。
- **铁律**：1（无静默失败 —— 三处都是「文档自称与事实相反」）、9（不得静默扩大范围 ——
  不给 `memory-counts` 加规则、不自行改号、不伪造 TASK-313 卡文件）、10（契约先行 ——
  只按既有 ADR 的既有口径收口，不新造机制）。
- **禁止**：产品代码 / schema / trait / ErrorCode / lint / CI；回溯建卡；改号段池。
- **验收**：见正文命令；关键判据 = `rg -n "当前为空" docs/memory/archive/README.md` = 0 命中。
- **依赖**：TASK-309（§7 登记该欠账，InProgress）、TASK-312（Done，PR #447 / `39d0835`）、
  TASK-313（PR #453 / `7b81bfb`，无卡文件）—— 均已在 `LEDGER.md` 与 `origin/main` 核对。
- **疑问**：无（三处欠账的事实与证据都已在仓内；号段池缺口按 PL-118 留给裁决）。

### 2. 实际改动文件

- `docs/memory/archive/README.md`：「当前为空」→「归档清单的唯一落点 = `MEMORY.md` §7」，
  附 TASK-314 / PL-116 的现场说明与 ADR-0072 依据；归档规则段一字未改。
- `plans/stage-1-pilots.md`：跨阶段治理卡表追加 TASK-312 与 TASK-314 两行（**TASK-313 不追加** ——
  `card-check/number-missing-file` 会因缺卡文件判 Error，见 §3 与 §6）；头部「当前进度」块内容
  前置本卡条目（ADR-0041 D1② 允许改「当前进度」块内容）。
- `LEDGER.md`：追加两行 —— TASK-313 台账补记（PR #453 / merge `7b81bfb` / 5 文件 +41 −2）、
  本卡行（Review，PR / CI / merge hash pending）。
- `docs/PARKING_LOT.md`：追加五行 —— PL-114 闭环证据回填、PL-116 新提、PL-116 处置（闭环）、
  PL-117 新提（Open：无卡文件号既不能在计划表登记、也没有卡文件）、PL-118 新提（Open：300+ 号段池缺口）。
- `PLAN.md`：当前状态块「更新日期」改 `2026-10-18` 并把本卡条目前置到「当前阶段」「当前任务卡」两行。
- `README.md`：状态行前置本卡条目；`## 当前阶段` 与 `## 最近进展` 各增一段。
- `docs/automations/2026-10-17-round-4.md`：本轮留痕。
- 本卡。

**工具偏差（如实记）**：`PLAN.md` / `plans/stage-1-pilots.md` 的「当前进度」句与 `README.md`
状态行都是**单行数千字符**，`apply_patch` 的上下文是整行匹配，逐字复现风险高于收益。故这三处
长行改用一次性 Python 脚本做**等长锚点替换**（脚本对每处断言「锚点必须恰好命中 1 次」，不满足即
中止且不写文件），其余改动仍走 `apply_patch`。产物 = `~/.codex/automations/…-0620/apply_edits.py`
（仓库外，不入库）；替换前后的内容见 §3 与 §5。

### 3. 验收输出摘要

```text
python apply_edits.py
patched PLAN.md: +83 chars / +225 chars / +117 chars
patched plans/stage-1-pilots.md: +176 chars / +695 chars
patched README.md: +225 chars
appended 1123 chars to LEDGER.md
appended 2393 chars to docs/PARKING_LOT.md
```

```text
rg -n "当前为空" docs/memory/archive/README.md   -> 0 命中（改动前 1 命中）
rg -n "MEMORY.md" docs/memory/archive/README.md -> 指针命中
```

```text
# 首跑 card-check：计划表里加了「—（无卡文件）」的 TASK-313 行 → 被机器判据拦住
ERROR card-check/number-missing-file  plans/stage-1-pilots.md:24  plans/ 引用 TASK-313，但 `tasks/TASK-313-*.md` 不存在
ERROR card-check/number-missing-file  plans/stage-1-pilots.md:415 plans/ 引用 TASK-313，但 `tasks/TASK-313-*.md` 不存在
ERROR card-check/number-missing-file  plans/stage-1-pilots.md:416 plans/ 引用 TASK-313，但 `tasks/TASK-313-*.md` 不存在
-- summary: 5 error(s), 34 warning(s)   -- verdict: FAILED

# 按门禁要求撤掉该行后
-- summary: 0 error(s), 34 warning(s)   -- verdict: PASSED
```

结论（写进 PL-117）：**先有卡文件、再有计划行** —— 计划表不能用来「占位登记」一个没有卡文件的号。

静态验收（本卡提交前实跑，原始输出见 `docs/automations/2026-10-17-round-4.md` §4）：
`cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` /
`cargo test --workspace` / `cargo test -p assistant-core arch::` /
`verify-schemas` / `codegen --check` / `hygiene` / `memory-counts` / `adr-index` /
`check-comments` / `refscan` / `docscan` / `card-check` / `check-ledger` /
`check-migrations` / `cargo deny check` 全部通过。

### 4. DoD 逐条核对

- [x] `archive/README.md` 不再声明「当前为空」，改为指向 `MEMORY.md` §7
- [x] 治理池表含 TASK-312 / TASK-314 完成标记行，且 `card-check` 0 Error（TASK-313 号不入计划表）
- [x] `LEDGER.md` 有 TASK-313 事实行与本卡行
- [x] `docs/PARKING_LOT.md` 有 PL-114 证据回填 + PL-116 / PL-117 / PL-118
- [x] `PLAN.md` 当前状态块与 `README.md` 三处同步（`check-ledger` PASSED 为机器证据）
- [x] 静态验收全绿；9 节记录填写
- [x] 无 Out of scope 文件被修改（改动清单见 §2，全部在 write scope 内）

### 5. 偏差

- **NONE**（三处欠账与号段缺口都按仓内证据处理；无放宽、无静默改判）。
- 记一次**工具层面**的偏差：长行用 Python 做锚点替换（理由与安全约束见 §2）。这不是契约偏差，
  但如果评审认为「文档长行只能手改」是硬约束，请把 `plans/*` 的「当前进度」句与 `README.md`
  状态行拆成多行——那是另一张卡（本卡不顺手重构）。

### 6. 更合理做法

- **① 归档清单不该有第二份**：本卡按 ADR-0072 把它改成指针，但这只是止住了这一次。
  真正的机器保障是「归档目录 ↔ `MEMORY.md` §7 索引」的一致性检查 —— 归 `memory-counts` 最自然，
  可惜 ADR-0030 D1 把它的规则集写死为 8 条（加第 9 条 = 漂移触发器 ④）。若人类认可，
  正确路径是「ADR 扩 rule set」而不是让 agent 顺手加。
- **②「卡号 → 留痕」的机器判据**：本次实测说明 **`plans/` → 卡文件**这一半**已经有**机器判据
  （`card-check/number-missing-file`）—— 它当场拦住了我第一版「在计划表里给无卡号的 TASK-313
  占位」的写法，这是它正确的行为（先有卡文件、再有计划行）。缺的是另一半：**卡文件与台账行的
  存在性**没有互检（`adr_index` 有 11 条交叉规则，`card-check` 目前只查计划表引用），所以
  TASK-313 这种「有 PR、有卡号、无卡文件、无台账行」只能靠人读 `git log` 发现。加规则同样是
  新规则 → 需 ADR。
- **③ 号段池要跟着实际用号走**：`plans/stage-1-pilots.md` 的「现状」列是手抄的派生值，
  224~313 的占用早已超出它的描述（同 PL-022 的病）。要么指针化（指向 `dir /b tasks`），
  要么让 `card-check` 算出来。

### 7. 遗留问题

- **PL-117（Open）**：TASK-313 的卡文件是否补建 —— 需要 Orchestrator 派单（证据只有一份已合并
  diff，卡片「目标 / write scope / 验收 / DoD」现在补写等于事后编造）。本卡只补了**台账事实行**；
  **计划行补不了**：`card-check/number-missing-file` 要求被引用的号必须有卡文件（门禁表现得对）。
- **PL-118（Open）**：300+ 号段未在 ADR-0037 的池定义里，且号段表「现状」列已陈旧；需 ADR 或
  Orchestrator 裁决（新增 300~399 池 or 明确 300+ 历史用号）。
- **TASK-309 仍未 Done**：`paint.tool.select` handler 扩展与真机端到端需要真实 GUI，保持
  `InProgress`；`fill_tool_button` / 颜色选取器缺非本地化锚点，`大小` / 轮廓 / 填充三个 flyout
  按钮本身无 aid。
- **PL-113 / PL-116 之外的 GUI 批次**：TASK-044 十次真机运行、TASK-045~057、TASK-256 / 257
  真机验收仍全部跳过（无人值守不能切前台窗口）。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED` 条目：本卡的三处结论都是「仓内已有登记 + 本卡改正」，
  没有新的可复用事实；号段池与归档索引的机器化建议已写进 §6 与 PL-117 / PL-118，不重复落 L1。

### 9. 给审阅者的关注点

- `docs/memory/archive/README.md` 只改了第一句与新增的一段说明，**归档规则四条一字未动** ——
  请核对「指针 + 规则」的分工是否与 ADR-0021 / `docs/memory/README.md` 一致。
- `LEDGER.md` 里 TASK-313 那一行是**补记**（该卡此前完全没有台账行），我把证据来源写死为
  `gh pr view 453` 与 `git show --stat 7b81bfb`；请确认这种「事后补记事实」符合 gov §9.2 的
  只追加口径（我没有改动任何既有行）。
- `plans/stage-1-pilots.md` 新增三行只带**完成标记**与状态，未动任何既有条目的正文 / 排期
  （ADR-0041 D2 / D3）。
