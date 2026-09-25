# ADR-0047　后置断言没有自由字符串字段 `assert`（永久不实现）

状态：**Accepted**（2026-09-25，人类 chat「PL-086：按你建议做」）　日期：2026-09-25　Supersedes：—　Superseded by：—
关联：`cross-platform-ai-assistant-architecture-v2.md` §7.4 / §5.2 / §11 / 附录 A `#/$defs/assertion`、`docs/PARKING_LOT.md` PL-086、`tasks/TASK-023-verify-postcondition-assertion-engine.md` §5 `DRIFT-023-4`、`crates/verify`（TASK-023）、ADR-0031（一卡一文件）

## 背景（为什么现在要决定）

架构 v2 附录 A 的 `#/$defs/assertion` 声明了一个自由字符串字段：

```json
"assert": { "type": "string" }
```

§5.2 / §11 的示例按它写了 `{"kind": "state_assert", "assert": "target.text.contains(new_text)"}` 这类**表达式**。

TASK-023 实现 `crates/verify` 时**没有**实现它，并在解析期拒绝（记 `DRIFT-023-4` → **PL-086**）。两条路摆在人类面前：

1. **接受永久不实现**并同步改附录 A / spec；
2. 立卡补一门表达式语言（须先给出「模型可枚举」的语法子集与注入面分析）。

人类 2026-09-25 裁决 = **选项 1**（「PL-086：按你建议做」）。

**为什么选项 1 是对的**：支持 `assert` 等于引入一门**表达式语言** —— lexer / parser / evaluator / scope 四件套 = 新抽象层（漂移触发器 ⑨）；更硬的理由是它与 §7.4 已否决的 `if`-`then`-`else` 同病：**模型无法枚举可写形式**，写错只能得到运行时失败（铁律 1 的静默失败面）。而结构化 `field` + `op` + `value`（以及各 kind 的专用字段）已能表达 §7.4 的全部 11 种后置断言。

**不修的代价**（PL-086 原文）：`verify` 之外的调用方 —— TASK-028 的 Planner、未来的 Adapter 作者 —— 读附录 A 时会以为 `assert` 可用。

## 决策（一句话）

**后置断言永久没有自由字符串字段 `assert`**：判定条件一律写成**结构化字段**（`field` / `op` / `value` / `name` / `min` / `max` / `selector` / `path` / `expect` / `key` / `within_ms` / `fingerprint_scope`），**未知字段解析期拒绝**（fail-closed）；附录 A 与 §5.2 / §7.4 / §11 的示例同步改为结构化写法。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | **`assert` 字段从附录 A 删除**，而不是「标注为未实现」—— 留一个「声明了但不可用」的字段只会继续误导读者（铁律 1：不许用「存在但不可用」冒充契约） |
| **D2** | **替代写法 = 结构化字段**。附录 A 补齐实现**真正接受**的字段（`field` / `op` / `name` / `min` / `max` / `selector` / `path` / `expect` / `key`），并给 `assertion` 对象加 `description` 注记 |
| **D3** | **`kind` 的归属写清**：`visual_assert` → TASK-042；`target_resolvable` / `capability` → §5.3 **前置条件**（本 ADR 只标注，不改它们的实现状态 —— 它们仍无归属卡） |
| **D4** | **文档示例必须可实现**：§5.2 的 `preconditions` / `postconditions`、§7.4 的类型表、§11 的撤销剧本示例全部改成结构化写法。顺带修掉 §11 示例里的 `optional_if_absent` —— 附录 A **从未声明**它，`crates/verify` 也会拒绝（同一个「文档声明了不存在的东西」的毛病） |
| **D5** | **零代码改动**：`crates/verify` 的行为不变（它本来就拒绝 `assert`）；本 ADR 是**文档与契约对齐**，不是新功能 |
| **D6** | **永久性**：本否决登记进 `docs/memory/rejected.md`（防止同一方案被反复重新提出）。若将来真要做表达式语言，**须先给「模型可枚举」的语法子集 + 注入面分析**，并**新开 ADR 取代本 ADR** |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | **接受永久不实现 + 改附录 A / spec（本 ADR）** | ✅ **采纳** | 结构化字段已能表达 §7.4 的全部 11 种断言；零新抽象层；消除「文档声明了不存在的东西」 |
| 2 | 立卡补表达式语言（lexer / parser / evaluator / scope） | ❌ 否决 | 新抽象层（漂移触发器 ⑨）；**模型无法枚举可写形式**；解析不可信输入 = 新的注入面 |
| 3 | 保留 `assert` 字段但在附录 A 标注「未实现」 | ❌ 否决 | 契约里留一个「声明了但不可用」的字段 = 静默失败的温床；读者仍会照抄 §5.2 的示例 |
| 4 | 保留 `assert` 但只支持固定几个字面量 | ❌ 否决 | 那不是「表达式」，只是伪装成字符串的 `enum` —— 应直接写成结构化 `enum` 字段 |

## 影响（需要改的文档）

- `cross-platform-ai-assistant-architecture-v2.md`：附录 A `#/$defs/assertion`（删 `assert` + 补结构化字段 + 对象级 `description`）、§5.2 示例、§7.4 类型表、§11 撤销剧本示例
- `docs/adr/README.md`：登记表 §1 新增 0047 行 + 「下一个可用编号」→ **0048**
- `docs/memory/decisions.md`：追加索引条目
- `docs/memory/rejected.md`：登记「自由字符串 `assert`」永久否决
- `docs/PARKING_LOT.md`：PL-086 闭环
- `PLAN.md`：当前状态块的「阻塞项」删去 ⑥ PL-086（仅该块，ADR-0039 D4）
- `LEDGER.md`：追加一行
- **不改**：`crates/verify` 及任何 crate 代码（D5）、`AGENTS.md`、`docs/spec/tool-schema.md`（已核对该文件**没有** `assert` 引用）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 删字段后仍有读者按旧示例写 `assert` | §7.4 表下加 ADR-0047 注记；附录 A 的 `description` 明写「没有自由字符串 `assert`」；`crates/verify` 的解析错误消息指向结构化写法 |
| 附录 A 的结构化字段集与实现漂移 | 附录 A 的字段集**逐条对照** `crates/verify/src/postcondition.rs` 的 `only_keys(...)` 列表；将来实现变化须同步改附录 A |
| 前置条件（`target_resolvable` / `capability`）被误当成已实现 | 附录 A 的 `description` 与 §7.4 注记都写明它们是 §5.3 前置条件、**无归属卡** |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo run -p xtask -- adr-index` 绿灯（登记表 ↔ ADR 文件 ↔ `decisions.md` 三方一致）；
2. 附录 A 仍是**合法 JSON** 且 `$defs.assertion.properties` **不含** `assert`（交付时已用脚本解析核对：10/10 个 json fence 合法）；
3. 附录 A 的字段集与 `crates/verify/src/postcondition.rs` 的 `only_keys` 列表一致；
4. **何时重新评估**：若将来需要「模型可枚举的表达式子集」（不是自由字符串）→ 新开 ADR 取代本 ADR（D6）。
