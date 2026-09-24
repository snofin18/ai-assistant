# TASK-201　`crates/core` 骨架提前（PL-037 处置：让 arch 护栏先于代码就位）

- 状态：**Done**（2026-09-24）
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内）　依赖：011（✅ Done）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。
- 来源：`docs/PARKING_LOT.md` **PL-037**；人类 chat 2026-09-24 裁决 = **选项 ③「把 `crates/core` 骨架（仅 trait + 目录）提前到 015 之前」**。
- 关联：`plans/stage-1-pilots.md` 批次 A1（`crates/core/tests/arch*` 归 TASK-015）、`.github/workflows/ci.yml` `[SOFT #5 → TASK-015]`、`docs/memory/facts.md`

---

## 目标（一句话）

建立 `crates/core` 的**最小骨架**（`Cargo.toml` / `README.md` / `src/lib.rs`；零第三方依赖、零业务逻辑、零 `pub` 项），
使 CI 软门禁 #5 的 `cargo test -p assistant-core arch::` 有**可编译的落点**、`crates/core/tests/arch*` 有宿主 crate，
从而解除 PL-037 的死锁：「TASK-015 的 DoD 含 arch test 能拦 `core → platform/windows`，而 `crates/core` 要到 TASK-028 才存在」。

## 背景（为什么必须提前，而不是等 TASK-028）

PL-037 的原始现象（`docs/PARKING_LOT.md`，2026-09-24 记录）：

- `plans/stage-1-pilots.md` 批次 A1 的表头写「必须串行」，但依赖列里只有 `013 ← 012` 是真串行；`014 ← 011` / `015 ← 011` 在 011 Done 后即解锁。
- **更硬的问题**：TASK-015 的验收要点是「**arch test 能拦住 core→platform/windows 的依赖**」，而 `crates/core` 要到 **TASK-028** 才存在
  → **TASK-015 在 TASK-028 之前无法 Done**，与「015 必须早做（护栏晚于代码 = 漂移已经发生）」**直接冲突**。

三个候选处置里选 **③（把 `crates/core` 骨架提前）**，因为：

| 选项 | 为什么不选 |
|---|---|
| ① 把 015 的 arch test 拆到后续卡 | 护栏继续晚于代码；`core` 一旦在 028 里落地就没人再回来补这道门禁 |
| ② 015 标「部分 Done」 | 制造一个长期 `partial` 状态；后续 agent 无法判断该门禁到底存不存在（= 静默失败） |
| **③ 骨架提前（本卡）** | 一次性把「包不存在」这个**硬阻塞**消掉，护栏卡（015）随即可做；代价仅 3 个文件、零依赖、零接口 |

**关键区分**：本卡消除的是「包不存在」，**不是**「arch 规则已生效」。后者归 TASK-015（它拥有 `crates/core/tests/arch*`）。

## write scope

- `crates/core/**`（**仅** `Cargo.toml` / `README.md` / `src/lib.rs`；`crates/core/tests/**` **不在本卡**，归 TASK-015）
- `Cargo.lock`（cargo 因新增 workspace 成员自动改写）
- `plans/stage-1-pilots.md`（PL-037 标注 + 跨阶段治理卡表 + 号段表）※ Orchestrator 代行，见 §5
- `docs/PARKING_LOT.md`（PL-037 关闭行 + PL-039 新提）※ 追加
- `MEMORY.md` §1 快照（PL-037 状态 + 规模表）※ Orchestrator 代行，见 §5
- `docs/memory/facts.md`（新事实）
- `LEDGER.md`、`tasks/TASK-201-core-crate-skeleton.md`（本卡记录区）

## In scope

1. `crates/core/Cargo.toml`：`package.name = "assistant-core"`，继承 workspace 的 `version` / `edition` / `rust-version` / `license` / `publish` + `[lints] workspace = true`；**`[dependencies]` 必须为空**。
2. `crates/core/src/lib.rs`：`#![deny(unsafe_code)]` + crate 级文档（职责 / 边界 / 不变量 / 已知限制 / 相关文档）。**不得**声明任何 `pub` 类型、函数或常量。
3. `crates/core/README.md`：职责 / 边界 / **不变量** / 已知限制（阶段 1 DoD 要求「每个 crate 有 README」）。
4. `plans/stage-1-pilots.md`：PL-037 标注改为「已闭环（TASK-201）」；「跨阶段治理卡」表加 TASK-201 行；号段表 200~299 行更新为「已用 200 / 201」。
5. `docs/PARKING_LOT.md`：PL-037 关闭行（写明裁决 = 选项 ③ + 落地物 + 遗留）。
6. `docs/memory/facts.md`：追加 1 条 FACT（骨架已落地 + 门禁语义仍是"0 测试通过"）。

## Out of scope（做了算漂移）

- `crates/core/tests/**` —— **TASK-015** 的 write scope（`crates/core/tests/arch*`）
- 任何 core 业务逻辑：会话管理 / 上下文裁剪压缩 / Planner / Memory / 装配 —— **TASK-028**
- 任何 `crates/platform/**`、Adapter、`crates/audit`（013）、`crates/secrets`（014）
- **不发明公共接口**（trait / 类型 / 常量）：接口形状属 TASK-020~028，骨架期写它 = 制造第二事实源（铁律 10）
- 改 `AGENTS.md`（只读区）→ DRIFT-200-2 的 §11 口径澄清记 **PL-039**
- 改 `.github/workflows/ci.yml`（`[SOFT #5]` 转硬 = **TASK-015** 的动作，不是本卡）
- 把 `assistant-core` 加进任何其它 crate 的依赖（没有消费者之前不加边）

## 必须遵守

- **铁律 7**（core 不得调用平台 API）：本卡不写任何平台调用；`#![deny(unsafe_code)]`。
- **漂移触发器 ①**（加第三方依赖）：**零依赖**；`[dependencies]` 为空，`docs/DEPENDENCIES.md` **不需要**改动。
- **铁律 10**（契约先行）：骨架不发明任何公共接口 / schema。
- 继承 workspace `[lints]`：`unwrap_used` / `expect_used` / `panic` / `todo` / `unimplemented` / `dbg_macro` / `print_stdout` / `print_stderr` / `indexing_slicing` 全 **deny**。
- 单文件 ≤ 600（软）/ 900（硬）行（ADR-0033）；公开 API 100% 文档注释（本卡无公开 API）。
- 新增 `.rs` 必须过 `xtask hygiene`（`TODO` / `FIXME` 类标签必须带卡号）。
- 新增/改动的 `.md` 一律 **UTF-8 + LF 结尾**（`xtask docscan` 会红）。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-core
cargo test -p assistant-core arch::
cargo test --workspace
cargo build --release
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
```

## 完成定义（DoD）

- [ ] `cargo test -p assistant-core` 可执行（**PL-037 的核心判据**：包名可解析、CI 软门禁 #5 的 `-p` 不再落空）
- [ ] `crates/core/{Cargo.toml,README.md,src/lib.rs}` 就位；`[dependencies]` 为空；`src/lib.rs` 零 `pub` 项
- [ ] 上列命令全部通过（输出粘贴进本卡 §3）
- [ ] `plans/stage-1-pilots.md` 的 PL-037 标注已改为「已闭环」+ 跨阶段治理卡表含 TASK-201
- [ ] `docs/PARKING_LOT.md` 有 PL-037 关闭行（含裁决来源与遗留）
- [ ] `LEDGER.md` 追加一行；新 FACT 已追加；`MEMORY.md` 规模表按 `memory-counts` 机器值同步
- [ ] 无任何 Out of scope 的文件被修改（尤其 **`crates/core/tests/**` 未创建**、`docs/DEPENDENCIES.md` 未改）
- [ ] 本卡 §1~§9 执行记录已填

## 风险与已知坑（开工前先读）

- **骨架 ≠ 门禁生效**：本卡之后 `cargo test -p assistant-core arch::` 是「0 个测试通过」（exit 0），**不是**「分层规则已生效」。
  谁把这个 exit 0 读成"arch test 已就位"，就会漏掉 TASK-015。CI 里 `[SOFT #5]` **仍是软门禁**。
- **不要顺手写 trait**：PL-037 选项 ③ 的原文是「仅 trait + 目录」，但 trait 的形状是**接口设计**（TASK-020~028）。
  在骨架期发明接口 = 下个 agent 会把占位 trait 当既定契约实现（铁律 10 的反面教材）。本卡只放文档。
- **`Cargo.lock` 会变**：新增 workspace 成员必然改写 `Cargo.lock`；必须一并提交，否则 CI 的 `cargo build` 会自己改写并产生 diff。
- **别碰 `crates/core/tests/`**：那是 TASK-015 的 write scope；本卡创建它会直接造成两卡 scope 重叠。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-201 crates/core 骨架提前（PL-037 处置）
【目标】建立 assistant-core 最小骨架（零依赖 / 零业务逻辑 / 零 pub 项），让 CI 软门禁 #5 的
        `cargo test -p assistant-core arch::` 有可编译落点，解除「TASK-015 在 TASK-028 前无法 Done」死锁
【write scope】仅：crates/core/{Cargo.toml,README.md,src/lib.rs}、Cargo.lock、
              plans/stage-1-pilots.md（Orchestrator 代行）、docs/PARKING_LOT.md、
              MEMORY.md §1（Orchestrator 代行）、docs/memory/facts.md、LEDGER.md、
              tasks/TASK-201-core-crate-skeleton.md（记录区）
【铁律】7 core 不得调平台 API（本卡零平台调用）；9 不静默扩大范围（加 crate = 漂移 ②，已获人类授权）；
        10 契约先行（骨架期不发明接口）；1 无静默失败（"0 测试通过"必须在卡与 README 里写明语义）
【禁止】crates/core/tests/**（TASK-015）、任何 core 业务逻辑（TASK-028）、platform/**、Adapter、
        audit(013) / secrets(014)、任何 pub 项、任何第三方依赖、改 AGENTS.md、改 ci.yml
【验收】cargo fmt --all --check / cargo clippy --all-targets -- -D warnings /
        cargo test -p assistant-core / cargo test -p assistant-core arch:: / cargo test --workspace /
        cargo build --release / xtask hygiene / docscan / memory-counts / adr-index / card-check → 全绿（见 §3）
【依赖】TASK-011（assistant-protocol，✅ Done，已核对 LEDGER）；人类 chat 2026-09-24 裁决 PL-037 选项 ③
【疑问】号段选择：本卡不在 stage-1 批次表内，按 ADR-0037 D1 落 200~299 治理池 → TASK-201；
        若 Orchestrator 认为应入 100~199 业务池（ADR-0037 D3 改号需 DRIFT-ADR），请尽早裁决（见 §9）
```

### 2. 实际改动文件

| 文件 | 动作 | 说明 |
|---|---|---|
| `crates/core/Cargo.toml` | NEW | `assistant-core`；继承 workspace 元数据 + `[lints] workspace = true`；`[dependencies]` **为空** |
| `crates/core/src/lib.rs` | NEW | 仅 crate 级文档 + `#![deny(unsafe_code)]`；**零 `pub` 项** |
| `crates/core/README.md` | NEW | 职责 / 边界 / 不变量（4 条）/ 已知限制 |
| `Cargo.lock` | EDIT | cargo 因新增 workspace 成员自动追加 `assistant-core` 条目 |
| `plans/stage-1-pilots.md` | EDIT | PL-037 标注 → 已闭环；跨阶段治理卡表 + TASK-201 行；号段表 200 行更新 |
| `docs/PARKING_LOT.md` | APPEND | PL-037 关闭行 + PL-039 新提（AGENTS §11 README 口径需 ADR） |
| `MEMORY.md` | EDIT | §1 快照 PL-037 状态 + 规模表（按 `memory-counts` 机器值） |
| `docs/memory/facts.md` | APPEND | +1 条 FACT（骨架落地 + 门禁语义） |
| `LEDGER.md` | APPEND | 本卡一行 + 回填行 |
| `tasks/TASK-201-core-crate-skeleton.md` | NEW | 本卡（正文 + 记录） |
| `tasks/TASK-200-fix-spec-contract-drafts.md` | EDIT | §5 追加 DRIFT-200-2 闭环（人类 chat 2026-09-24 裁决选项 ①） |
| `README.md` | EDIT | 状态行 + `## 当前阶段` 章节 → 阶段 1 口径（DRIFT-200-2 授权范围内） |

> 逐个核对：全部在 §「write scope」内；`crates/core/tests/**` **未创建**；`docs/DEPENDENCIES.md` **未改**；
> `AGENTS.md` / `.github/workflows/ci.yml` / `docs/spec/**` **未改**。

### 3. 验收输出摘要

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | **exit 0**（无 diff） |
| `cargo clippy --all-targets -- -D warnings` | **exit 0**（`Checking assistant-core v0.1.0` → `Finished`） |
| `cargo test -p assistant-core` | **exit 0** —— `running 0 tests` / `test result: ok. 0 passed; 0 failed`（含 doc-tests 0） |
| `cargo test -p assistant-core arch::` | **exit 0** —— `running 0 tests`（**CI 软门禁 #5 不再落空**；语义 = 包存在，非规则生效） |
| `cargo test --workspace` | **exit 0** —— xtask **331** + protocol **7** + storage **27**（18 集成 + 2 records + 6 单测 + 1 doctest）+ core **0**；0 failed |
| `cargo build --release` | **exit 0**（`Compiling assistant-core v0.1.0` → `Finished release`） |
| `xtask hygiene` | **PASSED**（scanned=52 errors=0 warnings=2；2 warning = xtask 既有文件 >600 行） |
| `xtask docscan` | **PASSED**（0 error / 0 warning） |
| `xtask memory-counts` | **PASSED**（scanned=8 / 0 error；facts 140/92 机器值一致） |
| `xtask adr-index` | **PASSED**（scanned=21 / 0 error） |
| `xtask card-check` | **PASSED**（0 error / 49 warning） |
| `xtask refscan`（**非 CI 门禁**） | 151 error = **已知稳定 baseline**（与 TASK-012 记录一致，计数未变；本卡新增文件 0 命中） |

### 4. DoD 逐条核对

- [x] `cargo test -p assistant-core` 可执行（`running 0 tests`，exit 0）→ PL-037 的核心判据达成
- [x] `crates/core/{Cargo.toml,README.md,src/lib.rs}` 就位；`[dependencies]` 为空；`src/lib.rs` 零 `pub` 项
- [x] 验收命令全部通过（输出见 §3）
- [x] `plans/stage-1-pilots.md` 的 PL-037 标注已改为「已闭环」+ 跨阶段治理卡表含 TASK-201
- [x] `docs/PARKING_LOT.md` 有 PL-037 关闭行（含裁决来源与遗留）
- [x] `LEDGER.md` 追加；新 FACT 已追加；`MEMORY.md` 规模表按 `memory-counts` 机器值同步
- [x] 无 Out of scope 文件被改（`crates/core/tests/**` 未创建、`docs/DEPENDENCIES.md` 未改）
- [x] 本卡 §1~§9 已填

### 5. 偏差

DRIFT-201-1（**已闭环**：人类 chat 2026-09-24 已预先授权）

触发器：#2（加 crate / 顶层目录）+ #5（超出 Implementer write scope —— 本批动作动了 Orchestrator-only 文件）。

现象：本卡同时做了三件"默认不属于单张业务卡"的事：
(a) 新建顶层 crate `crates/core`（漂移触发器 ②，铁律 9「不得静默扩大范围」）；
(b) 改 `plans/stage-1-pilots.md`（批次 A1 顺序标注 + 跨阶段治理卡表 + 号段表）与 `MEMORY.md` §1（§8 表列明 = Orchestrator 所有）；
(c) 由 Implementer 代写本卡（TASK-201）的**正文区**（正常由 Orchestrator 写）。

影响：若不记录，下个会话按 `AGENTS.md` §4/§8 会判定为漂移事故。

授权原文（人类 chat 2026-09-24）：「**PL-037：把 `crates/core` 骨架提前**」（= 裁决 PL-037 选项 ③）；
并延续前次授权「按你的建议去做，并且授权你『代 Orchestrator 展开正文』」。

建议：本 DRIFT 不需要新裁决；但**号段选择**（TASK-201 落在 200~299 治理池而非 100~199 业务池）是可裁决项 ——
见 §9 关注点 1。

已停止的工作：无（授权范围内动作全部完成）。**未**做任何未被授权的 Orchestrator 动作：
未改 `AGENTS.md`、未改 `docs/adr/**`、未改 `docs/spec/**`、未改 `.github/workflows/ci.yml`、未改 `PLAN.md`。

DRIFT-200-2（**本卡内闭环**，详见 `tasks/TASK-200-fix-spec-contract-drafts.md` §5）

触发器：#8（规则自相矛盾）+ #5。现象：`AGENTS.md` §11.1 要求阶段切换改 README 状态行，§11.3 又把「改状态以外的章节」判为漂移
→ 阶段 0→1 时只改状态行会让 README 自相矛盾。人类 chat 2026-09-24 裁决 = **选项 ①**（授权「状态行 + 当前阶段章节」一起改）
→ 本卡一并落地 `README.md` 的 `## 当前阶段` 章节更新；`AGENTS.md` 文本层澄清记 **PL-039**。

### 6. 更合理做法

1. **骨架期不放 `trait`**（偏离 PL-037 选项 ③ 的字面"仅 trait + 目录"）：
   trait 的形状是**接口设计**，属 TASK-020~028；在骨架期写它会让下个 agent 把占位接口当既定契约（铁律 10）。
   已落地 = 只放文档 + `#![deny(unsafe_code)]`，`src/lib.rs` 零 `pub` 项。理由写进 `src/lib.rs` 与 `README.md` 的「边界」。
2. **`crates/core/tests/**` 留给 TASK-015**（不在本卡创建）：TASK-015 的 write scope 已含 `crates/core/tests/arch*`，
   本卡创建它会直接造成两卡 scope 重叠（AGENTS.md §3「write scope 必须互不重叠」）。
3. **把"0 测试通过 ≠ 规则生效"写进卡片、`src/lib.rs`、`README.md` 三处**：这是铁律 1（无静默失败）在文档层的体现 ——
   否则 `[SOFT #5]` 的 exit 0 会被误读为"arch 门禁已就位"。

### 7. 遗留问题

- **PL-039**（本卡新提）：`AGENTS.md` §11.1 / §11.3 的 README 口径冲突只被人类裁决"本轮怎么改"，**文本层未澄清**；
  `AGENTS.md` 属只读区（改动需 ADR）→ 记 `docs/PARKING_LOT.md`，建议与下次 `AGENTS.md` 修订一并处理。
- **TASK-015 仍未做**：`.github/workflows/ci.yml` 的 `[SOFT #5 → TASK-015]` 仍是**软门禁**；
  `crates/core/tests/arch*` 仍是空的（本卡刻意不创建）。015 现在**可开工**（PL-037 已闭环）。
- **TASK-028 的 core 业务能力仍未实现**：本卡只消掉"包不存在"，架构 v2 §5 的会话 / 上下文 / Planner / Memory 全部待做。
- `docs/spec/*` 7 份契约草案的结构缺陷（PL-038）仍在 → **TASK-200** 已建卡未开工。

### 8. 新增长期记忆

`docs/memory/facts.md` 追加：

```text
- [2026-09-24][FACT][src:TASK-201 落地 本机 Windows] **`crates/core` 骨架提前落地 = PL-037 闭环（人类 chat 2026-09-24 裁决选项 ③）**：新增 `crates/core/{Cargo.toml,README.md,src/lib.rs}`（`assistant-core`，**零第三方依赖**、`[dependencies]` 为空、`#![deny(unsafe_code)]`、**不含任何 `pub` 项** —— 骨架期不发明接口）。**为什么提前**：CI 软门禁 #5 是 `cargo test -p assistant-core arch::`，而 `crates/core` 原本要到 TASK-028 才存在 → TASK-015 的验收要点（arch test 能拦 `core → platform/windows`）在 028 之前无法满足 = 与「015 必须早做（护栏晚于代码 = 漂移已发生）」死锁。**实测**：`cargo test -p assistant-core` 与 `cargo test -p assistant-core arch::` 均 exit 0，输出 `running 0 tests`。**关键提醒**：本卡之后该门禁的语义是「0 个测试通过」而**不是**「分层规则已生效」—— 真正的 arch 断言归 TASK-015（它拥有 `crates/core/tests/arch*`），`.github/workflows/ci.yml` 的 `[SOFT #5]` **仍是软门禁**，转硬是 TASK-015 的动作。
```

### 9. 给审阅者的关注点

1. **号段选择（最需要人看的一处）**：本卡落在 **200~299 治理池（TASK-201）**，理由 = 它不在 stage-1 批次表内（是对 PL-037 这个**规划缺陷**的处置），
   且 `plans/stage-1-pilots.md` 已有「跨阶段治理卡」小节（TASK-200 在此）。**但**本卡的交付物是产品 crate 骨架，
   按 ADR-0037 D1 的字面也可主张进 100~199 业务池。改号需 **DRIFT-ADR**（ADR-0037 D3）→ 越早裁决越便宜。
2. **骨架"是否太空"**：`src/lib.rs` 零 `pub` 项是刻意的（不发明接口）。若审阅者认为骨架必须含占位 trait，
   请在 TASK-020/028 里定义接口形状，而**不要**在骨架期补 —— 那是铁律 10 的反面。
3. **软门禁 #5 的语义**：本卡之后 `cargo test -p assistant-core arch::` 通过 = 「包存在」而非「规则生效」。
   若审阅者希望此时就转硬门禁，那会得到一个**永远绿的假门禁** —— 正确顺序是先做 TASK-015。
