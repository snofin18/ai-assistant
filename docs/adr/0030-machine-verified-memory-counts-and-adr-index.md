# ADR-0030　把「记忆规模计数」与「ADR 编号登记表」从手工维护改为机器校验

状态：**Accepted**（2026-09-18，人类指示 #4）　日期：2026-09-18　Supersedes：—（但**取代** ADR-0026 D1 的「登记表由人工维护」与 PL-030 的「建议」状态；并**叠加 supersede** ADR-0025 CI 门禁计数口径 = 17 行清单 ↔ 16 步骤 = 7 硬 + 9 软 → 本 ADR §1 决策 1 改为 18 行 ↔ 17 步骤 = 8 硬 + 9 软；2026-09-20 由 TASK-068 显式登记）　Superseded by：—

来源：2026-09-18 人类会话。原话：「**「MEMORY.md 的计数又过期了 —— PL-022 的现场复现」，修复，避免今后再发生。**」
关联：`MEMORY.md`「各文件当前规模」表、`docs/adr/README.md`（ADR-0026 D1 建立的编号登记表）、
`docs/memory/decisions.md`、`xtask/src/memory_counts.rs` / `adr_index.rs`（本 ADR 新增）、
`.github/workflows/ci.yml`（新增 `[HARD #12b] doc-consistency`）、`docs/governance-ai-agent-execution.md` §5.1（新增 #12b 行）、
`docs/adr/0019-hard-gate-negative-verification.md`（负向验证登记表追加一行）、
`docs/adr/0021-memory-layering-and-app-profiles.md`（150 行硬上限）、`docs/adr/0025-hygiene-rule-count-unification.md`（计数口径 SSOT 的先例）、
`docs/PARKING_LOT.md` PL-022 / PL-028 / PL-030 / PL-031

---

## 背景（为什么现在要决定）

### 事故一：MEMORY.md 的计数第二次过期（PL-022 的现场复现）

`MEMORY.md` 的「各文件当前规模」表手写了 6 个文件的**行数**与**条目数**，正文的「迁移核对」引文块里
又手写了 **155 / 200 / 7 / 207** 四个合计数字。这些数字**全部是派生值** —— 它们可以从
`docs/memory/*.md` 直接算出来，却被抄进了文档。

于是每次往 L1 追加条目，都必须记得手工改 MEMORY.md 的 7~10 个数字。**已经失败两次**：

| 时间 | 症状 | 怎么发现的 |
|---|---|---|
| 2026-09-18 上午 | ADR-0021 分层迁移后，规模表的行数/条目数与实测不符 | 人工逐行核对 |
| 2026-09-18 下午（commit `b829e67`） | ADR-0026 落地后，4 处计数与清单过期 | 又一次人工逐行核对 |

第二次是**在刚刚修完第一次之后**发生的。这说明问题不在「不够仔细」，而在**结构**：
同一个事实手写多处（SSOT 违规），就必然会漂移；靠人工核对来兜，等于把护栏建在注意力上。

**危害不只是数字难看**：MEMORY.md 的规模表是「该读多少」的路由依据（`AGENTS.md` §3 启动流程第 ⑤ 步）。
条目数偏小 → agent 以为文件很短而全量读，浪费上下文；偏大 → 以为可以只 grep，漏读关键条目。
更糟的是 `docs/memory/archive/` 的归档触发条件是「任一文件 > 400 行」—— **行数过期就意味着归档永远不会被触发**。

### 事故二：ADR 编号 0019 被双重占用（ADR-0026）

`docs/adr/README.md` 的编号登记表目前是**纯人工维护**：新建 ADR 文件后要记得回填 §1 表，
预留待建号要记得回填 §2 表，「下一个可用编号」要记得手工 +1。0019 号事故的直接成因就是
「建文件时没查预留表」。ADR-0026 用**登记表 + 规程**（新建前必查、新建后必回填）来防复发，
但规程和 MEMORY.md 的计数一样，是**建在注意力上的护栏**。

PL-030 已经提出「给 xtask 加 `adr-index` 把登记表变成机器校验」，但当时被归到 TASK-015 延后。
人类指示 #4 的「避免今后再发生」把优先级提上来了：**同类根因（手工维护派生事实）已经造成两次真实事故，
不应再等一张未来的卡**。

### 为什么用 `xtask` 而不是「写文档时更小心」

| 方案 | 能否防复发 |
|---|---|
| 在 `AGENTS.md` 里加一条「改记忆文件必须同步计数」 | ❌ 已经加过等价约束（MEMORY.md「更新职责」明写 Orchestrator 维护规模表），仍然失败两次 |
| 每次人工核对 | ❌ 就是当前做法；成本随文件数线性增长，且核对本身也会看错 |
| **机器校验 + CI 硬门禁** | ✅ 派生值由工具算，人只负责把工具给的数字粘进去；不一致立刻红灯 |

`xtask` 是既有的、零第三方依赖的、已接进 CI 与所有文档路由表的护栏工具（ADR-0028 选项 3 已论证过复用它的理由），
本 ADR 沿用同一判断。

## 决策（一句话）

**给 `xtask` 增加两个只读子命令 `memory-counts` 与 `adr-index`，把「记忆规模计数」与「ADR 编号登记表」
这两处派生事实改为机器校验；同时删除 MEMORY.md 正文里的派生合计数字（消除重复书写，而不是校验重复书写）；
CI 新增 `[HARD #12b] doc-consistency`，gov §5.1 按 8b 先例增加 #12b 子编号行。**

拆成六条：

- **D1　`xtask memory-counts`**（只读，退出码沿用现有约定：0 通过 / 1 有 Error）。
  扫描对象 = `docs/memory/` 下的 `facts.md` / `pitfalls.md` / `rejected.md` / `decisions.md` / `open.md`
  ＋ `docs/memory/apps/*.md`（**自动发现**，新增应用档案不需要改代码）。
  与 `MEMORY.md`「各文件当前规模」表逐格比对，8 条规则：

  | 规则 id | 级别 | 判据 |
  |---|---|---|
  | `memory/file-missing` | Error | 表里登记的路径在磁盘上不存在 |
  | `memory/file-unlisted` | Error | 扫描集里的文件没有出现在表中（漏登记） |
  | `memory/line-count-mismatch` | Error | 表中行数 ≠ 实测行数 |
  | `memory/entry-count-mismatch` | Error | 表中条目数 ≠ 实测条目数 |
  | `memory/index-too-long` | Error | `MEMORY.md` 自身行数 > **150**（ADR-0021 硬上限，不得提高） |
  | `memory/derived-total-in-prose` | Error | `MEMORY.md` 正文出现「现合计」这类**会随追加漂移的派生合计**（见 D2） |
  | `memory/scale-table-unparsable` | Error | 「各文件当前规模」表缺失或表头/列数不合规 → **必须失败**而不是静默当作一致（铁律 1）；消息里写明期望的表头形态 |
  | `memory/archive-threshold` | Warning | 任一被扫描文件实测行数 > **400** → 触发 `docs/memory/archive/` 的归档规则（ADR-0021）。此前该触发条件因行数手写过期而**永远不会被触发**，故一并机器化 |

  **计数判据（必须精确定义，否则工具自己就成了漂移源）**：
  - **行数** = 文件内容按 `\n` 切分后的行元素个数；文件以恰好一个 `\n` 结尾时即等于常识行数；空文件 = 0。
  - **条目数** = 行首（**不允许缩进**）匹配 `- [YYYY-MM-DD]` 形态的行数，即
    以 `- [` 开头、紧接 4 位数字 `-` 2 位数字 `-` 2 位数字、再紧接 `]`。
    这与 `MEMORY.md`「条目格式」一节的规定完全一致，不另立判据。
  - **表解析** = 定位 `## 各文件当前规模` 之后第一张 Markdown 表，取每行第 1（路径，去反引号）、
    2（行数）、3（条目数）列；路径相对于 `docs/memory/`。

- **D2　消除重复书写（这是「避免今后再发生」的关键，比校验更重要）**：
  `MEMORY.md`「迁移核对」引文块里的 **155 / 200 / 7 / 207** 四个数字，性质不同，必须分开处理：
  - **155 / 200 / 7** 是**历史事实**（2026-09-18 迁移当时的值，不随后续追加变化）→ **保留**，
    但把该块显式标注为「历史快照（数字为当时值，不随后续追加变化）」，消除「它是当前值」的误读；
  - **207（「L1 现合计 207 条」）是派生的当前值** → **删除**，改为指向规模表：
    「L1 合计条目数见上表，由 `xtask memory-counts` 校验」。
  规则 `memory/derived-total-in-prose` 禁止「现合计」字样复发。
  **原则**：能让派生值只存在一处，就不要让它存在两处再去校验两处。

- **D3　`xtask adr-index`**（只读）。三方交叉校验 `docs/adr/README.md`（登记表）↔
  `docs/adr/NNNN-*.md`（文件集）↔ `docs/memory/decisions.md`（待建条目），11 条规则：

  | 规则 id | 级别 | 判据 |
  |---|---|---|
  | `adr/file-not-in-registry` | Error | 存在 `NNNN-*.md` 文件但 §1 表无此行 |
  | `adr/registry-row-without-file` | Error | §1 表登记了某号但磁盘无对应文件 |
  | `adr/pending-not-in-decisions` | Error | §2 待建表的号在 `decisions.md` 找不到 `[ADR:待建 NNNN]` |
  | `adr/decisions-not-in-pending` | Error | `decisions.md` 的待建号（减去退役清单）不在 §2 表 |
  | `adr/number-collision` | Error | §1 表 ∩ §2 表 ≠ ∅ —— **这就是 0019 事故的机器判据** |
  | `adr/retired-number-reallocated` | Error | 退役清单里的号又出现在 §1 或 §2 表（**退役号永久不复用**） |
  | `adr/next-number-wrong` | Error | 「下一个可用编号」≠ max(全部已用号) + 1 |
  | `adr/missing-status-line` | Error | ADR 文件找不到 `状态：` 行 |
  | `adr/status-mismatch` | Error | §1 表状态列的首个关键词 ≠ 文件状态行的状态关键词 |
  | `adr/superseded-not-marked` | Error | 文件状态行的 `Superseded by：` 非 `—`，但 §1 表状态列未标 Superseded |
  | `adr/registry-section-missing` | Error | 登记表缺少四节锚点之一（§1 / §2 / §3 / `已退役编号：`）→ 锚点缺失时工具**无法**证明一致性，必须失败 |

  **退役清单（新增的机器可读约定）**：`docs/adr/README.md` §3 必须含一行以 `已退役编号：` 开头、
  后接逗号分隔的 4 位编号（无退役号时写 `已退役编号：（无）`）。
  为什么需要它：`decisions.md` 是**只追加**的，0019 那条原始待建条目**永远留在文件里**
  （ADR-0026 D2 明确「原条目不改写」）。若不排除退役号，`adr/decisions-not-in-pending` 会永久报错。
  退役清单把这个「历史遗留」显式化，而不是让工具去猜测哪条被 supersedes 了。

- **D4　裸引用检查（PL-028）本轮不实现**，登记为 **PL-032**，归 TASK-015。
  理由（这是刻意的，不是遗漏）：现存 2 处违规（`0021-*.md:4` 的 `ADR-0017`、`0023-*.md:179` 的 `ADR-0016`）
  **都在 ADR 文件正文里**，而 ADR「只增不改」→ 实现了这条规则就是一条**永久红灯**，
  而永久红灯会让人学会忽略 CI（`ci.yml` 头部注释已把这条写成设计原则）。
  正确顺序是：先决定豁免机制（类似 D3 的退役清单），再实现规则。

- **D5　CI 与 gov 集成（按 8b 先例，不重编号）**：
  - `ci.yml` 新增独立 job `doc-consistency`，步骤名 `[HARD #12b] xtask memory-counts + adr-index`；
    头部注释「硬门禁 7 项」→ **8 项**，并说明 12b 的来源是本 ADR。
  - `gov §5.1` 表格新增 **#12b** 行（**子编号**，与 8b 同理：保持 16 行主编号稳定，
    因为主编号被 `tasks/TASK-001-repo-skeleton.md`、`xtask/src/deferred.rs`、`ci.yml` 与
    `plans/stage-1-pilots.md` 交叉引用）；表后说明段与 §10 落地清单的
    「17 行清单 ↔ 16 个步骤（7 硬 + 9 软）」相应更新为「**18 行清单 ↔ 17 个步骤（8 硬 + 9 软）**」。
  - **为什么不放进 #12（hygiene）**：`hygiene` 的规则总数已被 ADR-0025 钉死为 **13**（gov §5.4 表格行数为 SSOT）。
    把这 19 条文档一致性规则塞进 hygiene，会立刻作废 ADR-0025 的口径并牵连 `deferred.rs` 的数量自洽测试。
    它们检查的对象也不同：hygiene 查**源码形态**，doc-consistency 查**文档之间的交叉一致性**。
  - **负向验证（ADR-0019 元门禁）**：形式 **N1** —— `memory_counts.rs` / `adr_index.rs` 的单测里
    每条规则都有「不一致 → 必须产生 Error」的负向用例；在 `0019` 的登记表追加 #12b 一行。

- **D6　两个子命令都是纯函数 + IO 集中在 `main.rs`**（沿用本 crate 既有分层不变量）：
  - `memory_counts::check(index_text, measured_files) -> Vec<Finding>`
  - `adr_index::check(registry_text, adr_files, decisions_text) -> Vec<Finding>`
  读文件、遍历目录、渲染、退出码全在 `main.rs`。这样每条规则都能用字符串字面量做白盒测试，
  **不需要临时目录**（这正是 `hygiene.rs` 能覆盖 13 条规则的原因）。
  `xtask` 不变量 1（零第三方依赖）与不变量 3（只读）保持不变 —— 本 ADR 不新增任何写操作。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 让工具**自动改写** MEMORY.md 的规模表 | ❌ 否决 | 违反 xtask 不变量 3（只读）；且「工具写文档」会让 `guard`（ADR-0028）的锁语义复杂化。让工具**打印可直接粘贴的正确行**，人来粘贴，成本极低且保留审阅 |
| 2 | 把规模表整段删掉，让 agent 自己 `wc -l` | ❌ 否决 | 表的价值不只是数字，还有第 4 列「读法」（grep 优先 / 全量读）。删掉数字等于删掉路由信息 |
| 3 | 用 git pre-commit hook 校验 | ❌ 否决 | hook 不随 clone 传播（本项目未来要开源，外部贡献者拿不到）；CI 才是唯一对所有人生效的位置 |
| 4 | 写一个 Python 脚本放 `tools/` | ❌ 否决 | `tools/` 顶层目录尚未进 ADR 白名单；与 Rust-first 栈相悖；xtask 已有覆盖全部分支的白盒测试与 CI 接线（同 ADR-0028 选项 3） |
| 5 | 把 19 条规则并入 `hygiene`（#12） | ❌ 否决 | 见 D5：会作废 ADR-0025 钉死的 13 项口径，并牵连 `deferred.rs` 的数量自洽测试 |
| 6 | **两个独立子命令 + `[HARD #12b]` 子编号 + 删除派生合计** | ✅ 采纳 | 与 8b 先例同构；不动任何既有口径；D2 从根上消除重复书写，D1/D3 兜住「仍然手写的部分」 |

## 影响（需要改的文档 / 代码）

| 对象 | 改动 |
|---|---|
| `xtask/src/memory_table.rs` | 新建：规模表解析 + 行数/条目数判据（D1「计数判据」的唯一落点）+ 17 个白盒测试 |
| `xtask/src/memory_counts.rs` | 新建：**8** 条规则的纯函数 + 14 个白盒测试（每条规则至少 1 正 1 负） |
| `xtask/src/adr_registry.rs` | 新建：登记表 / ADR 状态行 / 待建号解析 + 15 个白盒测试 |
| `xtask/src/adr_index.rs` | 新建：**11** 条规则的纯函数 + 18 个白盒测试 |
| `xtask/src/doccheck.rs` | 新建：两个子命令的 IO 边界（读文件、递归列目录、渲染、退出码）；判定逻辑不在这里 |
| `xtask/src/adr_index_tests.rs` | 新建：`adr_index` 的私有单测用 `#[path]` 外置（gov §5.4 单文件 600 行硬上限）；`guard` / `guard_runner` 同法外置，见 ADR-0028 |
| `xtask/src/cli.rs` | `USAGE` 增两个子命令与退出码说明；`parse_args` 不变（这两个子命令不接受操作数） |
| `xtask/src/main.rs` | 模块表 + 分派 + IO；退出码沿用 0/1/2/4 |
| `xtask/README.md` | 增两节（用途、判据、如何修正） |
| `docs/governance-ai-agent-execution.md` §5.1 / §10 | 新增 #12b 行；口径 17→18 行、16→17 步骤、7→8 硬 |
| `.github/workflows/ci.yml` | 新增 `doc-consistency` job；头部注释 7→8 |
| `docs/adr/0019-*.md` | 负向验证登记表追加 #12b 一行（该 ADR 自身规则授权追加） |
| `docs/adr/README.md` | §3 增「已退役编号：0019」机器可读行；§1/§2 表补 0028/0029/0030；下一可用号 → 0031 |
| `MEMORY.md` | 按 D2 改写「迁移核对」块；规模表数字以工具输出为准 |
| `docs/PARKING_LOT.md` | PL-022 / PL-030 关闭；新增 PL-032（裸引用检查归 TASK-015） |
| `LEDGER.md` / `docs/memory/*.md` | 追加事件行与 FACT/DECISION 条目 |
| 产品代码 / `docs/spec/*` / schema / 任务卡 | **不改**（工程元层，零产品影响 —— ADR-0029 D5） |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| **判据本身写错** → 工具天天报假错，人类学会忽略它 | 判据在 D1/D3 里以「精确到字符」的方式写死；每条规则必须有负向单测；上线前先对本仓库跑一次，**结果必须为 0 Error**（若不为 0，先判断是文档错还是判据错，并在 LEDGER 记明） |
| 表格解析脆弱（有人改了表头文字就失效） | 定位锚点用 `## 各文件当前规模` / `## 1.` / `## 2.` 这类标题；解析不到表 → **Error（不是静默通过）**，消息里写明期望的表头形态（铁律 1） |
| `apps/*.md` 自动发现 → 新增档案当天 CI 就红 | 这是**期望行为**：`memory/file-unlisted` 的报错消息直接给出应粘贴的表格行，修复成本 = 一次复制粘贴 |
| 归档触发（>400 行）此前因行数过期而失效 | `memory/line-count-mismatch` 修好后，规模表的行数即真实值；本 ADR 额外要求工具在行数 > 400 时输出 **Warning**（`memory/archive-threshold`，见 D1 表），把归档触发也机器化 |
| 退役清单被滥用成「把不想修的违规塞进去」 | 退役清单只能由 ADR 变更（它是 `docs/adr/README.md` 的一部分，而 ADR 编号分配变更须走 ADR —— ADR-0026 D1）；且 `adr/retired-number-reallocated` 会阻止退役号被复用 |
| 新增 CI job 拖慢流水线 | 该 job 只跑 `cargo run -p xtask`（复用 `rust-cache`），实测秒级；且它是**独立 job**，失败不阻塞其他 job 的诊断信息 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo run -p xtask -- memory-counts` 对本仓库 → **0 Error**，且打印 6 个文件的实测行数/条目数。
2. **负向实证（N1）**：把 `MEMORY.md` 规模表里 `facts.md` 的行数改成 `1` → 工具退出码 **1**
   且输出含 `memory/line-count-mismatch` 与正确值；改回后重新为 0。
3. `cargo run -p xtask -- adr-index` → **0 Error**；§1 表编号集合 == `docs/adr/NNNN-*.md` 文件集合。
4. **碰撞实证**：往 §2 表临时加一行 `0019` → 退出码 **1** 且输出含 `adr/number-collision`
   （证明 0019 那类事故今后会被机器拦住）。
5. `MEMORY.md` 中不再出现「现合计」；规模表仍是唯一的当前计数落点。
6. `cargo test --workspace` 全绿（本 ADR 新增 **64** 个测试：`memory_table` 17 + `memory_counts` 14 + `adr_registry` 15 + `adr_index` 18）；`cargo clippy --all-targets -- -D warnings` 无告警；
   `xtask hygiene` 仍 PASSED（新文件 < 600 行、公共 API 全注释、无裸 `unwrap`）。
7. CI 的 `doc-consistency` job 首次运行即绿；`ci.yml` 头部注释与 gov §5.1 的行数口径一致。
8. **重新评估触发条件**：① 若将来 `MEMORY.md` 的规模表改为由 `codegen` 自动生成（TASK-011 之后 xtask
   获得写权限），D1 的「打印正确行让人粘贴」应改为「直接写回 + `--check` 模式」；
   ② 若 `docs/memory/` 文件数超过 15 个，规模表本身会成为阅读负担，应改为「工具按需查询」而不是「表」；
   ③ 若 ADR 数量超过 50，`adr-index` 应增加「Supersedes 链条闭环」检查（当前只查单向标注）。
