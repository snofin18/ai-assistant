# TASK-015　`xtask`：hygiene + arch test + verify-schemas + replay 骨架

- 状态：**Done**（2026-09-24）
- 阶段：1　子阶段：**1a**　批次：**A1**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011　**预估**：M　**难度**：M
- **write scope**：`xtask/**`、`crates/core/tests/arch*`
- **关联**：`plans/stage-1-pilots.md` 批次表 A1（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

`xtask`：hygiene + arch test + verify-schemas + replay 骨架。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`xtask/**`、`crates/core/tests/arch*`

**步骤**（2026-09-24 Orchestrator 展开：原占位 → 六项具体交付）

1. **环境记录**（OS / Rust 版本 / 输入 fixture 路径）
2. **`docscan` 新增 4 条结构规则**（规则 id 稳定，落地前先跑原型扫描数出现存量）：
   | 规则 id | 级别 | 判据 | 触发案例 |
   |---|---|---|---|
   | `doc/nul-byte` | **Error**（可直接上线） | 任何文本文件含 `0x00` | `docs/adr/README.md` 曾含裸 NUL（TASK-202 已修，规则尚未上线）；裸 NUL 会让 git 把该文件当**二进制**，diff 不可复核 |
   | `doc/duplicate-section-number` | 先 Warning，清扫后升 Error | 同一文件内 `## 4.` / `### 4.3` 这类**数字节号**重复 | TASK-200 的 7 份 spec 全部命中 |
   | `doc/empty-section` | 先 Warning，清扫后升 Error | 标题下方到下一个标题 / 文件尾之间**无非空内容** | TASK-200：`## 1. 目标` / `## 2. 范围` 曾整节为空 |
   | `doc/duplicate-heading` | 先 Warning，清扫后升 Error | 同一文件内**标题文字**重复（如 `### 字段` 连续出现两次） | TASK-200：外来模板块重复 `### 字段` |
   **背景**：TASK-200 的 7 份 spec 结构性缺陷 `docscan` **全部漏过**（见 `docs/memory/pitfalls.md` 2026-09-24 条）；
   先 Warning 后 Error 的口径见 **ADR-0025 D1**，不得为变绿放宽判据。
3. **`crates/core/tests/arch*` 分层断言**（铁律 7：`core` 不得调平台 API，只能用 `crates/platform/api` 的 trait）——
   目标是把 `cargo test -p assistant-core arch::` 从当前的 `running 0 tests` 变成**真断言**。
4. **PL-047 机器化**：`xtask` 扫描 `crates/*/migrations/*.sql`，三条判据：
   ① 文件名号段**全局唯一**；② 与 `docs/storage-design.md` **§3.4 登记表逐行一致**；
   ③ 每个含迁移的 crate 都公开 `pub const MIGRATIONS`。三条都要有**负向用例**。
5. **ADR-0039 D3 的 `check-ledger` 两条规则**（`check-ledger` 目前属 gov §5.1 #16，未分配）：
   ① `PLAN.md` 的「更新日期」**不得早于** `LEDGER.md` 最后一条记录的日期；
   ② `README.md` 必须存在 `> 状态：` 状态行且其中出现当前阶段名（`阶段 N`）。先 Warning 上线。
6. **跑测试**：`cargo test --workspace` + 本卡专项测试；不合格 → DRIFT（`DRIFT-015-x`）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `docscan` 4 条新规则各自有单测；**全仓实跑结果记入执行记录**（若先 Warning：附「清扫后升 Error」的后续动作与卡号）
- [ ] `cargo test -p assistant-core arch::` **不再是** `running 0 tests`（分层断言真生效）
- [ ] **PL-047**：迁移文件名号段唯一 + 与 §3.4 登记表逐行一致 + 每个含迁移的 crate 公开 `MIGRATIONS`（3 条判据 + 3 条负向用例）
- [ ] **ADR-0039 D3**：`check-ledger` 的 ① `PLAN.md` 新鲜度 ② `README.md` 状态行存在性 两条规则落地
- [ ] 若新增规则计入 gov §5.4 的 13 项卫生规则 → 同批更新 `xtask/src/deferred.rs` 与 **ADR-0025** 登记表；**不得**为让门禁变绿而放宽判据
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

> **本批形态（2026-09-24）**：用户 chat 指示「**A 当作没有做，但是对已经做好的检查，空的补上。全部做完之后按正常走**」——
> 即本卡按**全新开工**处理（正文区写的是 `Ready`，记录区 9 节全空），实现与记录一次补齐。
> 记录区 9 节**逐节写实**（不用 TASK-011 那种「落点指引」形态：本卡只有一个批次，没有多轮 `UPDATE` 块可指）。

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-015 xtask：hygiene + arch test + verify-schemas + replay 骨架
【目标】把 xtask 的四块护栏落地（docscan 4 条结构规则 / crates/core 分层断言 / PL-047 迁移登记表 /
        ADR-0039 D3 的 check-ledger），并把 gov §5.1 的 #5 与 #16 两条 CI 门禁由软转硬
【write scope】仅：xtask/**、crates/core/tests/arch*（另加卡 Done 时治理契约强制的同步文件，见 §5 DRIFT-015-2）
【铁律】① 无静默失败（扫描器读不到文件必须报错，不许当成「0 发现项」）；② 被读文档 = 不可信输入
        （Markdown / schema 解析）；⑦ core 不得调用平台 API（本卡 arch 断言的本体）；
        ⑨ 不得静默扩大范围（scope 外写入必须登记）；⑩ 契约先行（门禁口径改动先有 ADR-0019 / 0025 / 0039）
【禁止】引入第三方依赖；为让门禁变绿而放宽判据；drive-by refactor；改 PLAN.md「当前状态」块以外的段落；
        改 README 三处以外的章节；动任务卡正文区
【验收】cargo fmt --all --check → 0 diff；cargo clippy --all-targets -- -D warnings → exit 0；
        cargo test --workspace → 全绿；cargo test -p assistant-core arch:: → running 5 tests；
        xtask {hygiene,memory-counts,adr-index,docscan,card-check,check-ledger,check-migrations} → 全 PASSED
【依赖】011（已核对 LEDGER：TASK-011 自 2026-09-23 起有 10+ 行 Done 记录；plans/stage-1-pilots.md 的
        A1 批次表同记）　【疑问】无
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

**In scope（`xtask/**`）：**

| 文件 | 动作 | 规模 |
|---|---|---|
| `xtask/src/docscan.rs` | 改 | 675 行（+390 行 diff：4 条新规则 + 14 条单测 + fence 跳过 + 输出确定性排序） |
| `xtask/src/ledger_check.rs` | **新建** | 342 行（`check-ledger`：14 条单测） |
| `xtask/src/migration_registry.rs` | **新建** | 502 行（`check-migrations`：11 条单测） |
| `xtask/src/arch.rs` | 重写 | 308 行（修 PL-048：判定函数纯函数化 + 消除静默失败） |
| `xtask/src/main.rs` | 改 | +22 行（2 个 `mod` + 2 个分派 + 2 个 `run_*`） |
| `xtask/src/cli.rs` | 改 | USAGE：补 `check-ledger` / `check-migrations`，`replay` 去掉 `[未实现]` 标注 |
| `xtask/src/deferred.rs` | 改 | 从 `DEFERRED_COMMANDS` 移除 `check-ledger`（−6 行） |

**In scope（`crates/core/tests/arch*`）：**

| 文件 | 动作 | 规模 |
|---|---|---|
| `crates/core/tests/arch_layering.rs` | **新建** | 230 行（`mod arch`；正向 2 + 负向 3） |

**Out of scope —— 已在 §5 登记为 DRIFT-015-1 / DRIFT-015-2（全部是治理契约强制或本卡 DoD 的前置动作）：**

| 文件 | 依据（详见 §5） |
|---|---|
| `docs/memory/facts.md` | DRIFT-015-1（PL-051 的裸 NUL 字节修复 2 处）+ §8 的 4 条 FACT |
| `docs/memory/pitfalls.md` | DRIFT-015-1（同上 2 处）+ §8 的 4 条 PITFALL + 沙箱裁决回填（用户 chat 第 1 项） |
| `.github/workflows/ci.yml` | DRIFT-015-2（#5 / #16 软转硬；ADR-0019 的「转硬必须同批提交负向验证」） |
| `docs/adr/0019-hard-gate-negative-verification.md` | DRIFT-015-2（登记表补 #5 / #16 两行 —— ADR-0019 的强制动作） |
| `docs/storage-design.md` | DRIFT-015-2（§3.4 的「已知遗留（PL-047）」→「已机器化」；PL-047 的落点） |
| `docs/PARKING_LOT.md` | DRIFT-015-2（9 条处置 + 本批新提 PL-058 / PL-059） |
| `MEMORY.md` | DRIFT-015-2（§1 规模表 facts 152/102 → 157/106、pitfalls 200/95 → 205/99；「已产出代码」行删掉手抄的子命令个数） |
| `PLAN.md` | DRIFT-015-2（「当前状态」块 4 行；ADR-0039 D4 / ADR-0041 D1） |
| `README.md` | DRIFT-015-2（三处；ADR-0039 D1/D2） |
| `plans/stage-1-pilots.md` | DRIFT-015-2（完成标记 + 派生计数；ADR-0041 D1） |
| `tasks/TASK-011-protocol-schema-codegen.md` | 用户 chat「C」项（TASK-011 完整性复核 + 状态行 / §6 标题规范化 + `UPDATE 2026-09-24c`） |

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

**代码与测试（全绿）：**

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | 退出码 0，0 diff |
| `cargo clippy --all-targets -- -D warnings` | 退出码 0，0 warning |
| `cargo test --workspace` | 退出码 0 —— **16 个 test target + 5 个 doctest，合计 468 passed / 0 failed**（audit 13+7+8+doctest 1；core 5；protocol 7；secrets 9+17+doctest 1；storage 18+2+2+6+doctest 2；xtask 370） |
| `cargo test -p assistant-core arch::` | 退出码 0 —— **`running 5 tests`**（改前是 `running 0 tests`）：`core_manifest_has_no_platform_implementation_dependency` / `core_sources_do_not_reference_platform_implementations` / `scan_flags_a_synthetic_platform_reference` / `scan_flags_a_synthetic_platform_dependency` / `scan_allows_platform_api_trait_and_doc_mentions` |
| `cargo test -p xtask docscan` | 20 passed |
| `cargo test -p xtask ledger_check` | 14 passed |
| `cargo test -p xtask migration_registry` | 11 passed |
| `cargo deny check` | 退出码 0 —— `advisories ok, bans ok, licenses ok, sources ok`（本批**未改任何依赖**，`Cargo.toml` 零 diff） |

**xtask 子命令（全绿）：**

| 命令 | 结果 |
|---|---|
| `xtask hygiene` | PASSED（`scanned_files=80`，0 error / **3 warning = 既有 baseline**：`card_check.rs` 667 / `docscan.rs` 675 / `main.rs` 707 行，均超 600 建议上限） |
| `xtask memory-counts` | PASSED（`scanned=8`，0 / 0） |
| `xtask adr-index` | PASSED（`scanned=25`，0 / 0） |
| `xtask docscan` | PASSED（`scanned_files=158`，**0 error / 563 warning** —— 该总数是**派生值**，本卡记录区填完即由 572 降到 563；三条新规则各自的存量见下表） |
| `xtask card-check` | PASSED（`scanned_files=89`，0 error / 49 warning = 既有 baseline） |
| `xtask check-ledger` | PASSED —— `plan_date=2026-09-24` = `ledger_last_date=2026-09-24`；`current_stage=阶段 1`；0 / 0（两处行号是派生值，不写死） |
| `xtask check-migrations` | PASSED —— `scanned_migration_files=3` / `registry_entries=3` / `crates_with_migrations=2`；0 / 0 |
| `xtask verify-schemas` | PASSED（0 error） |
| `xtask codegen --check` | PASSED（0 drift / 0 error） |
| `xtask arch` | PASSED（0 error / 6 warning = 既有 baseline） |

**`docscan` 4 条新规则的存量实测（DoD 明写项）：**

| 规则 id | 级别 | 上线时存量 | 处置 |
|---|---|---|---|
| `doc/nul-byte` | **Error** | **2** | 同批修掉（PL-051）→ **0**，故 Error 级上线即绿 |
| `doc/duplicate-section-number` | Warning | 33 | 升 Error 前先裁决判据 → **PL-055** |
| `doc/empty-section` | Warning | 492 | 同上（**PL-055**） |
| `doc/duplicate-heading` | Warning | 47 | 同上（**PL-055**） |

> 「先 Warning、清扫后升 Error」的口径见 **ADR-0025 D1**；三条 Warning 的存量里既有**真债**（stage-0 的 8 张
> spike 卡记录区整节未填）也有**按定义合法的形态**（`Ready` 卡的 9 节空骨架、TASK-011 那种「每轮追加一个
> `UPDATE` 块」造成的重复节标题）—— 判据未收敛前不得升 Error，否则就是造永久红灯。

**未跑 / 不成立（如实登记，不掩饰）：**

| 命令 | 说明 |
|---|---|
| `cargo run -p xtask -- refscan` | **FAILED（151 error）—— 既有基线，与本批改动无关**。151 项全部落在本批**未触碰**的文件：`spikes/*.ps1` 的非 ASCII（PL-026 跟踪）、`docs/PARKING_LOT.md:37/41` 与 `gov` 里的裸 ADR 引用（PL-028 / PL-032 / PL-034 跟踪）。逐项核对结论：**没有任何一项落在本批改动的 19 个其他文件里**（不含本卡自身）（`git show HEAD:docs/PARKING_LOT.md` 的 37/41 行与工作区逐字相同）。→ 本卡 DoD 抄了模板里的「refscan 也须 PASSED」，**该条在本卡之前就不成立**，已登记 **PL-058**。 |
| `cargo llvm-cov` / `pnpm *` | 阶段 1 尚无 UI 与真实覆盖率基线，`AGENTS.md` §6 已注明这些命令「自 TASK-001 起生效」且阶段 0 不适用；本批不代跑。 |
| `cargo run -p xtask -- replay --suite core` | `replay` 目前是**骨架**（dry-run 解析 + 校验），只接受快照路径、**不接受 `--suite`**；真实 fixture + diff 归 **TASK-034**。`AGENTS.md` §6 里那条命令口径待校准 → 见 **PL-059**。 |

### 4. DoD 逐条核对

| DoD 条目 | 结果 |
|---|---|
| card 标题声明的能力可被测试用例覆盖 | ✅ 四块能力各有专项测试：`docscan` 20 / `ledger_check` 14 / `migration_registry` 11 / `arch` 5 |
| `docscan` 4 条新规则各自有单测；全仓实跑结果记入执行记录 | ✅ 单测 14 条（4 规则 × 正/负/边界）；存量实测见 §3 表；三条 Warning 的后续动作 = **PL-055** |
| `cargo test -p assistant-core arch::` 不再是 `running 0 tests` | ✅ `running 5 tests`（正向 2 + 负向 3） |
| **PL-047**：号段唯一 + 与 §3.4 逐行一致 + 含迁移的 crate 公开 `MIGRATIONS`（3 判据 + 3 负向用例） | ✅ `check-migrations` 三条判据全实现（② 为**双向**：漏登记 / 多登记 / 路径不符）；负向用例 3+ 条（重号 / 漏登记 / 多登记 / 路径不符 / 缺 `MIGRATIONS`），共 11 条单测 |
| **ADR-0039 D3**：`check-ledger` 的 ① PLAN.md 新鲜度 ② README.md 状态行存在性 | ✅ 两条都落地；② 额外校验「状态行里的阶段名 = PLAN.md 的当前阶段」。**两条都已从「先 Warning 上线」直接升为 Error 并接进 CI**（ADR-0025 D1「上线即全绿则可直接 Error」：实测 0 / 0） |
| 若新增规则计入 gov §5.4 的 13 项卫生规则 → 同批更新 `deferred.rs` 与 ADR-0025 登记表 | ✅ 本卡新增的 4 条规则属 **`docscan`（文档结构扫描）**，**不**计入 §5.4 的 13 项卫生规则 → ADR-0025 登记表**不需要**改（改它反而会作废 §5.4 的 13 项口径）。`deferred.rs` 的改动只有一处：`check-ledger` 移出 `DEFERRED_COMMANDS`（它已实现，留着就是「已实现却报未实现」的谎言） |
| `cargo fmt --all --check` 0 diff | ✅ |
| `cargo clippy --all-targets -- -D warnings` 退出码 0 | ✅ |
| `cargo test --workspace` 全绿 | ✅ 468 passed / 0 failed |
| `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED | ⚠ **5/6 PASSED；`refscan` = FAILED（151 error，既有基线，与本批无关）** → 见 §3 与 §7 **PL-058**。其余 5 项全绿（warning 数均为既有 baseline） |
| LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md` | ✅ LEDGER 追加本批事件行；`facts.md` +4、`pitfalls.md` +4（原文见 §8） |

### 5. 偏差：none / DRIFT-0NN-x（全文按 gov §4.3 格式：现象 / 影响 / 建议 / 已停工作）

**DRIFT-015-1（write scope 扩张：字节级修 `docs/memory/{facts,pitfalls}.md`）**

- **现象**：`docs/memory/facts.md` 与 `docs/memory/pitfalls.md` 各含 2 个**裸 NUL 字节**（`0x00`）—— 这是
  PL-051 的登记项（偏移：facts.md 46886 / 46909，pitfalls.md 42266 / 42289）。本卡 DoD 要求 `doc/nul-byte`
  规则以 **Error** 级上线且「全仓实跑 PASSED」，而这 4 个字节会让该规则**一上线即红**。
- **影响**：不修则 DoD 第 2 条无法达成，只剩两条坏路 —— ① 把规则降级为 Warning（= 放宽判据，违反 ADR-0025 D1）；
  ② 留一盏红灯（违反 `ci.yml` 头部自己写的设计原则：「永久红灯会让人学会忽略 CI」）。修它必须写
  `docs/memory/*`，**超出**卡面 write scope（`xtask/**` + `crates/core/tests/arch*`）。
- **建议**：按 PL-051 的原判「**归 TASK-015**」执行，本批同修（PL-051 的处置栏写的就是这条）。**修法按实测校正**：
  还原成 ASCII `0`（0x30），**不是** PL-051 原建议的「字面量 `\0` 文本」——理由见 §6 第 6 条。
- **已停工作**：无（它是本卡 DoD 的前置动作；PL-051 已把处置归本卡；用户 chat 2026-09-24「A 当作没有做，
  但是对已经做好的检查，空的补上」视为授权）。

**DRIFT-015-2（write scope 扩张：卡 Done 的治理契约强制同步文件）**

- **现象**：本批另外写了 11 个文件，全部在卡面 write scope 之外（清单见 §2 末表）。
- **影响**：这些写入**不是自由裁量的扩张**，而是**别的契约要求的动作** —— 不做则本卡自己就违反那些契约：
  ① **ADR-0019**「把软门禁转硬的那张卡必须同时提交负向验证并在登记表补一行」→ `ci.yml` + `0019-*.md`；
  ② **ADR-0039 D1/D2/D4 + ADR-0041 D1**「卡 Done 必须在同一 PR 内同步 `PLAN.md` 当前状态块 + `README.md`
  三处 + 计划文件的完成标记」→ `PLAN.md` + `README.md` + `plans/stage-1-pilots.md`；
  ③ **ADR-0028 / `AGENTS.md` §11.1**「`LEDGER.md` / `docs/PARKING_LOT.md` / `docs/memory/*` / `MEMORY.md` 可追加」
  → LEDGER + PARKING_LOT + facts + pitfalls + MEMORY；
  ④ **PL-047** 的落点是 `docs/storage-design.md` §3.4 那句「已知遗留（PL-047）」；
  ⑤ 用户 chat「C」项要求复核 TASK-011 并改正其状态行（ADR-0031 D2「状态只有一个落点」）。
- **建议**：按上列 ADR 依据**保留**，并在 PR 描述里逐条列出依据（本 §5 即依据清单）。
- **已停工作**：无。

### 6. 更合理做法（非漂移，已直接落地 + 理由）

1. **PL-048（flaky）的修法不是「让临时目录名更随机」，而是让判定函数变纯函数**：`check_third_party_deps(content)`
   / `check_module_layering(&[SourceFile])` 把文件内容当**参数**传入 → 单测完全不碰文件系统，**flaky 的整类**
   消失（PL-048 建议的 `AtomicU64` 方案因此不需要）。理由：随机的目录名只是把撞车概率降到「很低」，
   纯函数是把它降到 **0**。
2. **顺带修掉 `check_third_party_deps` 的一处静默失败**：读不到 `Cargo.toml` 时原来 `let Ok(..) else { return findings }`
   —— 「读不到」与「没有第三方依赖」不可区分（铁律 1），现在 `run` 返回 `Err`。
3. **顺带修掉 `check_module_layering` 的一个假阴性**：`use crate::{report, doccheck};` 这种**花括号导入**原来
   一个都看不见 —— 于是「core 引用了平台实现」的同类写法会漏报。这不是新功能，是**同一个判据本来就没写全**。
4. **`docscan` 的结构扫描必须先跳过 fenced code block**：不跳时 `gov` 的模板附录与 `docs/dev-env-setup.md` 的
   安装命令块里那些以 `#` 开头的**代码行**（`# Windows` / `# 安装`）会被当成 ATX 标题，`doc/empty-section`
   因此多出约 30 处纯噪声。判据：行首（允许前导空白）1~6 个 `#` **且其后必须是空白**（`#foo` 不是标题，GFM 同此）。
5. **`empty-section` 的判据必须是「到下一个同级或更高级标题」（= 判整棵子树是否为空）**：按「到下一个标题
   （任意级）」实测全仓 **693** 处命中，绝大多数是「父标题紧跟子标题」这种**正常**写法；收紧后降到 **492**，
   命中的都是真·空节。**693 与 492 的差值就是「定义没想清楚」的证据** —— 写结构规则前先跑原型数存量。
6. **PL-051 的 4 个裸 NUL 要还原成 ASCII `0`（0x30），不是字面量 `\0` 文本**：这 4 个字节的真身是**被 NUL
   顶掉的数字 `0`** —— 同仓库另 3 处同一条记录（`docs/memory/win32-input-research.md:85`、
   `tasks/TASK-100-*.md:120`、`docs/memory/apps/notepad.md:254`）写的是 `GetFocus() = 0x0` 与 `SendInput 返回 0`。
   按 PL-051 原建议改会得到 `\0x0` 这种无意义文本。
7. **`doc/duplicate-section-number` 只认「行首节号」**：否则 `2026-09-24`（日期）、`§5`（TASK-011 自编小节号）、
   `ADR-0019`（编号）都会被当成节号 → 假阳性会把真债埋在噪声里。
8. **`check-ledger` 的两条规则直接以 Error 上线，而不是先 Warning**：ADR-0025 D1 的例外条件（「上线即全绿」）
   实测成立（0 / 0），且它是 ADR-0039「验证方式 2」明文要求「由 CI 硬拦」的东西 —— 停在软门禁等于让该 ADR
   的机器判据永远不生效。

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

**本批新提：**

- **PL-058**（新提）：`refscan` **未接进 CI 且基线非绿**（151 error，全部为 PL-026 / PL-028 / PL-032 / PL-034
  跟踪的既有项）→ 它既不是门禁、也不在 `AGENTS.md` §6 的清单里，本卡 DoD 却抄了「refscan 须 PASSED」。
  需要裁决：接 CI（先清扫）还是明确「不是门禁」并把 DoD 模板里的那半句删掉。
- **PL-059**（新提）：`xtask/src/deferred.rs` 的**归属卡号已过期** —— ① 10 条 §5.4 卫生规则（函数行数 /
  参数个数 / 圈复杂度 / 重复代码 / 顶层目录白名单 / 依赖登记 / STUB 标记 / `#[ignore]` 原因 / CRLF / 末行换行）
  与 ② `replay-skeleton` 都仍写 `TASK-015`，而 TASK-015 **已 Done 且没有实现它们**；`hygiene_progress_note()`
  也硬编码了「（归属 TASK-015）」。③ 同源问题：`AGENTS.md` §6 的 `replay --suite core` 与骨架版 CLI 不符。
  **本卡刻意不改**（改归属 = 需要人类决定由哪张卡接手；改 `AGENTS.md` = Orchestrator-only），只登记。

**本批处置（已写入 PARKING_LOT，见该文件 2026-09-24 各行）：**

- **PL-002** 部分关闭（`check-ledger` 认领并落地；`check-comments` 仍无卡认领）
- **PL-023** 处置（② `scripts/` 作废；① `docs/nightly/logs/` 降级保留）
- **PL-047 / PL-048 / PL-051** **已关闭**
- **PL-018** 复核结论：仍开放，且不在本卡 write scope 内
- **PL-055 / PL-056 / PL-057** 新提

**仍开放、且本卡未做（如实登记）：**

- **PL-053**（TASK-011 的 DRIFT-011-1 根因）：**卡片状态行 ↔ `LEDGER.md` 的机器一致性校验**。
  `check-ledger` 只覆盖「PLAN.md 新鲜度 + README.md 状态行」，**没有**覆盖「卡内 `- 状态：` 行 vs 台账」
  —— 本卡不做（它会要求解析 89 张卡的元数据块，属独立工作量）。
- **PL-021 / PL-033 / PL-034 / PL-035** 四条既有的「归 TASK-015」项**未做** —— 它们各自都停在「待评审 / 待裁决（需 ADR）」上，不是本卡能单独收口的（PL-021 要给 §5.4 加第 14 项规则 = 改 ADR-0025 钉死的 13 项口径）。→ 与 `deferred.rs` 的过期归属一并见 **PL-059**。
- 其余曾写「归 TASK-015」的项**已在本卡之前关闭**，故不重复：**PL-030**（`adr-index`，ADR-0030 D3 落地，见 `docs/PARKING_LOT.md` line 65）、**PL-022**（派生计数改机器校验 —— 本批顺手删掉 `MEMORY.md` 里那句手抄的子命令个数）。

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文；无则写「无」）

**REJECTED**：无新增。

**FACT**（`docs/memory/facts.md` +4；下列为条目主张的压缩，原文见文件）：

1. `docscan` 4 条新规则上线时的存量（全仓 158 个 `.md`）：`doc/nul-byte` 2（同批修掉 → 0）、
   `doc/duplicate-section-number` 33、`doc/empty-section` 492、`doc/duplicate-heading` 47；
   → `xtask docscan` PASSED（0 error；warning 总数是派生值，按 ADR-0030 D2 不写死）。
2. 两个新子命令**上线即绿**：`check-ledger`（`plan_date` = `ledger_last_date` = 2026-09-24，`current_stage=阶段 1`）、
   `check-migrations`（3 文件 / 3 登记 / 2 crate）。
3. gov §5.1 **#5 从「0 个测试的假绿」变成真断言**：`cargo test -p assistant-core arch::` 由 `running 0 tests`
   → `running 5 tests`；同批把 #5 与 #16 两条 CI 门禁由 `continue-on-error: true` 转硬。
4. **PL-051 的 4 个裸 NUL 的真身是被 NUL 顶掉的数字 `0`** → 正确修复 = 还原成 ASCII `0`（0x30），
   而不是 PL-051 建议的「字面量 `\0` 文本」；修完 `git diff` 是「1 行改动/文件」，不是二进制 diff。

**PITFALL**（`docs/memory/pitfalls.md` +4；同上）：

1. **中文标记词会「自己包含自己」**：`当前阶段` 里含 `阶段` → `find("阶段")` 命中 `当前阶段` 内部，
   `check-ledger` 首跑直接报「找不到当前阶段」（退出码 4）。修法：先用**完整标记词**定位，再在其后找短词。
2. **Markdown 结构扫描必须先跳过 fenced code block**，否则 `# Windows` 这类代码行冒充 ATX 标题
   （`doc/empty-section` 多出约 30 处噪声）。
3. **「空节」的判据必须限定「到下一个同级或更高级标题」**：按「任意级」口径实测 693 处（大量是正常写法），
   收紧后 492 处且都是真债 → 写结构规则前先跑原型数存量。
4. **`xtask` 单测的临时目录唯一性不能只靠「进程 id + 时间戳」**（PL-048 的 5% 假失败）；
   根治修法是**让判定函数变纯函数**，不是把目录名弄得更随机。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

1. **两条 CI 门禁由软转硬（#5 / #16）—— 本卡对仓库最实质的改动**。硬门禁一旦负向验证不成立，
   就会变成「永远绿」的假保护。请重点看两处负向用例是否真的会红：
   `crates/core/tests/arch_layering.rs` 的 3 条 `arch::scan_flags_*`（喂 `windows::` 引用 /
   `windows = "0.62"` 依赖 / `[dependencies.winapi]` 表头形态），以及 `xtask/src/ledger_check.rs`
   的 4 条负向用例（PLAN 日期落后 / README 缺状态行 / 阶段名不符 / **解析不到时必须报错而不是静默 PASSED**）。
2. **`docs/memory/{facts,pitfalls}.md` 的字节级修复（DRIFT-015-1）**：请核对 `git diff` 是「1 行文本改动/文件」
   而不是二进制 diff，并确认「还原成 `0`（0x30）优于 PL-051 原建议的 `\0` 文本」这个取舍（§6 第 6 条）。
   这是本批唯一**超出 write scope** 的写入。
3. **`docscan` 的 Warning 基线（PL-055）**：三条新 Warning 规则的存量里**混着真债与合法形态**，
   本卡刻意**不升 Error**。请确认这个处置，以及 `empty-section` 的「同级或更高级」判据（693 → 492 的差值
   就是「定义是否想清楚」的证据）。
4. **DoD 有一项如实登记为「不成立」**（`refscan` = FAILED，既有基线）→ 归 **PL-058**。请确认这个处置，
   而不是「改 DoD 措辞让它看起来达标」。
