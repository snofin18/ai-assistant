# ADR-0042　能力相关的三个名字：`CapabilityCatalog` / `CapabilityMatrix` / `CapabilityEntry`

状态：**Accepted**（2026-09-24，人类 chat「按你的建议去改」授权）　日期：2026-09-24　Supersedes：—　Superseded by：—
关联：架构 v2 §13.1.2、`docs/spec/capability-matrix.md`、`protocol/capability-matrix/capability-1.0.json`、`docs/spec/naming.md` §7、`crates/platform/api`（TASK-016）、`docs/PARKING_LOT.md` PL-064

## 背景（为什么现在要决定）

TASK-016 在 `crates/platform/api` 落地了 `CapabilityMatrix`，取的是**架构 v2 §13.1.2 的运行时探测结果**语义
（`probed_at` / `platform` / `session` / `channels` / `degradations`）。
同批发现 `protocol/capability-matrix/capability-1.0.json` 的 schema `title` **也叫 `CapabilityMatrix`**，
但它装的是**另一件事**：稳定的能力标识目录（`<layer>.<capability>` 2 段式 + `stability` + `version`）。

**同一个名字在 schema / spec / 代码三处指两件事** → 读契约的人会把「目录」当「探测结果」（或反过来）。
这正是铁律 10「契约先行」要防的那类混淆。TASK-016 因改 `title` 属改契约而**停在登记**（PL-064），
人类 2026-09-24 chat 授权「按你的建议去改」后由本 ADR 定案。

## 决策（一句话）

把「能力」相关的**三个**概念分别定名：**`CapabilityCatalog`** = 稳定能力标识目录；
**`CapabilityMatrix`** = 运行时探测结果；**`CapabilityEntry`** = 单条能力的风险/审批元数据。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | **`CapabilityCatalog`** = `protocol/capability-matrix/capability-1.0.json`（schema `title` 由 `CapabilityMatrix` 改为 `CapabilityCatalog`）。内容 = 稳定标识 + `stability` + `version`；**不含**风险/审批列 |
| **D2** | **`CapabilityMatrix` 保留原义** = 运行时探测结果（架构 v2 §13.1.2；Rust `assistant_platform_api::CapabilityMatrix`）。**现存全部引用都是这个意思**（已逐处核对：架构 v2 line 1898、`docs/storage-design.md`、`README.md`、`plans/stage-1-pilots.md`、`LEDGER.md`、TASK-016 卡），故**一处不改** |
| **D3** | **`CapabilityEntry`**（+ `RiskLevel` / `ApprovalRequirement`）= 单条能力的**风险与审批声明**（`docs/spec/capability-matrix.md` §3 / §4 的四维）。它是**策略引擎的输入**（TASK-021），既不是目录也不是探测结果 |
| **D4** | **目录名 `protocol/capability-matrix/` 不改**：它是 codegen 与 `verify-schemas` 的**硬编码路径**（`xtask/src/render.rs:41`、`verify_schemas.rs:30`），改名要动护栏代码而**零语义收益** |
| **D5** | 本 ADR **授权**更新 ① `protocol/capability-matrix/capability-1.0.json` 的 `title` 字段；② `docs/spec/capability-matrix.md` 的**名称澄清段**（**不改** §4 四条不变量的语义）。另授权回填本登记表与 `docs/memory/decisions.md` |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 维持现状：两个都叫 `CapabilityMatrix`，靠上下文区分 | ❌ 否决 | PL-064 已证明会造成误读；且 schema `title` 是**给人读**的字段，它必须自解释 |
| 2 | 改 Rust 类型名（`CapabilityMatrix` → 别的），保留 schema `title` | ❌ 否决 | 架构 v2 §13.1.2（SSOT）用的就是这个名字，改它要动 SSOT；且「矩阵」（多通道 × 多能力）对**探测结果**是贴切的，对**目录**反而贴切度低 |
| 3 | **目录改名 + 另两义保留**（本 ADR） | ✅ **采纳** | 改名落在**契约文件自身**（schema `title`），且**零代码影响** —— 已实测 codegen 的 `render_capability()` 不接收 `title`、`verify_schemas` 不校验 `title` |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `protocol/capability-matrix/capability-1.0.json` 的 `title`（**唯一**的契约改动）
- `docs/spec/capability-matrix.md` 增加名称澄清段（D5 ②）
- `docs/adr/README.md` 登记表 + `docs/memory/decisions.md` 索引行
- **不改**：架构 v2（D2）、`crates/platform/api/**`（D2）、`xtask/**`（D4）、任何 Done 卡的正文（ADR-0031 D3/D5）、任何排期（ADR-0041 D2）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 改 `title` 会让 codegen / `verify-schemas` 变红 | 已实测两者都不读 `title`（`render.rs` 的 `render_capability()` 无参数；`verify_schemas.rs` 只查 `capabilities` 数组）→ 本 ADR 落地时跑 `verify-schemas` + `codegen --check` **当场验证** |
| 下个会话照旧文档把它改回去 | 本 ADR（`Accepted`）+ PL-064 的关闭行 + schema `title` 自身三处同时说同一件事 |
| 「三个名字」再被混用（如拿 `CapabilityEntry` 当目录项） | D3 写明三者的**归属层**（契约 / 运行时 / 策略）；`docs/spec/naming.md` §7 的「能力标识」行仍是标识格式的唯一事实源 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **立即验证**：`cargo run -p xtask -- verify-schemas` 与 `cargo run -p xtask -- codegen --check` 均绿；
   且 `protocol/capability-matrix/capability-1.0.json` 的 `title` = `CapabilityCatalog`、
   `crates/platform/api` 的 Rust 类型仍叫 `CapabilityMatrix`（`grep` 可验）。
2. **重新评估触发条件**：出现第四个「能力」概念；或 D4 被推翻（决定重命名 `protocol/capability-matrix/` 目录）——
   那时必须同步改 `xtask/src/render.rs` 与 `verify_schemas.rs` 的硬编码路径。
