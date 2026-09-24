# TASK-020　`tool-bus`：MCP client(`rmcp`) + in-process server + JSON Schema 校验 + **统一返回信封**（`untrusted`/`truncated`）+ 工具集指纹 + 动态挂载

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011（`crates/protocol` 的 schema + codegen + `ErrorCode` 已落地；本卡**复用**，不改 schema）
- **预估**：M　**难度**：M
- **write scope**：`crates/tool-bus/**`（新 crate `assistant-tool-bus`）、`docs/DEPENDENCIES.md`（**仅追加**本卡实际引入的依赖行）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）；`docs/spec/tool-schema.md`（§3 类型 / §4 不变量 1~7）；`docs/spec/envelope.md`（§4 不变量 2 / 3 / 4）；`docs/spec/error-codes.md`；架构 v2 §5.2（Tool Schema）/ **§5.3（统一信封）** / **§5.4（MCP 作为唯一工具协议）** / §5.5（工具数量治理）；ADR-0021（受控词）；ADR-0024（依赖登记与 `spike-deny`）；**铁律 1 / 3 / 5 / 8**

**目标**

新建 `crates/tool-bus`，把「工具从哪来、怎么被校验、怎么被挂载、怎么被调用、结果长什么样」收敛成**唯一一条通道**：

- **MCP 是唯一工具协议**（架构 v2 §5.4）：Agent Core = MCP client；内置工具 = **in-process MCP server**（同机、零网络）。
- **统一返回信封**（架构 v2 §5.3）：所有工具返回走 `crates/protocol::ToolEnvelope`，`untrusted` / `truncated` / `source` / `evidence` / `metrics` 字段**齐全**。
- **JSON Schema 校验**：不符合 `ToolSchema.input` 的调用**直接拒**（`ErrorCode::ToolInvalidArgs`），不得进执行层。
- **工具集指纹**：每次会话记录「实际挂载的工具集版本」，审计时可复现「模型当时看到了什么」。
- **动态挂载 + 数量治理**：按目标应用挂载；单次上下文工具数 > 40 → **告警**（不得静默）。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/tool-bus/Cargo.toml`（`assistant-tool-bus`）+ `src/lib.rs` + `README.md`（职责 / 边界 / **不变量** / 已知限制） | gov §5.4 + 阶段 1 DoD |
| 2 | **MCP client 封装**（基于 `rmcp`）：连接 in-process server、`tools/list`、`tools/call`；**上层不感知 MCP 细节** | 架构 v2 §5.4 + §11.4 |
| 3 | **in-process MCP server**：把「已注册的内置工具」暴露为 MCP 工具；本卡只需 ≥1 个**测试用**内置工具（真实 Adapter 工具归 TASK-035 起） | 架构 v2 §5.4 |
| 4 | **输入校验**：按 `ToolSchema.input`（JSON Schema）校验调用参数；不合法 → `ErrorCode::ToolInvalidArgs` + **可读原因**（铁律 1） | `docs/spec/tool-schema.md` §4 不变量 2 / 6 |
| 5 | **统一信封组装**：`ok` / `tool` / `task_id` / `step_id` / `data` / `untrusted` / `source` / `truncated` / `evidence` / `metrics` / `error` 全字段语义落地（复用 `crates/protocol::ToolEnvelope`，**不自建**） | 架构 v2 §5.3 |
| 6 | **截断契约**：超 `max_bytes` → 截断 + `truncated.occurred = true` + `reason` + `original_bytes`（**禁止**静默截断） | 架构 v2 §5.3 规则 3 |
| 7 | **工具集指纹**：对「已挂载工具集」计算稳定指纹（同一集合 → 同一指纹；顺序无关）；随会话记录 | 架构 v2 §5.5 第 4 条 |
| 8 | **动态挂载 + 元工具**：`toolset.list` / `toolset.search` 两个元工具；单次挂载 > 40 → 结构化告警 + audit event | 架构 v2 §5.5 第 1 / 2 / 5 条 |
| 9 | `docs/DEPENDENCIES.md` 追加本卡引入的第三方依赖行（`rmcp` 必登；见「待裁决」Q1） | 登记表「登记规则」1 |
| 10 | 单测（正向 + **负向**，ADR-0019 N1）：信封字段齐全 / 非法 schema 直接拒 / > 40 告警 / 指纹稳定与顺序无关 | gov §5.5 |

**Out of scope（做了算漂移）**

- `crates/policy/**`（白名单 / 风险分级 / 默认拒绝 / 审批决策）→ **TASK-021**（**本卡不做任何放行判断** —— 铁律 3）
- `crates/model-gateway/**`（模型侧 tool_call 归一化）→ TASK-026
- `apps/automation-host/**`、`crates/ipc/**`（跨进程传输）→ TASK-019
- 真实 Adapter 工具与 Notepad / Paint / Edge 工具集 → TASK-035 起
- **外部** MCP server（stdio / streamable-http）加载与治理 → 阶段 3（架构 v2 §5.4 明示）
- 工具结果缓存、prompt cache 提示 → TASK-026 / TASK-028
- 改 `crates/protocol/**`（schema 与生成物）；确需改 → DRIFT（触发器 ③）
- 改 `crates/platform/**`、`crates/core/**`
- 任何 `#[allow]` 放宽（触发器 ⑥）

**必须遵守**

1. **铁律 3（策略引擎是唯一放行点）**：`tool-bus` **不得**自行判断权限 / 风险 / 是否允许调用；它只做「校验参数 + 组装信封 + 转发」。
2. **铁律 1（无静默失败）**：校验失败、挂载失败、调用失败一律返回带 `ErrorCode` 的错误；**禁止** `let _ =`、`unwrap()`、`expect()`、`panic!()`、`todo!()`（workspace lint 已 deny）。
3. **铁律 5（API 优先）**：工具按架构 v2 §5.2 的字段语义注册；`readOnlyHint` / `destructiveHint` / `idempotentHint` / `openWorldHint` 与自研 `risk_level` 的**双向映射**按架构 v2 §5.2 要点落地（只用已有字段，不新增 schema 字段）。
4. **复用 `crates/protocol` 生成物**：`ToolEnvelope` / `ToolSchema` / `RiskLevel` / `ErrorCode` / `Capability` **一律复用**；**禁止**在 `crates/tool-bus` 手写同名结构体（铁律 10 + ADR-0030 同源动机）。注意实际字段名以生成物为准（如 `ToolSchema.input` / `ToolSchema.output`，**不是** `input_schema`）。
5. **`untrusted` 语义**：凡内容来自目标应用 / 外部文档 / 网页 → `untrusted = true` 且 `source.kind = app_content`；**不得**让这类内容在信封里「看起来可信」。
6. **MCP 规范细节以官方为准**：`rmcp` 的版本、feature、以及 2026-07-28 规范的核心变化（stateless-first / Extensions / Tasks 移出核心）若与本卡默认取向冲突 → **先查官方文档再改**，并把结论记入 §6 或 `docs/memory/facts.md`。
7. **公共热点文件写前取锁**（ADR-0028）：`LEDGER.md` / `PLAN.md` / `README.md` / `docs/memory/*` / `docs/PARKING_LOT.md` / `plans/*` 写前 `guard acquire`，写完**立刻** release；超时（exit 5）必须在 LEDGER 追一行，**不得** `--force`。
8. **规模**：单文件 ≤ 400 行（软）/ 600（硬）；函数 ≤ 80 行；参数 ≤ 6 个。
9. **不得**为让门禁变绿而改测试断言或放宽 lint（触发器 ⑥ / ⑦）。

**待裁决（命中即记 `DRIFT-020-x`；夜间运行时按人类 2026-09-25 授权**自决**，裁决必须落盘 §5 + LEDGER）**

- **Q1（`rmcp` 依赖）**：架构 v2 §5.4 明文指定官方 Rust SDK = **`rmcp`**，但它是**第三方依赖**（触发器 ①）。**默认取向 = 引入 `rmcp`**（架构已定口径，换库 = 改口径），并在 `docs/DEPENDENCIES.md` 登记版本 / 许可证 / 替代方案；若 `rmcp` 需要 `tokio` 等连带依赖 → **逐个登记**，不得只登顶层。
- **Q2（JSON Schema 校验实现）**：候选 (a) `jsonschema` crate（新增依赖），(b) 复用 `rmcp` 自带的校验，(c) 手写最小子集校验。**默认取向 = 先查 (b)/(a)**：若 `rmcp` 已能校验则 (b) 优先；否则用 (a) 并登记（手写完整 JSON Schema 校验的实现风险远高于依赖成本）。若选 (c) → 必须在 §6 写明**支持的子集**与**不支持即拒绝**的判据（不得「不支持就放行」）。
- **Q3（工具集指纹的算法）**：本卡无既定算法。**默认取向 = 对「工具名 + 版本 + 规范化后的 schema」做稳定序列化后取 SHA-256**（`sha2` 已登记且已批准，使用方追加 `crates/tool-bus`）；**不得**用 `DefaultHasher`（跨进程 / 跨版本不稳定）。
- **Q4（in-process server 的传输方式）**：`rmcp` 的 in-process 传输若需 async 运行时 → 与 Q1 的连带依赖合并处理；若 `rmcp` 只提供 stdio / http 传输而无 in-process → **记 DRIFT**（这会改变架构 v2 §5.4「内置工具 = in-process server」的落地方式），**不要**擅自改成子进程 stdio。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-tool-bus
cargo run -p xtask -- hygiene / memory-counts / adr-index / docscan / card-check / check-ledger
```

**完成定义（DoD）**

- [ ] 上列命令全部通过（`refscan` 只需**不新增** Error —— 既有 151 项 baseline 见 PL-058）
- [ ] **内部工具经 MCP 表达**：至少 1 个内置工具能经 MCP `tools/list` + `tools/call` 走通全链路（有测试证据）
- [ ] **信封字段齐全**：`ok` / `tool` / `task_id` / `step_id` / `untrusted` / `source` / `truncated` / `error` 全部有断言
- [ ] **schema 不合法直接拒**：非法参数 → `ErrorCode::ToolInvalidArgs`（**不进**执行层，有负向用例）
- [ ] **工具数 > 40 时告警**：有测试证明告警被产生且**不是**静默（铁律 1）
- [ ] 工具集指纹：同集合不同插入顺序 → 指纹相同；集合变化 → 指纹变化
- [ ] `crates/tool-bus/README.md` 含职责 / 边界 / 不变量 / 已知限制四节
- [ ] `docs/DEPENDENCIES.md` 已登记本卡引入的**全部**第三方依赖（含连带依赖）
- [ ] `LEDGER.md` 追加一行；如新增事实/坑 → `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 文件被修改

**夜间自动化补充约束（仅当本卡由夜间 automation 执行时适用；依据 = 人类 chat 2026-09-25 授权）**

1. 每次运行**只做一张卡**（AGENTS.md §3）。
2. 分支 = `task/TASK-020-tool-bus-mcp-rmcp-server`，从**最新 `origin/main`** 切出；**PR base 必须是 `main`**（PL-075）。
3. 需要裁决的项**自决**，但必须**落盘**（卡 §5 全文 + `LEDGER.md` 一行）。
4. 合并自己的 PR 需**同时**满足：CI 全部 success **且** `mergeable_state == clean` **且** base = `main`；否则只开 PR、**不合并**。
5. 结束后按 ADR-0039 / ADR-0041 同步 `PLAN.md` 当前状态块（4 行）、`README.md` 三处、`plans/stage-1-pilots.md` 头部进度句与本卡完成标记、`LEDGER.md` 一行。
6. **本卡是 A2 批次里第一次引入较重第三方依赖（`rmcp` + 连带）的卡** → 引入后必须复跑 `cargo deny check`（许可证白名单 + sources），失败即回退并记 DRIFT。
<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
