# TASK-205　`crates/tool-bus/src/schema.rs` 按职责拆分（注册期检查 / 运行期实例校验）

- 状态：**Done（2026-10-01；LEDGER Done + PR #121 / merge `dc1d118`；schema split）**
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内）　依赖：020 / 204（✅ 均 Done）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。
- 来源：TASK-204 收尾时 `hygiene` 报 `WARN hygiene/file-too-long crates/tool-bus/src/schema.rs:892`（> gov §5.4 的 600 行建议线）。TASK-204 已靠「压缩模块文档 58 → 42 行」把它从 908 压回 892（< 硬限 900），但**余量只剩 8 行**。人类 chat 2026-09-25 裁决：**「`schema.rs`：合适的时候立卡，拆文件吧」**。
- 契约依据：`crates/tool-bus/README.md`「不变量」与「已知限制」、`docs/spec/tool-schema.md` §4 不变量 8

---

## 目标（一句话）

把 `crates/tool-bus/src/schema.rs`（892 行）按**职责**拆成模块目录，让每个文件回到 600 行建议线以下，**行为零变化**（既有测试与 `codegen --check` 保持绿）。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 该文件 **892 行** > gov §5.4 建议上限 600（< 硬限 900） | `cargo run -p xtask -- hygiene` → `WARN hygiene/file-too-long crates/tool-bus/src/schema.rs:892` |
| 2 | TASK-204 只靠压缩文档把它从 908 压到 892，**余量仅 8 行** | `tasks/TASK-204-draft07-keyword-verdict.md` §6 / §7 |
| 3 | 文件里已经是**两个职责**：注册期 schema 检查（`collect_unenforceable_constructs` / `walk_schema` / `check_keyword_shape` / `check_sub_schema_keyword` / 三张拒绝表 / `classify_keyword` / `check_declared_dialect`）与运行期实例校验（`validate_arguments` 系列） | 文件内分区注释 |
| 4 | TASK-020 §7 的遗留建议**早已提到**拆分 | `tasks/TASK-020-tool-bus-mcp-rmcp-server.md` |
| 5 | 后续再往该文件加关键字**必须先拆**（TASK-204 §7 已写明） | `tasks/TASK-204-draft07-keyword-verdict.md` §7 |

## write scope

- `crates/tool-bus/src/schema.rs` → 拆为 `crates/tool-bus/src/schema/mod.rs` + 若干子模块（建议：`registration.rs` = 注册期检查 + 三张表；`instance.rs` = 运行期实例校验；`keywords.rs` = 五张表 + 分类；具体划分由 Implementer 定，**不新增 crate / 顶层目录**）
- `crates/tool-bus/src/lib.rs`（仅当 `pub use` 路径需调整）
- `crates/tool-bus/tests/schema_keywords.rs`（仅**新增**；不得改既有断言 —— 漂移触发器 ⑦）
- `crates/tool-bus/README.md`（仅当「不变量 / 已知限制」里引用了具体文件路径且路径确实变化）
- `tasks/TASK-205-schema-module-split.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步

## Out of scope（写了就停）

- **不改任何行为**：`SUPPORTED_KEYWORDS` 的 21 条、三张拒绝表的成员与理由、四类标签（`never-supported` / `not-yet-supported` / `other-dialect` / `unknown`）、`$schema` 方言判据与 URI 白名单、pointer 格式（含 schema 侧 `/properties/<name>`）、深度上限 64、条数上限 8 —— **全部一字不改**
- **不加关键字**、不改判据、不放宽任何 lint、不加 `#[allow]`、不加 `unsafe`
- **不引依赖**
- **不改** `registry.rs` 的流程
- **不顺手重构**：拆分 = 同一段代码换文件，不是改代码（gov §3.3）

## 必须遵守

- **纯搬移**：`git diff` 读起来必须是「这段代码搬到了那个文件」，不允许借机重命名公共 API、改可见性（`pub` / `pub(crate)` 的**对外可见性集合必须不变**）
- **模块树可变、模块路径尽量不变**：`assistant_tool_bus::schema::*` 的公开项路径应保持（`lib.rs` 的 re-export 兜住）
- **拆完后 `hygiene` 的 `file-too-long` warning 必须少 1 条**（892 行那条消失）—— 这是本卡唯一的**可机器验证**的成功判据
- **新增模块属漂移触发器 ②**：本卡的存在本身即人类 2026-09-25 的授权（「合适的时候立卡，拆文件吧」），Implementer 不再另立 DRIFT

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

- [ ] `crates/tool-bus/src/schema.rs` 不再是「单文件 > 600 行」形态（拆为模块目录，或单文件 < 600 行）
- [ ] `hygiene` 的 `file-too-long` warning 数**减 1**（892 行那条消失），且**不新增**其它 warning
- [ ] 全部既有测试**零改动**通过（含 `tests/schema_keywords.rs` 的 9 个用例）
- [ ] 行为零变化：三张表成员 / 四类标签 / pointer 格式 / `$schema` 判据 / 两个上限均未变
- [ ] `pub` 可见性集合不变（`assistant_tool_bus::{SUPPORTED_KEYWORDS, ANNOTATION_KEYWORDS, REJECTED_FOREVER_KEYWORDS, REJECTED_FOR_NOW_KEYWORDS, NON_DRAFT07_KEYWORDS, collect_unenforceable_constructs, validate_arguments}` 仍可达）
- [ ] 上列 14 条验收命令全绿
- [ ] §11.1 进度同步：`PLAN.md` 当前状态块 / `README.md` 三处 / `LEDGER.md` / `MEMORY.md` 规模表

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-205 `crates/tool-bus/src/schema.rs` 按职责拆分
【目标】把 892 行单文件纯搬移到 `schema/` 模块目录，行为零变化并减少一条 `file-too-long` 警告
【write scope】仅：`crates/tool-bus/src/schema.rs` → `schema/mod.rs` + 子模块、`crates/tool-bus/src/lib.rs`（仅必要时）、`crates/tool-bus/tests/schema_keywords.rs`（仅新增）、`crates/tool-bus/README.md`（仅路径变化时）、本卡记录、进度同步文件
【铁律】1 无静默失败；8 不以拆分改变公共可见性；9 不扩大范围；10 契约先行
【禁止】不改行为、不加关键字、不放宽 lint、不加 `#[allow]`/`unsafe`、不引依赖、不改 `registry.rs`、不顺手重构
【验收】卡面 14 条命令全绿；`hygiene` 的 `schema.rs:892` 警告消失且不新增警告；既有测试零改动
【依赖】TASK-020、TASK-204 已在 `LEDGER.md` 记为 Done
【疑问】无

### 2. 实际改动文件

- `crates/tool-bus/src/schema.rs` → 删除单文件形态。
- `crates/tool-bus/src/schema/mod.rs`：保留原模块文档并只做公共项 re-export。
- `crates/tool-bus/src/schema/registration.rs`：注册期关键字表、方言校验、schema 形状检查与递归遍历。
- `crates/tool-bus/src/schema/instance.rs`：运行期实例校验、对象 / 数组 / 字符串 / 数值 / 组合子检查。
- `crates/tool-bus/src/schema/shared.rs`：递归深度、JSON pointer 与错误位置 helper。
- 本卡记录与 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 的强制进度同步。

### 3. 验收输出摘要

```text
cargo fmt --all --check                                      → PASS
cargo clippy --all-targets -- -D warnings                    → PASS
cargo test --workspace                                       → PASS（0 failed）
cargo test -p assistant-tool-bus                             → PASS（22 passed / 0 failed）
cargo run -p xtask -- verify-schemas                         → PASS
cargo run -p xtask -- codegen --check                        → PASS（0 drift）
cargo run -p xtask -- hygiene                                → 0E / 3W（基线 4W；schema.rs:892 警告消失）
cargo run -p xtask -- docscan / card-check / memory-counts   → PASS
cargo run -p xtask -- adr-index / check-ledger               → PASS
cargo run -p xtask -- check-migrations / refscan             → PASS
cargo run -p xtask -- check-comments                         → 0E / 67W（基线）
cargo deny check                                             → advisories / bans / licenses / sources OK
PR #121 push / pull_request CI                               → 全绿；mergeable=CLEAN
PR #121 merge                                                → `dc1d118`
```

### 4. DoD 逐条核对

- [x] `schema.rs` 已拆为模块目录，所有文件均低于 600 行。
- [x] `hygiene` 警告数由 4 降至 3，且没有新增 warning。
- [x] 既有测试零改动通过；`tests/schema_keywords.rs` 9 个用例保持原样。
- [x] 行为零变化：关键字表、四类标签、pointer 格式、`$schema` 判据与两个上限均未变。
- [x] `pub` 可见性集合不变；`lib.rs` 仍从 `schema` re-export 同一组公共项。
- [x] 卡面 14 条验收命令全绿。
- [x] §11.1 进度同步已随本卡 Done 一并提交。

### 5. 偏差

none。所有改动都在卡面 write scope 内；没有新增依赖、公共接口、`#[allow]` 或 `unsafe`。

### 6. 更合理做法

按职责拆成注册期、运行期与共享 helper 三层；`mod.rs` 只保留模块文档和 re-export，避免将原 892 行文件换名后继续堆积。

### 7. 遗留问题

无新增遗留。仓库另有 3 条既有 `file-too-long` warning（`xtask/src/card_check.rs` / `docscan.rs` / `main.rs`），不属本卡 write scope。

### 8. 新增长期记忆

无。

### 9. 给审阅者的关注点

1. 重点检查 `mod.rs` 的 re-export 集合是否与原 `schema.rs` 的公开项完全一致。
2. 重点检查 `registration.rs` 与 `instance.rs` 是否只是搬移；共享 helper 仅提升到 `pub(super)`，没有扩大外部 API。
3. `hygiene` 只应减少 `schema.rs:892` 一条，不应影响其余三条基线 warning。
