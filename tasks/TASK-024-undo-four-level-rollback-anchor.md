# TASK-024　`undo`：可逆性四级 + 锚点（内容快照/影子副本/步数级）+ 回滚剧本执行 + 冲突检测 + incident 上报

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：012,023　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：012,023　**预估**：M　**难度**：M
- **write scope**：`crates/undo/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

`undo`：可逆性四级 + 锚点（内容快照/影子副本/步数级）+ 回滚剧本执行 + 冲突检测 + incident 上报。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/undo/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-024 `undo`：可逆性四级 + 锚点（内容快照/影子副本/步数级）+ 回滚剧本执行 + 冲突检测 + incident 上报
【目标】新建纯逻辑 `crates/undo`，实现 L0~L3 可逆性、三类锚点、回滚剧本执行、默认最保守的冲突检测，以及撤销失败的结构化 incident 与上报注入点。
【write scope】仅：`crates/undo/**`；另按流程更新本卡记录区，并在逐个取 `guard` 后同步 `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `LEDGER.md` / 必要时 `docs/memory/*`。
【铁律】1 无静默失败（校验、执行、冲突、上报失败均显式返回）/ 2 不可信输入先校验（锚点、指纹、剧本、工具名）/
        4 回滚后必须验证到锚点指纹，否则 incident / 9 不静默扩大范围 / 10 先按架构 v2 §9、§7.3 与现有协议契约实现
【禁止】改 `protocol/**`、`crates/protocol/**`、`crates/platform/**` 公共接口或 `ErrorCode`；任何平台 API、真实 IO、UI、凭据、网络；
        执行通用 shell；新增第三方依赖；顺手实现精确 diff、租约、HITL 或持久化；修改计划条目正文/排期。
【验收】`cargo fmt --all --check`；`cargo clippy --all-targets -- -D warnings`；`cargo test --workspace`；
        `cargo run -p xtask -- hygiene`、`memory-counts`、`adr-index`、`refscan`、`docscan`、`card-check` → 全绿/既有基线不新增。
【依赖】TASK-012、TASK-023；已核对 `LEDGER.md`，两张卡均 Done。
【疑问】无。卡面是占位正文，默认按标题/DoD 与架构 v2 §9 的最优闭合实现；不扩公共 schema，不改 `- 状态：` 正文区。
```

### 2. 实际改动文件

**write scope 内**

| 文件 | 行数 | 内容 |
|---|---:|---|
| `crates/undo/Cargo.toml` | 17 | `assistant-undo`，只依赖 workspace crate `assistant-platform-api`；零新增第三方 crate |
| `crates/undo/README.md` | 123 | 职责 / 边界 / 不变量 / 典型用法 / 已知限制 / 关联文档 |
| `crates/undo/src/lib.rs` | 60 | crate 模块文档与公开 re-export |
| `crates/undo/src/id.rs` | 110 | `AnchorId` / `StepId` / `TargetId` / `IncidentId` newtype |
| `crates/undo/src/error.rs` | 202 | `UndoError` + 既有 `ErrorCode` 映射 + 可读 `Display` |
| `crates/undo/src/reversibility.rs` | 91 | L0~L3、稳定解析、`worst()` 木桶规则、L3 人工确认边界 |
| `crates/undo/src/anchor.rs` | 523 | 内容摘要 / 影子副本路径 / undo 预算 / L0~L3 锚点策略与 payload 校验 |
| `crates/undo/src/recipe.rs` | 270 | 工具名 / undo 次数 / 内容与影子恢复 / 补偿调用；主 recipe 与 L0 fallback recipe |
| `crates/undo/src/conflict.rs` | 87 | 状态比较、`NoConflict` / `AlreadyAtAnchor` / `UserChanged` / `EvidenceMissing` 与默认阻断 |
| `crates/undo/src/incident.rs` | 188 | incident report、严重度、恢复指引、`IncidentReporter` 与显式上报失败 |
| `crates/undo/src/rollback.rs` | 580 | 执行入口、冲突门、动作顺序、L0→L1 fallback、最终指纹验证、incident 组装 |
| `crates/undo/tests/anchor_contract.rs` | 239 | 10 个可逆性与锚点契约测试 |
| `crates/undo/tests/rollback_contract.rs` | 433 | 13 个执行 / 冲突 / fallback / incident 契约测试 |
| `Cargo.lock` | 自动 | workspace package `assistant-undo` 入库；无新第三方依赖 |

合计 **13 个新文件 / 2923 行**；最大源文件 `src/rollback.rs` **580 行**，小于 600 行建议上限。

### 3. 验收输出摘要

```text
cargo fmt --all --check                         -> PASS（0 diff）
cargo clippy --all-targets -- -D warnings       -> PASS（exit 0）
cargo test --workspace                          -> PASS（全部 target 全绿；assistant-undo 23 tests + 0 doctest）
cargo test -p assistant-undo                    -> 23 passed / 0 failed（anchor_contract 10 + rollback_contract 13）
xtask hygiene                                   -> PASS（212 files / 0 error / 4 warning = 既有基线）
xtask memory-counts                             -> PASS（8 files / 0 error / 0 warning）
xtask adr-index                                 -> PASS（30 files / 0 error / 0 warning）
xtask docscan                                   -> PASS（177 files / 0 error / **497 warning**；占位记录区填满后较 506 基线下降 9）
xtask card-check                                -> PASS（94 files / 0 error / 27 warning = 既有基线）
xtask refscan                                   -> FAILED（404 files / 151 error = PL-058 既有基线，本次未新增）
```

### 4. DoD 逐条核对

- [x] 可逆性四级：L0~L3 有稳定解析、木桶 `worst()`、L3 人工确认谓词与负向解析测试。
- [x] 锚点：undo-stack / content-snapshot / shadow-copy / compensating / evidence-only 全量校验；L0 支持 L1 snapshot fallback。
- [x] 回滚剧本执行：动作顺序、动作类型、工具名与撤销次数受限；全部走注入 `RollbackExecutor`，crate 内无平台/IO。
- [x] 冲突检测：当前指纹异于 post 指纹即 `UserChanged`；缺证据绝不猜测；默认 `FailClosed`，只有显式 `RestoreOverall` 才能越过用户改动。
- [x] incident 上报：冲突、执行失败、缺最终指纹、最终指纹不一致都返回结构化 `IncidentReport`；`publish_incident` 失败显式返回 `ErrorCode::Fatal`。
- [x] 回滚后验证：所有成功路径都要求最终指纹等于锚点 pre 指纹；不相等即 incident，绝不返回成功。
- [x] `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全绿。
- [x] `hygiene` / `memory-counts` / `adr-index` / `docscan` / `card-check` PASSED；`refscan` 仅保留 PL-058 既有 151 error。
- [x] `MEMORY.md` 规模表同步；`docs/memory/facts.md` +1、`docs/memory/pitfalls.md` +1。

### 5. 偏差

无。新建 `crates/undo` 属卡面 write scope；未改公共协议 / `ErrorCode` / 平台层；未新增第三方依赖；未修改计划条目正文与排期。

### 6. 更合理做法

1. **L0 的 L1 兜底先写成可选 payload，而不是“后续再说”**：架构 v2 §9.1 明确 L0 与 L1 叠加。实现把可选 content snapshot digest 放在 L0 anchor 内，undo 失败或指纹不达锚点时自动执行兜底，并返回 `used_fallback=true`。
2. **incident 作为 `RollbackOutcome` 的终态，而不是普通 `Err`**：撤销失败是必须保留证据的事故。把它建模成带 expected/observed/action/evidence/guidance 的返回值，调用方无法把失败当成普通构造错误吞掉。
3. **错误只手写 `Display`，不引入 `thiserror`**：三平台依赖已批准，但本卡 write scope 只有 `crates/undo/**`；避免为新增使用方越界改 `docs/DEPENDENCIES.md`，同时保持每个错误变体都有可读原因。

### 7. 遗留问题

- 精确反向应用 Agent diff 未实现；当前冲突处理是保守的 whole-anchor restore 或 incident。
- 影子副本的存在性、完整性和保留期由 storage/executor 负责；本 crate 只校验路径形状与摘要形状。
- 状态行仍是正文区的 `Ready`：按 ADR-0031 / ADR-0041，Implementer 不修改分界线以上内容；完成状态由 `plans/*`、`PLAN.md`、`README.md`、`LEDGER.md` 同步（PL-073 仍未裁决）。

### 8. 新增长期记忆

- `docs/memory/facts.md` +1：`assistant-undo` 的三条安全路径、L0 fallback 与最终指纹验证基线。
- `docs/memory/pitfalls.md` +1：缺证据时 `RestoreOverall` 也不能放行；incident 上报失败必须显式传播。

### 9. 给审阅者的关注点

1. **冲突默认最保守**：`FailClosed` 是 `default`；`RestoreOverall` 只在用户明确选择后放行。缺 post 或当前指纹时，即使 `RestoreOverall` 也阻断并生成 incident。
2. **L0 fallback 的执行语义**：undo action 失败或最终指纹不达锚点时，引擎自动改用 `RestoreContentSnapshot`，成功结果显式标 `used_fallback=true`；请重点检查该路径的动作顺序与证据合并。
3. **错误码映射**：现有 13 类 `ErrorCode` 没有 undo 专属类别；冲突/最终不一致映射 `VerifyFailed`，上报失败映射 `Fatal`。若认为需要新增 `UndoFailed` / `UndoConflict`，先走 ADR，不在本卡私改协议。
