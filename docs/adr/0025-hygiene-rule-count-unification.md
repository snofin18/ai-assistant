# ADR-0025　仓库卫生规则口径统一：gov §5.4 由 11 项 → **13 项**（PL-001 / PL-011 / PL-020 一并裁决）

状态：Accepted　日期：2026-09-18　Supersedes：—　Superseded by：—
关联：`docs/governance-ai-agent-execution.md` §5.4、`xtask/src/deferred.rs`、
`tasks/TASK-001-repo-skeleton.md` §5.1/§5.2、`plans/stage-0-spikes.md`、`plans/stage-1-pilots.md`、
`README.md`、`docs/PARKING_LOT.md` PL-001 / PL-011 / PL-020、人类指示 #12（2026-09-18「其他按最佳方案实行」）

## 背景（为什么现在要决定）

三条停车位条目其实是**同一个问题的三个面**，一直悬着没裁决，导致仓库里同一件事有**两套数字**：

| 条目 | 说的什么 |
|---|---|
| **PL-001** | gov §5.4 的表格实际是 **11 行**，但 `plans/stage-1-pilots.md` 的 TASK-015 DoD 写「hygiene **12** 项检查全部生效」→ 数字不一致 |
| **PL-011** | 建议增加第 12 项：**文件不得含 CRLF**（`hygiene/crlf-line-endings`，Error 级）。真实踩到：编辑脚本按旧约定写回 CRLF，`.gitattributes` 只管入库形态、**管不住工作区**，而 `cargo fmt --check` 会在 Linux runner 上因此变红 |
| **PL-020** | 建议增加：**文本文件必须以单个 `\n` 结尾**（`hygiene/missing-final-newline`）。真实踩到：`docs/DEPENDENCIES.md` 末行无换行，导致以「整行 + `\n`」为锚点的编辑脚本 `count==0` 断言失败，**而报错信息与真因毫无关系** |

数字口径不一致的代价是**具体的**：`xtask` 有一条单测把
`IMPLEMENTED + DEFERRED.len() == TOTAL` 锁死（不变量 1），而 `TOTAL_HYGIENE_RULE_COUNT`
硬编码为 11；`plans/*`、任务卡、`README.md`、`cli.rs` 的帮助文本里散落着 `3/11`。
只要 gov §5.4 加行而不同步这 6 处，就会出现「文档说 13 项、工具说 11 项」——
这正是 ADR-0019 要消灭的那类"护栏与事实脱钩"。

## 决策

**gov §5.4 的规则总数定为 13 项**：原 11 项 + PL-011（禁 CRLF）+ PL-020（必须末行换行）。
`plans/stage-1-pilots.md` 里 TASK-015 DoD 的「12 项」是**笔误**，以 13 为准。
**PL-001 / PL-011 / PL-020 三条一并关闭。**

### D1　两条新规则的定义

| # | 规则 id | 检查什么 | 级别 | 归属 |
|---|---|---|---|---|
| 12 | `hygiene/crlf-line-endings` | 任何**文本**文件的字节里不得出现 `\r` | **Error** | TASK-015 |
| 13 | `hygiene/missing-final-newline` | 任何**文本**文件必须以**单个** `\n` 结尾（不得无换行、不得以空行结尾） | 先 **Warning**，清扫完成后升 **Error** | TASK-015 |

**为什么第 13 条要先 Warning**：全仓库扫描（2026-09-17，排除 `.git`/`target`）发现
**17 个文件末尾缺换行**，其中 2 个当时已补齐，**其余 15 个是 TASK-001 的既有文件**
（`.gitattributes`、`.gitignore`、`Cargo.toml`、`CLAUDE.md`、`clippy.toml`、`deny.toml`、
`LICENSE`、`README.md`、`rust-toolchain.toml`、`rustfmt.toml`、`ci.yml`、
`overnight-automation-charter.md`、`tasks/TASK-001-repo-skeleton.md`、
`xtask/Cargo.toml`、`xtask/README.md`）。
规则一上线就是 Error 会**当场红 15 处**，而修它们属于 drive-by（gov §3.3 禁止）。
→ 正确顺序：**规则以 Warning 上线 → TASK-015 一次性清扫这批文件 → 升为 Error**。

> **[2026-09-18 当日重新扫描，清扫数量从 15 降到 11]** 本 ADR 落地过程中的文档编辑
> （锚点式补丁工具会强制补上末行换行）**顺手修好了其中 5 个**（`README.md`、`ci.yml`、
> `overnight-automation-charter.md`、`tasks/TASK-001-repo-skeleton.md`、`xtask/README.md`）。
> 另发现本规则还有**第二种违反形态**：文件以 `

` 结尾（多一个空行），全仓 3 个，
> 其中 2 个（`architecture-v2.md`、`gov`）因本次已在编辑已修，剩 `target-apps-feasibility.md` 1 个。
> **→ TASK-015 实际需清扫 11 个文件：10 个缺末行换行 + 1 个末尾多空行**（完整清单见 PL-027）。
> 上方列的 15 个文件名**保留为 2026-09-17 的扫描历史**，不再代表当前状态。
第 12 条不需要这个过渡：仓库当前 CRLF 计数为 0（`.gitattributes` + 编辑脚本都已强制 LF）。

**「文本文件」的判定**：按**扩展名白名单**（`.rs .toml .md .yml .yaml .json .ps1 .sh .ts .tsx`
＋无扩展名的 `.gitignore` `.gitattributes` `LICENSE`），而不是靠"内容是否可打印"的启发式探测。
理由：启发式会把二进制 fixture（将来 `fixtures/apps/*` 的 UIA 树快照可能是 `.bin`/`.zst`）
误判成文本，产生**无法消除的永久噪声** —— 与 ADR-0024 D2「不查 bans 以免得到永久噪声」同一逻辑。

### D2　`xtask` 的登记变更（**本 ADR 即授权**）

| 文件 | 改动 |
|---|---|
| `xtask/src/deferred.rs` | `TOTAL_HYGIENE_RULE_COUNT` 11 → **13**；`DEFERRED_HYGIENE_RULES` 增 **2** 条（8 → **10**）；单测断言「未实现 8 项」→「未实现 **10** 项」；模块注释同步 |
| `xtask/src/hygiene.rs` | 模块头注释「11 项中的 3 项；其余 8 项」→「**13** 项中的 3 项；其余 **10** 项」 |
| `xtask/src/main.rs` | 注释「避免 PASSED 被误读成"全部 11 项都过了"」→ **13** |
| `xtask/src/cli.rs` | 帮助文本「当前实现 3/11 项」→ **3/13** |
| `xtask/README.md` | 职责节「`gov §5.4 的 11 项中已实现 3 项`」→ **13 项**。⚠️ 本行是 **2026-09-18 补全**：首轮只盘了 `src/` 下的 4 个文件，漏了同目录的 README —— 正是 PL-022「同一数字手写两处」的又一个实例 |

**`IMPLEMENTED_HYGIENE_RULE_COUNT` 保持 3 不变** —— 本 ADR 只统一**口径**，
不实现任何新规则（实现归 TASK-015）。不变量 1 因此仍然成立：`3 + 10 == 13`，
由既有单测 `test_hygiene_rule_counts_are_consistent` 继续锁死。

### D3　文档侧的同步清单（防止"文档说 13、工具说 11"）

`tasks/TASK-001-repo-skeleton.md`（§5.1/§5.2 与卡面共 5 处：`3/11`、`8 项`、`11 项`、`3/12`）、
`plans/stage-0-spikes.md`（3 处，含 `-- deferred-rules` 的**期望输出行**）、
`plans/stage-1-pilots.md`（TASK-015 DoD 的「12 项」→「13 项」）、`README.md`（1 处）。

**2026-09-18 补全（首轮漏盘的 7 处，已全部改完）**：本 ADR 落地后做了一次全仓正则扫描
（`1[124] ?项` / `3/1[123]` / `从 8 条` × 行内含 `hygiene|门禁|卫生规则|deferred`），又找出 **7** 处：

| 文件 | 原文 | 改为 |
|---|---|---|
| `xtask/README.md` §职责 | `gov §5.4 的 11 项中已实现 3 项` | **13** 项（已列入 D2 表） |
| `docs/governance-ai-agent-execution.md` §10 清单 | `CI：§5.1 的 14 项门禁` | 17 行清单 ↔ 16 个步骤（7 硬 + 9 软） |
| `cross-platform-ai-assistant-architecture-v2.md` 阶段 1 范围行 | `仓库骨架与 CI 14 项门禁` | 同上 |
| `cross-platform-ai-assistant-architecture-v2.md` 治理对照表 | `14 项 CI 门禁（含 arch test …）` | 同上 |
| `docs/overnight-automation-charter.md` W2 验收 | `未实现规则应从 8 条降到 5 条` | 从 **10** 条降到 **7** 条 |
| `docs/overnight-automation-charter.md` W2 重叠说明 | `TASK-015 的验收含 hygiene 12 项` | hygiene **13** 项 |
| `plans/stage-1-pilots.md` TASK-039 行 | `CI 14 项门禁全启用` | 同上口径 |

> **教训**：口径统一类的 ADR **不能只列“我想到的文件”**，必须附一条**可重跑的全仓扫描命令**作为验收步骤（见下方「验证方式」第 5 条）。
> 这 7 处里有 2 处在 **SSOT 本身**（architecture-v2）—— 最不应该错的地方错了。

> **`plans/stage-0-spikes.md` 里那处期望输出行必须逐字符同步**，因为它被 TASK-001 的
> 验收命令当作判定依据（「退出码 0 且该行数字自洽 = 通过」）。数字不同步 = 验收判据失效。

### D4　顺带统一「CI 门禁项数」的口径（PL-001 的另一半）

PL-001 的处置记录里还提到：裁决 PL-001 时需一并统一 **gov §5.1 的行数**与
`plans/stage-1-pilots.md`「CI 14 项门禁」的表述。定案如下：

| 口径 | 数字 | 说明 |
|---|---|---|
| gov §5.1 **清单行数** | **17** | 原 16 行（#1~#16）+ **子编号 #8b**（spike-deny，ADR-0024 D2） |
| CI **实际步骤数** | **16** = 7 硬 + 9 软 | #3「禁用项 lint」由 #2 的 `-D warnings` + `[workspace.lints]` 一并覆盖，故 17 − 1 = 16 |
| `plans/stage-1-pilots.md` 的「CI 14 项门禁」 | ❌ **过时表述，已改述** | 改为「gov §5.1 的 17 行清单 ↔ 16 个 CI 步骤（7 硬 + 9 软）」 |
| `cross-platform-ai-assistant-architecture-v2.md` 的 2 处「14 项门禁」 | ❌ **过时表述，2026-09-18 已改述** | 同上口径（见 D3 补全表） |
| `gov` §10 落地清单、`docs/overnight-automation-charter.md` W2、`plans/stage-1-pilots.md` TASK-039 | ❌ **过时表述，2026-09-18 已改述** | 同上口径 |

**为什么 #8b 用子编号而不是插一行 #9 并把后面全部顺延**：主编号被
`tasks/TASK-001-repo-skeleton.md`、`xtask/src/deferred.rs`、`ci.yml` 头部注释、
`plans/stage-0-spikes.md` 与 `plans/stage-1-pilots.md` **多处交叉引用**；
重编号会引发一轮纯粹为了对齐数字的改动（口径漂移），而 PL-001 本身就是这么产生的。
→ **规则**：gov §5.1 的主编号一旦分配**永不重排**；新增门禁用子编号（`#8b`、`#12b` …）。

**唯一的长期解**：让这些数字**不再手写在两处**。`hygiene` 侧的对应提案是
「让 `xtask` 直接解析 gov §5.4 的表格行数」（PL-022）；CI 侧同理可由 `xtask` 校验
「gov §5.1 行数 ↔ `ci.yml` 的 job 数」。在那之前，本 D4 的表就是口径的事实源。

## 影响

- `docs/governance-ai-agent-execution.md` §5.4：表格 11 行 → **13 行**（已核实行数）。
- `xtask`：**5** 个文件的常量/注释/单测/README（见 D2）。**必须 `cargo test --workspace` 全绿**。
- 任务卡与 plans：见 D3。
- `docs/PARKING_LOT.md`：PL-001 / PL-011 / PL-020 三条追加处置行（**关闭**）；新增 **PL-021 ~ PL-025**（PL-023/024/025 为本次落地过程中新发现）。
- `plans/stage-1-pilots.md`：阶段 1 DoD 的「CI 14 项门禁」与 TASK-015 DoD 的「hygiene 12 项」（见 D3 / D4）。
- **2026-09-18 补全**：`xtask/README.md`、`gov` §10 清单、`architecture-v2` ×2、`charter` W2 ×2、
  `plans/stage-1-pilots.md` TASK-039 行 —— 共 **7** 处（详表见 D3）。
- `docs/memory/decisions.md`：补一条 2026-09-18 的 ADR-0025 DECISION 条目（原本只到 ADR-0024）；
  同文件里 2026-09-16 的两条旧 DECISION（写「14 项 CI 门禁」与「11 项只实现 3 项」）
  按记忆层「只追加不改写」规矩**保留原样**，由新条目负责纠偏。
- **不影响** `ci.yml` / `gate-selftest.yml`：硬门禁 #12 的命令与判定方式都没变，
  只是 `hygiene` 打印的进度声明数字变了 → **不需要新的负向验证**（ADR-0019 登记表不动）。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 维持 11 项，把 PL-011/PL-020 记为"建议但不采纳" | ❌ 否决 | 两条都**真实踩过**且都会导致"报错信息与真因无关"，正是护栏该覆盖的退化信号 |
| 2 | 采纳 PL-011 但不采纳 PL-020（→ 12 项） | ❌ 否决 | 恰好会得到 `plans/stage-1-pilots.md` 里那个「12」，看起来"对上了"，但那是**巧合而非因果** —— 那处的 12 本来就是笔误。用一个巧合去掩盖口径问题，下次仍会漂 |
| 3 | 13 项，但两条新规则**当场实现** | ❌ 否决 | 实现属 TASK-015（需要文件类型判定、Warning/Error 分级、15 个文件的清扫）；本 ADR 是**人类会话**产物而非任务卡，当场实现 = 无 write scope、无验收、无 review |
| 4 | 13 项，但先不改 `xtask` 常量，等 TASK-015 一起改 | ❌ 否决 | 会留下一个**已知的**「文档 13 / 工具 11」不一致窗口，而窗口里跑的每一次 `hygiene` 都在打印错的进度声明。改 4 个常量 + 1 处断言的成本远低于这个风险 |
| 5 | 把「所有 `Cargo.toml` 必须有 `license` 字段」也加成第 14 项 | ⏸ **推迟（PL-021）** | 该规则是 2026-09-18 实测 `error[unlicensed]` 后新提出的（ADR-0024 D1a 陷阱 3），方向正确但**尚未走人类裁决** → 记入 PARKING_LOT，不在本 ADR 夹带 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 改了 `TOTAL` 却没加满 2 条 deferred 条目 → 单测红 | 不变量 1 的单测**就是**为此存在；本 ADR 落地后立即 `cargo test --workspace` 验证 |
| 文档多处漏改 | D3 列了逐处清单；`plans/stage-0-spikes.md` 的期望输出行是**机器可判定**的（TASK-001 验收命令），漏改会在下次验收时暴露 |
| 第 13 条长期停在 Warning，永远不升 Error | TASK-015 的 DoD 必须写「清扫 15 个文件 + 升级为 Error」两件事，缺一不算完成（已记入 PL-020 的处置行） |
| 扩展名白名单漏掉某种文本文件 | 白名单**显式列出**在 D1；新增文件类型时必须回来加一行（漏加的后果是"少检查"，不是"误报"，方向安全） |

## 验证方式

1. `cargo test --workspace` 全绿（含 `test_hygiene_rule_counts_are_consistent`：3 + 10 == 13）。
2. `cargo run -p xtask -- hygiene` 打印
   `-- deferred-rules: gov §5.4 共 13 项，已实现 3 项，未实现 10 项（归属 TASK-015；…）`，退出码 0。
3. `cargo run -p xtask -- --list-deferred` 的未实现卫生规则清单**恰好 10 条**，含新增的
   `hygiene/crlf-line-endings` 与 `hygiene/missing-final-newline`。
4. gov §5.4 表格**恰好 13 行**（已核实）。
5. 全仓库检索 `3/11`、`未实现 8 项`、`11 项中的 3 项`、`14 项门禁`、`hygiene 12 项` ——
   **除下列四类“只追加不改写”的历史记载外零命中**：
   ① 本 ADR 自身（它必须引用旧值才能说清楚改了什么）；
   ② `docs/PARKING_LOT.md` 与 `LEDGER.md`（两者按定义只追加）；
   ③ `tasks/TASK-001-repo-skeleton.md` 的历史段（已逐处附 `**[2026-09-18 口径更正，ADR-0025]**` 标注）；
   ④ `docs/memory/{facts,rejected,decisions}.md` 里**带早期日期的**条目（记录当时的认知，由 2026-09-18 新条目纠偏）。
   可重跑的扫描命令（PowerShell）：

   ```powershell
   git grep -n -E "3/11|未实现 8 项|11 项中的 3 项|14 项门禁|hygiene 12 项" -- "*.md" "*.rs" "*.yml" "*.toml"
   ```

   2026-09-18 实跑结果：命中均落在上述四类豁免位置，**无任何“活的”过时口径**。

## 重新评估的触发条件

- TASK-015 实现这两条规则时若发现扩展名白名单方案不可行（例如必须处理无扩展名文件）→ 回本 ADR 修 D1。
- **更好的长期做法**：让 `xtask` **直接解析 gov §5.4 的表格行数**得到总数，从根上消灭"两处数字"。
  已记入 PARKING_LOT（PL-022）；在那之前，本表行数仍是唯一事实源。
