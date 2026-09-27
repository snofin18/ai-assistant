# TASK-208　`core`：Memory（App Map 加载 + 消费 storage 的 FTS5 检索 API）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**（原 TASK-028 的拆卡）　依赖：**206** / 028　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`；契约见 `docs/spec/core-orchestration.md` 与 `docs/adr/0053-core-orchestration-layer-interface.md`。
- 本卡与 TASK-028 / TASK-207 **共用 `crates/core/**`** → **必须严格串行**（依赖列已保证顺序），不得与它们并行。
- 来源：**DRIFT-028-1**（原 TASK-028 §5；触发器 ⑤）+ **DRIFT-028-2**（触发器 ⑩）+ **ADR-0053 D4 / D6 / D7**

---

## 目标（一句话）

在 `crates/core` 落地 **Memory**：按需加载 **App Map**（应用档案片段）并把「哪些片段要注入本轮上下文」与 **TASK-206 的检索 API** 结合起来 —— 检索本身**不**在本卡实现。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 原 TASK-028 把「Memory(App Map 加载 / **FTS5 检索**)」写在同一张卡里，而 FTS5 属存储层 → 触发器 ⑤ | `tasks/TASK-028-core-session-context-planner-memory.md` §5 的 **DRIFT-028-1** |
| 2 | 人类 2026-09-26 裁决采纳「立前置卡」：存储侧先做 `memory_fts` + 检索 API + 测试 | **ADR-0053 D6** + `tasks/TASK-206-storage-memory-fts5-search.md` |
| 3 | 本卡只**消费**该 API（不持有 SQLite 连接、不拼 SQL） | `crates/core/README.md` 边界 + `docs/spec/core-orchestration.md` 不变量 5 |
| 4 | App Map 是**文件**（人类可编辑、可 diff），其内容属**不可信输入** | `docs/storage-design.md` §4 归属表（App Map / Adapter / 策略 / 出域配置 = 文件）；铁律 2 |
| 5 | 「按需片段注入」是阶段 1 的验收要点 | `plans/stage-1-pilots.md` 批次表 A2 的 TASK-028 行验收要点「App Map 按需片段注入」 |

## write scope

- `crates/core/src/**`（**Memory 相关模块** + `lib.rs` 的 re-export；新增模块属漂移触发器 ②，本卡的存在即 ADR-0053 的授权）
- `crates/core/tests/**`（**新增**测试；不得改既有断言 —— 漂移触发器 ⑦）
- `crates/core/Cargo.toml`（**仅当**需要白名单内的依赖；白名单见 ADR-0053 D2）
- `crates/core/README.md`（仅当「职责 / 边界 / 不变量 / 已知限制」因本卡需要同步）
- `tasks/TASK-208-core-memory-app-map-fts-retrieval.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步

## Out of scope（写了就停）

- **不实现** FTS5 虚表 / 检索 SQL / 迁移（归 **TASK-206**，本卡只调用它的公开 API）
- **不做**向量检索 / 语义检索 / 外部索引（阶段 1 Out of scope）
- **不写** `adapters/**` 的 App Map 内容本身（归各 Adapter 卡；本卡只定义**加载与校验**）
- **不判权限**、不执行工具、不调平台 API（铁律 3 / 7）
- **不引**黑名单 crate（ADR-0053 D3）；**不定义**跨进程 / 持久化结构（归 `protocol` / `storage`）
- **不放宽**任何 lint、不加 `#[allow]`、不加 `unsafe`、不改 `protocol` schema / `ErrorCode`
- **不顺手重构** TASK-028 / TASK-207 落下的模块

## 必须遵守

- **App Map = 不可信输入**（铁律 2）：加载时做形状 / 版本 / 路径安全校验（**禁止**用 App Map 里的路径做未经校验的文件访问；路径穿越按 `crates/policy` 的既有护栏口径 fail-closed）
- **按需注入**：不得把整份 App Map 无差别塞进上下文；注入的每一段都要**可追溯到来源文件与位置**
- **无静默失败**（铁律 1）：App Map 缺失 / 损坏 / 版本不匹配、检索后端报错 → 带 `ErrorCode` 返回；**不**用默认 App Map 或空结果冒充成功
- **不持有连接**：持久化只经 `assistant-storage` 的公开 API（`docs/spec/core-orchestration.md` 不变量 5）
- **可注入**：文件读取 / 时钟 / 随机 / UUID 一律 trait 注入（AGENTS.md §5.3，保证可回放）
- **文档注释**：公共 API 100% 有文档注释（含**错误语义**与幂等性）
- **单文件 ≤ 600 行**（硬限 900）；函数 ≤ 80 行、参数 ≤ 6 个
- **只追加文件**（`LEDGER.md` / `docs/PARKING_LOT.md`）**不改写既有行**

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core
cargo test -p assistant-core arch::
cargo test -p assistant-storage
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo deny check
```

## DoD

- [ ] App Map 加载：合法文件可加载；缺失 / 损坏 / 版本不匹配 / 路径穿越尝试 → **四类负向用例**各自带 `ErrorCode` 失败
- [ ] 检索：能消费 TASK-206 的公开检索 API 并按需组装片段；检索后端报错时**透传** storage 的错误语义且保留原始 `reason_code`
- [ ] 注入的每个片段都带**来源与位置**（可追溯）；超预算时**显式标注**被丢弃的内容与理由（不静默截断）
- [ ] `core` 的 `[dependencies]` ⊆ ADR-0053 D2 白名单；`cargo test -p assistant-core arch::` 全绿
- [ ] `core` 单元测试**零真实 IO / 网络 / 时钟**（文件读取经注入的 trait；可回放）
- [ ] `core` 行覆盖 ≥ 85%（阶段 1 DoD）
- [ ] 既有测试零改动通过（漂移触发器 ⑦）
- [ ] 上列 15 条验收命令全绿；`hygiene` / `card-check` / `docscan` 的 warning **不新增**
- [ ] §11.1 进度同步：`PLAN.md` 当前状态块 / `README.md` 三处 / `LEDGER.md` / `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md` 规模表
- [ ] 若产生新 FACT / PITFALL → 追加 `docs/memory/{facts,pitfalls}.md`

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-208 `core` Memory：App Map 加载 + 消费 storage FTS5 检索
【目标】提供可注入、fail-closed 的记忆片段装配，所有片段带来源/位置，超预算显式记录 omission。
【write scope】crates/core/src/**、crates/core/tests/**、crates/core/Cargo.toml、
crates/core/README.md、本卡记录区及 AGENTS.md §11.1 指定进度文件。
【铁律】不可信输入先校验；无静默失败；core 不持有连接/不拼 SQL；契约先行。
【禁止】FTS5 实现；向量检索；写 adapters；权限判定；黑名单 crate；改 protocol schema。
【验收】卡内 15 条命令全绿，core 行覆盖 ≥85%，warning 不新增。
【依赖】TASK-206 / TASK-028 Done。
【疑问】storage 检索 API 直接接 `&Connection`；本卡用注入式 `MemoryRetriever` 消费 storage
结果，TASK-029 装配层负责把 trait 接到 storage 公开 API。
```

### 2. 实际改动文件

- `crates/core/src/app_map.rs`（新增）
- `crates/core/src/memory.rs`（新增）
- `crates/core/src/error.rs`
- `crates/core/src/lib.rs`
- `crates/core/Cargo.toml`
- `crates/core/tests/app_map_tests.rs`（新增）
- `crates/core/tests/memory_tests.rs`（新增）
- `crates/core/README.md`
- `tasks/TASK-208-core-memory-app-map-fts-retrieval.md`（仅记录区）

### 3. 验收输出摘要

- `cargo fmt --all --check` → PASS（0 diff）
- `cargo clippy --all-targets -- -D warnings` → PASS
- `cargo test --workspace` → PASS
- `cargo test -p assistant-core` → PASS；新增 Memory 测试 17 个
- `cargo test -p assistant-core arch::` → PASS（10 个 arch tests）
- `cargo test -p assistant-storage` → PASS（40 tests + 2 doctests）
- `cargo run -p xtask -- verify-schemas` / `codegen --check` / `hygiene` 0E/4W /
  `docscan` 0E/468W / `card-check` 0E/27W / `memory-counts` / `adr-index` /
  `check-ledger` → 全 PASS
- `cargo deny check` → PASS
- `cargo llvm-cov -p assistant-core --fail-under-lines 85` → PASS（**88.27%**）

### 4. DoD 逐条核对

- [x] App Map 加载：合法文件可加载；缺失 / 损坏 / 版本不匹配 / 路径穿越四类负向用例全部带 `ErrorCode`
- [x] 检索：通过注入式 `MemoryRetriever` 消费 storage 结果；后端错误保留原始 `reason_code` 与 `ErrorCode`
- [x] 每个注入片段带 `source_reference` 与 `MemorySegmentOrigin`；超预算 / 重复来源产生显式 omission
- [x] `core` 依赖 ⊆ ADR-0053 D2；`arch::` 全绿
- [x] Memory 测试零真实 IO / 网络 / 时钟；文件读取与检索均注入
- [x] `assistant-core` 行覆盖 ≥ 85%（88.27%）
- [x] 既有测试零改动通过
- [x] 15 条验收命令全绿；warning 与既有基线一致
- [ ] §11.1 进度同步：实现 PR 合并后由 closeout 提交完成状态与当前进度块
- [x] 未产生需要新增的 FACT / PITFALL

### 5. 偏差

无行为偏差。为满足单文件行数门禁，把 App Map 加载/校验拆入 `app_map.rs`，Memory 装配留在
`memory.rs`；两者都属本卡 `crates/core/src/**` write scope。独立 review 后补强：
App Map 的声明 token estimate 只能抬高保守字符数估算、content 有字节上限；来源引用由
validated path + entry index 生成；omission 携带 origin/token；预算检查先于重复来源判定。
同一轮 review 发现测试文件超过 hygiene 建议上限，故把 App Map 契约测试拆入
`app_map_tests.rs`，使 `hygiene` 回到 0E/4W 基线。

### 6. 更合理做法

App Map 与检索适配器都做成注入 trait，Core 只做纯校验与预算装配；这样单测无真实 IO，
真实文件读取与 SQLite 连接都留在 TASK-029 装配层。

### 7. 遗留问题

- TASK-029 需实现 `AppMapFileReader` 与 `MemoryRetriever` 的生产 adapter。
- App Map v1 只接受已知字段与 caller 指定 entry id，不做隐式全量注入。
- 检索结果的 token estimate 由 adapter 提供，Core 不猜测 provider 分词。

### 8. 新增长期记忆

无。

### 9. 给审阅者的关注点

1. 最高风险：`MemoryRetriever` 是否足以表达 storage 错误透传与 token 估计边界。
2. 次风险：App Map v1 严格拒绝未知字段，是否会阻碍后续 Adapter 扩展。
3. 超预算 omission 是显式成功投影，不会自动请求更多预算；调用方需决定是否重试。
